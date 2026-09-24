//! Append-only file stream implementation
//!
//! Per PST-02, append-only storage for high-volume event/transcript data
//! without relational overhead. Each stream is a file with newline-delimited entries.

use crate::error::M31AError;
use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Trait for stream storage operations.
#[async_trait::async_trait]
pub trait StreamStore: Send + Sync {
    /// Append data to a named stream.
    async fn append(&self, stream_name: &str, data: &[u8]) -> Result<(), M31AError>;

    /// Read all entries from a named stream.
    async fn read_all(&self, stream_name: &str) -> Result<Vec<Vec<u8>>, M31AError>;
}

/// File-based append-only stream storage.
///
/// Each stream is a file in the base directory. Entries are written
/// as newline-delimited binary data.
pub struct FileStream {
    base_dir: PathBuf,
}

impl FileStream {
    /// Create a new FileStream with the given base directory.
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    /// Get the file path for a stream name.
    fn stream_path(&self, stream_name: &str) -> PathBuf {
        // Sanitize stream name to prevent path traversal
        let safe_name = stream_name.replace(['/', '\\'], "_");
        self.base_dir.join(format!("{}.stream", safe_name))
    }

    /// Ensure the base directory exists.
    async fn ensure_dir(&self) -> Result<(), M31AError> {
        tokio::fs::create_dir_all(&self.base_dir)
            .await
            .map_err(|e| {
                M31AError::persistence(format!("failed to create stream directory: {}", e))
            })
    }
}

#[async_trait::async_trait]
impl StreamStore for FileStream {
    /// Append data to a stream file.
    ///
    /// Opens the file in append mode, writes the data followed by a newline delimiter.
    async fn append(&self, stream_name: &str, data: &[u8]) -> Result<(), M31AError> {
        self.ensure_dir().await?;

        let path = self.stream_path(stream_name);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|e| M31AError::persistence(format!("failed to open stream file: {}", e)))?;

        file.write_all(data)
            .await
            .map_err(|e| M31AError::persistence(format!("failed to write stream data: {}", e)))?;
        file.write_all(b"\n").await.map_err(|e| {
            M31AError::persistence(format!("failed to write stream delimiter: {}", e))
        })?;
        file.flush()
            .await
            .map_err(|e| M31AError::persistence(format!("failed to flush stream: {}", e)))?;

        Ok(())
    }

    /// Read all entries from a stream file.
    ///
    /// Returns a vector of byte vectors, one per line/entry.
    /// Returns empty vector if stream doesn't exist.
    async fn read_all(&self, stream_name: &str) -> Result<Vec<Vec<u8>>, M31AError> {
        let path = self.stream_path(stream_name);

        if !path.exists() {
            return Ok(vec![]);
        }

        let mut file = File::open(&path)
            .await
            .map_err(|e| M31AError::persistence(format!("failed to open stream file: {}", e)))?;

        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .await
            .map_err(|e| M31AError::persistence(format!("failed to read stream file: {}", e)))?;

        // Split on newline delimiter
        let entries: Vec<Vec<u8>> = contents
            .split(|b| *b == b'\n')
            .filter(|chunk| !chunk.is_empty())
            .map(|chunk| chunk.to_vec())
            .collect();

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_file_stream_append_and_read() {
        let _dir = tempdir().unwrap(); // Keep TempDir alive
        let stream = FileStream::new(_dir.path());

        // Append some data
        stream.append("test-stream", b"entry 1").await.unwrap();
        stream.append("test-stream", b"entry 2").await.unwrap();
        stream.append("test-stream", b"entry 3").await.unwrap();

        // Read all entries
        let entries = stream.read_all("test-stream").await.unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0], b"entry 1");
        assert_eq!(entries[1], b"entry 2");
        assert_eq!(entries[2], b"entry 3");
    }

    #[tokio::test]
    async fn test_file_stream_empty_stream() {
        let _dir = tempdir().unwrap(); // Keep TempDir alive
        let stream = FileStream::new(_dir.path());

        // Read from non-existent stream
        let entries = stream.read_all("non-existent").await.unwrap();
        assert_eq!(entries.len(), 0);
    }

    #[tokio::test]
    async fn test_file_stream_multiple_streams() {
        let _dir = tempdir().unwrap(); // Keep TempDir alive
        let stream = FileStream::new(_dir.path());

        stream.append("stream-a", b"data a1").await.unwrap();
        stream.append("stream-b", b"data b1").await.unwrap();
        stream.append("stream-a", b"data a2").await.unwrap();

        let entries_a = stream.read_all("stream-a").await.unwrap();
        let entries_b = stream.read_all("stream-b").await.unwrap();

        assert_eq!(entries_a.len(), 2);
        assert_eq!(entries_a[0], b"data a1");
        assert_eq!(entries_a[1], b"data a2");

        assert_eq!(entries_b.len(), 1);
        assert_eq!(entries_b[0], b"data b1");
    }

    #[tokio::test]
    async fn test_file_stream_path_sanitization() {
        let _dir = tempdir().unwrap(); // Keep TempDir alive
        let stream = FileStream::new(_dir.path());

        // Stream name with path traversal attempts should be sanitized
        stream.append("../evil", b"bad").await.unwrap();
        stream.append("normal", b"good").await.unwrap();

        // Should not have created files outside base_dir
        let entries_normal = stream.read_all("normal").await.unwrap();
        assert_eq!(entries_normal.len(), 1);
        assert_eq!(entries_normal[0], b"good");

        // The sanitized name should work
        let entries_sanitized = stream.read_all(".._evil").await.unwrap();
        assert_eq!(entries_sanitized.len(), 1);
        assert_eq!(entries_sanitized[0], b"bad");
    }

    #[tokio::test]
    async fn test_file_stream_binary_data() {
        let _dir = tempdir().unwrap(); // Keep TempDir alive
        let stream = FileStream::new(_dir.path());

        // Binary data without newlines (newlines are delimiters)
        let binary_data = vec![0u8, 1, 2, 3, 4, 0, 255, 128, 64];
        stream.append("binary", &binary_data).await.unwrap();

        let entries = stream.read_all("binary").await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], binary_data);
    }
}
