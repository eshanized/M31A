//! Conversation attachments, image metadata, and file reference models.
//!
//! Models user-supplied files, images, and documents attached to conversation turns.

use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

pub const MAX_ATTACHMENT_SIZE_BYTES: u64 = 25 * 1024 * 1024; // 25 MB

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub storage_path: PathBuf,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AttachmentIndex {
    attachments: HashMap<String, Attachment>,
}

#[derive(Clone)]
pub struct AttachmentStore {
    base_dir: PathBuf,
    index: Arc<RwLock<AttachmentIndex>>,
}

impl AttachmentStore {
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_dir)
            .with_context(|| format!("Failed to create attachment directory at {:?}", base_dir))?;

        let index_file = base_dir.join("index.json");
        let index = if index_file.exists() {
            let content = fs::read_to_string(&index_file).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            AttachmentIndex::default()
        };

        Ok(Self {
            base_dir,
            index: Arc::new(RwLock::new(index)),
        })
    }

    pub fn default_store() -> Result<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let dir = PathBuf::from(home)
            .join(".config")
            .join("m31a")
            .join("attachments");
        Self::new(dir)
    }

    /// Ingests a local file into the content-addressable attachment store.
    pub fn ingest_file(&self, path: &Path) -> Result<Attachment> {
        if !path.exists() {
            bail!("Source file for attachment does not exist: {:?}", path);
        }

        let meta = path.metadata()?;
        if meta.len() > MAX_ATTACHMENT_SIZE_BYTES {
            bail!(
                "Attachment size ({} bytes) exceeds maximum allowable limit of {} bytes",
                meta.len(),
                MAX_ATTACHMENT_SIZE_BYTES
            );
        }

        let bytes = fs::read(path)?;
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed_attachment");

        let mime = detect_mime_type(filename, &bytes);
        self.ingest_bytes(filename, &bytes, Some(&mime))
    }

    /// Ingests raw bytes into the attachment store.
    pub fn ingest_bytes(
        &self,
        filename: &str,
        bytes: &[u8],
        mime_type: Option<&str>,
    ) -> Result<Attachment> {
        if bytes.len() as u64 > MAX_ATTACHMENT_SIZE_BYTES {
            bail!(
                "Attachment size ({} bytes) exceeds maximum allowable limit of {} bytes",
                bytes.len(),
                MAX_ATTACHMENT_SIZE_BYTES
            );
        }

        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let sha256 = format!("{:x}", hasher.finalize());

        let id = format!("att_{}", &sha256[..16]);
        let storage_filename = format!("{}_{}", &sha256[..16], sanitize_filename(filename));
        let storage_path = self.base_dir.join(storage_filename);

        if !storage_path.exists() {
            fs::write(&storage_path, bytes).with_context(|| {
                format!("Failed to write attachment data to {:?}", storage_path)
            })?;
        }

        let mime = mime_type
            .map(|s| s.to_string())
            .unwrap_or_else(|| detect_mime_type(filename, bytes));

        let attachment = Attachment {
            id: id.clone(),
            filename: filename.to_string(),
            mime_type: mime,
            sha256,
            size_bytes: bytes.len() as u64,
            storage_path,
            created_at: Utc::now().to_rfc3339(),
        };

        {
            let mut idx = self.index.write().unwrap();
            idx.attachments.insert(id, attachment.clone());
            self.save_index(&idx)?;
        }

        Ok(attachment)
    }

    /// Retrieves metadata for an attachment by its ID.
    pub fn get_attachment(&self, id: &str) -> Option<Attachment> {
        let idx = self.index.read().unwrap();
        idx.attachments.get(id).cloned()
    }

    /// Reads raw attachment binary data from disk.
    pub fn read_bytes(&self, id: &str) -> Result<Vec<u8>> {
        let att = self
            .get_attachment(id)
            .ok_or_else(|| anyhow::anyhow!("Attachment ID '{}' not found", id))?;

        fs::read(&att.storage_path)
            .with_context(|| format!("Failed to read attachment from {:?}", att.storage_path))
    }

    /// Lists all registered attachments.
    pub fn list_attachments(&self) -> Vec<Attachment> {
        let idx = self.index.read().unwrap();
        idx.attachments.values().cloned().collect()
    }

    fn save_index(&self, idx: &AttachmentIndex) -> Result<()> {
        let path = self.base_dir.join("index.json");
        let content = serde_json::to_string_pretty(idx)?;
        fs::write(path, content)?;
        Ok(())
    }
}

fn detect_mime_type(filename: &str, bytes: &[u8]) -> String {
    let ext = filename
        .rsplit('.')
        .next()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "png" => "image/png".to_string(),
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "gif" => "image/gif".to_string(),
        "webp" => "image/webp".to_string(),
        "svg" => "image/svg+xml".to_string(),
        "pdf" => "application/pdf".to_string(),
        "json" => "application/json".to_string(),
        "rs" | "rust" => "text/x-rust".to_string(),
        "py" | "python" => "text/x-python".to_string(),
        "js" | "javascript" => "text/javascript".to_string(),
        "ts" | "typescript" => "text/typescript".to_string(),
        "md" | "markdown" => "text/markdown".to_string(),
        "txt" | "log" => "text/plain".to_string(),
        _ => {
            // Sniff magic bytes
            if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                "image/png".to_string()
            } else if bytes.starts_with(b"\xff\xd8\xff") {
                "image/jpeg".to_string()
            } else if bytes.starts_with(b"%PDF") {
                "application/pdf".to_string()
            } else {
                "application/octet-stream".to_string()
            }
        }
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_attachment_store_ingestion_and_deduplication() {
        let dir = tempdir().unwrap();
        let store = AttachmentStore::new(dir.path().to_path_buf()).unwrap();

        let data = b"Hello, M31A attachment world!";
        let att1 = store.ingest_bytes("hello.txt", data, None).unwrap();
        assert_eq!(att1.filename, "hello.txt");
        assert_eq!(att1.mime_type, "text/plain");
        assert_eq!(att1.size_bytes, data.len() as u64);

        // Same content -> same SHA256 ID
        let att2 = store.ingest_bytes("duplicate.txt", data, None).unwrap();
        assert_eq!(att1.id, att2.id);
        assert_eq!(att1.sha256, att2.sha256);

        // Read bytes back
        let read = store.read_bytes(&att1.id).unwrap();
        assert_eq!(read, data);

        let list = store.list_attachments();
        assert_eq!(list.len(), 1);
    }
}
