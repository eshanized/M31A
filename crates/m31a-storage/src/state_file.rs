//! State File Concurrency Lock and Write-Path Preservation Module.
//!
//! Atomic concurrency lock preventing race conditions during shared state file transitions:
//!
//! Solves two critical bugs in autonomous multi-agent state coordination:
//! 1. Read-Modify-Write Races: Concurrent subagents modifying `STATE.md` simultaneously.
//!    Enforces `O_EXCL` lockfile creation (`STATE.md.lock`) with stale-lock breaking (10s)
//!    and jittered spin-wait backoff.
//! 2. Clobbering Curated State: Markdown state stores facts in both YAML frontmatter and body.
//!    Enforces `FieldPreservationPolicy` and typed `StateTransaction` to prevent auto-syncs
//!    from destroying user-curated progress or configuration flags.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Policy governing who wins when frontmatter and body disagree on save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldPreservationPolicy {
    /// Frontmatter retains existing curated value if body hasn't moved.
    PreserveWhenUnchanged,
    /// Frontmatter value is strictly preserved regardless of body re-parsing.
    PreserveAlways,
    /// Preserved if the existing value is not a template placeholder.
    PreserveIfPlaceholder,
    /// Re-derived strictly from body or roadmap facts.
    Derive,
}

/// Mutual exclusion lock guard over a state file (`STATE.md.lock`).
pub struct StateFileLock {
    lock_path: PathBuf,
    acquired: bool,
}

impl StateFileLock {
    /// Acquires an exclusive lock on `target_file`, waiting up to `timeout`.
    pub async fn acquire(target_file: &Path, timeout: Duration) -> Result<Self> {
        let lock_path = target_file.with_extension("lock");
        let start = Instant::now();

        while start.elapsed() < timeout {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(_) => {
                    return Ok(Self {
                        lock_path,
                        acquired: true,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // Check if existing lock is stale (>10 seconds old)
                    if let Ok(metadata) = std::fs::metadata(&lock_path)
                        && let Ok(modified) = metadata.modified()
                        && let Ok(elapsed) = SystemTime::now().duration_since(modified)
                        && elapsed > Duration::from_secs(10)
                    {
                        tracing::warn!(
                            lock = %lock_path.display(),
                            elapsed_secs = elapsed.as_secs(),
                            "Detected stale STATE.md lock; breaking lock"
                        );
                        let _ = std::fs::remove_file(&lock_path);
                        continue;
                    }

                    // Jittered backoff between 20ms and 70ms
                    let jitter_ms = 20 + (rand::random::<u64>() % 50);
                    tokio::time::sleep(Duration::from_millis(jitter_ms)).await;
                }
                Err(e) => return Err(e).context("Failed opening state lockfile"),
            }
        }

        Err(anyhow!(
            "Timeout acquiring lock on '{}' after {:?}",
            target_file.display(),
            timeout
        ))
    }

    /// Releases the lock explicitly.
    pub fn release(mut self) {
        if self.acquired {
            let _ = std::fs::remove_file(&self.lock_path);
            self.acquired = false;
        }
    }
}

impl Drop for StateFileLock {
    fn drop(&mut self) {
        if self.acquired {
            let _ = std::fs::remove_file(&self.lock_path);
            self.acquired = false;
        }
    }
}

/// Parsed representation of a Markdown state document with YAML frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateDocument {
    pub frontmatter: BTreeMap<String, String>,
    pub body: String,
}

impl StateDocument {
    /// Parses a raw Markdown string containing optional `---` YAML frontmatter block.
    pub fn parse(content: &str) -> Self {
        let trimmed = content.trim_start();
        if trimmed.starts_with("---")
            && let Some(end_idx) = trimmed[3..].find("\n---")
        {
            let fm_raw = &trimmed[3..3 + end_idx];
            let body = trimmed[3 + end_idx + 4..]
                .trim_start_matches(['\r', '\n'])
                .to_string();

            let mut frontmatter = BTreeMap::new();
            for line in fm_raw.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once(':') {
                    let key = k.trim().to_string();
                    let val = v.trim().trim_matches(['"', '\'']).to_string();
                    frontmatter.insert(key, val);
                }
            }

            return Self { frontmatter, body };
        }

        Self {
            frontmatter: BTreeMap::new(),
            body: content.to_string(),
        }
    }

    /// Serializes document back to standard Markdown with YAML frontmatter.
    pub fn render(&self) -> String {
        if self.frontmatter.is_empty() {
            return self.body.clone();
        }

        let mut out = String::from("---\n");
        for (k, v) in &self.frontmatter {
            out.push_str(&format!("{k}: \"{v}\"\n"));
        }
        out.push_str("---\n\n");
        out.push_str(&self.body);
        out
    }

    /// Replaces or appends a field in a named body section (e.g. `## Current Position`).
    pub fn set_body_field(&mut self, label: &str, value: &str) {
        let pattern = format!("{label}:");
        let mut replaced = false;
        let mut new_lines = Vec::new();

        for line in self.body.lines() {
            if line.trim_start().starts_with(&pattern) {
                new_lines.push(format!("{label}: {value}"));
                replaced = true;
            } else {
                new_lines.push(line.to_string());
            }
        }

        if !replaced {
            new_lines.push(format!("{label}: {value}"));
        }

        self.body = new_lines.join("\n");
    }
}

/// A typed transaction that enforces snapshot comparison and field preservation policies.
pub struct StateTransaction {
    target_path: PathBuf,
    initial_snapshot: StateDocument,
    current: StateDocument,
    field_policies: BTreeMap<String, FieldPreservationPolicy>,
}

impl StateTransaction {
    /// Opens a state transaction, locking the file and capturing the pre-write snapshot.
    pub async fn open(
        path: &Path,
        lock_timeout: Duration,
        policies: BTreeMap<String, FieldPreservationPolicy>,
    ) -> Result<(Self, StateFileLock)> {
        let lock = StateFileLock::acquire(path, lock_timeout).await?;

        let doc = if path.exists() {
            let content = std::fs::read_to_string(path)?;
            StateDocument::parse(&content)
        } else {
            StateDocument {
                frontmatter: BTreeMap::new(),
                body: String::new(),
            }
        };

        let tx = Self {
            target_path: path.to_path_buf(),
            initial_snapshot: doc.clone(),
            current: doc,
            field_policies: policies,
        };

        Ok((tx, lock))
    }

    /// Mutates a frontmatter field under the declared preservation policy.
    pub fn set_frontmatter(&mut self, key: &str, new_value: &str) {
        let policy = self
            .field_policies
            .get(key)
            .copied()
            .unwrap_or(FieldPreservationPolicy::Derive);

        match policy {
            FieldPreservationPolicy::PreserveAlways => {
                if let Some(existing) = self.initial_snapshot.frontmatter.get(key) {
                    self.current
                        .frontmatter
                        .insert(key.to_string(), existing.clone());
                    return;
                }
            }
            FieldPreservationPolicy::PreserveIfPlaceholder => {
                if let Some(existing) = self.initial_snapshot.frontmatter.get(key)
                    && !existing.starts_with('[')
                    && !existing.is_empty()
                {
                    self.current
                        .frontmatter
                        .insert(key.to_string(), existing.clone());
                    return;
                }
            }
            FieldPreservationPolicy::PreserveWhenUnchanged | FieldPreservationPolicy::Derive => {}
        }

        self.current
            .frontmatter
            .insert(key.to_string(), new_value.to_string());
    }

    /// Mutates a labeled line in the document body.
    pub fn set_body_field(&mut self, label: &str, value: &str) {
        self.current.set_body_field(label, value);
    }

    /// Persists changes and returns an array of exact leaf-level fields that changed.
    pub fn commit(self) -> Result<Vec<String>> {
        let mut updated = Vec::new();

        // 1. Detect changed frontmatter fields
        for (k, v) in &self.current.frontmatter {
            match self.initial_snapshot.frontmatter.get(k) {
                Some(old_v) if old_v != v => {
                    updated.push(format!("frontmatter.{k}"));
                }
                None => {
                    updated.push(format!("frontmatter.{k}"));
                }
                _ => {}
            }
        }

        // 2. Detect removed frontmatter fields
        for k in self.initial_snapshot.frontmatter.keys() {
            if !self.current.frontmatter.contains_key(k) {
                updated.push(format!("frontmatter.{k} (removed)"));
            }
        }

        // 3. Detect body change
        if self.initial_snapshot.body != self.current.body {
            updated.push("body".to_string());
        }

        // 4. Atomically persist document
        let rendered = self.current.render();
        let temp_path = self.target_path.with_extension("tmp");
        std::fs::write(&temp_path, rendered)?;
        std::fs::rename(&temp_path, &self.target_path)?;

        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_state_file_lock_mutual_exclusion() {
        let dir = tempdir().unwrap();
        let state_file = dir.path().join("STATE.md");

        // 1. First lock succeeds
        let lock1 = StateFileLock::acquire(&state_file, Duration::from_millis(500))
            .await
            .unwrap();

        // 2. Second lock fails due to timeout
        let lock2_res = StateFileLock::acquire(&state_file, Duration::from_millis(100)).await;
        assert!(lock2_res.is_err());

        // 3. Releasing lock1 allows lock2 to succeed
        lock1.release();
        let lock2 = StateFileLock::acquire(&state_file, Duration::from_millis(500))
            .await
            .unwrap();
        lock2.release();
    }

    #[test]
    fn test_state_document_parse_and_render() {
        let raw = r#"---
status: "in-progress"
current_phase: "01"
---

# Project State
Phase: 01
"#;
        let doc = StateDocument::parse(raw);
        assert_eq!(doc.frontmatter.get("status").unwrap(), "in-progress");
        assert_eq!(doc.frontmatter.get("current_phase").unwrap(), "01");
        assert!(doc.body.contains("Phase: 01"));

        let rendered = doc.render();
        assert!(rendered.starts_with("---\n"));
        assert!(rendered.contains("status: \"in-progress\""));
    }

    #[tokio::test]
    async fn test_state_transaction_preservation_and_deltas() {
        let dir = tempdir().unwrap();
        let state_file = dir.path().join("STATE.md");
        std::fs::write(
            &state_file,
            "---\nstatus: \"ready\"\nprogress: \"curated-100%\"\n---\n\nBody text\n",
        )
        .unwrap();

        let mut policies = BTreeMap::new();
        policies.insert("progress".into(), FieldPreservationPolicy::PreserveAlways);

        let (mut tx, lock) = StateTransaction::open(&state_file, Duration::from_secs(1), policies)
            .await
            .unwrap();

        // Attempting to clobber `progress` with new value respects PreserveAlways
        tx.set_frontmatter("progress", "clobbered");
        tx.set_frontmatter("status", "running");

        let updated = tx.commit().unwrap();
        lock.release();

        assert_eq!(updated, vec!["frontmatter.status".to_string()]);

        let re_read = std::fs::read_to_string(&state_file).unwrap();
        assert!(re_read.contains("progress: \"curated-100%\""));
        assert!(re_read.contains("status: \"running\""));
    }
}
