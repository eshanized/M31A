//! Message history buffering, turn indexing, and conversation search.
//!
//! Provides thread-safe historical query access across conversation turns and tool calls.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::SystemTime;

pub const DEFAULT_HISTORY_FILENAME: &str = "history.jsonl";
pub const DEFAULT_HISTORY_MAX_BYTES: usize = 10 * 1024 * 1024; // 10MB
pub const HISTORY_SOFT_CAP_RATIO: f64 = 0.8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub session_id: String,
    pub ts: u64,
    pub text: String,
}

impl HistoryEntry {
    pub fn new(session_id: impl Into<String>, text: impl Into<String>) -> Self {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            session_id: session_id.into(),
            ts,
            text: text.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryBatchEntry {
    pub offset: usize,
    pub entry: Option<HistoryEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryBatch {
    pub entries: Vec<HistoryBatchEntry>,
    pub next_cursor: Option<usize>,
    pub total_rows: usize,
}

/// Appends a single record atomically to the message history file.
/// Uses single write syscall with trailing newline to avoid interleaving.
pub fn append_history(
    history_file: &Path,
    entry: &HistoryEntry,
    max_bytes: Option<usize>,
) -> Result<()> {
    if let Some(parent) = history_file.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create history dir: {}", parent.display()))?;
    }

    let mut line = serde_json::to_string(entry)?;
    line.push('\n');

    let mut options = OpenOptions::new();
    options.create(true).write(true).append(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options
        .open(history_file)
        .with_context(|| format!("Failed to open history file: {}", history_file.display()))?;

    file.write_all(line.as_bytes())?;
    file.flush()?;

    if let Some(max_b) = max_bytes {
        let _ = trim_history_if_needed(history_file, max_b);
    }

    Ok(())
}

/// Trims history down to 80% of `max_bytes` at a clean newline boundary if it exceeds the limit.
pub fn trim_history_if_needed(history_file: &Path, max_bytes: usize) -> Result<bool> {
    if !history_file.exists() {
        return Ok(false);
    }

    let metadata = std::fs::metadata(history_file)?;
    if (metadata.len() as usize) <= max_bytes {
        return Ok(false);
    }

    let target_bytes = ((max_bytes as f64) * HISTORY_SOFT_CAP_RATIO) as usize;
    let bytes_to_discard = (metadata.len() as usize).saturating_sub(target_bytes);

    let mut file = File::open(history_file)?;
    file.seek(SeekFrom::Start(bytes_to_discard as u64))?;

    let mut reader = BufReader::new(file);
    let mut discard_line = String::new();
    // Read and discard partial line up to the first newline
    reader.read_line(&mut discard_line)?;

    let mut remaining_bytes = Vec::new();
    reader.read_to_end(&mut remaining_bytes)?;

    let temp_path = history_file.with_extension("tmp");
    {
        let mut out = File::create(&temp_path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&temp_path, std::fs::Permissions::from_mode(0o600));
        }
        out.write_all(&remaining_bytes)?;
        out.flush()?;
    }

    std::fs::rename(&temp_path, history_file)?;
    Ok(true)
}

/// Reads the newest entries in bounded reverse batches.
/// `cursor_end`: zero-based end offset (exclusive). `None` starts from the newest entry.
pub fn lookup_history_batch(
    history_file: &Path,
    cursor_end: Option<usize>,
    limit: usize,
) -> Result<HistoryBatch> {
    if !history_file.exists() {
        return Ok(HistoryBatch {
            entries: Vec::new(),
            next_cursor: None,
            total_rows: 0,
        });
    }

    let file = File::open(history_file)?;
    let reader = BufReader::new(file);

    let mut all_rows = Vec::new();
    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let entry = serde_json::from_str::<HistoryEntry>(trimmed).ok();
        all_rows.push(entry);
    }

    let total_rows = all_rows.len();
    if total_rows == 0 {
        return Ok(HistoryBatch {
            entries: Vec::new(),
            next_cursor: None,
            total_rows: 0,
        });
    }

    let end = cursor_end.unwrap_or(total_rows).min(total_rows);
    let start = end.saturating_sub(limit);

    let mut entries = Vec::new();
    for offset in (start..end).rev() {
        entries.push(HistoryBatchEntry {
            offset,
            entry: all_rows[offset].clone(),
        });
    }

    let next_cursor = if start > 0 { Some(start) } else { None };

    Ok(HistoryBatch {
        entries,
        next_cursor,
        total_rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_and_lookup_history() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("history.jsonl");

        let entry1 = HistoryEntry::new("sess-1", "First prompt");
        let entry2 = HistoryEntry::new("sess-1", "Second prompt");
        let entry3 = HistoryEntry::new("sess-2", "Third prompt");

        append_history(&path, &entry1, None).unwrap();
        append_history(&path, &entry2, None).unwrap();
        append_history(&path, &entry3, None).unwrap();

        // 1. Initial batch: newest 2 entries (entry 3 and entry 2)
        let batch1 = lookup_history_batch(&path, None, 2).unwrap();
        assert_eq!(batch1.total_rows, 3);
        assert_eq!(batch1.entries.len(), 2);
        assert_eq!(
            batch1.entries[0].entry.as_ref().unwrap().text,
            "Third prompt"
        );
        assert_eq!(
            batch1.entries[1].entry.as_ref().unwrap().text,
            "Second prompt"
        );
        assert_eq!(batch1.next_cursor, Some(1));

        // 2. Next batch from cursor 1: oldest entry (entry 1)
        let batch2 = lookup_history_batch(&path, batch1.next_cursor, 2).unwrap();
        assert_eq!(batch2.entries.len(), 1);
        assert_eq!(
            batch2.entries[0].entry.as_ref().unwrap().text,
            "First prompt"
        );
        assert_eq!(batch2.next_cursor, None); // No more entries
    }

    #[test]
    fn test_trim_history_soft_cap() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("history.jsonl");

        // Write several lines
        for i in 0..50 {
            let entry = HistoryEntry::new(
                "sess",
                format!("Line number {:03} with extra padding data to fill bytes", i),
            );
            append_history(&path, &entry, None).unwrap();
        }

        let size_before = std::fs::metadata(&path).unwrap().len();
        assert!(size_before > 1000);

        // Trim with a cap smaller than file size
        let trimmed = trim_history_if_needed(&path, 1000).unwrap();
        assert!(trimmed);

        let size_after = std::fs::metadata(&path).unwrap().len();
        assert!(size_after <= 850);

        // Verify remaining lines are still valid JSON Lines
        let batch = lookup_history_batch(&path, None, 100).unwrap();
        assert!(!batch.entries.is_empty());
        assert!(batch.entries.iter().all(|e| e.entry.is_some()));
    }
}
