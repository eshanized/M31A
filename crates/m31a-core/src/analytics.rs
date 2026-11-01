//! Session telemetry, token consumption metrics, and anonymous repository fingerprinting.
//!
//! Provides non-identifying telemetry metrics, cost accounting algorithms, and execution statistics.

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedLineCounts {
    pub accepted_added_lines: u64,
    pub accepted_deleted_lines: u64,
}

/// Parses a unified diff and counts added and deleted lines inside hunks,
/// excluding git diff headers and hunk range markers (`@@ ... @@`).
pub fn accepted_line_counts_from_unified_diff(unified_diff: &str) -> AcceptedLineCounts {
    let mut in_hunk = false;
    let mut accepted_added_lines = 0;
    let mut accepted_deleted_lines = 0;

    for line in unified_diff.lines() {
        if line.starts_with("diff --git ") {
            in_hunk = false;
            continue;
        }

        if line.starts_with("@@ ") {
            in_hunk = true;
            continue;
        }

        if !in_hunk && (line.starts_with("+++ ") || line.starts_with("--- ")) {
            continue;
        }

        if in_hunk {
            if line.starts_with('+') && !line.starts_with("+++ ") {
                accepted_added_lines += 1;
                continue;
            }

            if line.starts_with('-') && !line.starts_with("--- ") {
                accepted_deleted_lines += 1;
            }
        }
    }

    AcceptedLineCounts {
        accepted_added_lines,
        accepted_deleted_lines,
    }
}

/// Computes a deterministic privacy-preserving fingerprint hash for attribution.
///
/// Uses standard format: `file-line-v1\0<domain>\0<value>`
pub fn fingerprint_hash(domain: &str, value: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(b"file-line-v1\0");
    hasher.update(domain.as_bytes());
    hasher.update(b"\0");
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Canonicalizes git remote URLs for repository fingerprinting.
pub fn canonicalize_git_remote(url: &str) -> String {
    let mut s = url.trim();
    if let Some(stripped) = s.strip_prefix("git@github.com:") {
        s = stripped;
    } else if let Some(stripped) = s.strip_prefix("https://github.com/") {
        s = stripped;
    } else if let Some(stripped) = s.strip_prefix("http://github.com/") {
        s = stripped;
    } else if let Some(stripped) = s.strip_prefix("ssh://git@github.com/") {
        s = stripped;
    }

    s = s.strip_suffix(".git").unwrap_or(s);
    s.to_lowercase()
}

/// Repository hash for a given remote URL.
pub fn repo_hash_from_remote(remote_url: &str) -> String {
    let canonical = canonicalize_git_remote(remote_url);
    fingerprint_hash("repo", &canonical)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionStatus {
    Success,
    Failure,
    Denied,
    Timeout,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCallFact {
    pub tool_name: String,
    pub status: ToolExecutionStatus,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommandExecutionFact {
    pub command: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub timed_out: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TurnMetricsFact {
    pub turn_id: String,
    pub thread_id: String,
    pub model: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub accepted_lines: AcceptedLineCounts,
    pub duration_ms: u64,
}

/// In-memory analytics reducer that aggregates session facts into a unified telemetry snapshot.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnalyticsReducer {
    pub total_turns: u64,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_accepted_added_lines: u64,
    pub total_accepted_deleted_lines: u64,
    pub tool_calls_by_name: HashMap<String, u64>,
    pub tool_failures_by_name: HashMap<String, u64>,
    pub command_executions: u64,
    pub command_failures: u64,
    pub guards_denied_count: u64,
    pub compactions_count: u64,
}

impl AnalyticsReducer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_turn(&mut self, fact: &TurnMetricsFact) {
        self.total_turns += 1;
        self.total_prompt_tokens += fact.prompt_tokens;
        self.total_completion_tokens += fact.completion_tokens;
        self.total_accepted_added_lines += fact.accepted_lines.accepted_added_lines;
        self.total_accepted_deleted_lines += fact.accepted_lines.accepted_deleted_lines;
    }

    pub fn record_tool_call(&mut self, fact: &ToolCallFact) {
        *self
            .tool_calls_by_name
            .entry(fact.tool_name.clone())
            .or_insert(0) += 1;
        if fact.status == ToolExecutionStatus::Failure
            || fact.status == ToolExecutionStatus::Timeout
        {
            *self
                .tool_failures_by_name
                .entry(fact.tool_name.clone())
                .or_insert(0) += 1;
        } else if fact.status == ToolExecutionStatus::Denied {
            self.guards_denied_count += 1;
        }
    }

    pub fn record_command(&mut self, fact: &CommandExecutionFact) {
        self.command_executions += 1;
        if fact.timed_out || fact.exit_code.map(|c| c != 0).unwrap_or(true) {
            self.command_failures += 1;
        }
    }

    pub fn record_compaction(&mut self) {
        self.compactions_count += 1;
    }

    pub fn render_summary_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Session Analytics Summary ===\n");
        report.push_str(&format!("Total Turns: {}\n", self.total_turns));
        report.push_str(&format!(
            "Tokens: {} prompt, {} completion ({} total)\n",
            self.total_prompt_tokens,
            self.total_completion_tokens,
            self.total_prompt_tokens + self.total_completion_tokens
        ));
        report.push_str(&format!(
            "Code Changes Accepted: +{} / -{} lines\n",
            self.total_accepted_added_lines, self.total_accepted_deleted_lines
        ));
        report.push_str(&format!(
            "Commands: {} run ({} failed)\n",
            self.command_executions, self.command_failures
        ));
        if self.guards_denied_count > 0 {
            report.push_str(&format!(
                "Safety Guard Denials: {}\n",
                self.guards_denied_count
            ));
        }
        if self.compactions_count > 0 {
            report.push_str(&format!(
                "Context Compactions: {}\n",
                self.compactions_count
            ));
        }
        if !self.tool_calls_by_name.is_empty() {
            report.push_str("Tool Invocations:\n");
            let mut entries: Vec<_> = self.tool_calls_by_name.iter().collect();
            entries.sort_by_key(|e| std::cmp::Reverse(*e.1));
            for (tool, count) in entries {
                let fails = self.tool_failures_by_name.get(tool).copied().unwrap_or(0);
                if fails > 0 {
                    report.push_str(&format!(
                        "  - {}: {} calls ({} failed)\n",
                        tool, count, fails
                    ));
                } else {
                    report.push_str(&format!("  - {}: {} calls\n", tool, count));
                }
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parses_accepted_line_counts() {
        let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,5 @@
-old line
+fn useful() {
+}
+    return user.id;
 context
"#;

        assert_eq!(
            accepted_line_counts_from_unified_diff(diff),
            AcceptedLineCounts {
                accepted_added_lines: 3,
                accepted_deleted_lines: 1,
            }
        );
    }

    #[test]
    fn test_skips_added_file_metadata_headers() {
        let diff = r#"diff --git a/new.py b/new.py
new file mode 100644
index 0000000..1111111
--- /dev/null
+++ b/new.py
@@ -0,0 +1 @@
+print('hello')
"#;

        assert_eq!(
            accepted_line_counts_from_unified_diff(diff),
            AcceptedLineCounts {
                accepted_added_lines: 1,
                accepted_deleted_lines: 0,
            }
        );
    }

    #[test]
    fn test_fingerprint_hash_format() {
        let hash = fingerprint_hash("repo", "openai/codex");
        assert_eq!(hash.len(), 40); // 40 hex chars for SHA-1
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

        let canon = canonicalize_git_remote("git@github.com:openai/codex.git");
        assert_eq!(canon, "openai/codex");

        let repo_h = repo_hash_from_remote("https://github.com/openai/codex.git");
        assert_eq!(repo_h, hash);
    }

    #[test]
    fn test_analytics_reducer_aggregation() {
        let mut reducer = AnalyticsReducer::new();

        reducer.record_turn(&TurnMetricsFact {
            turn_id: "turn-1".to_string(),
            thread_id: "thread-1".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            prompt_tokens: 1500,
            completion_tokens: 250,
            accepted_lines: AcceptedLineCounts {
                accepted_added_lines: 24,
                accepted_deleted_lines: 5,
            },
            duration_ms: 3200,
        });

        reducer.record_tool_call(&ToolCallFact {
            tool_name: "exec_command".to_string(),
            status: ToolExecutionStatus::Success,
            duration_ms: 450,
        });

        reducer.record_tool_call(&ToolCallFact {
            tool_name: "exec_command".to_string(),
            status: ToolExecutionStatus::Failure,
            duration_ms: 120,
        });

        reducer.record_command(&CommandExecutionFact {
            command: "cargo check".to_string(),
            exit_code: Some(0),
            duration_ms: 1200,
            timed_out: false,
        });

        assert_eq!(reducer.total_turns, 1);
        assert_eq!(reducer.total_prompt_tokens, 1500);
        assert_eq!(reducer.total_completion_tokens, 250);
        assert_eq!(reducer.total_accepted_added_lines, 24);
        assert_eq!(reducer.total_accepted_deleted_lines, 5);
        assert_eq!(*reducer.tool_calls_by_name.get("exec_command").unwrap(), 2);
        assert_eq!(
            *reducer.tool_failures_by_name.get("exec_command").unwrap(),
            1
        );
        assert_eq!(reducer.command_executions, 1);
        assert_eq!(reducer.command_failures, 0);

        let report = reducer.render_summary_report();
        assert!(report.contains("Total Turns: 1"));
        assert!(report.contains("Code Changes Accepted: +24 / -5 lines"));
        assert!(report.contains("exec_command: 2 calls (1 failed)"));
    }
}
