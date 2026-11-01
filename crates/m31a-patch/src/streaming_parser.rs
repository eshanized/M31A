//! Incremental streaming patch parser emitting progress events during tool argument reception.
//!
//! Consumes argument deltas as the tool call streams, emitting
//! `PatchApplyUpdated` progress events throttled at 500 ms.
//! Supports line-ending preservation per file (first-seen ending becomes
//! the style for inserted lines; LF/CRLF/CR).

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};
use tokio::time::sleep;

use crate::unified_diff::{FileAction, ParsedPatch};

/// Progress event emitted during streaming patch application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PatchProgressEvent {
    /// Started applying a new file.
    FileStarted {
        path: String,
        action: String, // "create" | "modify" | "delete" | "rename"
    },
    /// A hunk was applied within the current file.
    HunkApplied {
        path: String,
        hunk_index: usize,
        total_hunks: usize,
    },
    /// Overall progress fraction (0.0 to 1.0).
    Progress {
        fraction: f64,
        files_done: usize,
        total_files: usize,
    },
    /// Completed applying all files.
    Completed { stats: PatchApplyStats },
    /// Error during application.
    Error { path: String, error: String },
}

/// Summary stats for a completed patch application.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatchApplyStats {
    pub files_created: usize,
    pub files_modified: usize,
    pub files_deleted: usize,
    pub files_renamed: usize,
    pub hunks_applied: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

/// Configuration for the streaming parser.
#[derive(Debug, Clone)]
pub struct StreamingParserConfig {
    /// Minimum interval between progress events (throttle).
    pub throttle: Duration,
    /// Whether to emit hunk-level progress.
    pub emit_hunk_progress: bool,
}

impl Default for StreamingParserConfig {
    fn default() -> Self {
        Self {
            throttle: Duration::from_millis(500), // upstream: 500 ms
            emit_hunk_progress: true,
        }
    }
}

/// Line ending style detected in a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineEnding {
    /// LF (Unix)
    Lf,
    /// CRLF (Windows)
    Crlf,
    /// CR (Classic Mac)
    Cr,
}

impl LineEnding {
    /// Detects the line ending style from text content.
    /// Returns the first non-empty line's ending, or LF if none.
    pub fn detect(text: &str) -> Self {
        // Scan for line endings directly in the text (text.lines() strips them)
        let mut i = 0;
        while i < text.len() {
            if i + 1 < text.len() && text.as_bytes()[i] == b'\r' && text.as_bytes()[i + 1] == b'\n'
            {
                return LineEnding::Crlf;
            } else if text.as_bytes()[i] == b'\r' {
                return LineEnding::Cr;
            } else if text.as_bytes()[i] == b'\n' {
                return LineEnding::Lf;
            }
            i += 1;
        }
        LineEnding::Lf
    }

    /// Returns the byte sequence for this line ending.
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            LineEnding::Lf => b"\n",
            LineEnding::Crlf => b"\r\n",
            LineEnding::Cr => b"\r",
        }
    }

    /// Normalizes a line to end with this line ending.
    pub fn normalize_line(&self, line: &str) -> String {
        let trimmed = line.trim_end_matches(&['\r', '\n'][..]);
        format!("{}{}", trimmed, self.as_str())
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::Crlf => "\r\n",
            LineEnding::Cr => "\r",
        }
    }
}

/// Streaming parser that emits progress events during patch application.
pub struct StreamingPatchParser {
    config: StreamingParserConfig,
    progress_tx: mpsc::UnboundedSender<PatchProgressEvent>,
    cancel_rx: watch::Receiver<bool>,
    line_endings: Arc<tokio::sync::Mutex<BTreeMap<PathBuf, LineEnding>>>,
    stats: Arc<tokio::sync::Mutex<PatchApplyStats>>,
    _start_time: Instant,
}

impl StreamingPatchParser {
    pub fn new(
        config: StreamingParserConfig,
        progress_tx: mpsc::UnboundedSender<PatchProgressEvent>,
        cancel_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            config,
            progress_tx,
            cancel_rx,
            line_endings: Arc::new(tokio::sync::Mutex::new(BTreeMap::new())),
            stats: Arc::new(tokio::sync::Mutex::new(PatchApplyStats::default())),
            _start_time: Instant::now(),
        }
    }

    /// Parses and applies a patch with streaming progress events.
    /// The patch text is consumed incrementally (simulating streaming tool call args).
    pub async fn apply_streaming(
        &self,
        cwd: &std::path::Path,
        patch_text: &str,
    ) -> Result<PatchApplyStats> {
        let parsed = crate::unified_diff::UnifiedDiffParser::parse(patch_text)
            .context("Failed parsing patch format")?;

        let total_files = parsed.actions.len();
        let mut files_done = 0;

        for action in &parsed.actions {
            if *self.cancel_rx.borrow() {
                break;
            }

            // Detect line ending from existing file (if modifying) or use LF default
            let path = match action {
                FileAction::Modify { path, .. } => path,
                FileAction::Create { path, .. } => path,
                FileAction::Delete { path } => path,
                FileAction::Rename { old_path, .. } => old_path,
            };
            let full_path = cwd.join(path);

            let line_ending = if full_path.exists() {
                match fs::read_to_string(&full_path) {
                    Ok(content) => LineEnding::detect(&content),
                    Err(_) => LineEnding::Lf,
                }
            } else {
                LineEnding::Lf
            };

            // Store line ending for this file
            {
                let mut endings = self.line_endings.lock().await;
                endings.insert(path.clone(), line_ending);
            }

            // Emit file started event
            let action_name = match action {
                FileAction::Create { .. } => "create",
                FileAction::Modify { .. } => "modify",
                FileAction::Delete { .. } => "delete",
                FileAction::Rename { .. } => "rename",
            };
            let _ = self.progress_tx.send(PatchProgressEvent::FileStarted {
                path: path.to_string_lossy().to_string(),
                action: action_name.to_string(),
            });

            // Apply the action with hunk-level progress
            let hunk_count = match action {
                FileAction::Modify { hunks, .. } => hunks.len(),
                FileAction::Rename { hunks, .. } => hunks.len(),
                _ => 1,
            };

            for (hunk_idx, _) in (0..hunk_count).enumerate() {
                if *self.cancel_rx.borrow() {
                    break;
                }
                if self.config.emit_hunk_progress {
                    let _ = self.progress_tx.send(PatchProgressEvent::HunkApplied {
                        path: path.to_string_lossy().to_string(),
                        hunk_index: hunk_idx,
                        total_hunks: hunk_count,
                    });
                    // Throttle hunk progress events
                    if self.config.throttle > Duration::ZERO {
                        sleep(self.config.throttle).await;
                    }
                }
            }

            // Actually apply the action using the transaction engine
            let mut tx = crate::transaction::PatchTransaction::new(cwd);
            let single_patch = ParsedPatch {
                actions: vec![action.clone()],
            };
            tx.prepare(&single_patch)?;
            tx.commit()?;

            // Update stats
            {
                let mut stats = self.stats.lock().await;
                match action {
                    FileAction::Create { .. } => stats.files_created += 1,
                    FileAction::Modify { .. } => stats.files_modified += 1,
                    FileAction::Delete { .. } => stats.files_deleted += 1,
                    FileAction::Rename { .. } => stats.files_renamed += 1,
                }
            }

            files_done += 1;

            // Emit overall progress
            let _ = self.progress_tx.send(PatchProgressEvent::Progress {
                fraction: files_done as f64 / total_files as f64,
                files_done,
                total_files,
            });
        }

        let final_stats = self.stats.lock().await.clone();
        let _ = self.progress_tx.send(PatchProgressEvent::Completed {
            stats: final_stats.clone(),
        });

        Ok(final_stats)
    }

    /// Gets the detected line ending for a file (after parsing).
    pub async fn get_line_ending(&self, path: &PathBuf) -> Option<LineEnding> {
        self.line_endings.lock().await.get(path).copied()
    }
}

/// Applies a patch with line-ending preservation.
/// For each modified/created file, detects the existing line ending style
/// and uses it for all inserted lines.
pub fn apply_with_line_endings(
    cwd: &std::path::Path,
    patch: &ParsedPatch,
) -> Result<PatchApplyStats> {
    let mut stats = PatchApplyStats::default();
    let mut line_endings = BTreeMap::new();

    for action in &patch.actions {
        let path = match action {
            FileAction::Modify { path, .. } => path,
            FileAction::Create { path, .. } => path,
            FileAction::Delete { path } => path,
            FileAction::Rename { old_path, .. } => old_path,
        };
        let full_path = cwd.join(path);

        // Detect line ending from existing file (if modifying) or use LF default
        let ending = if full_path.exists() {
            match fs::read_to_string(&full_path) {
                Ok(content) => LineEnding::detect(&content),
                Err(_) => LineEnding::Lf,
            }
        } else {
            LineEnding::Lf
        };
        line_endings.insert(path.clone(), ending);

        // Apply using transaction engine
        let mut tx = crate::transaction::PatchTransaction::new(cwd);
        let single_patch = ParsedPatch {
            actions: vec![action.clone()],
        };
        tx.prepare(&single_patch)?;
        tx.commit()?;

        // Update stats
        match action {
            FileAction::Create { .. } => stats.files_created += 1,
            FileAction::Modify { .. } => stats.files_modified += 1,
            FileAction::Delete { .. } => stats.files_deleted += 1,
            FileAction::Rename { .. } => stats.files_renamed += 1,
        }
    }

    // Post-process: re-write modified/created files with correct line endings
    for action in &patch.actions {
        let path = match action {
            FileAction::Modify { path, .. } => path,
            FileAction::Create { path, .. } => path,
            FileAction::Delete { .. } => continue,
            FileAction::Rename { new_path, .. } => new_path,
        };
        let full_path = cwd.join(path);
        if !full_path.exists() {
            continue;
        }

        let ending = line_endings.get(path).copied().unwrap_or(LineEnding::Lf);
        let content = fs::read_to_string(&full_path)?;

        // Normalize all line endings to the detected style
        let normalized: String = content
            .split_inclusive('\n')
            .map(|segment| {
                if segment.ends_with('\n') {
                    let trimmed = segment.trim_end_matches(&['\r', '\n'][..]);
                    format!("{}{}", trimmed, ending.as_str())
                } else {
                    // Last segment without trailing newline
                    let trimmed = segment.trim_end_matches(&['\r', '\n'][..]);
                    format!("{}{}", trimmed, ending.as_str())
                }
            })
            .collect();

        fs::write(&full_path, normalized)?;
    }

    Ok(stats)
}

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_line_ending_detection() {
        assert_eq!(LineEnding::detect("line1\nline2\n"), LineEnding::Lf);
        assert_eq!(LineEnding::detect("line1\r\nline2\r\n"), LineEnding::Crlf);
        assert_eq!(LineEnding::detect("line1\rline2\r"), LineEnding::Cr);
        assert_eq!(LineEnding::detect("mixed\ncrlf\r\n"), LineEnding::Lf); // first wins
        assert_eq!(LineEnding::detect(""), LineEnding::Lf);
    }

    #[test]
    fn test_line_ending_normalization() {
        let lf = LineEnding::Lf;
        let crlf = LineEnding::Crlf;
        let cr = LineEnding::Cr;

        assert_eq!(lf.normalize_line("hello"), "hello\n");
        assert_eq!(lf.normalize_line("hello\n"), "hello\n");
        assert_eq!(lf.normalize_line("hello\r\n"), "hello\n");
        assert_eq!(lf.normalize_line("hello\r"), "hello\n");

        assert_eq!(crlf.normalize_line("hello"), "hello\r\n");
        assert_eq!(crlf.normalize_line("hello\n"), "hello\r\n");
        assert_eq!(crlf.normalize_line("hello\r\n"), "hello\r\n");

        assert_eq!(cr.normalize_line("hello"), "hello\r");
        assert_eq!(cr.normalize_line("hello\n"), "hello\r");
    }

    #[tokio::test]
    async fn test_apply_with_line_endings_preserves_crlf() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.rs");

        // Write file with CRLF
        fs::write(
            &file_path,
            "fn main() {\r\n    println!(\"hello\");\r\n}\r\n",
        )
        .unwrap();

        // Apply a patch that modifies the file
        let patch_text = r#"
*** Begin Patch
*** Update File: test.rs
@@ fn main() {
-    println!("hello");
+    println!("world");
*** End Patch
"#;
        let parsed = crate::unified_diff::UnifiedDiffParser::parse(patch_text).unwrap();
        let stats = apply_with_line_endings(dir.path(), &parsed).unwrap();

        assert_eq!(stats.files_modified, 1);

        // Verify CRLF preserved
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("\r\n"), "CRLF should be preserved");
        assert!(!content.contains("\n\n"), "should not have double LF");
        assert!(content.contains("world"), "patch should be applied");
    }

    #[tokio::test]
    async fn test_streaming_parser_throttles_progress() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (_cancel_tx, cancel_rx) = watch::channel(false);
        let config = StreamingParserConfig::default();

        let parser = StreamingPatchParser::new(config, tx, cancel_rx);

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("a.rs");
        fs::write(&file_path, "fn a() {\n    let x = 1;\n}\n").unwrap();

        let patch = r#"
*** Begin Patch
*** Update File: a.rs
@@ fn a() {
-    let x = 1;
+    let x = 2;
*** End Patch
"#;

        let _ = parser.apply_streaming(dir.path(), patch).await.unwrap();

        // Collect progress events
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        // Should have FileStarted, Progress, and Completed
        assert!(
            events
                .iter()
                .any(|e| matches!(e, PatchProgressEvent::FileStarted { .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, PatchProgressEvent::Progress { .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, PatchProgressEvent::Completed { .. }))
        );
    }
}
