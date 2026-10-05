//! Stage 7: PolicyGate Evaluation Stage (TL-03, per D-09).

use std::sync::Arc;

use crate::agent::runner::ActionRequest;
use crate::kernel::seams::policy::{
    PolicyDecision, PolicyDecisionContract, PolicyEvaluationRequest, PolicyGate,
};
use crate::pipeline::error::ToolError;
use crate::pipeline::stages::resource_scope::ResourceScopedState;
use sqlx::SqlitePool;

use crate::policy::audit::DurablePolicyAuditor;
use crate::tools::definition::{AnyTool, ToolExecutionContext};
use crate::tools::risk::EffectiveRisk;

/// Typed state envelope proving PolicyGate evaluation completed.
pub struct PolicyEvaluatedState {
    pub action: ActionRequest,
    pub tool: Arc<dyn AnyTool>,
    pub decoded_args: serde_json::Value,
    pub effective_risk: EffectiveRisk,
    pub decision: PolicyDecision,
    pub decision_record: Option<PolicyDecisionContract>,
}

/// Stage 7: Constructs `PolicyEvaluationRequest` and queries `PolicyGate::evaluate_record()`.
pub struct PolicyGateStage;

impl PolicyGateStage {
    pub async fn execute(
        state: ResourceScopedState,
        context: &ToolExecutionContext,
        policy_gate: &dyn PolicyGate,
    ) -> Result<PolicyEvaluatedState, ToolError> {
        Self::execute_with_pool(state, context, policy_gate, None).await
    }

    pub async fn execute_with_pool(
        state: ResourceScopedState,
        context: &ToolExecutionContext,
        policy_gate: &dyn PolicyGate,
        pool: Option<&SqlitePool>,
    ) -> Result<PolicyEvaluatedState, ToolError> {
        let mission_id = context.mission_id.unwrap_or_default();
        let task_id = context.task_id.unwrap_or_default();
        let tool_id = state.tool.id().to_string();

        // Typed canonical request: role / mode / workspace flow as typed
        // values, never reconstructed from a string digest. Missing identity
        // fails closed inside `try_from_request`.
        let mut req = PolicyEvaluationRequest::new(mission_id, task_id, tool_id.clone())
            .with_workspace(context.workspace_root.clone())
            .with_arguments(state.decoded_args.clone())
            .with_effective_risk(format!("{:?}", state.effective_risk.effective_risk()));
        if let Some(role) = context.agent_role.clone() {
            req = req.with_role(role);
        }
        if let Some(mode) = context.autonomy_mode {
            req = req.with_autonomy_mode(mode);
        }
        if let Some(agent_id) = context.agent_id {
            req = req.with_agent_id(agent_id);
        }
        // Canonical target paths: explicit path-like arguments as typed paths.
        {
            let mut paths = Vec::new();
            if let Some(obj) = state.decoded_args.as_object() {
                for key in [
                    "path",
                    "target_path",
                    "file_path",
                    "destination_path",
                    "destination",
                    "file",
                    "dir",
                    "cwd",
                    "target",
                ] {
                    if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
                        paths.push(std::path::PathBuf::from(s));
                    }
                }
                if let Some(arr) = obj.get("paths").and_then(|v| v.as_array()) {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            paths.push(std::path::PathBuf::from(s));
                        }
                    }
                }
            }
            if !paths.is_empty() {
                req = req.with_target_paths(paths);
            }
        }

        let (decision, decision_record) = policy_gate.evaluate_record(req).await.map_err(|e| {
            ToolError::permission_denied(
                "POLICY_GATE_EVALUATION_FAILED",
                format!("PolicyGate evaluation failed: {}", e),
                None,
            )
            .with_provenance("PolicyGateStage")
        })?;

        // Commit durable pre-execution policy audit record if SQLite pool is provided (POL-05)
        if let (Some(pool), Some(record)) = (pool, &decision_record) {
            let resource_scope = state
                .decoded_args
                .get("path")
                .or_else(|| state.decoded_args.get("target_path"))
                .and_then(|p| p.as_str())
                .unwrap_or("*");

            let _ = DurablePolicyAuditor::new()
                .record_decision(
                    pool,
                    mission_id,
                    context.task_id,
                    None,
                    Some(&state.action.id),
                    state.tool.id(),
                    record,
                    &state.decoded_args,
                    resource_scope,
                )
                .await;
        }

        Ok(PolicyEvaluatedState {
            action: state.action,
            tool: state.tool,
            decoded_args: state.decoded_args,
            effective_risk: state.effective_risk,
            decision,
            decision_record,
        })
    }
}
