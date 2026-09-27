//! Tier 3 Automated Test Suite Verification Runner (VER-01, VER-03).
//!
//! Invokes automated test commands (`cargo test`), parses test failures,
//! panics, and assertion outcomes, and stores evidence in `ArtifactStore`.

use async_trait::async_trait;
use regex::Regex;
use std::path::Path;
use std::sync::{Arc, LazyLock};

use super::VerificationRunner;
use crate::ids::{ArtifactId, MissionId, TaskId};
use crate::persistence::artifacts::ArtifactStore;
use crate::verification::types::{CheckStatus, CheckTier, VerificationCheck};

pub static TEST_FAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^test\s+([^\s]+)\s+\.\.\.\s+FAILED").expect("valid test regex")
});

pub static PANIC_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"panicked at [^:]+:\d+:\d+:\s*(.+)").expect("valid panic regex"));

pub static TEST_RESULT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"test result:\s+(ok|FAILED)\.\s+(\d+)\s+passed;\s+(\d+)\s+failed")
        .expect("valid summary regex")
});

#[derive(Clone)]
pub struct TestRunner {
    pub test_command: String,
    pub artifact_store: Option<Arc<dyn ArtifactStore>>,
    pub simulated_output: Option<(i32, String, String)>,
    pub timeout_secs: Option<u64>,
}

impl Default for TestRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl TestRunner {
    pub fn new() -> Self {
        Self {
            test_command: "cargo test".to_string(),
            artifact_store: None,
            simulated_output: None,
            timeout_secs: None,
        }
    }

    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    pub fn with_artifact_store(mut self, store: Arc<dyn ArtifactStore>) -> Self {
        self.artifact_store = Some(store);
        self
    }

    pub fn with_command(mut self, cmd: impl Into<String>) -> Self {
        self.test_command = cmd.into();
        self
    }

    pub fn with_simulated(
        mut self,
        exit_code: i32,
        stdout: impl Into<String>,
        stderr: impl Into<String>,
    ) -> Self {
        self.simulated_output = Some((exit_code, stdout.into(), stderr.into()));
        self
    }

    pub fn parse_test_output(
        exit_code: i32,
        stdout: &str,
        stderr: &str,
    ) -> (CheckStatus, Vec<String>, String) {
        let mut failed_tests = Vec::new();
        for cap in TEST_FAIL_RE.captures_iter(stdout) {
            failed_tests.push(cap[1].to_string());
        }

        let mut panic_messages = Vec::new();
        for cap in PANIC_RE.captures_iter(stdout) {
            panic_messages.push(cap[1].to_string());
        }
        for cap in PANIC_RE.captures_iter(stderr) {
            panic_messages.push(cap[1].to_string());
        }

        if exit_code == 0 && failed_tests.is_empty() {
            let summary = if let Some(cap) = TEST_RESULT_RE.captures(stdout) {
                format!("All tests passed ({})", &cap[0])
            } else {
                "All tests passed successfully".to_string()
            };
            (CheckStatus::Passed, Vec::new(), summary)
        } else {
            let mut reasons = Vec::new();
            if !failed_tests.is_empty() {
                reasons.push(format!("Failed tests: [{}]", failed_tests.join(", ")));
            }
            if !panic_messages.is_empty() {
                reasons.push(format!("Panics: [{}]", panic_messages.join("; ")));
            }
            if reasons.is_empty() {
                let first_few_lines = stderr.lines().take(3).collect::<Vec<_>>().join("; ");
                if first_few_lines.is_empty() {
                    reasons.push("Test process exited with non-zero status".to_string());
                } else {
                    reasons.push(first_few_lines);
                }
            }
            (CheckStatus::Failed, failed_tests, reasons.join("; "))
        }
    }
}

#[async_trait]
impl VerificationRunner for TestRunner {
    async fn execute(
        &self,
        mission_id: MissionId,
        task_id: TaskId,
        workspace_root: &Path,
        snapshot_hash: &str,
    ) -> Result<VerificationCheck, String> {
        let (exit_code, stdout, stderr) =
            if let Some((code, ref out, ref err)) = self.simulated_output {
                (code, out.clone(), err.clone())
            } else {
                let parts: Vec<&str> = self.test_command.split_whitespace().collect();
                if parts.is_empty() {
                    // No test command configured (target-neutral
                    // verification). Explicit not-applicable verdict rather
                    // than failure: nothing was requested to run.
                    return Ok(VerificationCheck::passed(
                        mission_id,
                        task_id,
                        CheckTier::Tests,
                        "no_test_command",
                        format!("workspace_root={}", workspace_root.display()),
                        None,
                        "No test command configured; test tier not applicable",
                        snapshot_hash,
                    ));
                }

                let mut cmd = tokio::process::Command::new(parts[0]);
                if parts.len() > 1 {
                    cmd.args(&parts[1..]);
                }
                cmd.current_dir(workspace_root);
                cmd.kill_on_drop(true);
                // Enforce minimal resource consumption during test execution
                cmd.env("RUST_TEST_THREADS", "2");
                cmd.env("CARGO_BUILD_JOBS", "2");

                let timeout_dur = std::time::Duration::from_secs(self.timeout_secs.unwrap_or(60));
                let output_res = tokio::time::timeout(timeout_dur, cmd.output()).await;

                match output_res {
                    Ok(Ok(output)) => {
                        let code = output.status.code().unwrap_or(-1);
                        let out = String::from_utf8_lossy(&output.stdout).to_string();
                        let err = String::from_utf8_lossy(&output.stderr).to_string();
                        (code, out, err)
                    }
                    Ok(Err(e)) => {
                        return Err(format!(
                            "Failed to invoke test command '{}': {}",
                            self.test_command, e
                        ));
                    }
                    Err(_) => {
                        let err = format!(
                            "Test command '{}' timed out after {} seconds",
                            self.test_command,
                            timeout_dur.as_secs()
                        );
                        (-1, String::new(), err)
                    }
                }
            };

        let (status, _failed_tests, summary) = Self::parse_test_output(exit_code, &stdout, &stderr);

        let mut evidence_artifact_id = None;
        if let Some(ref store) = self.artifact_store {
            let combined_logs = format!("--- STDOUT ---\n{}\n--- STDERR ---\n{}", stdout, stderr);
            let art_id = ArtifactId::new();
            if store
                .store(art_id, combined_logs.as_bytes(), "log")
                .await
                .is_ok()
            {
                evidence_artifact_id = Some(art_id);
            }
        }

        if status.is_passed() {
            Ok(VerificationCheck::passed(
                mission_id,
                task_id,
                CheckTier::Tests,
                &self.test_command,
                format!("workspace_root={}", workspace_root.display()),
                evidence_artifact_id,
                summary,
                snapshot_hash,
            ))
        } else {
            Ok(VerificationCheck::failed(
                mission_id,
                task_id,
                CheckTier::Tests,
                &self.test_command,
                format!("workspace_root={}", workspace_root.display()),
                evidence_artifact_id,
                summary,
                Some("Test".to_string()),
                snapshot_hash,
            ))
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_parse_test_output_passed() {
        let stdout = "running 2 tests\ntest test_a ... ok\ntest test_b ... ok\n\ntest result: ok. 2 passed; 0 failed; 0 ignored\n";
        let (status, failed, summary) = TestRunner::parse_test_output(0, stdout, "");
        assert!(status.is_passed());
        assert!(failed.is_empty());
        assert!(summary.contains("All tests passed"));
    }

    #[test]
    fn test_parse_test_output_failed() {
        let stdout = "running 2 tests\ntest test_a ... ok\ntest test_b ... FAILED\n\ntest result: FAILED. 1 passed; 1 failed; 0 ignored\n";
        let (status, failed, summary) = TestRunner::parse_test_output(101, stdout, "");
        assert!(status.is_failed());
        assert_eq!(failed, vec!["test_b"]);
        assert!(summary.contains("Failed tests: [test_b]"));
    }
}
