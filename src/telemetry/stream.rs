//! Append-only NDJSON Stream Writer for High-Volume Telemetry (OBS-01, D-01).
//!
//! Stores raw span events, debug traces, and telemetry payloads in
//! `.m31a/telemetry/<mission_id>.ndjson` without polluting relational SQLite tables.

use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions, create_dir_all, metadata, rename};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::ids::MissionId;

/// Default maximum size before file rotation (50 MB).
pub const DEFAULT_MAX_STREAM_BYTES: u64 = 50 * 1024 * 1024;

/// Append-only NDJSON writer for telemetry records.
#[derive(Debug, Clone)]
pub struct NdjsonStreamWriter {
    base_dir: PathBuf,
    max_file_bytes: u64,
}

impl NdjsonStreamWriter {
    /// Create a new stream writer targeting the given directory.
    ///
    /// canonical construction requires an explicit resolved telemetry dir
    /// (usually `StorageLayout::global_telemetry_dir()`). no pathless
    /// `Default` is provided: implicit `.m31a/telemetry` selection is a
    /// legacy hazard and must never happen in production code.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
            max_file_bytes: DEFAULT_MAX_STREAM_BYTES,
        }
    }

    /// Configure maximum file size before rotation.
    pub fn with_max_bytes(mut self, max_bytes: u64) -> Self {
        self.max_file_bytes = max_bytes;
        self
    }

    /// Resolve the primary stream path for a mission.
    pub fn stream_path(&self, mission_id: &MissionId) -> PathBuf {
        self.base_dir.join(format!("{}.ndjson", mission_id))
    }

    /// Ensure the base directory exists.
    async fn ensure_dir(&self) -> std::io::Result<()> {
        if !self.base_dir.exists() {
            create_dir_all(&self.base_dir).await?;
        }
        Ok(())
    }

    /// Check if rotation is needed and rotate if file exceeds size threshold.
    async fn maybe_rotate(&self, path: &Path, mission_id: &MissionId) -> std::io::Result<()> {
        if let Ok(meta) = metadata(path).await
            && meta.len() >= self.max_file_bytes
        {
            let timestamp = chrono::Utc::now().timestamp_millis();
            let rotated_path = self
                .base_dir
                .join(format!("{}.{}.ndjson", mission_id, timestamp));
            rename(path, rotated_path).await?;
        }
        Ok(())
    }

    /// Append a serialized JSON value as a single NDJSON line.
    pub async fn append_entry(
        &self,
        mission_id: &MissionId,
        entry: &serde_json::Value,
    ) -> std::io::Result<()> {
        self.ensure_dir().await?;
        let path = self.stream_path(mission_id);
        self.maybe_rotate(&path, mission_id).await?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;

        let line = serde_json::to_string(entry)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;

        Ok(())
    }

    /// Read all entries from a mission's primary NDJSON stream.
    pub async fn read_entries(
        &self,
        mission_id: &MissionId,
    ) -> std::io::Result<Vec<serde_json::Value>> {
        let path = self.stream_path(mission_id);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&path).await?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut entries = Vec::new();

        while let Some(line) = lines.next_line().await? {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                entries.push(val);
            }
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_ndjson_append_and_read() {
        let dir = tempdir().unwrap();
        let writer = NdjsonStreamWriter::new(dir.path().to_path_buf());
        let mission_id = MissionId::new();

        let entry1 = serde_json::json!({"event": "start", "seq": 1});
        let entry2 = serde_json::json!({"event": "finish", "seq": 2});

        writer.append_entry(&mission_id, &entry1).await.unwrap();
        writer.append_entry(&mission_id, &entry2).await.unwrap();

        let entries = writer.read_entries(&mission_id).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["event"], "start");
        assert_eq!(entries[1]["event"], "finish");
    }

    #[tokio::test]
    async fn test_ndjson_stream_rotation() {
        let dir = tempdir().unwrap();
        let writer = NdjsonStreamWriter::new(dir.path().to_path_buf()).with_max_bytes(50);
        let mission_id = MissionId::new();

        let entry = serde_json::json!({"message": "this is a fairly long telemetry payload"});
        writer.append_entry(&mission_id, &entry).await.unwrap();
        // Second append should exceed 50 bytes and trigger rotation
        writer.append_entry(&mission_id, &entry).await.unwrap();

        let entries = writer.read_entries(&mission_id).await.unwrap();
        // New file should have 1 entry (the second one)
        assert_eq!(entries.len(), 1);
    }
}
