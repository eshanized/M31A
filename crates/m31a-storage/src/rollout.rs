//! Append-only JSONL rollout logging for crash-resilient session reconstruction and telemetry.
//!
//! Each thread's history is written to `rollout-<timestamp>-<thread_id>.jsonl`
//! under a sessions root: SQLite stays the query index, while JSONL is the
//! replayable append-only record. Architecture:
//!
//! - **Deferred file creation** — no file until the first successful persist
//!   (ephemeral sessions leave no file behind).
//! - **Buffered items with monotonic ordinals** — items are enqueued with a
//!   per-thread, monotonically increasing ordinal; ordinals are never reused
//!   and never compared across threads.
//! - **Drain only successfully written items** — a partial write failure
//!   leaves the un-written remainder buffered.
//! - **Recovery** — on write failure the file handle is dropped, buffered
//!   items are retained, and the write is retried once onto a fresh
//!   attempt-suffixed rollout file (`write_pending_with_recovery`).
//! - **Append-fixup on reopen** — `recover` checks newline termination and
//!   resumes ordinals from the file's maximum.
//!
//! Persist policy: payloads are byte-truncated at 64 KiB (upstream command /
//! MCP output persist budget).

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Upstream persist policy: command/MCP outputs are truncated at 64 KB before
/// persisting to a rollout.
pub const PERSIST_MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// Byte-truncates a payload for persist (64 KB policy).
pub fn truncate_payload_for_persist(payload: &str) -> String {
    if payload.len() <= PERSIST_MAX_OUTPUT_BYTES {
        return payload.to_string();
    }
    let mut end = PERSIST_MAX_OUTPUT_BYTES;
    while end > 0 && !payload.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n[Truncated for persist: {} of {} bytes]",
        &payload[..end],
        end,
        payload.len()
    )
}

/// One buffered rollout record.
#[derive(Debug, Clone, PartialEq)]
pub struct RolloutItem {
    pub ordinal: u64,
    pub ts_ms: i64,
    pub kind: String,
    pub payload: Value,
}

impl RolloutItem {
    pub fn to_line(&self) -> String {
        serde_json::json!({
            "ordinal": self.ordinal,
            "ts_ms": self.ts_ms,
            "kind": self.kind,
            "payload": self.payload,
        })
        .to_string()
    }
}

/// A writer for one thread's rollout file. File creation is deferred until
/// the first successful `write_pending`.
pub struct RolloutWriter {
    dir: PathBuf,
    thread_id: String,
    /// Resolved rollout file path; `None` until the first persist creates it.
    path: Option<PathBuf>,
    file: Option<File>,
    buffer: VecDeque<RolloutItem>,
    next_ordinal: u64,
    attempt: u32,
}

impl RolloutWriter {
    /// Creates a deferred writer: nothing exists on disk yet.
    pub fn new(dir: &Path, thread_id: impl Into<String>) -> Self {
        Self {
            dir: dir.to_path_buf(),
            thread_id: thread_id.into(),
            path: None,
            file: None,
            buffer: VecDeque::new(),
            next_ordinal: 0,
            attempt: 0,
        }
    }

    /// Reopens a thread's rollout history (recovery path): scans ALL matching
    /// rollout files in name order (primary file precedes its recovery
    /// attempts lexicographically), verifies newline termination of the last
    /// file (append-fixup), and resumes ordinals past the global maximum.
    /// The append target is the latest file.
    pub fn recover(dir: &Path, thread_id: &str) -> Result<Option<Self>> {
        let files = rollout_files_for(dir, thread_id)?;
        if files.is_empty() {
            return Ok(None);
        }
        let target = files.last().cloned().expect("non-empty");

        let mut next_ordinal = 0u64;
        for path in &files {
            next_ordinal = next_ordinal.max(scan_max_ordinal(path)?);
        }

        // Append-fixup: a file whose last line lost its newline (e.g. a crash
        // mid-write) gets its terminator restored before any future append.
        fixup_newline_termination(&target)?;

        // Preserve an existing recovery-attempt suffix so a subsequent
        // recovery retry does not regenerate the same name.
        let name = target
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let attempt = name
            .rsplit_once("_r")
            .and_then(|(_, tail)| tail.strip_suffix(".jsonl"))
            .and_then(|tail| tail.parse::<u32>().ok())
            .unwrap_or(0);

        Ok(Some(Self {
            dir: dir.to_path_buf(),
            thread_id: thread_id.to_string(),
            path: Some(target),
            file: None,
            buffer: VecDeque::new(),
            next_ordinal,
            attempt,
        }))
    }

    /// Enqueues a record; assigns the next monotonic ordinal. The file is NOT
    /// created by this call (deferred creation).
    pub fn enqueue(&mut self, kind: &str, payload: &Value) -> u64 {
        let ordinal = self.next_ordinal;
        self.next_ordinal += 1;
        self.buffer.push_back(RolloutItem {
            ordinal,
            ts_ms: chrono::Utc::now().timestamp_millis(),
            kind: kind.to_string(),
            payload: payload.clone(),
        });
        ordinal
    }

    pub fn pending_count(&self) -> usize {
        self.buffer.len()
    }

    /// Resolved rollout path, if the file has been created.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn is_created(&self) -> bool {
        self.file.is_some() || self.path.is_some()
    }

    /// Writes all buffered items, appending to (or creating) the rollout file.
    /// Only successfully written items are drained; on failure the remaining
    /// buffer is intact.
    pub fn write_pending(&mut self) -> Result<usize> {
        if self.buffer.is_empty() {
            return Ok(0);
        }
        if self.file.is_none() {
            let path = self.rollout_name(self.attempt);
            std::fs::create_dir_all(&self.dir)
                .with_context(|| format!("Failed creating sessions dir: {}", self.dir.display()))?;
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .with_context(|| format!("Failed opening rollout: {}", path.display()))?;
            self.file = Some(file);
            self.path = Some(path);
        }

        let mut written = 0usize;
        // Drain only what successfully writes: any error leaves the rest buffered.
        while let Some(item) = self.buffer.front() {
            let line = item.to_line();
            let file = self.file.as_mut().expect("file opened above");
            if let Err(e) = file
                .write_all(line.as_bytes())
                .and_then(|_| file.write_all(b"\n"))
                .and_then(|_| file.flush())
            {
                self.file = None; // drop the handle; buffer retains `item` and the rest
                let _ = e;
                break;
            }
            self.buffer.pop_front();
            written += 1;
        }
        Ok(written)
    }

    /// Upstream recovery path: on write failure, drop the writer (handle +
    /// current file reference), retain buffered items, and retry ONCE onto a
    /// fresh attempt-suffixed rollout file.
    pub fn write_pending_with_recovery(&mut self) -> Result<usize> {
        match self.write_pending() {
            Ok(n) => Ok(n),
            Err(_) => {
                // Drop the writer state; a fresh file name avoids retrying
                // onto a possibly-corrupt file.
                self.file = None;
                self.attempt += 1;
                let path = self.rollout_name(self.attempt);
                std::fs::create_dir_all(&self.dir)?;
                let file = OpenOptions::new().create(true).append(true).open(&path)?;
                self.file = Some(file);
                self.path = Some(path);
                self.write_pending()
            }
        }
    }

    fn rollout_name(&self, attempt: u32) -> PathBuf {
        let ts = chrono::Local::now().format("%Y-%m-%dT%H-%M-%S");
        let name = if attempt == 0 {
            format!("rollout-{ts}-{}.jsonl", self.thread_id)
        } else {
            format!("rollout-{ts}-{}_r{}.jsonl", self.thread_id, attempt)
        };
        self.dir.join(name)
    }
}

/// All rollout files for a thread, sorted lexicographically (name order:
/// `rollout-<ts>-<id>.jsonl` precedes `rollout-<ts>-<id>_rN.jsonl`, and the
/// embedded timestamp format sorts chronologically). The LAST file is the
/// append target.
fn rollout_files_for(dir: &Path, thread_id: &str) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let primary = format!("-{thread_id}.jsonl");
    let recovery = format!("-{thread_id}_r");
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("rollout-")
            && name.ends_with(".jsonl")
            && (name.ends_with(&primary) || name.contains(&recovery))
        {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

/// Reads one rollout file's records; malformed lines are skipped (replay is
/// best-effort over whatever survived).
fn scan_records(path: &Path) -> Result<Vec<RolloutItem>> {
    let file =
        File::open(path).with_context(|| format!("Failed reading rollout: {}", path.display()))?;
    let mut items = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
            items.push(RolloutItem {
                ordinal: v.get("ordinal").and_then(|x| x.as_u64()).unwrap_or(0),
                ts_ms: v.get("ts_ms").and_then(|x| x.as_i64()).unwrap_or(0),
                kind: v
                    .get("kind")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string(),
                payload: v.get("payload").cloned().unwrap_or(Value::Null),
            });
        }
    }
    Ok(items)
}

fn scan_max_ordinal(path: &Path) -> Result<u64> {
    Ok(scan_records(path)?
        .iter()
        .map(|i| i.ordinal + 1)
        .max()
        .unwrap_or(0))
}

/// Append-fixup: restores a missing trailing newline so future appends start
/// on a fresh line.
fn fixup_newline_termination(path: &Path) -> Result<()> {
    let file =
        File::open(path).with_context(|| format!("Failed reading rollout: {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut last_line_terminated = true;
    let mut last_len = 0usize;
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            break;
        }
        last_line_terminated = line.ends_with('\n');
        last_len = n;
    }
    drop(reader);
    if !last_line_terminated && last_len > 0 {
        let mut fix = OpenOptions::new().append(true).open(path)?;
        fix.write_all(b"\n")?;
        fix.flush()?;
    }
    Ok(())
}

/// Replays a thread's full rollout history: all matching files in name order
/// (primary first, then recovery attempts), records concatenated.
pub fn read_rollout(dir: &Path, thread_id: &str) -> Result<Vec<RolloutItem>> {
    let mut items = Vec::new();
    for path in rollout_files_for(dir, thread_id)? {
        items.extend(scan_records(&path)?);
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;

    fn writable_tempdir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn test_deferred_file_creation_and_naming() {
        let dir = writable_tempdir();
        let mut w = RolloutWriter::new(dir.path(), "thread-1");

        // Nothing on disk before first persist (deferred creation).
        assert!(!w.is_created());
        assert!(w.write_pending().unwrap() == 0);
        assert!(!w.is_created());

        // First item creates the file with the upstream naming shape.
        let ord = w.enqueue("session_meta", &json!({"cwd": "/tmp"}));
        assert_eq!(ord, 0);
        w.write_pending().expect("write");
        assert!(w.is_created());
        let path = w.path().unwrap().to_path_buf();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("rollout-"), "name: {name}");
        assert!(name.contains("-thread-1.jsonl"), "name: {name}");
        assert!(name.ends_with(".jsonl"));

        // File contains the JSON record with ordinal 0.
        let content = std::fs::read_to_string(&path).unwrap();
        let record: Value = serde_json::from_str(content.trim_end()).unwrap();
        assert_eq!(record["ordinal"], 0);
        assert_eq!(record["kind"], "session_meta");
    }

    #[test]
    fn test_monotonic_ordinals_and_roundtrip() {
        let dir = writable_tempdir();
        let mut w = RolloutWriter::new(dir.path(), "thread-2");
        w.enqueue("item", &json!({"i": 1}));
        w.enqueue("item", &json!({"i": 2}));
        w.enqueue("session_meta", &json!({"cwd": "/x"}));
        w.write_pending().unwrap();

        let path = w.path().unwrap().to_path_buf();
        let content = std::fs::read_to_string(&path).unwrap();
        let ordinals: Vec<u64> = content
            .lines()
            .map(|l| {
                serde_json::from_str::<Value>(l).unwrap()["ordinal"]
                    .as_u64()
                    .unwrap()
            })
            .collect();
        assert_eq!(ordinals, vec![0, 1, 2]);
        assert_eq!(w.pending_count(), 0);
    }

    #[test]
    fn test_drain_only_successful_and_recovery_retry() {
        let dir = writable_tempdir();

        // Root bypasses DAC — permission-based failure simulation cannot be
        // observed (visibility check, not a silent pass).
        let probe = dir.path().join("probe.txt");
        let root = std::fs::write(&probe, b"x").is_ok() && {
            let mut perms = std::fs::metadata(&probe).unwrap().permissions();
            perms.set_mode(0o400);
            std::fs::set_permissions(&probe, perms).is_ok() && std::fs::write(&probe, b"y").is_ok()
        };
        std::fs::remove_file(&probe).ok();
        if root {
            eprintln!("running as root — skipping permission-failure assertions");
            return;
        }

        let mut w = RolloutWriter::new(dir.path(), "thread-3");
        w.enqueue("item", &json!({"i": 1}));
        w.enqueue("item", &json!({"i": 2}));
        w.write_pending().unwrap();
        assert_eq!(w.pending_count(), 0);
        let original_path = w.path().unwrap().to_path_buf();

        // Simulate a crash mid-session: the handle is lost; on restart the
        // persisted file has been made read-only → re-opening for append
        // fails with EACCES (the crash-recovery scenario).
        w.file = None;
        let mut perms = std::fs::metadata(&original_path).unwrap().permissions();
        perms.set_mode(0o400);
        std::fs::set_permissions(&original_path, perms).unwrap();

        let ord = w.enqueue("item", &json!({"i": 3}));
        assert_eq!(ord, 2, "ordinal continues across the failure");
        let result = w.write_pending();
        assert!(result.is_err(), "write must fail against a read-only file");
        assert_eq!(w.pending_count(), 1, "failed item must remain buffered");
        assert_eq!(
            w.path(),
            Some(original_path.as_path()),
            "path survives the failure"
        );

        // Upstream recovery: retry ONCE onto a fresh attempt-suffixed file.
        let written = w.write_pending_with_recovery().expect("recovery write");
        assert_eq!(written, 1);
        assert_eq!(w.pending_count(), 0);

        let recovery_path = w.path().unwrap().to_path_buf();
        assert_ne!(
            recovery_path, original_path,
            "recovery must use a fresh file"
        );
        let name = recovery_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert!(name.contains("_r1.jsonl"), "recovery name: {name}");
        let content = std::fs::read_to_string(&recovery_path).unwrap();
        let record: Value = serde_json::from_str(content.trim_end()).unwrap();
        assert_eq!(
            record["ordinal"], 2,
            "ordinals are globally monotonic across recovery"
        );
        assert_eq!(record["payload"]["i"], 3);

        // Full replay concatenates [primary(0,1), _r1(2)].
        let items = read_rollout(dir.path(), "thread-3").expect("replay");
        assert_eq!(
            items.iter().map(|i| i.ordinal).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );

        // Restore for TempDir cleanup.
        let mut restore = std::fs::metadata(&original_path).unwrap().permissions();
        restore.set_mode(0o600);
        std::fs::set_permissions(&original_path, restore).unwrap();
    }

    #[test]
    fn test_recover_resumes_past_all_files() {
        // After a recovery retry, recover() must resume ordinals past the
        // max across ALL files (primary + _r1), and target the latest file.
        let dir = writable_tempdir();
        let mut w = RolloutWriter::new(dir.path(), "thread-6");
        w.enqueue("item", &json!({"i": 1}));
        w.enqueue("item", &json!({"i": 2}));
        w.write_pending().unwrap();

        // Real recovery sequence: crash (handle lost) + read-only primary.
        w.file = None;
        let orig = w.path().unwrap().to_path_buf();
        let mut perms = std::fs::metadata(&orig).unwrap().permissions();
        perms.set_mode(0o400);
        std::fs::set_permissions(&orig, perms).unwrap();
        w.enqueue("item", &json!({"i": 3}));
        assert!(w.write_pending().is_err());
        w.write_pending_with_recovery().expect("recovery retry");

        let r1_name = w
            .path()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert!(r1_name.contains("_r1.jsonl"), "recovery name: {r1_name}");

        // Reopen: ordinals resume past 2; appends land in the _r1 file.
        let mut w2 = RolloutWriter::recover(dir.path(), "thread-6")
            .expect("recover")
            .expect("rollout exists");
        assert_eq!(
            w2.path().unwrap().file_name().unwrap().to_string_lossy(),
            r1_name,
            "recover must target the latest file"
        );
        assert_eq!(
            w2.enqueue("item", &json!({"i": 4})),
            3,
            "ordinal resumes past all files"
        );
        w2.write_pending().unwrap();

        let items = read_rollout(dir.path(), "thread-6").expect("replay");
        assert_eq!(
            items.iter().map(|i| i.ordinal).collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );

        // Restore for TempDir cleanup.
        let mut restore = std::fs::metadata(&orig).unwrap().permissions();
        restore.set_mode(0o600);
        std::fs::set_permissions(&orig, restore).unwrap();
    }

    #[test]
    fn test_recover_resumes_ordinals_and_appends() {
        let dir = writable_tempdir();
        let mut w = RolloutWriter::new(dir.path(), "thread-4");
        w.enqueue("item", &json!({"i": 1}));
        w.enqueue("item", &json!({"i": 2}));
        w.write_pending().unwrap();

        // Recover: ordinals resume at max+1; new appends land in the SAME file.
        let mut w2 = RolloutWriter::recover(dir.path(), "thread-4")
            .expect("recover")
            .expect("rollout exists");
        assert_eq!(w2.path().unwrap(), w.path().unwrap());
        assert_eq!(
            w2.enqueue("item", &json!({"i": 3})),
            2,
            "ordinal resumes at max+1"
        );
        w2.write_pending().unwrap();

        let content = std::fs::read_to_string(w2.path().unwrap()).unwrap();
        let ordinals: Vec<u64> = content
            .lines()
            .map(|l| {
                serde_json::from_str::<Value>(l).unwrap()["ordinal"]
                    .as_u64()
                    .unwrap()
            })
            .collect();
        assert_eq!(ordinals, vec![0, 1, 2]);
    }

    #[test]
    fn test_recover_fixes_missing_newline_termination() {
        let dir = writable_tempdir();
        let mut w = RolloutWriter::new(dir.path(), "thread-5");
        w.enqueue("item", &json!({"i": 1}));
        w.write_pending().unwrap();
        let path = w.path().unwrap().to_path_buf();

        // Simulate a crash mid-write: strip the trailing newline.
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.ends_with('\n'));
        std::fs::write(&path, content.trim_end()).unwrap();

        let mut w2 = RolloutWriter::recover(dir.path(), "thread-5")
            .expect("recover")
            .expect("rollout exists");
        assert_eq!(w2.enqueue("item", &json!({"i": 2})), 1);
        w2.write_pending().unwrap();

        // After fixup the file must parse line-by-line with no merged lines.
        let content = std::fs::read_to_string(&path).unwrap();
        for line in content.lines() {
            let v: Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("unparseable line after fixup: {line:?} ({e})"));
            assert!(v.get("ordinal").is_some());
        }
    }

    #[test]
    fn test_recover_no_rollout_returns_none() {
        let dir = writable_tempdir();
        let recovered = RolloutWriter::recover(dir.path(), "never-existed").unwrap();
        assert!(recovered.is_none());
    }

    #[test]
    fn test_persist_truncation_64kb() {
        let small = "x".repeat(1000);
        assert_eq!(truncate_payload_for_persist(&small), small);

        let big = "y".repeat(PERSIST_MAX_OUTPUT_BYTES + 5_000);
        let out = truncate_payload_for_persist(&big);
        assert!(out.len() < PERSIST_MAX_OUTPUT_BYTES + 200);
        assert!(out.contains("[Truncated for persist:"));
        // Char-boundary safety: multibyte payload truncates cleanly too.
        let multibyte = "é".repeat(PERSIST_MAX_OUTPUT_BYTES / 2 + 100); // 2 bytes each
        let out = truncate_payload_for_persist(&multibyte);
        assert!(out.contains("[Truncated for persist:"));
        // The truncated head is valid UTF-8 (parse roundtrip proves it).
        let head = out.split('\n').next().unwrap();
        assert!(std::str::from_utf8(head.as_bytes()).is_ok());
    }
}
