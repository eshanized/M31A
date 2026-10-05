use sqlx::SqlitePool;
use std::sync::Arc;

use crate::agent::runner::ActionRequest;
use crate::ids::ToolCallId;
use crate::kernel::seams::policy::{PolicyDecision, ResolvedAction, resolve_decision};
use crate::pipeline::error::ToolError;
use crate::pipeline::stages::policy_gate::PolicyEvaluatedState;
use crate::policy::approval::coordinator::DEFAULT_APPROVAL_TIMEOUT;
use crate::policy::approval::{
    ApprovalAction, ApprovalCoordinator, ApprovalRequest, ApprovalResolutionScope, PolicyGrant,
    PolicyGrantStore,
};
use crate::state::intake::AutonomyMode;
use crate::tools::definition::{AnyTool, ToolExecutionContext};
use crate::tools::risk::EffectiveRisk;

/// Typed state envelope proving execution authorization was fully resolved.
pub struct ExecutionAuthorizedState {
    pub action: ActionRequest,
    pub tool: Arc<dyn AnyTool>,
    pub decoded_args: serde_json::Value,
    pub effective_risk: EffectiveRisk,
}

/// Stage 8: Evaluates `PolicyDecision` against `AutonomyMode`.
/// In unattended or safe mode, converts `Ask` to `Deny` (AUT-03).
pub struct ApprovalResolutionStage;

impl ApprovalResolutionStage {
    pub fn execute(
        state: PolicyEvaluatedState,
        autonomy_mode: AutonomyMode,
    ) -> Result<ExecutionAuthorizedState, ToolError> {
        Self::execute_sync(state, autonomy_mode)
    }

    pub fn execute_sync(
        state: PolicyEvaluatedState,
        autonomy_mode: AutonomyMode,
    ) -> Result<ExecutionAuthorizedState, ToolError> {
        let decision = state.decision;
        let tool_id = state.tool.id();

        // In unattended or safe automated execution, there is no interactive operator channel
        let has_escalation_channel = false;
        let resolution = resolve_decision(
            autonomy_mode,
            decision,
            &format!("Policy check on tool '{}'", tool_id),
            has_escalation_channel,
        );

        match resolution {
            ResolvedAction::Proceed => Ok(ExecutionAuthorizedState {
                action: state.action,
                tool: state.tool,
                decoded_args: state.decoded_args,
                effective_risk: state.effective_risk,
            }),
            ResolvedAction::Deny { reason } => Err(ToolError::permission_denied(
                "POLICY_DENIED",
                format!("Tool '{}' execution denied by policy: {}", tool_id, reason),
                Some("Review policy rules or adjust autonomy mode permissions".to_string()),
            )
            .with_provenance("ApprovalResolutionStage")),
            ResolvedAction::PauseForApproval { reason } => {
                // Per AUT-03 & D-09: Unattended or non-interactive execution converts Ask to Deny.
                // The hint is machine-actionable: the denial is final for this task, so the
                // model must switch tools instead of retrying indefinitely. Deny semantics unchanged.
                Err(ToolError::permission_denied(
                    "INTERACTIVE_APPROVAL_UNAVAILABLE",
                    format!(
                        "Tool '{}' requires operator approval ({}), but running in non-interactive mode: converted to DENY",
                        tool_id, reason
                    ),
                    Some("This denial is final in non-interactive mode: retrying the same tool call will never succeed. Do not retry it. Use a different tool permitted for your role (e.g. read_file with an explicit path for inspection) or complete the task with evidence already gathered.".to_string()),
                )
                .with_provenance("ApprovalResolutionStage"))
            }
            ResolvedAction::Escalate { reason } => Err(ToolError::permission_denied(
                "POLICY_ESCALATION_REQUIRED",
                format!(
                    "Tool '{}' requires policy escalation ({}), but no escalation channel is available: converted to DENY",
                    tool_id, reason
                ),
                Some("This denial is final in non-interactive mode: retrying the same tool call will never succeed. Do not retry it. Use a different tool permitted for your role or complete the task with evidence already gathered.".to_string()),
            )
            .with_provenance("ApprovalResolutionStage")),
        }
    }

    /// Asynchronously resolve approval, dispatching to ApprovalCoordinator on PolicyDecision::Ask.
    pub async fn execute_async(
        state: PolicyEvaluatedState,
        context: &ToolExecutionContext,
        autonomy_mode: AutonomyMode,
        coordinator: Option<&ApprovalCoordinator>,
        pool: Option<&SqlitePool>,
    ) -> Result<ExecutionAuthorizedState, ToolError> {
        match state.decision {
            PolicyDecision::Allow => Ok(ExecutionAuthorizedState {
                action: state.action,
                tool: state.tool,
                decoded_args: state.decoded_args,
                effective_risk: state.effective_risk,
            }),
            PolicyDecision::Deny => {
                let reason = state
                    .decision_record
                    .as_ref()
                    .map(|r| r.explanation.clone())
                    .unwrap_or_else(|| "Denied by policy".to_string());
                Err(ToolError::permission_denied(
                    "POLICY_DENIED",
                    format!(
                        "Tool '{}' execution denied by policy: {}",
                        state.tool.id(),
                        reason
                    ),
                    Some("Review policy rules or adjust autonomy mode permissions".to_string()),
                )
                .with_provenance("ApprovalResolutionStage"))
            }
            PolicyDecision::Ask => {
                // Invariant (D-02, D-09, AUT-03): Unattended or Plan mode strictly converts ASK to DENY
                if autonomy_mode == AutonomyMode::Unattended || autonomy_mode == AutonomyMode::Plan
                {
                    return Err(ToolError::permission_denied(
                        "POLICY_DENIED",
                        "Unattended mode strictly converts unresolved ASK to DENY",
                        Some("Configure policy to allow or run in interactive mode".to_string()),
                    )
                    .with_provenance("ApprovalResolutionStage"));
                }

                if let Some(coord) = coordinator {
                    let mission_id = context.mission_id.unwrap_or_default();
                    let task_id = context.task_id;
                    let agent_id = context.agent_id;
                    let tool_call_id = ToolCallId::new();

                    let mut affected_resources = Vec::new();
                    if let Some(path_str) = state.decoded_args.get("path").and_then(|p| p.as_str())
                    {
                        affected_resources.push(path_str.to_string());
                    }
                    if let Some(path_str) = state
                        .decoded_args
                        .get("target_path")
                        .and_then(|p| p.as_str())
                    {
                        affected_resources.push(path_str.to_string());
                    }

                    let matched_rule_id = state
                        .decision_record
                        .as_ref()
                        .and_then(|r| r.matched_rule_id.clone());
                    let policy_hash = state
                        .decision_record
                        .as_ref()
                        .map(|r| r.policy_version_or_hash.clone())
                        .unwrap_or_default();

                    // Durable grants are a real runtime input: an eligible
                    // Ask may be resolved by a matching grant (mission, task,
                    // tool, resource, argument, expiry, revocation, policy
                    // hash, and provenance all verified inside the store)
                    // WITHOUT bothering the operator again. Deny is never
                    // grant-resolvable — this path only runs on Ask.
                    if let Some(pool_ref) = pool
                        && !policy_hash.is_empty()
                    {
                        let target_path = affected_resources.first().map(std::path::PathBuf::from);
                        match PolicyGrantStore::new()
                            .find_applicable_grants(
                                pool_ref,
                                mission_id,
                                task_id,
                                state.tool.id(),
                                target_path.as_deref(),
                                &context.workspace_root,
                                &state.decoded_args,
                                &policy_hash,
                            )
                            .await
                        {
                            Ok(grants) => {
                                if let Some(grant) = grants.into_iter().next() {
                                    // One-shot grants are consumed on use: a
                                    // replayed request must re-approve.
                                    if grant.scope_type == ApprovalResolutionScope::Once
                                        && !PolicyGrantStore::new()
                                            .consume_one_shot_grant(pool_ref, grant.id)
                                            .await
                                            .unwrap_or(false)
                                    {
                                        // Lost the consumption race (or the
                                        // grant vanished): fall through to
                                        // interactive approval rather than
                                        // allowing on a dead grant.
                                    } else {
                                        return Ok(ExecutionAuthorizedState {
                                            action: state.action,
                                            tool: state.tool,
                                            decoded_args: state.decoded_args,
                                            effective_risk: state.effective_risk,
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "durable grant lookup failed; falling through to interactive approval: {e}"
                                );
                            }
                        }
                    }
                    let reason = state
                        .decision_record
                        .as_ref()
                        .map(|r| r.explanation.clone())
                        .unwrap_or_else(|| {
                            format!("Tool '{}' requires operator approval", state.tool.id())
                        });

                    let req = ApprovalRequest::new(
                        mission_id,
                        task_id,
                        agent_id,
                        tool_call_id,
                        state.tool.id(),
                        state.decoded_args.clone(),
                        affected_resources.clone(),
                        state.effective_risk.effective_risk(),
                        matched_rule_id,
                        policy_hash.clone(),
                        reason,
                    );

                    let approval_request_id = req.id;
                    let action = coord
                        .request_approval(req, autonomy_mode, DEFAULT_APPROVAL_TIMEOUT)
                        .await
                        .map_err(|e| {
                            ToolError::permission_denied(
                                "APPROVAL_FAILED",
                                format!("Approval resolution failed: {}", e),
                                None,
                            )
                            .with_provenance("ApprovalResolutionStage")
                        })?;

                    if action.is_allowed() {
                        let persistent_scope = action
                            .resolution_scope()
                            .filter(|s| *s != ApprovalResolutionScope::Once);
                        if let (Some(scope), Some(pool_ref)) = (persistent_scope, pool) {
                            let resource_pattern = affected_resources
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "*".to_string());
                            // Provenance is mandatory: the grant names the
                            // approval request that created it, binds the
                            // approved arguments, and expires after 24h so a
                            // stale grant cannot authorize forever.
                            let grant = PolicyGrant::new(
                                mission_id,
                                if scope == ApprovalResolutionScope::Task {
                                    task_id
                                } else {
                                    None
                                },
                                scope,
                                state.tool.id(),
                                resource_pattern,
                                Some(state.decoded_args.clone()),
                                Some(approval_request_id),
                                policy_hash,
                                Some(chrono::Utc::now() + chrono::Duration::hours(24)),
                            );
                            if let Err(e) =
                                PolicyGrantStore::new().create_grant(pool_ref, &grant).await
                            {
                                // The operator DID allow this call, so the
                                // execution proceeds exactly once; only the
                                // durable grant is lost, and that loss is
                                // recorded instead of swallowed.
                                tracing::warn!(
                                    "persistent approval grant failed to persist; allowing this call once: {e}"
                                );
                            }
                        }

                        Ok(ExecutionAuthorizedState {
                            action: state.action,
                            tool: state.tool,
                            decoded_args: state.decoded_args,
                            effective_risk: state.effective_risk,
                        })
                    } else {
                        let deny_reason = match action {
                            ApprovalAction::Deny { reason } => reason,
                            ApprovalAction::DenyAndCancelTask { reason } => {
                                format!("Denied and cancelled: {}", reason)
                            }
                            _ => "Operator denied approval".to_string(),
                        };
                        Err(ToolError::permission_denied(
                            "POLICY_DENIED",
                            format!(
                                "Operator denied execution for tool '{}': {}",
                                state.tool.id(),
                                deny_reason
                            ),
                            None,
                        )
                        .with_provenance("ApprovalResolutionStage"))
                    }
                } else {
                    Self::execute_sync(state, autonomy_mode)
                }
            }
            PolicyDecision::Escalate => Err(ToolError::permission_denied(
                "POLICY_ESCALATION_REQUIRED",
                format!("Tool '{}' requires policy escalation", state.tool.id()),
                None,
            )
            .with_provenance("ApprovalResolutionStage")),
        }
    }
}
