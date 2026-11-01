//! Verification-gated task completion state machine and timeline tracker.
//!
//! Enforces empirical evidence of testing and build readiness before an agent
//! can declare victory, preventing premature or hallucinated task completions.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanCriterion {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentPlan {
    pub goal: String,
    pub steps: Vec<String>,
    pub criteria: Vec<PlanCriterion>,
    pub requires_preview: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompletionOutcome {
    Complete,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionEvidence {
    pub id: String,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCompletionReport {
    pub outcome: CompletionOutcome,
    pub summary: String,
    #[serde(default)]
    pub criteria: Vec<CriterionEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Success,
    Failure {
        exit_code: Option<i32>,
        diagnostics: String,
    },
    Timeout {
        duration_ms: u64,
    },
    Cancelled,
    NotExecuted {
        reason: String,
    },
    InfrastructureFailure {
        error: String,
    },
    InsufficientEvidence {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceType {
    CompilerCheck,
    UnitTest,
    IntegrationTest,
    RegressionTest,
    StaticAnalysis,
    SecurityTest,
    MutationAudit,
    RuntimeBehavior,
    ArtifactInspection,
    ArchitectureVerification,
    // Backwards compatibility
    FileInspection,
    TestExecution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub requirement_id: String,
    pub evidence_id: String,
    pub evidence_type: EvidenceType,
    pub executed_operation: String,
    pub execution_timestamp: i64,
    pub actual_result: String,
    pub success: bool,
    #[serde(default)]
    pub relevant_artifacts: Vec<String>,
    pub tool_execution_reference: Option<String>,
    #[serde(default)]
    pub test_and_diagnostic_references: Vec<String>,
    pub environment_info: String,
    pub verification_limitations: Option<String>,
    #[serde(default)]
    pub execution_turn: Option<usize>,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub tests_passed: Option<usize>,
    #[serde(default)]
    pub tests_failed: Option<usize>,
    #[serde(default)]
    pub tests_total: Option<usize>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub status: Option<VerificationStatus>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvidenceLedger {
    records: Vec<EvidenceRecord>,
}

impl EvidenceLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, record: EvidenceRecord) {
        self.records.push(record);
    }

    pub fn get_record(&self, evidence_id: &str) -> Option<&EvidenceRecord> {
        self.records.iter().find(|r| r.evidence_id == evidence_id)
    }

    pub fn records_for_requirement(&self, requirement_id: &str) -> Vec<&EvidenceRecord> {
        self.records
            .iter()
            .filter(|r| r.requirement_id == requirement_id)
            .collect()
    }

    pub fn has_valid_evidence_for(&self, requirement_id: &str, evidence_id: &str) -> bool {
        self.records.iter().any(|r| {
            r.requirement_id == requirement_id && r.evidence_id == evidence_id && r.success
        })
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }
}

/// Execution timeline tracking mutations, tests, and readiness states across turns.
#[derive(Debug, Clone, Default)]
pub struct VerificationTimeline {
    pub plan: Option<AgentPlan>,
    pub last_edit_turn: Option<usize>,
    pub last_passing_test_turn: Option<usize>,
    pub last_check_turn: Option<usize>,
    pub last_failure_turn: Option<usize>,
    pub preview_ready: bool,
    pub total_steps: usize,
    pub tool_evidences: Vec<String>,
    pub ledger: EvidenceLedger,
}

impl VerificationTimeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_plan(&mut self, plan: AgentPlan) {
        self.plan = Some(plan);
    }

    pub fn record_edit(&mut self, turn: usize) {
        self.last_edit_turn = Some(turn);
    }

    pub fn record_test_result(&mut self, turn: usize, success: bool, call_id: Option<&str>) {
        if success {
            self.last_passing_test_turn = Some(turn);
            if let Some(id) = call_id {
                self.tool_evidences.push(id.to_string());
            }
        } else {
            self.last_failure_turn = Some(turn);
        }
    }

    pub fn record_check_result(&mut self, turn: usize, success: bool, call_id: Option<&str>) {
        if success {
            self.last_check_turn = Some(turn);
            if let Some(id) = call_id {
                self.tool_evidences.push(id.to_string());
            }
        } else {
            self.last_failure_turn = Some(turn);
        }
    }

    pub fn record_failure(&mut self, turn: usize) {
        self.last_failure_turn = Some(turn);
    }

    pub fn record_preview_ready(&mut self, ready: bool) {
        self.preview_ready = ready;
    }

    pub fn record_criterion_evidence(&mut self, record: EvidenceRecord) {
        self.ledger.record(record);
    }

    /// Evaluates whether the agent meets all empirical conditions required to finish.
    pub fn evaluate_completion(&self, report: &AgentCompletionReport) -> Result<(), Vec<String>> {
        if report.outcome == CompletionOutcome::Blocked {
            // Blocked is a valid early exit with explanation
            return Ok(());
        }

        let mut problems = Vec::new();

        // 1. Mandatory plan check
        let plan = match &self.plan {
            Some(p) => p,
            None => {
                problems.push("No recorded task plan found. Call 'agent_plan' with observable criteria before completing.".to_string());
                return Err(problems);
            }
        };

        // Reject empty plan criteria
        if plan.criteria.is_empty() {
            problems.push("Empty plan criteria rejected. At least one observable plan criterion must be declared.".to_string());
        }

        // Duplicate criterion check in completion report
        let mut seen_criteria = std::collections::HashSet::new();
        for reported in &report.criteria {
            if !seen_criteria.insert(&reported.id) {
                problems.push(format!("Duplicate criterion reported: '{}'. Each criterion must be reported exactly once.", reported.id));
            }
        }

        // 2. Criteria reporting and evidence validation
        let planned_ids: std::collections::HashSet<&str> =
            plan.criteria.iter().map(|c| c.id.as_str()).collect();
        let reported_ids: std::collections::HashSet<&str> =
            report.criteria.iter().map(|c| c.id.as_str()).collect();

        for criterion in &plan.criteria {
            if !reported_ids.contains(criterion.id.as_str()) {
                problems.push(format!(
                    "Criterion '{}' from plan was not reported in completion call.",
                    criterion.id
                ));
            }
        }

        for reported in &report.criteria {
            if !planned_ids.contains(reported.id.as_str()) {
                problems.push(format!(
                    "Reported criterion '{}' was not declared in the original plan.",
                    reported.id
                ));
            }

            // Evidence array must not be empty
            if reported.evidence.is_empty() {
                problems.push(format!(
                    "Criterion '{}' reported with empty evidence array. Every criterion must map to one or more verified evidence IDs.",
                    reported.id
                ));
            } else {
                for ev_id in &reported.evidence {
                    if let Some(record) = self.ledger.get_record(ev_id) {
                        // Evidence belongs to a different criterion
                        if record.requirement_id != reported.id {
                            problems.push(format!(
                                "Criterion '{}' references foreign evidence ID '{}' belonging to requirement '{}'.",
                                reported.id, ev_id, record.requirement_id
                            ));
                        }
                        // Evidence failed or contains non-success verification status
                        if !record.success {
                            problems.push(format!(
                                "Criterion '{}' references failed evidence ID '{}'.",
                                reported.id, ev_id
                            ));
                        } else if let Some(ref status) = record.status
                            && !matches!(status, VerificationStatus::Success)
                        {
                            problems.push(format!(
                                    "Criterion '{}' references evidence ID '{}' with non-successful status '{:?}'.",
                                    reported.id, ev_id, status
                                ));
                        }

                        // Freshness check: Prevent Turn 0 or pre-modification evidence from satisfying post-modification criteria
                        if let Some(edit_turn) = self.last_edit_turn {
                            match record.execution_turn {
                                None => {
                                    problems.push(format!(
                                        "Criterion '{}' references evidence ID '{}' without an execution turn timestamp, but files were modified at turn {}.",
                                        reported.id, ev_id, edit_turn
                                    ));
                                }
                                Some(ev_turn) if ev_turn <= edit_turn => {
                                    problems.push(format!(
                                        "Criterion '{}' references pre-modification or Turn 0 evidence ID '{}' recorded at turn {} before latest edit at turn {}.",
                                        reported.id, ev_id, ev_turn, edit_turn
                                    ));
                                }
                                _ => {}
                            }
                        }

                        // Test evidence must execute > 0 tests: 0 executed tests rejected as verification proof
                        let is_test = matches!(
                            record.evidence_type,
                            EvidenceType::UnitTest
                                | EvidenceType::IntegrationTest
                                | EvidenceType::RegressionTest
                                | EvidenceType::SecurityTest
                                | EvidenceType::TestExecution
                        );
                        if is_test {
                            let is_zero_tests = if let Some(passed) = record.tests_passed {
                                passed == 0 || record.tests_total == Some(0)
                            } else {
                                let r = record.actual_result.trim();
                                r.is_empty()
                                    || r.contains("running 0 tests")
                                    || r.contains("no tests were found")
                                    || r.contains("collected 0 items")
                                    || r.lines().any(|l| {
                                        let trimmed = l.trim();
                                        trimmed.starts_with("test result:")
                                            && (trimmed.contains(". 0 passed;")
                                                || trimmed.contains(" 0 passed;"))
                                    })
                            };

                            if is_zero_tests {
                                problems.push(format!(
                                    "Criterion '{}' references test evidence ID '{}' that produced no test execution proof (0 tests executed).",
                                    reported.id, ev_id
                                ));
                            }
                        }
                    } else {
                        // Strict requirement: All criteria must map to criterion-bound evidence records in EvidenceLedger
                        problems.push(format!(
                            "Criterion '{}' references unverified evidence ID '{}' not found in EvidenceLedger. Global untargeted tool output is not permitted as criterion proof.",
                            reported.id, ev_id
                        ));
                    }
                }
            }
        }

        // 3. Post-edit verification: has code been modified?
        if let Some(edit_turn) = self.last_edit_turn {
            let tested_after_edit = self
                .last_passing_test_turn
                .map(|t| t > edit_turn)
                .unwrap_or(false);
            let checked_after_edit = self.last_check_turn.map(|t| t > edit_turn).unwrap_or(false);

            if !tested_after_edit && !checked_after_edit {
                problems.push(format!(
                    "Files were modified at turn {}, but neither tests nor compiler verification passed after the edit. Run verification checks before completing.",
                    edit_turn
                ));
            }

            if let Some(fail_turn) = self.last_failure_turn
                && fail_turn > edit_turn
            {
                problems.push(format!(
                        "A command failure occurred at turn {} (after the last edit at turn {}). Resolve all failures before completing.",
                        fail_turn, edit_turn
                    ));
            }
        }

        // 4. Preview requirement check
        if plan.requires_preview && !self.preview_ready {
            problems.push("Plan specified 'requires_preview: true', but no running preview server has been confirmed.".to_string());
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }

    /// Evaluates if an unprovoked turn completion (agent stopped without calling agent_complete)
    /// is premature when code mutations occurred.
    pub fn is_premature_stop(&self) -> Option<String> {
        if let Some(edit_turn) = self.last_edit_turn {
            let tested_after_edit = self
                .last_passing_test_turn
                .map(|t| t > edit_turn)
                .unwrap_or(false);
            let checked_after_edit = self.last_check_turn.map(|t| t > edit_turn).unwrap_or(false);

            if !tested_after_edit && !checked_after_edit {
                return Some(format!(
                    "Verification required: Code was edited at step {}, but no successful test or compiler check has passed since. Please run tests or compiler checks to verify your changes.",
                    edit_turn
                ));
            }

            if let Some(fail_turn) = self.last_failure_turn
                && fail_turn >= edit_turn
            {
                return Some(format!(
                    "Unresolved error: A failure occurred at step {} after recent edits. You must fix the error and verify before completing.",
                    fail_turn
                ));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_gating_blocks_untested_edits() {
        let mut timeline = VerificationTimeline::new();
        timeline.record_plan(AgentPlan {
            goal: "Refactor auth".to_string(),
            steps: vec!["Edit JWT".to_string(), "Run tests".to_string()],
            criteria: vec![PlanCriterion {
                id: "c1".to_string(),
                description: "Tests pass".to_string(),
            }],
            requires_preview: false,
        });

        timeline.record_edit(1);

        let report = AgentCompletionReport {
            outcome: CompletionOutcome::Complete,
            summary: "Finished edits".to_string(),
            criteria: vec![CriterionEvidence {
                id: "c1".to_string(),
                evidence: vec!["call-123".to_string()],
            }],
        };

        // Fails because no tests ran after turn 1
        let res = timeline.evaluate_completion(&report);
        assert!(res.is_err());
        let problems = res.unwrap_err();
        assert!(
            problems.iter().any(
                |p| p.contains("neither tests nor compiler verification passed after the edit")
            )
        );

        // Now test passes at turn 2 with recorded tool evidence
        timeline.record_test_result(2, true, Some("call-123"));
        timeline.record_criterion_evidence(EvidenceRecord {
            requirement_id: "c1".to_string(),
            evidence_id: "call-123".to_string(),
            evidence_type: EvidenceType::TestExecution,
            executed_operation: "cargo test".to_string(),
            execution_timestamp: 123456789,
            actual_result: "test result: ok. 5 passed; 0 failed".to_string(),
            success: true,
            relevant_artifacts: vec![],
            tool_execution_reference: Some("call-123".to_string()),
            test_and_diagnostic_references: vec![],
            environment_info: "linux-x86_64".to_string(),
            verification_limitations: None,
            execution_turn: Some(2),
            exit_code: Some(0),
            tests_passed: Some(5),
            tests_failed: Some(0),
            tests_total: Some(5),
            duration_ms: Some(100),
            status: Some(VerificationStatus::Success),
        });
        assert!(timeline.evaluate_completion(&report).is_ok());
    }

    #[test]
    fn test_verification_rejects_empty_and_fabricated_evidence() {
        let mut timeline = VerificationTimeline::new();
        timeline.record_plan(AgentPlan {
            goal: "Fix bug".to_string(),
            steps: vec!["Fix".to_string()],
            criteria: vec![PlanCriterion {
                id: "c1".to_string(),
                description: "Unit tests pass".to_string(),
            }],
            requires_preview: false,
        });

        // 1. Rejects empty evidence
        let empty_report = AgentCompletionReport {
            outcome: CompletionOutcome::Complete,
            summary: "Fixed".to_string(),
            criteria: vec![CriterionEvidence {
                id: "c1".to_string(),
                evidence: vec![],
            }],
        };
        let err1 = timeline.evaluate_completion(&empty_report).unwrap_err();
        assert!(err1.iter().any(|e| e.contains("empty evidence array")));

        // 2. Rejects fabricated evidence
        let fake_report = AgentCompletionReport {
            outcome: CompletionOutcome::Complete,
            summary: "Fixed".to_string(),
            criteria: vec![CriterionEvidence {
                id: "c1".to_string(),
                evidence: vec!["fabricated-evidence-999".to_string()],
            }],
        };
        let err2 = timeline.evaluate_completion(&fake_report).unwrap_err();
        assert!(err2.iter().any(|e| e.contains("unverified evidence ID")));

        // 3. Accepts valid evidence recorded in ledger
        timeline.ledger.record(EvidenceRecord {
            requirement_id: "c1".to_string(),
            evidence_id: "ev-proof-1".to_string(),
            evidence_type: EvidenceType::TestExecution,
            executed_operation: "cargo test".to_string(),
            execution_timestamp: 123456789,
            actual_result: "test result: ok. 10 passed; 0 failed".to_string(),
            success: true,
            relevant_artifacts: vec!["src/lib.rs".to_string()],
            tool_execution_reference: Some("call-test-1".to_string()),
            test_and_diagnostic_references: vec!["test_something".to_string()],
            environment_info: "linux-x86_64".to_string(),
            verification_limitations: None,
            execution_turn: None,
            exit_code: Some(0),
            tests_passed: Some(10),
            tests_failed: Some(0),
            tests_total: Some(10),
            duration_ms: Some(250),
            status: Some(VerificationStatus::Success),
        });

        let valid_report = AgentCompletionReport {
            outcome: CompletionOutcome::Complete,
            summary: "Fixed".to_string(),
            criteria: vec![CriterionEvidence {
                id: "c1".to_string(),
                evidence: vec!["ev-proof-1".to_string()],
            }],
        };
        assert!(timeline.evaluate_completion(&valid_report).is_ok());
    }

    #[test]
    fn test_passing_test_command_with_zero_tests_executed_rejected_as_evidence() {
        let mut timeline = VerificationTimeline::new();
        timeline.record_plan(AgentPlan {
            goal: "Run test suite".to_string(),
            steps: vec!["Execute tests".to_string()],
            criteria: vec![PlanCriterion {
                id: "c1".to_string(),
                description: "All unit tests pass".to_string(),
            }],
            requires_preview: false,
        });

        // Record evidence with 0 tests executed
        timeline.record_criterion_evidence(EvidenceRecord {
            requirement_id: "c1".to_string(),
            evidence_id: "ev-zero-tests".to_string(),
            evidence_type: EvidenceType::UnitTest,
            executed_operation: "cargo test --bin foo".to_string(),
            execution_timestamp: 100,
            actual_result: "running 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored"
                .to_string(),
            success: true,
            relevant_artifacts: vec![],
            tool_execution_reference: Some("call-zero".to_string()),
            test_and_diagnostic_references: vec![],
            environment_info: "linux".to_string(),
            verification_limitations: None,
            execution_turn: Some(1),
            exit_code: Some(0),
            tests_passed: Some(0),
            tests_failed: Some(0),
            tests_total: Some(0),
            duration_ms: Some(50),
            status: Some(VerificationStatus::Success),
        });

        let report = AgentCompletionReport {
            outcome: CompletionOutcome::Complete,
            summary: "Verified".to_string(),
            criteria: vec![CriterionEvidence {
                id: "c1".to_string(),
                evidence: vec!["ev-zero-tests".to_string()],
            }],
        };

        let err = timeline.evaluate_completion(&report).unwrap_err();
        assert!(
            err.iter()
                .any(|e| e.contains("0 executed tests") || e.contains("0 tests executed"))
        );
    }

    #[test]
    fn test_pre_modification_evidence_rejected_for_post_modification_verification() {
        let mut timeline = VerificationTimeline::new();
        timeline.record_plan(AgentPlan {
            goal: "Refactor core module".to_string(),
            steps: vec!["Edit code".to_string()],
            criteria: vec![PlanCriterion {
                id: "c1".to_string(),
                description: "Tests pass after edit".to_string(),
            }],
            requires_preview: false,
        });

        // Evidence was recorded at Turn 1
        timeline.record_criterion_evidence(EvidenceRecord {
            requirement_id: "c1".to_string(),
            evidence_id: "ev-turn-1".to_string(),
            evidence_type: EvidenceType::UnitTest,
            executed_operation: "cargo test".to_string(),
            execution_timestamp: 100,
            actual_result: "test result: ok. 5 passed; 0 failed".to_string(),
            success: true,
            relevant_artifacts: vec![],
            tool_execution_reference: Some("call-1".to_string()),
            test_and_diagnostic_references: vec![],
            environment_info: "linux".to_string(),
            verification_limitations: None,
            execution_turn: Some(1),
            exit_code: Some(0),
            tests_passed: Some(5),
            tests_failed: Some(0),
            tests_total: Some(5),
            duration_ms: Some(120),
            status: Some(VerificationStatus::Success),
        });

        // File modified at Turn 2
        timeline.record_edit(2);

        let report = AgentCompletionReport {
            outcome: CompletionOutcome::Complete,
            summary: "Finished".to_string(),
            criteria: vec![CriterionEvidence {
                id: "c1".to_string(),
                evidence: vec!["ev-turn-1".to_string()],
            }],
        };

        let err = timeline.evaluate_completion(&report).unwrap_err();
        assert!(
            err.iter()
                .any(|e| e.contains("pre-modification or Turn 0 evidence ID"))
        );
    }

    #[test]
    fn test_infrastructure_failure_marked_as_failure_rather_than_passed() {
        let mut timeline = VerificationTimeline::new();
        timeline.record_plan(AgentPlan {
            goal: "Build verification".to_string(),
            steps: vec!["Run check".to_string()],
            criteria: vec![PlanCriterion {
                id: "c1".to_string(),
                description: "Build passes".to_string(),
            }],
            requires_preview: false,
        });

        // Record evidence with InfrastructureFailure status
        timeline.record_criterion_evidence(EvidenceRecord {
            requirement_id: "c1".to_string(),
            evidence_id: "ev-infra-fail".to_string(),
            evidence_type: EvidenceType::CompilerCheck,
            executed_operation: "cargo check".to_string(),
            execution_timestamp: 100,
            actual_result: "Sandbox backend crashed".to_string(),
            success: false,
            relevant_artifacts: vec![],
            tool_execution_reference: Some("call-fail".to_string()),
            test_and_diagnostic_references: vec![],
            environment_info: "linux".to_string(),
            verification_limitations: None,
            execution_turn: Some(1),
            exit_code: None,
            tests_passed: None,
            tests_failed: None,
            tests_total: None,
            duration_ms: None,
            status: Some(VerificationStatus::InfrastructureFailure {
                error: "Bwrap segfault".to_string(),
            }),
        });

        let report = AgentCompletionReport {
            outcome: CompletionOutcome::Complete,
            summary: "Done".to_string(),
            criteria: vec![CriterionEvidence {
                id: "c1".to_string(),
                evidence: vec!["ev-infra-fail".to_string()],
            }],
        };

        let err = timeline.evaluate_completion(&report).unwrap_err();
        assert!(err.iter().any(|e| e.contains("failed evidence ID")));
    }

    #[test]
    fn test_verification_blocked_outcome_is_accepted() {
        let mut timeline = VerificationTimeline::new();
        timeline.record_edit(1);

        let report = AgentCompletionReport {
            outcome: CompletionOutcome::Blocked,
            summary: "Missing database credentials".to_string(),
            criteria: vec![],
        };

        assert!(timeline.evaluate_completion(&report).is_ok());
    }
}
