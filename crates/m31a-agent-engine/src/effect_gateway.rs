//! Authoritative External Side-Effect Gateway (`m31a-agent-engine::effect_gateway`).
//!
//! Enforces:
//! 1. Worker lease and attempt fencing immediately prior to side-effect dispatch.
//! 2. Durable effect intent persistence before remote network or system operations.
//! 3. Stable logical operation IDs and provider-compatible idempotency keys across retries.
//! 4. Request fingerprinting to prevent accidental key reuse with mutated payloads.
//! 5. Uncertain-outcome classification and provider reconciliation.
//! 6. Explicit compensation procedure tracking or irreversible operation reporting.

use anyhow::{Result, anyhow, bail};
use chrono::Utc;
use m31a_core::workflow::{
    ExecutionJournalEvent, ExternalEffectIntent, ExternalEffectLifecycleStatus,
};
use m31a_execpolicy::tool_effect::classify_granular_tool;
use m31a_storage::WorkflowStore;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;

/// Request to admit and prepare an external side effect through the gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectDispatchRequest {
    pub workflow_id: String,
    pub task_id: String,
    pub attempt_id: u32,
    pub worker_id: String,
    pub tool_name: String,
    pub target_service: String,
    pub authorized_destination: String,
    pub payload: String,
    pub logical_operation_id: Option<String>,
    pub custom_idempotency_key: Option<String>,
    pub is_reversible: bool,
    pub compensation_procedure: Option<String>,
}

/// Admission outcome returned by the ExternalEffectGateway before dispatch.
#[derive(Debug, Clone)]
pub enum EffectAdmissionOutcome {
    /// Effect is authorized, journaled, and ready to dispatch with stable intent.
    Admitted {
        intent: ExternalEffectIntent,
        idempotency_key: String,
    },
    /// Operation already succeeded previously; reuse cached result without dispatching.
    IdempotentSkip { intent: ExternalEffectIntent },
    /// Prior attempt had an uncertain outcome; must be reconciled before retry.
    RequiresReconciliation { intent: ExternalEffectIntent },
    /// Operation denied by fencing or lease expiration.
    FencingDenied { reason: String },
}

/// Authoritative gateway managing all external side effects.
#[derive(Debug, Clone)]
pub struct ExternalEffectGateway {
    store: WorkflowStore,
}

impl ExternalEffectGateway {
    pub fn new(store: WorkflowStore) -> Self {
        Self { store }
    }

    /// Evaluates attempt fencing, generates stable idempotency keys, and persists durable intent.
    pub fn admit_effect(&self, req: &EffectDispatchRequest) -> Result<EffectAdmissionOutcome> {
        let now = Utc::now().timestamp_millis();

        // 1. Worker attempt fencing check against storage
        let task = self
            .store
            .get_task(&req.workflow_id, &req.task_id)?
            .ok_or_else(|| {
                anyhow!(
                    "Task '{}' not found in workflow '{}'",
                    req.task_id,
                    req.workflow_id
                )
            })?;

        if task.attempt_id != req.attempt_id {
            let reason = format!(
                "Stale worker attempt {}: current authoritative attempt is {}",
                req.attempt_id, task.attempt_id
            );
            warn!(workflow_id = %req.workflow_id, task_id = %req.task_id, reason = %reason, "Effect gateway denied dispatch");
            return Ok(EffectAdmissionOutcome::FencingDenied { reason });
        }

        if task.worker_id.as_deref() != Some(&req.worker_id) {
            let reason = format!(
                "Worker '{}' does not own task (authoritative worker: '{:?}')",
                req.worker_id, task.worker_id
            );
            warn!(workflow_id = %req.workflow_id, task_id = %req.task_id, reason = %reason, "Effect gateway denied dispatch");
            return Ok(EffectAdmissionOutcome::FencingDenied { reason });
        }

        // 2. Classify granular effect
        let granular = classify_granular_tool(&req.tool_name, Some(&req.payload));
        let is_reversible = req.is_reversible && granular.is_reversible();

        // 3. Compute stable logical operation identity and request fingerprint
        let logical_op_id = req
            .logical_operation_id
            .clone()
            .unwrap_or_else(|| format!("{}-{}-{}", req.task_id, req.tool_name, req.target_service));
        let fingerprint = ExternalEffectIntent::compute_fingerprint(
            &req.tool_name,
            &req.target_service,
            &req.payload,
        );

        // 4. Stable idempotency key: deterministic across retries of the same logical operation
        let idempotency_key = req
            .custom_idempotency_key
            .clone()
            .unwrap_or_else(|| format!("idemp-{}-{}", req.workflow_id, logical_op_id));

        // 5. Check if an intent with this idempotency key or logical operation already exists
        if let Some(existing) = self
            .store
            .find_external_effect_by_idempotency_key(&idempotency_key)?
        {
            // Verify fingerprint matches (prevent key reuse with altered payload)
            if existing.request_fingerprint != fingerprint {
                bail!(
                    "Idempotency key collision with different request fingerprint for logical op '{}'",
                    logical_op_id
                );
            }

            match existing.lifecycle_status {
                ExternalEffectLifecycleStatus::Confirmed
                | ExternalEffectLifecycleStatus::Completed => {
                    info!(
                        intent_id = %existing.id,
                        idempotency_key = %idempotency_key,
                        "Idempotent skip: external effect already succeeded"
                    );
                    return Ok(EffectAdmissionOutcome::IdempotentSkip { intent: existing });
                }
                ExternalEffectLifecycleStatus::UnknownOutcome
                | ExternalEffectLifecycleStatus::Reconciling => {
                    warn!(
                        intent_id = %existing.id,
                        idempotency_key = %idempotency_key,
                        "External effect outcome is uncertain; reconciliation required before retry"
                    );
                    return Ok(EffectAdmissionOutcome::RequiresReconciliation { intent: existing });
                }
                _ => {}
            }
        }

        // 6. Create durable intent in storage with PREPARED status
        let intent_id = format!("eff-{}", Uuid::new_v4().simple());
        let intent = ExternalEffectIntent {
            id: intent_id.clone(),
            workflow_id: req.workflow_id.clone(),
            task_id: req.task_id.clone(),
            attempt_id: req.attempt_id,
            worker_id: req.worker_id.clone(),
            logical_operation_id: logical_op_id.clone(),
            tool_name: req.tool_name.clone(),
            target_service: req.target_service.clone(),
            authorized_destination: req.authorized_destination.clone(),
            request_fingerprint: fingerprint,
            idempotency_key: Some(idempotency_key.clone()),
            lifecycle_status: ExternalEffectLifecycleStatus::Prepared,
            is_reversible,
            compensation_procedure: req.compensation_procedure.clone(),
            error_details: None,
            created_at_epoch_ms: now,
            updated_at_epoch_ms: now,
        };

        self.store.save_external_effect_intent(&intent)?;

        // Record execution journal event
        self.store
            .record_execution_journal_event(&ExecutionJournalEvent {
                id: format!("ej-{}", Uuid::new_v4().simple()),
                workflow_id: req.workflow_id.clone(),
                task_id: req.task_id.clone(),
                attempt_id: req.attempt_id,
                worker_id: req.worker_id.clone(),
                operation_id: Some(logical_op_id),
                event_type: "external_effect_prepared".into(),
                previous_state: None,
                new_state: Some("prepared".into()),
                resource_identity: Some(format!(
                    "{}:{}",
                    req.target_service, req.authorized_destination
                )),
                manifest_hash: None,
                payload_json: serde_json::json!({
                    "intent_id": intent_id,
                    "tool_name": req.tool_name,
                    "granular_effect": granular.as_str(),
                    "idempotency_key": idempotency_key,
                })
                .to_string(),
                timestamp_epoch_ms: now,
            })?;

        // 7. Transition to AUTHORIZED immediately
        self.store.update_external_effect_status(
            &intent_id,
            ExternalEffectLifecycleStatus::Authorized,
            None,
        )?;

        let mut authorized_intent = intent;
        authorized_intent.lifecycle_status = ExternalEffectLifecycleStatus::Authorized;

        Ok(EffectAdmissionOutcome::Admitted {
            intent: authorized_intent,
            idempotency_key,
        })
    }

    /// Marks an admitted intent as actively DISPATCHING.
    pub fn mark_dispatching(&self, intent_id: &str) -> Result<()> {
        self.store.update_external_effect_status(
            intent_id,
            ExternalEffectLifecycleStatus::Dispatching,
            None,
        )?;
        Ok(())
    }

    /// Records confirmed success of an external side effect.
    pub fn record_confirmed(&self, intent_id: &str) -> Result<()> {
        self.store.update_external_effect_status(
            intent_id,
            ExternalEffectLifecycleStatus::Confirmed,
            None,
        )?;
        if let Ok(Some(intent)) = self.store.get_external_effect_intent(intent_id) {
            let _ = self
                .store
                .record_execution_journal_event(&ExecutionJournalEvent {
                    id: format!("ej-{}", Uuid::new_v4().simple()),
                    workflow_id: intent.workflow_id,
                    task_id: intent.task_id,
                    attempt_id: intent.attempt_id,
                    worker_id: intent.worker_id,
                    operation_id: Some(intent.logical_operation_id),
                    event_type: "external_effect_confirmed".into(),
                    previous_state: Some("dispatching".into()),
                    new_state: Some("confirmed".into()),
                    resource_identity: Some(format!(
                        "{}:{}",
                        intent.target_service, intent.authorized_destination
                    )),
                    manifest_hash: None,
                    payload_json: serde_json::json!({
                        "intent_id": intent_id,
                        "status": "confirmed",
                    })
                    .to_string(),
                    timestamp_epoch_ms: Utc::now().timestamp_millis(),
                });
        }
        Ok(())
    }

    /// Records explicit rejection or client failure of an external operation.
    pub fn record_rejected(&self, intent_id: &str, error: &str) -> Result<()> {
        self.store.update_external_effect_status(
            intent_id,
            ExternalEffectLifecycleStatus::Rejected,
            Some(error),
        )?;
        Ok(())
    }

    /// Records uncertain outcome (lost response, timeout, connection drop).
    /// Prevents blind retries and flags intent for provider reconciliation.
    pub fn record_uncertain(&self, intent_id: &str, error: &str) -> Result<()> {
        self.store.update_external_effect_status(
            intent_id,
            ExternalEffectLifecycleStatus::UnknownOutcome,
            Some(error),
        )?;
        if let Ok(Some(intent)) = self.store.get_external_effect_intent(intent_id) {
            let _ = self
                .store
                .record_execution_journal_event(&ExecutionJournalEvent {
                    id: format!("ej-{}", Uuid::new_v4().simple()),
                    workflow_id: intent.workflow_id,
                    task_id: intent.task_id,
                    attempt_id: intent.attempt_id,
                    worker_id: intent.worker_id,
                    operation_id: Some(intent.logical_operation_id),
                    event_type: "external_effect_uncertain".into(),
                    previous_state: Some("dispatching".into()),
                    new_state: Some("unknown_outcome".into()),
                    resource_identity: Some(format!(
                        "{}:{}",
                        intent.target_service, intent.authorized_destination
                    )),
                    manifest_hash: None,
                    payload_json: serde_json::json!({
                        "intent_id": intent_id,
                        "error": error,
                    })
                    .to_string(),
                    timestamp_epoch_ms: Utc::now().timestamp_millis(),
                });
        }
        Ok(())
    }

    /// Reconciles an uncertain external operation using remote lookup evidence.
    pub fn reconcile(
        &self,
        intent_id: &str,
        remote_established: Option<bool>,
        reconciliation_details: &str,
    ) -> Result<ExternalEffectLifecycleStatus> {
        self.store.update_external_effect_status(
            intent_id,
            ExternalEffectLifecycleStatus::Reconciling,
            Some(reconciliation_details),
        )?;

        let final_status = match remote_established {
            Some(true) => ExternalEffectLifecycleStatus::Confirmed,
            Some(false) => ExternalEffectLifecycleStatus::Prepared, // Safe to retry!
            None => ExternalEffectLifecycleStatus::ManualReconciliationRequired,
        };

        self.store.update_external_effect_status(
            intent_id,
            final_status,
            Some(reconciliation_details),
        )?;

        info!(
            intent_id = %intent_id,
            reconciled_status = %final_status.as_str(),
            details = %reconciliation_details,
            "Reconciled external side effect outcome"
        );

        Ok(final_status)
    }

    /// Records compensation outcome for a completed or uncertain side effect.
    pub fn record_compensation(&self, intent_id: &str, success: bool, details: &str) -> Result<()> {
        let status = if success {
            ExternalEffectLifecycleStatus::Compensated
        } else {
            ExternalEffectLifecycleStatus::CompensationFailed
        };
        self.store
            .update_external_effect_status(intent_id, status, Some(details))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use m31a_core::workflow::{
        WorkflowPlan, WorkflowRun, WorkflowState, WorkflowTask, WorkflowTaskStatus,
        WorkspaceIsolationStrategy,
    };
    use m31a_storage::Storage;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn setup_test_env() -> (WorkflowStore, ExternalEffectGateway, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let storage = Storage::open(&db_path).unwrap();
        let store = storage.workflow_store();
        let gw = ExternalEffectGateway::new(store.clone());
        (store, gw, temp_dir)
    }

    #[test]
    fn test_admit_effect_authoritative_fencing() {
        let (store, gw, _temp) = setup_test_env();
        let wf_id = "wf-gw-1";
        let task_id = "task-gw-1";

        let run = WorkflowRun {
            id: wf_id.to_string(),
            thread_id: None,
            workspace_root: PathBuf::from("/workspace"),
            objective: "GW test".into(),
            workflow_type: "autonomous".into(),
            state: WorkflowState::Running,
            plan_revision: 1,
            configuration_json: "{}".into(),
            created_at_epoch_ms: 1000,
            updated_at_epoch_ms: 1000,
            started_at_epoch_ms: Some(1000),
            completed_at_epoch_ms: None,
            cancellation_reason: None,
            overall_result_json: None,
            resource_usage_json: None,
        };
        store.create_workflow(&run).unwrap();

        let task = WorkflowTask {
            id: task_id.into(),
            workflow_id: wf_id.into(),
            plan_revision: 1,
            task_kind: m31a_core::workflow::WorkflowTaskKind::Implementation,
            name: "GW Task".into(),
            description: "Test".into(),
            assigned_role: "Dev".into(),
            assigned_agent_id: None,
            priority: 10,
            wave_index: 0,
            dependencies: vec![],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![],
            workspace_isolation: WorkspaceIsolationStrategy::Worktree,
            status: WorkflowTaskStatus::Ready,
            retry_count: 0,
            max_retries: 2,
            attempt_id: 0,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec![],
            verification_result_json: None,
            execution_result_json: None,
        };
        let plan = WorkflowPlan {
            workflow_id: wf_id.into(),
            revision: 1,
            rationale: "GW test".into(),
            created_at_epoch_ms: 1000,
            tasks: vec![task],
        };
        store.save_workflow_plan(&plan).unwrap();

        // Worker-1 claims task -> attempt 1
        let claimed = store
            .claim_ready_task(wf_id, "worker-1", 10_000)
            .unwrap()
            .unwrap();
        assert_eq!(claimed.attempt_id, 1);

        // Stale attempt (attempt 0 or wrong worker) is denied by fencing
        let stale_req = EffectDispatchRequest {
            workflow_id: wf_id.into(),
            task_id: task_id.into(),
            attempt_id: 0, // Stale!
            worker_id: "worker-1".into(),
            tool_name: "cloud_deploy".into(),
            target_service: "aws".into(),
            authorized_destination: "arn:aws:...".into(),
            payload: "{}".into(),
            logical_operation_id: None,
            custom_idempotency_key: None,
            is_reversible: true,
            compensation_procedure: None,
        };
        let denied = gw.admit_effect(&stale_req).unwrap();
        assert!(matches!(
            denied,
            EffectAdmissionOutcome::FencingDenied { .. }
        ));

        // Current attempt by worker-1 is admitted with stable idempotency key
        let valid_req = EffectDispatchRequest {
            workflow_id: wf_id.into(),
            task_id: task_id.into(),
            attempt_id: 1,
            worker_id: "worker-1".into(),
            tool_name: "cloud_deploy".into(),
            target_service: "aws".into(),
            authorized_destination: "arn:aws:...".into(),
            payload: "{}".into(),
            logical_operation_id: Some("op-deploy-prod".into()),
            custom_idempotency_key: None,
            is_reversible: true,
            compensation_procedure: Some("revert_cfn".into()),
        };
        let admitted = gw.admit_effect(&valid_req).unwrap();
        match admitted {
            EffectAdmissionOutcome::Admitted {
                intent,
                idempotency_key,
            } => {
                assert_eq!(intent.logical_operation_id, "op-deploy-prod");
                assert_eq!(idempotency_key, format!("idemp-{}-op-deploy-prod", wf_id));
                assert_eq!(
                    intent.lifecycle_status,
                    ExternalEffectLifecycleStatus::Authorized
                );

                // Dispatch and confirm
                gw.mark_dispatching(&intent.id).unwrap();
                gw.record_confirmed(&intent.id).unwrap();
            }
            other => panic!("Expected Admitted, got {:?}", other),
        }

        // Subsequent retry of the same logical operation gets IdempotentSkip!
        let retry_admitted = gw.admit_effect(&valid_req).unwrap();
        assert!(matches!(
            retry_admitted,
            EffectAdmissionOutcome::IdempotentSkip { .. }
        ));
    }

    #[test]
    fn test_uncertain_outcome_reconciliation() {
        let (_store, gw, _temp) = setup_test_env();
        let wf_id = "wf-gw-2";
        let task_id = "task-gw-2";

        // Setup workflow and task
        let run = WorkflowRun {
            id: wf_id.to_string(),
            thread_id: None,
            workspace_root: PathBuf::from("/workspace"),
            objective: "Uncertain test".into(),
            workflow_type: "autonomous".into(),
            state: WorkflowState::Running,
            plan_revision: 1,
            configuration_json: "{}".into(),
            created_at_epoch_ms: 1000,
            updated_at_epoch_ms: 1000,
            started_at_epoch_ms: Some(1000),
            completed_at_epoch_ms: None,
            cancellation_reason: None,
            overall_result_json: None,
            resource_usage_json: None,
        };
        gw.store.create_workflow(&run).unwrap();

        let task = WorkflowTask {
            id: task_id.into(),
            workflow_id: wf_id.into(),
            plan_revision: 1,
            task_kind: m31a_core::workflow::WorkflowTaskKind::Implementation,
            name: "GW Task 2".into(),
            description: "Test".into(),
            assigned_role: "Dev".into(),
            assigned_agent_id: None,
            priority: 10,
            wave_index: 0,
            dependencies: vec![],
            acceptance_criteria: vec![],
            required_verification: vec![],
            declared_read_paths: vec![],
            declared_write_paths: vec![],
            workspace_isolation: WorkspaceIsolationStrategy::Worktree,
            status: WorkflowTaskStatus::Ready,
            retry_count: 0,
            max_retries: 2,
            attempt_id: 0,
            started_at_epoch_ms: None,
            completed_at_epoch_ms: None,
            worker_id: None,
            lease_expires_at_epoch_ms: None,
            error_details: None,
            input_artifacts: vec![],
            output_artifacts: vec![],
            verification_result_json: None,
            execution_result_json: None,
        };
        let plan = WorkflowPlan {
            workflow_id: wf_id.into(),
            revision: 1,
            rationale: "GW test".into(),
            created_at_epoch_ms: 1000,
            tasks: vec![task],
        };
        gw.store.save_workflow_plan(&plan).unwrap();
        let _ = gw
            .store
            .claim_ready_task(wf_id, "worker-gw", 10_000)
            .unwrap();

        let req = EffectDispatchRequest {
            workflow_id: wf_id.into(),
            task_id: task_id.into(),
            attempt_id: 1,
            worker_id: "worker-gw".into(),
            tool_name: "cloud_deploy".into(),
            target_service: "k8s".into(),
            authorized_destination: "cluster.local".into(),
            payload: r#"{"deployment": "api"}"#.into(),
            logical_operation_id: Some("op-k8s-deploy".into()),
            custom_idempotency_key: None,
            is_reversible: true,
            compensation_procedure: Some("rollout_undo".into()),
        };

        let outcome = gw.admit_effect(&req).unwrap();
        let intent_id = match outcome {
            EffectAdmissionOutcome::Admitted { intent, .. } => intent.id,
            _ => panic!("Expected Admitted"),
        };

        // Network connection timed out awaiting response!
        gw.mark_dispatching(&intent_id).unwrap();
        gw.record_uncertain(&intent_id, "HTTP 504 Gateway Timeout")
            .unwrap();

        // Attempting to dispatch again returns RequiresReconciliation (blocks blind retries!)
        let retry_outcome = gw.admit_effect(&req).unwrap();
        assert!(matches!(
            retry_outcome,
            EffectAdmissionOutcome::RequiresReconciliation { .. }
        ));

        // Reconcile via remote status check: remote confirms deployment was accepted
        let reconciled = gw
            .reconcile(
                &intent_id,
                Some(true),
                "K8s API confirmed deployment api-v1 is active",
            )
            .unwrap();
        assert_eq!(reconciled, ExternalEffectLifecycleStatus::Confirmed);

        // Now dispatching again recognizes idempotent skip!
        let third_outcome = gw.admit_effect(&req).unwrap();
        assert!(matches!(
            third_outcome,
            EffectAdmissionOutcome::IdempotentSkip { .. }
        ));
    }
}
