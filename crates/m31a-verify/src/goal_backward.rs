//! Adversarial Goal-Backward Verifier for M31A.
//!
//! Adversarial verifier proving observable functional outcomes rather than trusting self-reported model claims:
//!
//! Core Axiom: "Task completion ≠ Goal achievement".
//! SUMMARY.md claims document what the model claims it did; the verifier proves what
//! ACTUALLY exists and functions in the codebase.
//!
//! Implements goal-backward falsification:
//! 1. Must-Have Truths: Proves observable behaviors against real source files and test runs.
//!    Resolves to `Verified`, `Failed` (BLOCKER), or `Uncertain` (WARNING).
//! 2. Artifact Existence & Stub Detection: Verifies files exist and are not empty placeholders.
//! 3. Key Links Wiring: Checks that created components are actually imported and wired.
//! 4. Automated Test Verification: Executes test commands and records diagnostics.
//! 5. Generates structured `VERIFICATION.md` report with remediation instructions for any gaps.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TruthStatus {
    Verified,
    Failed { reason: String },
    Uncertain { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TruthVerification {
    pub truth: String,
    pub status: TruthStatus,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactCheck {
    pub path: PathBuf,
    pub exists: bool,
    pub byte_size: u64,
    pub is_stub: bool,
    pub wired_in: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomatedTestCheck {
    pub command: String,
    pub passed: bool,
    pub exit_code: i32,
    pub stdout_snippet: String,
    pub stderr_snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationOutcome {
    Passed,
    GapsFound,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalBackwardReport {
    pub phase: String,
    pub outcome: VerificationOutcome,
    pub truths: Vec<TruthVerification>,
    pub artifacts: Vec<ArtifactCheck>,
    pub test_results: Vec<AutomatedTestCheck>,
    pub gaps: Vec<String>,
    pub markdown_summary: String,
}

pub struct GoalBackwardVerifier;

impl GoalBackwardVerifier {
    /// Runs a comprehensive goal-backward verification of a phase against the codebase.
    pub async fn verify_phase(
        repo_root: &Path,
        phase: &str,
        promised_truths: &[String],
        promised_artifacts: &[PathBuf],
        wiring_pairs: &[(PathBuf, PathBuf)], // (source_artifact, imported_by)
        automated_test_commands: &[String],
        timeout: Duration,
    ) -> Result<GoalBackwardReport> {
        let mut gaps = Vec::new();

        if promised_truths.is_empty() {
            gaps.push("No promised observable truths declared for phase verification".to_string());
        }

        // 1. Check Artifact Existence, Size, and Stub Detection
        let mut artifact_checks = Vec::new();
        for art in promised_artifacts {
            let full_path = repo_root.join(art);
            let exists = full_path.exists();
            let byte_size = if exists {
                std::fs::metadata(&full_path).map(|m| m.len()).unwrap_or(0)
            } else {
                0
            };

            // Stub heuristic: file is tiny (<30 bytes) or only contains "TODO" / "unimplemented!"
            let mut is_stub = false;
            if exists {
                if byte_size < 30 {
                    is_stub = true;
                } else if let Ok(content) = std::fs::read_to_string(&full_path) {
                    let trimmed = content.trim();
                    if trimmed.contains("unimplemented!()")
                        || trimmed.contains("// TODO: implement")
                    {
                        let lines: Vec<&str> = trimmed.lines().collect();
                        if lines.len() <= 3 {
                            is_stub = true;
                        }
                    }
                }
            }

            if !exists {
                gaps.push(format!("Artifact missing on disk: '{}'", art.display()));
            } else if is_stub {
                gaps.push(format!(
                    "Artifact is an empty/stub placeholder: '{}'",
                    art.display()
                ));
            }

            // Check wiring
            let mut wired_in = Vec::new();
            for (src, importer) in wiring_pairs {
                if src == art {
                    let imp_full = repo_root.join(importer);
                    if imp_full.exists()
                        && let Ok(content) = std::fs::read_to_string(&imp_full)
                    {
                        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                        if content.contains(stem) {
                            wired_in.push(importer.clone());
                        }
                    }
                }
            }

            if exists
                && !wiring_pairs.is_empty()
                && wired_in.is_empty()
                && wiring_pairs.iter().any(|(s, _)| s == art)
            {
                gaps.push(format!(
                    "Artifact '{}' exists but is not wired/imported anywhere",
                    art.display()
                ));
            }

            artifact_checks.push(ArtifactCheck {
                path: art.clone(),
                exists,
                byte_size,
                is_stub,
                wired_in,
            });
        }

        // 2. Run Automated Verification Test Commands with enforced timeout and process group reaping
        let mut test_results = Vec::new();
        for cmd_str in automated_test_commands {
            let mut cmd = tokio::process::Command::new("sh");
            cmd.arg("-c").arg(cmd_str).current_dir(repo_root);
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());
            #[cfg(unix)]
            {
                cmd.process_group(0);
            }
            cmd.kill_on_drop(true);

            match cmd.spawn() {
                Ok(mut child) => {
                    let child_pid = child.id();
                    let mut stdout = child.stdout.take();
                    let mut stderr = child.stderr.take();

                    const MAX_OUTPUT_BYTES: usize = 64 * 1024;
                    async fn drain_bounded_tokio_reader<R: tokio::io::AsyncRead + Unpin>(
                        mut reader: R,
                        max_retention: usize,
                    ) -> Vec<u8> {
                        let mut retained = Vec::new();
                        let mut chunk = [0u8; 8192];
                        use tokio::io::AsyncReadExt;
                        loop {
                            match reader.read(&mut chunk).await {
                                Ok(0) => break,
                                Ok(n) => {
                                    if retained.len() < max_retention {
                                        let available = max_retention - retained.len();
                                        let to_copy = n.min(available);
                                        retained.extend_from_slice(&chunk[..to_copy]);
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        retained
                    }

                    let stdout_fut = async {
                        if let Some(r) = stdout.as_mut() {
                            drain_bounded_tokio_reader(r, MAX_OUTPUT_BYTES).await
                        } else {
                            Vec::new()
                        }
                    };
                    let stderr_fut = async {
                        if let Some(r) = stderr.as_mut() {
                            drain_bounded_tokio_reader(r, MAX_OUTPUT_BYTES).await
                        } else {
                            Vec::new()
                        }
                    };

                    let run_fut = async {
                        let (status_res, out_bytes, err_bytes) =
                            tokio::join!(child.wait(), stdout_fut, stderr_fut);
                        let status = status_res?;
                        Ok::<_, std::io::Error>((status, out_bytes, err_bytes))
                    };

                    match tokio::time::timeout(timeout, run_fut).await {
                        Ok(Ok((status, out_bytes, err_bytes))) => {
                            let stdout = String::from_utf8_lossy(&out_bytes).to_string();
                            let stderr = String::from_utf8_lossy(&err_bytes).to_string();
                            let zero_tests = stdout.contains("running 0 tests")
                                || stdout.contains(" 0 passed; 0 failed");
                            let passed = status.success() && !zero_tests;
                            let exit_code = if !status.success() {
                                status.code().unwrap_or(1)
                            } else if zero_tests {
                                1
                            } else {
                                0
                            };

                            if !status.success() {
                                gaps.push(format!(
                                    "Automated test command failed: '{}' (exit code {})",
                                    cmd_str, exit_code
                                ));
                            } else if zero_tests {
                                gaps.push(format!(
                                    "Automated test command executed zero tests: '{}'",
                                    cmd_str
                                ));
                            }

                            test_results.push(AutomatedTestCheck {
                                command: cmd_str.clone(),
                                passed,
                                exit_code,
                                stdout_snippet: stdout
                                    .lines()
                                    .take(5)
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                                stderr_snippet: stderr
                                    .lines()
                                    .take(5)
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            });
                        }
                        Ok(Err(e)) => {
                            gaps.push(format!(
                                "Failed waiting for test command '{}': {e}",
                                cmd_str
                            ));
                            test_results.push(AutomatedTestCheck {
                                command: cmd_str.clone(),
                                passed: false,
                                exit_code: 1,
                                stdout_snippet: String::new(),
                                stderr_snippet: e.to_string(),
                            });
                        }
                        Err(_) => {
                            #[cfg(unix)]
                            if let Some(pid) = child_pid {
                                unsafe {
                                    libc::kill(-(pid as libc::pid_t), libc::SIGTERM);
                                }
                                tokio::time::sleep(Duration::from_millis(50)).await;
                                unsafe {
                                    libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
                                }
                            }
                            let _ = child.kill().await;
                            let _ = child.wait().await;

                            gaps.push(format!(
                                "Automated test command timed out after {:?}: '{}'",
                                timeout, cmd_str
                            ));
                            test_results.push(AutomatedTestCheck {
                                command: cmd_str.clone(),
                                passed: false,
                                exit_code: 124,
                                stdout_snippet: String::new(),
                                stderr_snippet: format!("Execution timed out after {:?}", timeout),
                            });
                        }
                    }
                }
                Err(e) => {
                    gaps.push(format!("Failed executing test command '{}': {e}", cmd_str));
                    test_results.push(AutomatedTestCheck {
                        command: cmd_str.clone(),
                        passed: false,
                        exit_code: 1,
                        stdout_snippet: String::new(),
                        stderr_snippet: e.to_string(),
                    });
                }
            }
        }

        // 3. Verify Must-Have Observable Truths
        let mut truth_checks = Vec::new();
        for truth in promised_truths {
            let associated_arts: Vec<_> = artifact_checks
                .iter()
                .filter(|a| {
                    let path_str = a.path.to_string_lossy();
                    let file_name = a
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_default();
                    truth.contains(&*path_str)
                        || (!file_name.is_empty() && truth.contains(&*file_name))
                        || artifact_checks.len() == 1
                })
                .collect();

            let associated_tests: Vec<_> = test_results
                .iter()
                .filter(|t| {
                    let cmd_lower = t.command.to_lowercase();
                    let truth_lower = truth.to_lowercase();
                    truth_lower.contains(&t.command)
                        || truth_lower
                            .split_whitespace()
                            .any(|word| word.len() > 3 && cmd_lower.contains(word))
                        || test_results.len() == 1
                })
                .collect();

            let missing_arts: Vec<_> = associated_arts
                .iter()
                .filter(|a| !a.exists || a.is_stub)
                .map(|a| a.path.display().to_string())
                .collect();

            let failed_tests: Vec<_> = associated_tests
                .iter()
                .filter(|t| !t.passed)
                .map(|t| t.command.clone())
                .collect();

            let (status, evidence) = if associated_arts.is_empty() && associated_tests.is_empty() {
                gaps.push(format!(
                    "No observable validation artifacts or automated test checks were associated with truth: '{}'",
                    truth
                ));
                (
                    TruthStatus::Uncertain {
                        reason: "No observable artifacts or automated test commands were supplied to prove this truth".to_string(),
                    },
                    "Missing verification proof: no associated artifacts or tests".into(),
                )
            } else if !missing_arts.is_empty() {
                (
                    TruthStatus::Failed {
                        reason: format!("Supporting artifacts missing or stub: {:?}", missing_arts),
                    },
                    format!("Missing dependencies: {}", missing_arts.join(", ")),
                )
            } else if !failed_tests.is_empty() {
                (
                    TruthStatus::Failed {
                        reason: format!("Underlying test commands failed: {:?}", failed_tests),
                    },
                    format!("Test failures: {}", failed_tests.join(", ")),
                )
            } else {
                (
                    TruthStatus::Verified,
                    "All supporting artifacts present and tests passed".into(),
                )
            };

            truth_checks.push(TruthVerification {
                truth: truth.clone(),
                status,
                evidence,
            });
        }

        // 4. Derive Overall Outcome
        let outcome = if gaps.is_empty() {
            VerificationOutcome::Passed
        } else if truth_checks
            .iter()
            .any(|t| matches!(t.status, TruthStatus::Failed { .. }))
        {
            VerificationOutcome::Failed
        } else {
            VerificationOutcome::GapsFound
        };

        // 5. Render Structured Markdown Report (`VERIFICATION.md`)
        let markdown_summary = Self::render_verification_markdown(
            phase,
            &outcome,
            &truth_checks,
            &artifact_checks,
            &test_results,
            &gaps,
        );

        Ok(GoalBackwardReport {
            phase: phase.to_string(),
            outcome,
            truths: truth_checks,
            artifacts: artifact_checks,
            test_results,
            gaps,
            markdown_summary,
        })
    }

    fn render_verification_markdown(
        phase: &str,
        outcome: &VerificationOutcome,
        truths: &[TruthVerification],
        artifacts: &[ArtifactCheck],
        tests: &[AutomatedTestCheck],
        gaps: &[String],
    ) -> String {
        let mut out = format!("# Phase {phase} Goal-Backward Verification Report\n\n");
        let outcome_badge = match outcome {
            VerificationOutcome::Passed => "✅ PASSED",
            VerificationOutcome::GapsFound => "⚠️ GAPS FOUND",
            VerificationOutcome::Failed => "❌ FAILED (BLOCKER)",
        };
        out.push_str(&format!("**Outcome:** {outcome_badge}\n\n"));

        // Must-have truths table
        out.push_str("## 1. Must-Have Truths Verification\n\n");
        out.push_str("| Observable Truth | Status | Evidence |\n");
        out.push_str("|---|---|---|\n");
        for t in truths {
            let status_str = match &t.status {
                TruthStatus::Verified => "✅ VERIFIED",
                TruthStatus::Failed { .. } => "❌ FAILED",
                TruthStatus::Uncertain { .. } => "⚠️ UNCERTAIN",
            };
            out.push_str(&format!(
                "| {} | {} | {} |\n",
                t.truth, status_str, t.evidence
            ));
        }
        out.push('\n');

        // Artifacts table
        out.push_str("## 2. Promised Artifacts & Wiring\n\n");
        out.push_str("| Artifact | Exists | Size | Non-Stub | Wired In |\n");
        out.push_str("|---|---|---|---|---|\n");
        for a in artifacts {
            out.push_str(&format!(
                "| `{}` | {} | {} B | {} | {} |\n",
                a.path.display(),
                if a.exists { "✅" } else { "❌" },
                a.byte_size,
                if !a.is_stub { "✅" } else { "❌" },
                if a.wired_in.is_empty() {
                    "None"
                } else {
                    "Wired"
                }
            ));
        }
        out.push('\n');

        // Test commands table
        out.push_str("## 3. Automated Test Command Results\n\n");
        out.push_str("| Command | Result | Exit Code |\n");
        out.push_str("|---|---|---|\n");
        for test in tests {
            out.push_str(&format!(
                "| `{}` | {} | {} |\n",
                test.command,
                if test.passed { "✅ PASS" } else { "❌ FAIL" },
                test.exit_code
            ));
        }
        out.push('\n');

        // Gaps & Remediation
        if !gaps.is_empty() {
            out.push_str("## 4. Gaps & Remediation Required\n\n");
            for gap in gaps {
                out.push_str(&format!("- ❌ {gap}\n"));
            }
            out.push('\n');
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_goal_backward_pass() {
        let dir = tempdir().unwrap();
        let art_path = PathBuf::from("src/calc.rs");
        let full = dir.path().join(&art_path);
        std::fs::create_dir_all(full.parent().unwrap()).unwrap();
        std::fs::write(
            &full,
            "pub fn add(a: i32, b: i32) -> i32 { a + b }\n#[test]\nfn test_add() { assert_eq!(add(2, 2), 4); }\n",
        )
        .unwrap();

        let report = GoalBackwardVerifier::verify_phase(
            dir.path(),
            "01",
            &["Addition function works".into()],
            &[art_path],
            &[],
            &["rustc --test src/calc.rs -o calc_test && ./calc_test".into()],
            Duration::from_secs(10),
        )
        .await
        .unwrap();

        assert_eq!(report.outcome, VerificationOutcome::Passed);
        assert_eq!(report.gaps.len(), 0);
        assert!(report.markdown_summary.contains("✅ PASSED"));
    }

    #[tokio::test]
    async fn test_goal_backward_falsifies_missing_artifact() {
        let dir = tempdir().unwrap();
        let missing_path = PathBuf::from("src/missing.rs");

        let report = GoalBackwardVerifier::verify_phase(
            dir.path(),
            "01",
            &["Component functions correctly".into()],
            &[missing_path],
            &[],
            &[],
            Duration::from_secs(5),
        )
        .await
        .unwrap();

        assert_eq!(report.outcome, VerificationOutcome::Failed);
        assert!(
            report
                .gaps
                .iter()
                .any(|g| g.contains("Artifact missing on disk"))
        );
        assert!(report.markdown_summary.contains("❌ FAILED"));
    }

    #[tokio::test]
    async fn test_goal_backward_timeout_enforcement() {
        let dir = tempdir().unwrap();

        let report = GoalBackwardVerifier::verify_phase(
            dir.path(),
            "01",
            &["Command finishes within deadline".into()],
            &[],
            &[],
            &["sleep 10".into()],
            Duration::from_millis(100),
        )
        .await
        .unwrap();

        assert_eq!(report.outcome, VerificationOutcome::Failed);
        assert!(report.gaps.iter().any(|g| g.contains("timed out")));
        assert_eq!(report.test_results.len(), 1);
        assert_eq!(report.test_results[0].exit_code, 124);
        assert!(!report.test_results[0].passed);
    }

    #[tokio::test]
    async fn test_goal_backward_rejects_empty_promised_truths() {
        let dir = tempdir().unwrap();

        let report = GoalBackwardVerifier::verify_phase(
            dir.path(),
            "01",
            &[], // Empty truths
            &[],
            &[],
            &["echo ok".into()],
            Duration::from_secs(5),
        )
        .await
        .unwrap();

        assert_ne!(report.outcome, VerificationOutcome::Passed);
        assert!(
            report
                .gaps
                .iter()
                .any(|g| g.contains("No promised observable truths declared"))
        );
    }

    #[tokio::test]
    async fn test_goal_backward_rejects_zero_tests() {
        let dir = tempdir().unwrap();

        let report = GoalBackwardVerifier::verify_phase(
            dir.path(),
            "01",
            &["Tests pass".into()],
            &[],
            &[],
            &["echo 'running 0 tests'".into()],
            Duration::from_secs(5),
        )
        .await
        .unwrap();

        assert_ne!(report.outcome, VerificationOutcome::Passed);
        assert!(
            report
                .gaps
                .iter()
                .any(|g| g.contains("executed zero tests"))
        );
        assert!(!report.test_results[0].passed);
    }
}
