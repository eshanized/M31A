//! Non-bypassable 11-stage tool execution pipeline runner (TL-03, per D-09, D-12).

use std::sync::Arc;

use crate::agent::runner::{ActionRequest, ActionResult};
use crate::kernel::seams::policy::PolicyGate;
use crate::persistence::artifacts::fs_store::ArtifactStore;
use crate::pipeline::capture::OutputCaptureManager;
use crate::pipeline::dedup::{
    FenceVerdict, MutationDedupFence, fingerprint_mutation, is_fenced_mutating_tool, sha_hex,
};
use crate::pipeline::stages::*;
use crate::state::intake::AutonomyMode;
use crate::tools::definition::ToolExecutionContext;
use crate::tools::registry::ToolRegistry;

/// Non-bypassable pipeline runner executing side-effects under runtime policy authority (TL-03).
use sqlx::SqlitePool;

use crate::policy::approval::ApprovalCoordinator;

/// Non-bypassable 11-stage tool execution pipeline runner.
#[derive(Clone)]
pub struct ToolPipelineRunner {
    tool_registry: Arc<ToolRegistry>,
    capture_manager: Arc<OutputCaptureManager>,
    artifact_store: Option<Arc<dyn ArtifactStore>>,
    db_pool: Option<SqlitePool>,
    approval_coordinator: Option<Arc<ApprovalCoordinator>>,
}

impl ToolPipelineRunner {
    /// Create a new pipeline runner with registered tools.
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            tool_registry,
            capture_manager: Arc::new(OutputCaptureManager::default()),
            artifact_store: None,
            db_pool: None,
            approval_coordinator: None,
        }
    }

    /// Attach an SQLite pool for durable policy auditing and session grants.
    pub fn with_db_pool(mut self, pool: SqlitePool) -> Self {
        self.db_pool = Some(pool);
        self
    }

    /// Attach an approval coordinator for interactive operator approvals.
    pub fn with_approval_coordinator(mut self, coordinator: Arc<ApprovalCoordinator>) -> Self {
        self.approval_coordinator = Some(coordinator);
        self
    }

    /// Attach an artifact store for externalizing oversized outputs.
    pub fn with_artifact_store(mut self, store: Arc<dyn ArtifactStore>) -> Self {
        self.artifact_store = Some(store);
        self
    }

    /// Configure a custom output capture manager.
    pub fn with_capture_manager(mut self, capture_manager: Arc<OutputCaptureManager>) -> Self {
        self.capture_manager = capture_manager;
        self
    }

    /// Access the registered tool catalog.
    pub fn tool_registry(&self) -> &Arc<ToolRegistry> {
        &self.tool_registry
    }

    /// Access the output capture manager.
    pub fn capture_manager(&self) -> &Arc<OutputCaptureManager> {
        &self.capture_manager
    }

    /// Access the configured artifact store.
    pub fn artifact_store(&self) -> Option<&Arc<dyn ArtifactStore>> {
        self.artifact_store.as_ref()
    }

    /// Access the configured approval coordinator.
    pub fn approval_coordinator(&self) -> Option<&Arc<ApprovalCoordinator>> {
        self.approval_coordinator.as_ref()
    }

    /// Access the attached database pool.
    pub fn db_pool(&self) -> Option<&SqlitePool> {
        self.db_pool.as_ref()
    }

    /// Executes an action request through the 11 canonical type-state stages in strict linear sequence.
    ///
    /// Governed single-file tool writes (Lane A). This pipeline owns admission
    /// (capability → policy → approval → sandbox) and single-file execution.
    /// It is NOT the multi-file proposal authority: atomic multi-file change sets
    /// belong to Lane B (`ChangeAuthority::execute_change_proposal`), which adds
    /// reconciliation, surface reservation, diff self-review, and provenance
    /// on top of the same `FileSystemService` containment. Neither lane
    /// bypasses the other's invariants; see `change/authority.rs` lane docs.
    ///
    /// Non-bypassability guarantee:
    /// - Stages 1–8 validate resolution, decoding, schema, semantics, capabilities, resource bounds,
    ///   policy authorization, and approval without mutations.
    /// - Stage 9 (`ToolExecutionStage`) is the ONLY stage that executes physical side effects.
    /// - Denials, validations, and missing capabilities abort BEFORE Stage 9.
    /// - Stage 10 normalizes results and externalizes oversized outputs.
    /// - Stage 11 emits telemetry records and builds the final `ActionResult`.
    pub async fn execute_action(
        &self,
        action: &ActionRequest,
        context: &ToolExecutionContext,
        policy_gate: &dyn PolicyGate,
        autonomy_mode: AutonomyMode,
    ) -> ActionResult {
        // Stage 1: Tool Resolution (ActionRequest -> ResolvedToolState)
        let resolved = match ToolResolutionStage::execute(action, &self.tool_registry) {
            Ok(state) => state,
            Err(err) => {
                return TelemetryDurableRecordStage::record_failure(
                    action.id.clone(),
                    action.tool_name.clone(),
                    err,
                );
            }
        };

        // Stage 2: Argument Decoding (ResolvedToolState -> DecodedArgsState)
        let decoded = match ArgDecodingStage::execute(resolved) {
            Ok(state) => state,
            Err(err) => {
                return TelemetryDurableRecordStage::record_failure(
                    action.id.clone(),
                    action.tool_name.clone(),
                    err,
                );
            }
        };

        // Stage 3: Schema Validation (DecodedArgsState -> SchemaValidatedState)
        let schema_val = match SchemaValidationStage::execute(decoded) {
            Ok(state) => state,
            Err(err) => {
                return TelemetryDurableRecordStage::record_failure(
                    action.id.clone(),
                    action.tool_name.clone(),
                    err,
                );
            }
        };

        // Stage 4: Semantic Validation (SchemaValidatedState -> SemanticallyValidatedState)
        let semantic_val = match SemanticValidationStage::execute(schema_val) {
            Ok(state) => state,
            Err(err) => {
                return TelemetryDurableRecordStage::record_failure(
                    action.id.clone(),
                    action.tool_name.clone(),
                    err,
                );
            }
        };

        // Stage 5: Capability Check (SemanticallyValidatedState -> CapabilityAuthorizedState)
        let cap_auth = match CapabilityCheckStage::execute(semantic_val, context) {
            Ok(state) => state,
            Err(err) => {
                return TelemetryDurableRecordStage::record_failure(
                    action.id.clone(),
                    action.tool_name.clone(),
                    err,
                );
            }
        };

        // Stage 6: Resource Scope & Lease (CapabilityAuthorizedState -> ResourceScopedState)
        let res_scoped = match ResourceScopeStage::execute(cap_auth, context) {
            Ok(state) => state,
            Err(err) => {
                return TelemetryDurableRecordStage::record_failure(
                    action.id.clone(),
                    action.tool_name.clone(),
                    err,
                );
            }
        };

        // Stage 7: PolicyGate Evaluation (ResourceScopedState -> PolicyEvaluatedState)
        let policy_eval = match PolicyGateStage::execute_with_pool(
            res_scoped,
            context,
            policy_gate,
            self.db_pool.as_ref(),
        )
        .await
        {
            Ok(state) => state,
            Err(err) => {
                return TelemetryDurableRecordStage::record_failure(
                    action.id.clone(),
                    action.tool_name.clone(),
                    err,
                );
            }
        };

        // Stage 8: Approval Resolution (PolicyEvaluatedState -> ExecutionAuthorizedState)
        let exec_auth = match ApprovalResolutionStage::execute_async(
            policy_eval,
            context,
            autonomy_mode,
            self.approval_coordinator.as_deref(),
            self.db_pool.as_ref(),
        )
        .await
        {
            Ok(state) => state,
            Err(err) => {
                return TelemetryDurableRecordStage::record_failure(
                    action.id.clone(),
                    action.tool_name.clone(),
                    err,
                );
            }
        };

        // Stage 8.5: Durable deduplication fence.
        // For idempotent file-mutating tools, an explicit claim is acquired before execution.
        // A recorded prior success suppresses duplicate side effects.
        // Concurrent mutations are rejected.
        // Failures to acquire claim fail closed.
        let fence_claim: Option<(MutationDedupFence, String)> = if is_fenced_mutating_tool(
            &action.tool_name,
        ) {
            match (self.db_pool.clone(), context.task_id) {
                (Some(pool), Some(task_id)) => {
                    let fence = MutationDedupFence::new(pool);
                    let fp = fingerprint_mutation(&task_id, &action.tool_name, &action.parameters);
                    match fence
                        .acquire_claim(&task_id, &action.tool_name, &fp, None)
                        .await
                    {
                        Ok(FenceVerdict::DuplicateSuppressed { .. }) => {
                            return ActionResult {
                                action_id: action.id.clone(),
                                success: true,
                                output: format!(
                                    "duplicate suppressed: identical '{}' mutation already committed for this task (fingerprint {}); no side effect repeated",
                                    action.tool_name, fp
                                ),
                                error: None,
                            };
                        }
                        Ok(FenceVerdict::ConcurrentInProgress) => {
                            return ActionResult {
                                action_id: action.id.clone(),
                                success: false,
                                output: String::new(),
                                error: Some(format!(
                                    "duplicate mutation rejected: identical '{}' mutation is currently in progress for task {}",
                                    action.tool_name, task_id
                                )),
                            };
                        }
                        Ok(FenceVerdict::Indeterminate { details }) => {
                            return ActionResult {
                                action_id: action.id.clone(),
                                success: false,
                                output: String::new(),
                                error: Some(format!(
                                    "mutation rejected: identical '{}' mutation previously ended in indeterminate state: {}",
                                    action.tool_name, details
                                )),
                            };
                        }
                        Ok(FenceVerdict::Proceed) => Some((fence, fp)),
                        Err(err) => {
                            return ActionResult {
                                action_id: action.id.clone(),
                                success: false,
                                output: String::new(),
                                error: Some(format!(
                                    "failed to acquire mutation fence claim for '{}': {err}",
                                    action.tool_name
                                )),
                            };
                        }
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        // Stage 9: Tool Execution (ExecutionAuthorizedState -> RawExecutionState)
        // [PHYSICAL SIDE-EFFECTS STRICTLY CONFINED TO THIS STAGE]
        let raw_exec = ToolExecutionStage::execute(exec_auth, context).await;

        // Stage 10: Capture & Normalization (RawExecutionState -> NormalizedResultState)
        let normalized = CaptureNormalizationStage::execute(
            raw_exec,
            &self.capture_manager,
            self.artifact_store.as_ref(),
        )
        .await;

        // Stage 10.5: record the fence outcome. Success arms future suppression;
        // failure explicitly permits retry. If durable recording fails, marks indeterminate
        // and returns an actionable error.
        if let (Some((fence, fp)), Some(task_id)) = (fence_claim, context.task_id) {
            if let Err(err) = fence
                .commit_outcome(
                    &task_id,
                    &action.tool_name,
                    &fp,
                    normalized.success,
                    Some(&sha_hex(normalized.output.as_bytes())),
                    None,
                )
                .await
            {
                tracing::error!(
                    task_id = %task_id,
                    tool_name = %action.tool_name,
                    error = %err,
                    "Failed to durably commit tool mutation fence outcome; state is indeterminate"
                );
                return ActionResult {
                    action_id: action.id.clone(),
                    success: false,
                    output: normalized.output,
                    error: Some(format!(
                        "Mutation executed but durable outcome recording failed: {err}. Outcome is indeterminate."
                    )),
                };
            }
        }

        // Stage 11: Telemetry & Durable Record (NormalizedResultState -> ActionResult)
        TelemetryDurableRecordStage::execute(normalized)
    }
}
