//! Nyquist Test Coverage Engine (`m31a-verify::nyquist`).
//!
//! Automated test coverage validator ensuring all phase requirements have corresponding automated verification suites:
//! - Maps phase requirements to automated test commands.
//! - Enforces Wave 0 test scaffolding: if tests do not yet exist, a plan MUST
//!   define Wave 0 tasks using the Nyquist sentinel `MISSING — Wave 0 must create <test-file>`.
//! - Evaluates continuous sampling checks and feedback latency (test execution duration).
//! - Generates standard `{phase}-VALIDATION.md` artifacts.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::plan_checker::{IssueSeverity, PlanIssue, PlanModel};

/// Nyquist Wave 0 sentinel prefix for missing test scaffolding.
pub const NYQUIST_SENTINEL_PREFIX: &str = "MISSING — Wave 0 must create";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NyquistCheckStatus {
    /// Test command succeeded; requirement is satisfied by automated test.
    Satisfied {
        duration_ms: u64,
        stdout_snippet: String,
    },
    /// Test failed or requirement is not verified by current tests.
    Gap {
        reason: String,
        stderr_snippet: String,
    },
    /// Requirement requires test scaffolding in Wave 0 (sentinel detected).
    Sentinel { target_test_file: String },
    /// Requirement lacks any verification command or sentinel.
    MissingCommand,
    /// Explicitly justified skip.
    Skipped { justification: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NyquistRequirement {
    pub id: String,
    pub description: String,
    pub test_command: Option<String>,
    pub test_file: Option<PathBuf>,
    pub wave: usize,
    pub plan_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NyquistItemResult {
    pub requirement: NyquistRequirement,
    pub status: NyquistCheckStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NyquistVerdict {
    /// 100% of requirements have passing tests or valid Wave 0 sentinels.
    Compliant,
    /// One or more requirements lack tests or fail assertions.
    GapsDetected,
    /// Critical blocker: requirements have zero verification coverage or broken Wave 0 contracts.
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NyquistReport {
    pub phase_id: String,
    pub total_requirements: usize,
    pub satisfied_count: usize,
    pub gap_count: usize,
    pub sentinel_count: usize,
    pub missing_count: usize,
    pub feedback_latency_ms: u64,
    pub compliance_rate: f64,
    pub verdict: NyquistVerdict,
    pub items: Vec<NyquistItemResult>,
}

pub struct NyquistEngine;

impl NyquistEngine {
    /// Extracts Nyquist requirements from a list of `PlanModel`.
    pub fn extract_requirements(plans: &[PlanModel]) -> Vec<NyquistRequirement> {
        let mut reqs = Vec::new();

        for plan in plans {
            if !plan.requirements.is_empty() {
                for req_id in &plan.requirements {
                    let (cmd, file) = plan
                        .tasks
                        .iter()
                        .find_map(|t| {
                            t.verify
                                .as_ref()
                                .map(|v| (v.clone(), t.files.first().cloned()))
                        })
                        .unwrap_or((String::new(), None));

                    let test_cmd = if cmd.trim().is_empty() {
                        None
                    } else {
                        Some(cmd)
                    };

                    reqs.push(NyquistRequirement {
                        id: req_id.clone(),
                        description: format!("Requirement {} in plan {}", req_id, plan.id),
                        test_command: test_cmd,
                        test_file: file,
                        wave: plan.wave,
                        plan_id: plan.id.clone(),
                    });
                }
            } else {
                for (idx, task) in plan.tasks.iter().enumerate() {
                    let test_cmd = task.verify.as_ref().and_then(|v| {
                        let trimmed = v.trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed.to_string())
                        }
                    });

                    reqs.push(NyquistRequirement {
                        id: format!("{}-task-{}", plan.id, idx + 1),
                        description: task.name.clone(),
                        test_command: test_cmd,
                        test_file: task.files.first().cloned(),
                        wave: plan.wave,
                        plan_id: plan.id.clone(),
                    });
                }
            }
        }

        reqs
    }

    /// Enforces Wave 0 test scaffolding across all plans in a phase.
    /// If any plan in wave > 0 declares a Nyquist sentinel `MISSING — Wave 0 must create`,
    /// there must exist a Wave 0 plan that produces the specified file.
    pub fn enforce_wave_0_scaffolding(plans: &[PlanModel]) -> Vec<PlanIssue> {
        let mut issues = Vec::new();
        let wave_0_files: Vec<PathBuf> = plans
            .iter()
            .filter(|p| p.wave == 0)
            .flat_map(|p| {
                let mut files = p.files_modified.clone();
                for t in &p.tasks {
                    files.extend(t.files.clone());
                }
                files
            })
            .collect();

        for plan in plans {
            for (t_idx, task) in plan.tasks.iter().enumerate() {
                if let Some(cmd) = &task.verify
                    && let Some(target) = Self::parse_sentinel(cmd)
                {
                    let target_path = PathBuf::from(&target);
                    let found = wave_0_files.iter().any(|f| {
                        f == &target_path || f.ends_with(&target_path) || target_path.ends_with(f)
                    });

                    if plan.wave > 0 && !found {
                        issues.push(PlanIssue {
                                dimension: "Nyquist Wave-0 Scaffolding".into(),
                                severity: IssueSeverity::Blocker,
                                plan_id: plan.id.clone(),
                                task_index: Some(t_idx),
                                description: format!(
                                    "Task '{}' declares Nyquist sentinel '{}', but no Wave 0 task creates '{}'",
                                    task.name, cmd, target
                                ),
                                fix_hint: format!(
                                    "Add a Wave 0 plan/task that scaffolds '{}' before Wave {} executes",
                                    target, plan.wave
                                ),
                            });
                    }
                }
            }
        }

        issues
    }

    /// Evaluates Nyquist coverage against the workspace by executing automated test commands.
    pub fn audit_coverage(
        phase_id: &str,
        requirements: &[NyquistRequirement],
        workspace_root: &Path,
    ) -> NyquistReport {
        let mut items = Vec::new();
        let mut satisfied_count = 0;
        let mut gap_count = 0;
        let mut sentinel_count = 0;
        let mut missing_count = 0;
        let mut total_latency_ms = 0;

        for req in requirements {
            let status = match &req.test_command {
                None => {
                    missing_count += 1;
                    NyquistCheckStatus::MissingCommand
                }
                Some(cmd) if cmd.trim().is_empty() => {
                    missing_count += 1;
                    NyquistCheckStatus::MissingCommand
                }
                Some(cmd) if Self::is_sentinel(cmd) => {
                    sentinel_count += 1;
                    let target = Self::parse_sentinel(cmd).unwrap_or_else(|| "unspecified".into());
                    NyquistCheckStatus::Sentinel {
                        target_test_file: target,
                    }
                }
                Some(cmd) => {
                    let start = Instant::now();
                    let exec_res = Command::new("sh")
                        .arg("-c")
                        .arg(cmd)
                        .current_dir(workspace_root)
                        .output();

                    let duration_ms = start.elapsed().as_millis() as u64;
                    total_latency_ms += duration_ms;

                    match exec_res {
                        Ok(output) if output.status.success() => {
                            satisfied_count += 1;
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            let snippet = stdout.lines().take(5).collect::<Vec<_>>().join("\n");
                            NyquistCheckStatus::Satisfied {
                                duration_ms,
                                stdout_snippet: snippet,
                            }
                        }
                        Ok(output) => {
                            gap_count += 1;
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            let err_msg = if !stderr.trim().is_empty() {
                                stderr
                            } else {
                                stdout
                            };
                            let snippet = err_msg.lines().take(5).collect::<Vec<_>>().join("\n");
                            NyquistCheckStatus::Gap {
                                reason: format!("Command exited with status {}", output.status),
                                stderr_snippet: snippet,
                            }
                        }
                        Err(e) => {
                            gap_count += 1;
                            NyquistCheckStatus::Gap {
                                reason: format!("Failed to spawn test command: {}", e),
                                stderr_snippet: String::new(),
                            }
                        }
                    }
                }
            };

            items.push(NyquistItemResult {
                requirement: req.clone(),
                status,
            });
        }

        let total = requirements.len();
        let compliance_rate = if total == 0 {
            1.0
        } else {
            satisfied_count as f64 / total as f64
        };

        let verdict = if gap_count > 0 || missing_count > 0 {
            if missing_count > 0 && satisfied_count == 0 {
                NyquistVerdict::Blocked
            } else {
                NyquistVerdict::GapsDetected
            }
        } else {
            NyquistVerdict::Compliant
        };

        NyquistReport {
            phase_id: phase_id.to_string(),
            total_requirements: total,
            satisfied_count,
            gap_count,
            sentinel_count,
            missing_count,
            feedback_latency_ms: total_latency_ms,
            compliance_rate,
            verdict,
            items,
        }
    }

    /// Checks if a string contains the Nyquist sentinel.
    pub fn is_sentinel(cmd: &str) -> bool {
        cmd.trim().starts_with(NYQUIST_SENTINEL_PREFIX)
    }

    /// Parses the target file from a Nyquist sentinel command.
    pub fn parse_sentinel(cmd: &str) -> Option<String> {
        let trimmed = cmd.trim();
        if let Some(stripped) = trimmed.strip_prefix(NYQUIST_SENTINEL_PREFIX) {
            let rest = stripped.trim();
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
        None
    }

    /// Renders the `{phase}-VALIDATION.md` markdown verification report.
    pub fn render_validation_markdown(report: &NyquistReport) -> String {
        let mut md = String::new();
        md.push_str(&format!(
            "# Phase {} Validation Report (Nyquist Test Audit)\n\n",
            report.phase_id
        ));
        md.push_str(&format!("- **Verdict**: {:?}\n", report.verdict));
        md.push_str(&format!(
            "- **Compliance Rate**: {:.1}%\n",
            report.compliance_rate * 100.0
        ));
        md.push_str(&format!(
            "- **Total Requirements**: {}\n",
            report.total_requirements
        ));
        md.push_str(&format!(
            "- **Satisfied (Passing Tests)**: {}\n",
            report.satisfied_count
        ));
        md.push_str(&format!(
            "- **Gaps (Failing Tests)**: {}\n",
            report.gap_count
        ));
        md.push_str(&format!(
            "- **Sentinels (Wave 0 Scaffolding)**: {}\n",
            report.sentinel_count
        ));
        md.push_str(&format!(
            "- **Missing Verification Commands**: {}\n",
            report.missing_count
        ));
        md.push_str(&format!(
            "- **Sampling Feedback Latency**: {}ms\n\n",
            report.feedback_latency_ms
        ));

        md.push_str("## Requirement Coverage Matrix\n\n");
        md.push_str("| Requirement ID | Description | Wave | Status | Command / Details |\n");
        md.push_str("|---|---|---|---|---|\n");

        for item in &report.items {
            let req = &item.requirement;
            let (status_str, details) = match &item.status {
                NyquistCheckStatus::Satisfied { duration_ms, .. } => (
                    "PASS",
                    format!(
                        "{} ({}ms)",
                        req.test_command.as_deref().unwrap_or(""),
                        duration_ms
                    ),
                ),
                NyquistCheckStatus::Gap {
                    reason,
                    stderr_snippet,
                } => (
                    "GAP",
                    format!("{}: {}", reason, stderr_snippet.replace('\n', " ")),
                ),
                NyquistCheckStatus::Sentinel { target_test_file } => (
                    "SENTINEL",
                    format!("Wave 0 scaffolding: {}", target_test_file),
                ),
                NyquistCheckStatus::MissingCommand => {
                    ("MISSING", "No automated test command defined".into())
                }
                NyquistCheckStatus::Skipped { justification } => ("SKIPPED", justification.clone()),
            };

            md.push_str(&format!(
                "| `{}` | {} | {} | **{}** | `{}` |\n",
                req.id,
                req.description.replace('|', "\\|"),
                req.wave,
                status_str,
                details.replace('|', "\\|")
            ));
        }

        md.push('\n');
        if report.gap_count > 0 || report.missing_count > 0 {
            md.push_str("## Action Required\n\n");
            md.push_str("> [!WARNING]\n");
            md.push_str("> Validation gaps detected. The phase cannot be signed off until failing tests are resolved or missing commands are provided.\n");
        } else {
            md.push_str("## Validation Sign-Off\n\n");
            md.push_str("> [!NOTE]\n");
            md.push_str("> All phase requirements are backed by automated tests meeting Nyquist sampling criteria.\n");
        }

        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_checker::TaskModel;

    #[test]
    fn test_sentinel_parsing() {
        let cmd = "MISSING — Wave 0 must create tests/nyquist_test.rs";
        assert!(NyquistEngine::is_sentinel(cmd));
        assert_eq!(
            NyquistEngine::parse_sentinel(cmd),
            Some("tests/nyquist_test.rs".to_string())
        );

        let non_sentinel = "cargo test -p m31a-verify";
        assert!(!NyquistEngine::is_sentinel(non_sentinel));
        assert_eq!(NyquistEngine::parse_sentinel(non_sentinel), None);
    }

    #[test]
    fn test_enforce_wave_0_scaffolding_catches_unscaffolded_sentinel() {
        let plan = PlanModel {
            id: "01-01".into(),
            phase: "01".into(),
            wave: 1,
            depends_on: vec![],
            requirements: vec!["REQ-01".into()],
            files_modified: vec![PathBuf::from("src/feature.rs")],
            must_haves_truths: vec![],
            must_haves_artifacts: vec![],
            must_haves_key_links: vec![],
            tasks: vec![TaskModel {
                name: "Feature task".into(),
                task_type: "auto".into(),
                files: vec![PathBuf::from("src/feature.rs")],
                action: "Implement feature".into(),
                verify: Some("MISSING — Wave 0 must create tests/feature_test.rs".into()),
                done: Some("Done".into()),
            }],
        };

        let plans = vec![plan.clone()];
        let issues = NyquistEngine::enforce_wave_0_scaffolding(&plans);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, IssueSeverity::Blocker);
        assert!(
            issues[0]
                .description
                .contains("no Wave 0 task creates 'tests/feature_test.rs'")
        );

        // Now add Wave 0 plan that scaffolds tests/feature_test.rs
        let wave_0_plan = PlanModel {
            id: "01-00".into(),
            phase: "01".into(),
            wave: 0,
            depends_on: vec![],
            requirements: vec![],
            files_modified: vec![PathBuf::from("tests/feature_test.rs")],
            must_haves_truths: vec![],
            must_haves_artifacts: vec![],
            must_haves_key_links: vec![],
            tasks: vec![TaskModel {
                name: "Scaffold tests".into(),
                task_type: "auto".into(),
                files: vec![PathBuf::from("tests/feature_test.rs")],
                action: "Create test scaffolding".into(),
                verify: Some("cargo check --tests".into()),
                done: Some("Scaffolded".into()),
            }],
        };

        let plans_after = vec![wave_0_plan, plan];
        let issues_after = NyquistEngine::enforce_wave_0_scaffolding(&plans_after);
        assert_eq!(issues_after.len(), 0);
    }

    #[test]
    fn test_audit_coverage_and_render_markdown() {
        let tmp = tempfile::tempdir().unwrap();
        let reqs = vec![
            NyquistRequirement {
                id: "REQ-01".into(),
                description: "Exit zero test".into(),
                test_command: Some("true".into()),
                test_file: None,
                wave: 1,
                plan_id: "01-01".into(),
            },
            NyquistRequirement {
                id: "REQ-02".into(),
                description: "Exit non-zero test".into(),
                test_command: Some("false".into()),
                test_file: None,
                wave: 1,
                plan_id: "01-01".into(),
            },
            NyquistRequirement {
                id: "REQ-03".into(),
                description: "Wave 0 sentinel".into(),
                test_command: Some("MISSING — Wave 0 must create tests/unit.rs".into()),
                test_file: None,
                wave: 1,
                plan_id: "01-01".into(),
            },
        ];

        let report = NyquistEngine::audit_coverage("phase-03", &reqs, tmp.path());
        assert_eq!(report.total_requirements, 3);
        assert_eq!(report.satisfied_count, 1);
        assert_eq!(report.gap_count, 1);
        assert_eq!(report.sentinel_count, 1);
        assert_eq!(report.verdict, NyquistVerdict::GapsDetected);

        let md = NyquistEngine::render_validation_markdown(&report);
        assert!(md.contains("# Phase phase-03 Validation Report (Nyquist Test Audit)"));
        assert!(md.contains("| `REQ-01` | Exit zero test | 1 | **PASS**"));
        assert!(md.contains("| `REQ-02` | Exit non-zero test | 1 | **GAP**"));
        assert!(md.contains("| `REQ-03` | Wave 0 sentinel | 1 | **SENTINEL**"));
        assert!(md.contains("Validation gaps detected"));
    }
}
