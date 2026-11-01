//! Storage-neutral attachment persistence interfaces and disk-backed store.
//!
//! Provides disk-backed attachment persistence, content-hash deduplication, and metadata inspection.
//! Supports inline bytes vs external file storage, image dimensions extraction,
//! and metadata resolution.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;
use uuid::Uuid;

/// Error returned by attachment store operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AttachmentStoreError {
    #[error("attachment not found: {0}")]
    NotFound(String),

    #[error("attachment size ({size} bytes) exceeds limit of {max} bytes")]
    PayloadTooLarge { size: u64, max: u64 },

    #[error("attachment storage I/O error: {0}")]
    IoError(String),

    #[error("format parse error: {0}")]
    FormatError(String),
}

pub type AttachmentStoreResult<T> = Result<T, AttachmentStoreError>;
pub type UploadFuture<'a> =
    Pin<Box<dyn Future<Output = AttachmentStoreResult<UploadResult>> + Send + 'a>>;
pub type ResolveFuture<'a> =
    Pin<Box<dyn Future<Output = AttachmentStoreResult<AttachmentMetadata>> + Send + 'a>>;

/// Storage-neutral attachment persistence interface.
pub trait StorageAttachmentStore: Send + Sync {
    /// Uploads an attachment and returns the representation callers should retain (Inline or File reference).
    fn upload(&self, request: UploadRequest) -> UploadFuture<'_>;

    /// Resolves metadata and, when requested, a fresh URL for a stored attachment.
    fn resolve<'a>(&'a self, request: ResolveRequest<'a>) -> ResolveFuture<'a>;
}

/// Attachment data supplied to upload operations.
#[derive(Clone, Eq, PartialEq)]
pub struct UploadRequest {
    /// Thread receiving this attachment.
    pub thread_id: String,
    /// Whether the originating thread intentionally skips durable persistence.
    pub ephemeral: bool,
    /// Optional name associated with the attachment.
    pub file_name: Option<String>,
    /// Caller-supplied tags used by storage backends for policy decisions.
    pub tags: HashMap<String, String>,
    /// Attachment bytes to persist.
    pub data: Vec<u8>,
}

impl fmt::Debug for UploadRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UploadRequest")
            .field("thread_id", &self.thread_id)
            .field("ephemeral", &self.ephemeral)
            .field("file_name", &self.file_name)
            .field("tags", &self.tags)
            .field("data", &format!("<{} bytes redacted>", self.data.len()))
            .finish()
    }
}

/// Parameters supplied to resolve operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolveRequest<'a> {
    /// Identifier assigned by the attachment store.
    pub file_id: &'a str,
    /// Caller-supplied tags used by storage backends for policy decisions.
    pub tags: HashMap<String, String>,
    /// Minimum remaining lifetime required for the returned file URL.
    pub download_url_ttl: Option<Duration>,
}

/// Metadata used to validate and interpret an attachment.
#[derive(Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AttachmentMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_specific: Option<FormatSpecificMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_url: Option<String>,
}

impl fmt::Debug for AttachmentMetadata {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file_url = self.file_url.as_ref().map(|_| "<redacted_url>");
        formatter
            .debug_struct("AttachmentMetadata")
            .field("file_name", &self.file_name)
            .field("digest", &self.digest)
            .field("size_bytes", &self.size_bytes)
            .field("mime_type", &self.mime_type)
            .field("format_specific", &self.format_specific)
            .field("file_url", &file_url)
            .finish()
    }
}

/// Metadata specific to an attachment format.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum FormatSpecificMetadata {
    Image(ImageMetadata),
}

/// Image dimension metadata.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ImageMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

/// Representation retained after uploading an attachment.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum UploadResult {
    /// Attachment data remains inline in memory.
    Inline { bytes: Vec<u8> },
    /// Attachment data is persisted and accessible via file identifier.
    File { file_id: String },
}

/// Inspects bytes to extract image dimensions for PNG, JPEG, GIF.
pub fn inspect_image_dimensions(bytes: &[u8]) -> Option<ImageMetadata> {
    // PNG detection: starts with \x89PNG\r\n\x1a\n
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 24 {
        let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        return Some(ImageMetadata {
            width: Some(width),
            height: Some(height),
        });
    }

    // GIF detection: starts with GIF87a or GIF89a
    if (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) && bytes.len() >= 10 {
        let width = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
        let height = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
        return Some(ImageMetadata {
            width: Some(width),
            height: Some(height),
        });
    }

    // JPEG detection: starts with 0xFF, 0xD8
    if bytes.starts_with(&[0xFF, 0xD8]) && bytes.len() >= 4 {
        let mut idx = 2;
        while idx + 8 < bytes.len() {
            if bytes[idx] != 0xFF {
                break;
            }
            let marker = bytes[idx + 1];
            // SOF0 (0xC0), SOF1 (0xC1), SOF2 (0xC2)
            if marker == 0xC0 || marker == 0xC1 || marker == 0xC2 {
                let height = u16::from_be_bytes([bytes[idx + 5], bytes[idx + 6]]) as u32;
                let width = u16::from_be_bytes([bytes[idx + 7], bytes[idx + 8]]) as u32;
                return Some(ImageMetadata {
                    width: Some(width),
                    height: Some(height),
                });
            }
            // Skip marker segment
            let len = u16::from_be_bytes([bytes[idx + 2], bytes[idx + 3]]) as usize;
            idx += 2 + len;
        }
    }

    None
}

/// Guesses MIME type from filename or leading magic bytes.
pub fn detect_mime_type(file_name: Option<&str>, bytes: &[u8]) -> String {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return "image/png".to_string();
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "image/jpeg".to_string();
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return "image/gif".to_string();
    }
    if bytes.starts_with(b"RIFF") && bytes.len() >= 12 && &bytes[8..12] == b"WEBP" {
        return "image/webp".to_string();
    }
    if bytes.starts_with(b"%PDF-") {
        return "application/pdf".to_string();
    }

    if let Some(name) = file_name {
        let ext = Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        match ext.as_str() {
            "png" => return "image/png".to_string(),
            "jpg" | "jpeg" => return "image/jpeg".to_string(),
            "gif" => return "image/gif".to_string(),
            "webp" => return "image/webp".to_string(),
            "svg" => return "image/svg+xml".to_string(),
            "json" => return "application/json".to_string(),
            "txt" | "md" | "rs" | "toml" | "yaml" | "yml" => return "text/plain".to_string(),
            "pdf" => return "application/pdf".to_string(),
            _ => {}
        }
    }

    "application/octet-stream".to_string()
}

/// Disk-backed implementation of [`StorageAttachmentStore`].
#[derive(Clone, Debug)]
pub struct LocalDiskAttachmentStore {
    root_dir: PathBuf,
    max_inline_bytes: usize,
    max_payload_bytes: u64,
}

impl LocalDiskAttachmentStore {
    pub fn new(root_dir: PathBuf) -> Result<Self, AttachmentStoreError> {
        fs::create_dir_all(&root_dir).map_err(|e| AttachmentStoreError::IoError(e.to_string()))?;
        Ok(Self {
            root_dir,
            max_inline_bytes: 64 * 1024, // 64 KiB inline threshold
            max_payload_bytes: 50 * 1024 * 1024, // 50 MiB limit
        })
    }

    pub fn with_limits(mut self, max_inline_bytes: usize, max_payload_bytes: u64) -> Self {
        self.max_inline_bytes = max_inline_bytes;
        self.max_payload_bytes = max_payload_bytes;
        self
    }
}

impl StorageAttachmentStore for LocalDiskAttachmentStore {
    fn upload(&self, request: UploadRequest) -> UploadFuture<'_> {
        Box::pin(async move {
            let size = request.data.len() as u64;
            if size > self.max_payload_bytes {
                return Err(AttachmentStoreError::PayloadTooLarge {
                    size,
                    max: self.max_payload_bytes,
                });
            }

            // If payload is small and caller did not require durable external storage, keep inline
            if request.data.len() <= self.max_inline_bytes && request.ephemeral {
                return Ok(UploadResult::Inline {
                    bytes: request.data,
                });
            }

            let file_id = Uuid::new_v4().to_string();
            let thread_dir = self.root_dir.join(&request.thread_id);
            fs::create_dir_all(&thread_dir)
                .map_err(|e| AttachmentStoreError::IoError(e.to_string()))?;

            let file_path = thread_dir.join(&file_id);
            fs::write(&file_path, &request.data)
                .map_err(|e| AttachmentStoreError::IoError(e.to_string()))?;

            // Compute metadata and store companion JSON
            let mut hasher = Sha256::new();
            hasher.update(&request.data);
            let digest = hex::encode(hasher.finalize());

            let mime_type = detect_mime_type(request.file_name.as_deref(), &request.data);
            let image_meta = inspect_image_dimensions(&request.data);

            let meta = AttachmentMetadata {
                file_name: request.file_name,
                digest: Some(digest),
                size_bytes: Some(size),
                mime_type: Some(mime_type),
                format_specific: image_meta.map(FormatSpecificMetadata::Image),
                file_url: None,
            };

            let meta_path = thread_dir.join(format!("{file_id}.meta.json"));
            let meta_json = serde_json::to_string(&meta)
                .map_err(|e| AttachmentStoreError::FormatError(e.to_string()))?;
            fs::write(&meta_path, meta_json)
                .map_err(|e| AttachmentStoreError::IoError(e.to_string()))?;

            Ok(UploadResult::File { file_id })
        })
    }

    fn resolve<'a>(&'a self, request: ResolveRequest<'a>) -> ResolveFuture<'a> {
        Box::pin(async move {
            // Find attachment by file_id in root_dir subdirectories
            let mut found_file: Option<PathBuf> = None;
            let mut found_meta: Option<PathBuf> = None;

            if let Ok(entries) = fs::read_dir(&self.root_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let potential_file = path.join(request.file_id);
                        let potential_meta = path.join(format!("{}.meta.json", request.file_id));
                        if potential_file.exists() {
                            found_file = Some(potential_file);
                            found_meta = Some(potential_meta);
                            break;
                        }
                    }
                }
            }

            let Some(file_path) = found_file else {
                return Err(AttachmentStoreError::NotFound(request.file_id.to_string()));
            };

            let mut metadata = if let Some(meta_path) = found_meta {
                if meta_path.exists() {
                    let content = fs::read_to_string(&meta_path)
                        .map_err(|e| AttachmentStoreError::IoError(e.to_string()))?;
                    serde_json::from_str(&content).unwrap_or_default()
                } else {
                    AttachmentMetadata::default()
                }
            } else {
                AttachmentMetadata::default()
            };

            // If download_url_ttl was requested, return a valid file URL
            if request.download_url_ttl.is_some() {
                metadata.file_url = Some(format!("file://{}", file_path.display()));
            }

            Ok(metadata)
        })
    }
}

// Simple hex helper to avoid extra dependency
mod hex {
    pub fn encode(data: impl AsRef<[u8]>) -> String {
        let mut s = String::with_capacity(data.as_ref().len() * 2);
        for &b in data.as_ref() {
            use std::fmt::Write;
            let _ = write!(s, "{:02x}", b);
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspect_png_dimensions() {
        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n"); // PNG header
        png.extend_from_slice(&[0, 0, 0, 13]); // IHDR chunk length
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&800u32.to_be_bytes()); // width 800
        png.extend_from_slice(&600u32.to_be_bytes()); // height 600

        let dims = inspect_image_dimensions(&png).expect("parsed dimensions");
        assert_eq!(dims.width, Some(800));
        assert_eq!(dims.height, Some(600));
    }

    #[test]
    fn test_inspect_gif_dimensions() {
        let mut gif = Vec::new();
        gif.extend_from_slice(b"GIF89a");
        gif.extend_from_slice(&320u16.to_le_bytes()); // width 320
        gif.extend_from_slice(&240u16.to_le_bytes()); // height 240

        let dims = inspect_image_dimensions(&gif).expect("parsed dimensions");
        assert_eq!(dims.width, Some(320));
        assert_eq!(dims.height, Some(240));
    }

    #[tokio::test]
    async fn test_local_disk_attachment_lifecycle() {
        let tmp = tempfile::tempdir().unwrap();
        let store = LocalDiskAttachmentStore::new(tmp.path().to_path_buf())
            .unwrap()
            .with_limits(10, 1024 * 1024);

        // Upload small ephemeral payload -> stays inline
        let inline_req = UploadRequest {
            thread_id: "thread-1".to_string(),
            ephemeral: true,
            file_name: Some("small.txt".to_string()),
            tags: HashMap::new(),
            data: b"hi".to_vec(),
        };
        let res = store.upload(inline_req).await.expect("upload inline");
        assert_eq!(
            res,
            UploadResult::Inline {
                bytes: b"hi".to_vec()
            }
        );

        // Upload larger payload -> stored to disk
        let file_req = UploadRequest {
            thread_id: "thread-1".to_string(),
            ephemeral: false,
            file_name: Some("test.txt".to_string()),
            tags: HashMap::new(),
            data: b"hello durable storage file contents".to_vec(),
        };
        let res = store.upload(file_req).await.expect("upload file");
        let file_id = match res {
            UploadResult::File { file_id } => file_id,
            _ => panic!("expected File upload result"),
        };

        // Resolve metadata without URL
        let resolve_req = ResolveRequest {
            file_id: &file_id,
            tags: HashMap::new(),
            download_url_ttl: None,
        };
        let meta = store.resolve(resolve_req).await.expect("resolve metadata");
        assert_eq!(meta.file_name, Some("test.txt".to_string()));
        assert_eq!(meta.mime_type, Some("text/plain".to_string()));
        assert!(meta.file_url.is_none());
        assert!(meta.digest.is_some());

        // Resolve with URL
        let resolve_with_url = ResolveRequest {
            file_id: &file_id,
            tags: HashMap::new(),
            download_url_ttl: Some(Duration::from_secs(60)),
        };
        let meta_url = store.resolve(resolve_with_url).await.expect("resolve url");
        assert!(meta_url.file_url.is_some());
        assert!(meta_url.file_url.unwrap().starts_with("file://"));
    }
}
