use std::sync::Arc;

use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::kernel::seams::recovery::{
    FailureClassification, FailureClassificationRequest, RecoveryAction, RecoveryEngine,
    RecoveryError, RecoveryStrategyRequest,
};
use crate::recovery::budget::{
    BudgetEvaluation, RecoveryAttemptRecord, RecoveryBudgetTracker, compute_mutation_fingerprint,
};
use crate::recovery::classifier::FailureClassifier;
use crate::verification::diagnostician::ModelDiagnostician;

/// Production implementation of `RecoveryEngine` seam trait.
#[derive(Clone)]
pub struct ProductionRecoveryEngine {
    classifier: FailureClassifier,
    budget_tracker: RecoveryBudgetTracker,
    pool: Option<SqlitePool>,
    diagnostician: Arc<ModelDiagnostician>,
}

impl Default for ProductionRecoveryEngine {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ProductionRecoveryEngine {
    pub fn new(pool: Option<SqlitePool>) -> Self {
        Self {
            classifier: FailureClassifier::new(),
            budget_tracker: RecoveryBudgetTracker::default(),
            pool,
            diagnostician: Arc::new(ModelDiagnostician::new()),
        }
    }

    pub fn with_components(
        classifier: FailureClassifier,
        budget_tracker: RecoveryBudgetTracker,
        pool: Option<SqlitePool>,
    ) -> Self {
        Self {
            classifier,
            budget_tracker,
            pool,
            diagnostician: Arc::new(ModelDiagnostician::new()),
        }
    }

    pub fn with_diagnostician(mut self, diagnostician: Arc<ModelDiagnostician>) -> Self {
        self.diagnostician = diagnostician;
        self
    }

    pub fn budget_tracker(&self) -> &RecoveryBudgetTracker {
        &self.budget_tracker
    }

    pub fn classifier(&self) -> &FailureClassifier {
        &self.classifier
    }

    pub fn diagnostician(&self) -> &ModelDiagnostician {
        &self.diagnostician
    }
}

#[async_trait]
impl RecoveryEngine for ProductionRecoveryEngine {
    async fn classify_failure(
        &self,
        req: FailureClassificationRequest,
    ) -> Result<FailureClassification, RecoveryError> {
        let class = self.classifier.classify(None, &req.error_message).await;
        Ok(class)
    }

    async fn determine_recovery(
        &self,
        req: RecoveryStrategyRequest,
    ) -> Result<RecoveryAction, RecoveryError> {
        // NOTE: a previous revision compared `req.task_id` against a freshly
        // minted default id. That comparison could never be true (every
        // default is a fresh random identifier), so it was dead logic
        // masquerading as a guard. It is removed: task-less recovery is
        // handled explicitly by the controller stages, which know when no
        // task is active.

        let task_max_retries: Option<i64> = if let Some(ref pool) = self.pool {
            let task_repo =
                crate::persistence::sqlite::repositories::SqliteTaskRepository::new(pool.clone());
            task_repo
                .get_max_retries(req.task_id)
                .await
                .ok()
                .flatten()
                .map(|r| r as i64)
        } else {
            None
        };

        if let Some(0) = task_max_retries {
            return Ok(RecoveryAction::AbortMission {
                reason: "Task recovery strategy is Fail (0 retries allowed)".to_string(),
            });
        }

        if !req.failure_class.is_retryable() {
            return Ok(RecoveryAction::AbortMission {
                reason: format!(
                    "Failure class '{:?}' is deterministically non-retryable under policy",
                    req.failure_class
                ),
            });
        }

        let (task_retries, mission_retries) = if let Some(ref pool) = self.pool {
            let t = RecoveryBudgetTracker::count_task_attempts(pool, req.task_id)
                .await
                .unwrap_or(req.retry_count);
            let m = RecoveryBudgetTracker::count_mission_attempts(pool, req.mission_id)
                .await
                .unwrap_or(req.retry_count);
            (t, m)
        } else {
            (req.retry_count, req.retry_count)
        };

        // Mission recovery ceiling check takes precedence over task-level
        // retry/replan exhaustion.
        if mission_retries >= self.budget_tracker.mission_retry_ceiling {
            return Ok(RecoveryAction::AbortMission {
                reason: format!(
                    "Mission recovery ceiling exhausted ({}/{})",
                    mission_retries, self.budget_tracker.mission_retry_ceiling
                ),
            });
        }

        if let Some(max_r) = task_max_retries
            && task_retries >= max_r as usize
        {
            return Ok(RecoveryAction::AbortMission {
                reason: format!("Task retries exhausted ({}/{})", task_retries, max_r),
            });
        }

        let action = if let Some(ref err_msg) = req.error_message
            && !err_msg.trim().is_empty()
        {
            let mut ctx = crate::verification::diagnostician::DiagnosticianContext::new(
                err_msg,
                Some(1),
                None,
                Some(err_msg.clone()),
            );
            ctx.attempt_count = task_retries;

            let ev = crate::verification::types::FailureEvidence::new(
                req.mission_id,
                req.task_id,
                err_msg,
            )
            .with_process_output(Some(1), None, Some(err_msg.clone()));
            ctx.failure_evidence = Some(ev);

            // Evidence-based diagnosis: prefer model reasoning over failure
            // evidence when a caller is attached; fall back to the deterministic
            // heuristic when the model is unavailable or its output is unusable.
            // Recovery itself never fails merely because diagnosis degraded —
            // boundedness is preserved either way.
            let hypothesis = match self.diagnostician.diagnose_hypothesis(&ctx).await {
                Ok(hyp) => hyp,
                Err(e) => {
                    tracing::warn!("model diagnosis unavailable ({e}); using heuristic hypothesis");
                    ModelDiagnostician::heuristic_hypothesis(&ctx)
                }
            };

            // Compute SemanticFailureSignature for circuit breaker
            let target_domain =
                crate::recovery::budget::extract_target_domain(err_msg, &hypothesis.affected_files);
            let norm_err = crate::recovery::budget::normalize_error_message(err_msg);
            let cand_fp = hypothesis
                .repair_proposal
                .as_ref()
                .map(compute_mutation_fingerprint);

            let failure_sig = crate::recovery::budget::SemanticFailureSignature::new(
                req.failure_class,
                target_domain,
                norm_err,
                cand_fp,
            );

            // Record this failure signature into history (including current attempt)
            self.budget_tracker
                .record_failure_signature(req.mission_id, failure_sig.clone());

            // Check if circuit breaker is tripped on repeated equivalent failure state
            if self
                .budget_tracker
                .is_circuit_broken(req.mission_id, &failure_sig)
            {
                return Ok(RecoveryAction::AbortMission {
                    reason: format!(
                        "Recovery circuit breaker tripped: repeated semantically equivalent failure in class '{:?}' on target '{}' without progress",
                        req.failure_class, failure_sig.target_domain
                    ),
                });
            }

            match hypothesis.recommended_action {
                crate::verification::diagnostician::RecoveryRecommendation::Repair => {
                    if let Some(proposal) = hypothesis.repair_proposal {
                        let eval = self.budget_tracker.evaluate_proposal_for_mission(
                            Some(req.mission_id),
                            req.task_id,
                            &proposal,
                            req.failure_class,
                            req.retry_count,
                            task_retries,
                            mission_retries,
                        );
                        match eval {
                            BudgetEvaluation::Permitted { .. } => {
                                self.budget_tracker.record_mission_attempt_fingerprint(
                                    req.mission_id,
                                    req.task_id,
                                    compute_mutation_fingerprint(&proposal),
                                );
                                RecoveryAction::Repair {
                                    proposal: Box::new(proposal),
                                    reason: hypothesis.root_cause,
                                }
                            }
                            BudgetEvaluation::RepeatedIdentical { reason } => {
                                RecoveryAction::Escalate {
                                    reason: format!(
                                        "Repeated identical repair attempt rejected: {reason}"
                                    ),
                                }
                            }
                            BudgetEvaluation::Exhausted { reason, .. } => RecoveryAction::Replan {
                                reason: format!("Repair budget exhausted, replanning: {reason}"),
                            },
                            BudgetEvaluation::NonRetryable { reason } => {
                                RecoveryAction::AbortMission { reason }
                            }
                        }
                    } else {
                        let eval = self.budget_tracker.evaluate(
                            req.failure_class,
                            req.retry_count,
                            task_retries,
                            mission_retries,
                            None,
                        );
                        match eval {
                            BudgetEvaluation::Permitted { backoff_delay, .. } => {
                                RecoveryAction::Retry {
                                    delay_ms: backoff_delay.as_millis() as u64,
                                }
                            }
                            _ => RecoveryAction::Replan {
                                reason: format!(
                                    "Repair recommended but no viable mutation synthesized: {}",
                                    hypothesis.root_cause
                                ),
                            },
                        }
                    }
                }
                crate::verification::diagnostician::RecoveryRecommendation::Replan => {
                    let eval = self.budget_tracker.evaluate(
                        req.failure_class,
                        req.retry_count,
                        task_retries,
                        mission_retries,
                        None,
                    );
                    match eval {
                        BudgetEvaluation::Exhausted { reason, .. } => {
                            RecoveryAction::AbortMission {
                                reason: format!(
                                    "Recovery budget exhausted, replan aborted: {reason}"
                                ),
                            }
                        }
                        BudgetEvaluation::NonRetryable { reason } => {
                            RecoveryAction::AbortMission { reason }
                        }
                        _ => RecoveryAction::Replan {
                            reason: hypothesis.root_cause,
                        },
                    }
                }
                crate::verification::diagnostician::RecoveryRecommendation::Rollback => {
                    RecoveryAction::Rollback {
                        reason: hypothesis.root_cause,
                    }
                }
                crate::verification::diagnostician::RecoveryRecommendation::Escalate => {
                    RecoveryAction::Escalate {
                        reason: hypothesis.root_cause,
                    }
                }
                crate::verification::diagnostician::RecoveryRecommendation::Retry => {
                    let evaluation = self.budget_tracker.evaluate(
                        req.failure_class,
                        req.retry_count,
                        task_retries,
                        mission_retries,
                        None,
                    );
                    match evaluation {
                        BudgetEvaluation::Permitted { backoff_delay, .. } => {
                            RecoveryAction::Retry {
                                delay_ms: backoff_delay.as_millis() as u64,
                            }
                        }
                        BudgetEvaluation::NonRetryable { reason } => {
                            RecoveryAction::AbortMission { reason }
                        }
                        BudgetEvaluation::Exhausted { reason, .. } => {
                            if reason.contains("Mission recovery ceiling") {
                                RecoveryAction::AbortMission { reason }
                            } else {
                                RecoveryAction::Replan {
                                    reason: format!(
                                        "Retry budget exhausted, escalating to replan: {reason}"
                                    ),
                                }
                            }
                        }
                        BudgetEvaluation::RepeatedIdentical { reason } => {
                            RecoveryAction::Escalate { reason }
                        }
                    }
                }
            }
        } else {
            let evaluation = self.budget_tracker.evaluate(
                req.failure_class,
                req.retry_count,
                task_retries,
                mission_retries,
                None,
            );
            match evaluation {
                BudgetEvaluation::Permitted { backoff_delay, .. } => RecoveryAction::Retry {
                    delay_ms: backoff_delay.as_millis() as u64,
                },
                BudgetEvaluation::NonRetryable { reason } => {
                    RecoveryAction::AbortMission { reason }
                }
                BudgetEvaluation::Exhausted { reason, .. } => {
                    if reason.contains("Mission recovery ceiling") {
                        RecoveryAction::AbortMission { reason }
                    } else {
                        RecoveryAction::Replan {
                            reason: format!(
                                "Recovery budget exhausted, escalating to replan: {reason}"
                            ),
                        }
                    }
                }
                BudgetEvaluation::RepeatedIdentical { reason } => {
                    RecoveryAction::Escalate { reason }
                }
            }
        };

        if let Some(ref pool) = self.pool {
            let record = RecoveryAttemptRecord {
                mission_id: req.mission_id,
                task_id: req.task_id,
                failure_class: req.failure_class,
                strategy: match &action {
                    RecoveryAction::Repair { proposal, .. } => {
                        format!("repair [fp:{}]", compute_mutation_fingerprint(proposal))
                    }
                    RecoveryAction::Replan { reason } => format!("replan: {reason}"),
                    RecoveryAction::Rollback { reason } => format!("rollback: {reason}"),
                    RecoveryAction::Escalate { reason } => format!("escalate: {reason}"),
                    _ => "layered_recovery".to_string(),
                },
                attempt_number: task_retries + 1,
                budget_consumed: mission_retries + 1,
                remaining_class_budget: req
                    .failure_class
                    .default_retry_limit()
                    .saturating_sub(req.retry_count + 1),
                remaining_overall_budget: self
                    .budget_tracker
                    .mission_retry_ceiling
                    .saturating_sub(mission_retries + 1),
                backoff_delay_ms: match &action {
                    RecoveryAction::Retry { delay_ms } => *delay_ms,
                    _ => 0,
                },
                action_taken: match &action {
                    RecoveryAction::Retry { .. } => "retry".to_string(),
                    RecoveryAction::Replan { .. } => "replan".to_string(),
                    RecoveryAction::SkipTask => "skip".to_string(),
                    RecoveryAction::AbortMission { .. } => "abort".to_string(),
                    RecoveryAction::Repair { .. } => "repair".to_string(),
                    RecoveryAction::Rollback { .. } => "rollback".to_string(),
                    RecoveryAction::Escalate { .. } => "escalate".to_string(),
                },
                result: "evaluated".to_string(),
                mutation_fingerprint: None,
                semantic_signature: None,
            };
            let _ = self.budget_tracker.record_attempt(pool, record).await;
        }

        Ok(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{MissionId, TaskId};

    #[tokio::test]
    async fn test_determine_recovery_retries_compiler_error_without_structured_fix() {
        // The heuristic classifier diagnoses the failure class but cannot
        // synthesize file mutations, so no Repair proposal exists and the
        // engine honestly retries within budget instead of applying a
        // fabricated repair.
        let engine = ProductionRecoveryEngine::default();
        let mission_id = MissionId::new();
        let task_id = TaskId::new();

        let req = RecoveryStrategyRequest::new(
            mission_id,
            task_id,
            FailureClassification::Compilation,
            0,
        )
        .with_error_message("error[E0432]: unresolved import `m31a::util`\n --> src/lib.rs:2:5");

        let action = engine
            .determine_recovery(req)
            .await
            .expect("determine recovery");
        assert!(
            matches!(action, RecoveryAction::Retry { .. }),
            "Expected honest Retry without structured fix, got {:?}",
            action
        );
    }

    #[tokio::test]
    async fn test_determine_recovery_replans_on_budget_exhaustion() {
        let engine = ProductionRecoveryEngine::default();
        let mission_id = MissionId::new();
        let task_id = TaskId::new();

        let req = RecoveryStrategyRequest::new(
            mission_id,
            task_id,
            FailureClassification::Compilation,
            3,
        )
        .with_error_message("error[E0432]: unresolved import `m31a::util`\n --> src/lib.rs:2:5");

        // Exhausted retry budget escalates to replan rather than looping.
        let action = engine
            .determine_recovery(req)
            .await
            .expect("determine recovery");
        assert!(matches!(action, RecoveryAction::Replan { .. }));
    }

    #[tokio::test]
    async fn test_determine_recovery_aborts_non_retryable_failure() {
        let engine = ProductionRecoveryEngine::default();
        let mission_id = MissionId::new();
        let task_id = TaskId::new();

        let req =
            RecoveryStrategyRequest::new(mission_id, task_id, FailureClassification::Permission, 0)
                .with_error_message("Permission denied to write outside workspace");

        let action = engine
            .determine_recovery(req)
            .await
            .expect("determine recovery");
        assert!(matches!(action, RecoveryAction::AbortMission { .. }));
    }
}
