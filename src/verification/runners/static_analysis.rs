//! Tier 4 Static Analysis & Linter Verification Runner (VER-01, VER-03).
//!
//! Invokes linter diagnostics (`cargo clippy -- -D warnings`), parses warnings and errors,
//! and reports static analysis compliance.

use async_trait::async_trait;
use std::path::Path;

use super::VerificationRunner;
use crate::ids::{MissionId, TaskId};
use crate::verification::types::{CheckStatus, CheckTier, VerificationCheck};

#[derive(Debug, Clone)]
pub struct StaticAnalysisRunner {
    pub tool_command: String,
    pub simulated_output: Option<(i32, String, String)>,
}

impl Default for StaticAnalysisRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl StaticAnalysisRunner {
    pub fn new() -> Self {
        Self {
            tool_command: "cargo clippy -- -D warnings".to_string(),
            simulated_output: None,
        }
    }

    pub fn with_command(cmd: impl Into<String>) -> Self {
        Self {
            tool_command: cmd.into(),
            simulated_output: None,
        }
    }

    pub fn with_simulated(
        exit_code: i32,
        stdout: impl Into<String>,
        stderr: impl Into<String>,
    ) -> Self {
        Self {
            tool_command: "simulated_clippy".to_string(),
            simulated_output: Some((exit_code, stdout.into(), stderr.into())),
        }
    }

    pub fn parse_linter_output(
        exit_code: i32,
        _stdout: &str,
        stderr: &str,
    ) -> (CheckStatus, String) {
        if exit_code == 0 && !stderr.contains("error:") && !stderr.contains("warning:") {
            (
                CheckStatus::Passed,
                "Static analysis passed with 0 warnings".to_string(),
            )
        } else {
            let error_lines: Vec<&str> = stderr
                .lines()
                .filter(|l| l.contains("error") || l.contains("warning"))
                .take(3)
                .collect();
            let summary = if !error_lines.is_empty() {
                format!("Static analysis violations: {}", error_lines.join("; "))
            } else {
                "Static analysis failed with non-zero exit code".to_string()
            };
            (CheckStatus::Failed, summary)
        }
    }
}

#[async_trait]
impl VerificationRunner for StaticAnalysisRunner {
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
                if self.tool_command.trim().is_empty() {
                    // No linter configured (target-neutral
                    // verification). Explicit not-applicable verdict.
                    return Ok(VerificationCheck::passed(
                        mission_id,
                        task_id,
                        CheckTier::StaticAnalysis,
                        "no_linter_configured",
                        format!("workspace_root={}", workspace_root.display()),
                        None,
                        "No linter configured; static analysis tier not applicable",
                        snapshot_hash,
                    ));
                }

                // Governed execution (P0-02): linter commands execute through
                // the single authoritative process boundary with a wall-clock
                // bound (previously unbounded). Never spawn directly here.
                let timeout_dur = std::time::Duration::from_secs(120);
                let output = crate::verification::executor::execute_governed_verification(
                    &self.tool_command,
                    workspace_root,
                    timeout_dur,
                    vec![],
                    None,
                )
                .await
                .map_err(|e| {
                    format!(
                        "Failed to invoke static analysis command '{}': {}",
                        self.tool_command, e
                    )
                })?;

                let code = output.exit_code;
                let out = output.stdout;
                let err = output.stderr;
                (code, out, err)
            };

        let (status, summary) = Self::parse_linter_output(exit_code, &stdout, &stderr);

        if status.is_passed() {
            Ok(VerificationCheck::passed(
                mission_id,
                task_id,
                CheckTier::StaticAnalysis,
                &self.tool_command,
                format!("workspace_root={}", workspace_root.display()),
                None,
                summary,
                snapshot_hash,
            ))
        } else {
            Ok(VerificationCheck::failed(
                mission_id,
                task_id,
                CheckTier::StaticAnalysis,
                &self.tool_command,
                format!("workspace_root={}", workspace_root.display()),
                None,
                summary,
                Some("Compilation".to_string()),
                snapshot_hash,
            ))
        }
    }
}
