//! Tier 2 Compiler & Type Checker Verification Runner (VER-01).
//!
//! Invokes compiler diagnostics (`cargo check`), parses `rustc` error codes (`error[E0...]`),
//! and reports structured compilation outcomes.

use async_trait::async_trait;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

use super::VerificationRunner;
use crate::ids::{MissionId, TaskId};
use crate::verification::types::{CheckStatus, CheckTier, VerificationCheck};

pub static RUSTC_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"error\[E(\d{4})\]:\s*(.+)").expect("valid rustc regex"));

#[derive(Clone)]
pub struct CompilerRunner {
    pub check_command: String,
    pub simulated_output: Option<(i32, String, String)>, // (exit_code, stdout, stderr)
    pub timeout_secs: Option<u64>,
}

impl Default for CompilerRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerRunner {
    pub fn new() -> Self {
        // Single source: the declarative Rust adapter asset. This
        // constructor is a thin default, not a second command table.
        let adapter = crate::verification::adapter::ProjectAdapter::adapter_for(
            crate::verification::adapter::ProjectType::Rust,
        );
        Self {
            check_command: adapter.compiler_command,
            simulated_output: None,
            timeout_secs: None,
        }
    }

    pub fn with_timeout_secs(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    pub fn with_command(cmd: impl Into<String>) -> Self {
        Self {
            check_command: cmd.into(),
            simulated_output: None,
            timeout_secs: None,
        }
    }

    pub fn with_simulated(
        exit_code: i32,
        stdout: impl Into<String>,
        stderr: impl Into<String>,
    ) -> Self {
        Self {
            check_command: "simulated".to_string(),
            simulated_output: Some((exit_code, stdout.into(), stderr.into())),
            timeout_secs: None,
        }
    }

    /// Parse compiler stdout and stderr into CheckStatus, extracted error codes, and summary.
    pub fn parse_compiler_output(
        exit_code: i32,
        stdout: &str,
        stderr: &str,
    ) -> (CheckStatus, Vec<String>, String) {
        let combined = format!("{stderr}\n{stdout}");
        let diags = crate::verification::diagnostics::parse_rust_diagnostics(&combined);
        let mut error_codes: Vec<String> = diags.iter().filter_map(|d| d.code.clone()).collect();
        for cap in RUSTC_CODE_RE.captures_iter(stderr) {
            let code = format!("E{}", &cap[1]);
            if !error_codes.contains(&code) {
                error_codes.push(code);
            }
        }

        if exit_code == 0 && error_codes.is_empty() && !stderr.contains("error:") {
            (
                CheckStatus::Passed,
                error_codes,
                "Compilation succeeded without errors".to_string(),
            )
        } else {
            let error_summary = if !error_codes.is_empty() {
                format!("Compiler errors detected: [{}]", error_codes.join(", "))
            } else {
                let first_few_lines = stderr.lines().take(3).collect::<Vec<_>>().join("; ");
                if first_few_lines.is_empty() {
                    "Compilation failed with non-zero exit code".to_string()
                } else {
                    format!("Compilation failed: {}", first_few_lines)
                }
            };
            (CheckStatus::Failed, error_codes, error_summary)
        }
    }
}

#[async_trait]
impl VerificationRunner for CompilerRunner {
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
                if self.check_command.trim().is_empty() {
                    // No compilation unit for this workspace (target-neutral
                    // verification). Absence of something to compile is not
                    // failure evidence; the verdict is explicit and recorded,
                    // never assumed success.
                    return Ok(VerificationCheck::passed(
                        mission_id,
                        task_id,
                        CheckTier::Compiler,
                        "no_compilation_unit",
                        format!("workspace_root={}", workspace_root.display()),
                        None,
                        "No compilation unit applicable to workspace; compiler tier not applicable",
                        snapshot_hash,
                    ));
                }

                // Governed execution (P0-02): verification configuration is
                // validated and executed through the single authoritative
                // process boundary (sanitized env, workspace containment,
                // process-group isolation). Never spawn directly here.
                let timeout_dur = std::time::Duration::from_secs(
                    self.timeout_secs
                        .unwrap_or(crate::config::canonical::DEFAULT_VERIFICATION_TIMEOUT_SECS),
                );
                let output = match crate::verification::executor::execute_governed_verification(
                    &self.check_command,
                    workspace_root,
                    timeout_dur,
                    vec![],
                    None,
                )
                .await
                {
                    Ok(output) => output,
                    Err(e) if e.contains("timed out") || e.contains("TimedOut") => {
                        let err = format!(
                            "Compiler command '{}' timed out after {} seconds",
                            self.check_command,
                            timeout_dur.as_secs()
                        );
                        crate::process::types::ProcessOutput {
                            exit_code: -1,
                            stdout: String::new(),
                            stderr: err,
                        }
                    }
                    Err(e) => {
                        return Err(format!(
                            "Failed to invoke compiler command '{}': {}",
                            self.check_command, e
                        ));
                    }
                };
                let (code, out, err) = (output.exit_code, output.stdout, output.stderr);
                (code, out, err)
            };

        let (status, error_codes, summary) =
            Self::parse_compiler_output(exit_code, &stdout, &stderr);

        if status.is_passed() {
            Ok(VerificationCheck::passed(
                mission_id,
                task_id,
                CheckTier::Compiler,
                &self.check_command,
                format!("workspace_root={}", workspace_root.display()),
                None,
                summary,
                snapshot_hash,
            ))
        } else {
            let failure_class = Some("Compilation".to_string());
            let inputs_desc = format!(
                "workspace_root={}, error_codes=[{}]",
                workspace_root.display(),
                error_codes.join(", ")
            );
            Ok(VerificationCheck::failed(
                mission_id,
                task_id,
                CheckTier::Compiler,
                &self.check_command,
                inputs_desc,
                None,
                summary,
                failure_class,
                snapshot_hash,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_compiler_output_success() {
        let stderr = "    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s\n";
        let (status, codes, summary) = CompilerRunner::parse_compiler_output(0, "", stderr);
        assert!(status.is_passed());
        assert!(codes.is_empty());
        assert_eq!(summary, "Compilation succeeded without errors");
    }

    #[test]
    fn test_parse_compiler_output_rustc_error() {
        let stderr = "error[E0308]: mismatched types\n  --> src/main.rs:2:5\n";
        let (status, codes, summary) = CompilerRunner::parse_compiler_output(101, "", stderr);
        assert!(status.is_failed());
        assert_eq!(codes, vec!["E0308"]);
        assert!(summary.contains("E0308"));
    }
}
