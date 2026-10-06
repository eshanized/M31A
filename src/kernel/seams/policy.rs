use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

use crate::ids::{AgentId, MissionId, TaskId};
use crate::state_machine::AutonomyMode;
use crate::state_machine::agent::AgentRole;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    #[serde(alias = "allow", alias = "Allow", alias = "ALLOW")]
    Allow,
    #[serde(alias = "deny", alias = "Deny", alias = "DENY")]
    Deny,
    #[serde(alias = "ask", alias = "Ask", alias = "ASK")]
    Ask,
    #[serde(alias = "escalate", alias = "Escalate", alias = "ESCALATE")]
    Escalate,
}

/// Typed immutable policy evaluation request.
///
/// Security-critical identity (role, autonomy mode, workspace, target
/// resources) is carried as strongly typed values consumed directly by the
/// policy engine. Nothing is reconstructed from a free-form string digest:
/// a missing role, mode, or workspace fails closed at evaluation time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicyEvaluationRequest {
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub agent_id: Option<AgentId>,
    pub agent_role: Option<AgentRole>,
    pub autonomy_mode: Option<AutonomyMode>,
    pub tool_or_action: String,
    pub workspace_root: Option<PathBuf>,
    pub canonical_target_paths: Vec<std::path::PathBuf>,
    pub canonical_arguments: serde_json::Value,
    pub effective_risk: Option<String>,
    pub authorization_id: Option<String>,
    pub policy_hash: Option<String>,
    pub request_id: String,
}

impl PolicyEvaluationRequest {
    pub fn new(mission_id: MissionId, task_id: TaskId, tool_or_action: impl Into<String>) -> Self {
        Self {
            mission_id,
            task_id,
            agent_id: None,
            agent_role: None,
            autonomy_mode: None,
            tool_or_action: tool_or_action.into(),
            workspace_root: None,
            canonical_target_paths: Vec::new(),
            canonical_arguments: serde_json::Value::Null,
            effective_risk: None,
            authorization_id: None,
            policy_hash: None,
            request_id: uuid::Uuid::now_v7().to_string(),
        }
    }

    pub fn with_agent_id(mut self, id: AgentId) -> Self {
        self.agent_id = Some(id);
        self
    }

    pub fn with_role(mut self, role: AgentRole) -> Self {
        self.agent_role = Some(role);
        self
    }

    pub fn with_autonomy_mode(mut self, mode: AutonomyMode) -> Self {
        self.autonomy_mode = Some(mode);
        self
    }

    pub fn with_workspace(mut self, ws: PathBuf) -> Self {
        self.workspace_root = Some(ws);
        self
    }

    pub fn with_target_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.canonical_target_paths = paths;
        self
    }

    pub fn with_arguments(mut self, args: serde_json::Value) -> Self {
        self.canonical_arguments = args;
        self
    }

    pub fn with_effective_risk(mut self, risk: impl Into<String>) -> Self {
        self.effective_risk = Some(risk.into());
        self
    }

    pub fn with_authorization(mut self, id: impl Into<String>) -> Self {
        self.authorization_id = Some(id.into());
        self
    }

    pub fn with_policy_hash(mut self, hash: impl Into<String>) -> Self {
        self.policy_hash = Some(hash.into());
        self
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PolicyError {
    #[error("policy gate evaluation failed: {0}")]
    EvaluationFailed(String),
    #[error("missing security-critical policy attribute: {0}")]
    MissingSecurityAttribute(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedAction {
    Proceed,
    Deny { reason: String },
    PauseForApproval { reason: String },
    Escalate { reason: String },
}

/// Resolves policy decision based on the active autonomy mode (Pattern 5, D-13, AUT-02, AUT-03, SEC-06).
pub fn resolve_decision(
    mode: AutonomyMode,
    decision: PolicyDecision,
    reason: &str,
    has_escalation_channel: bool,
) -> ResolvedAction {
    match (mode, decision) {
        (_, PolicyDecision::Allow) => ResolvedAction::Proceed,
        (_, PolicyDecision::Deny) => ResolvedAction::Deny {
            reason: reason.to_string(),
        },
        (AutonomyMode::Plan, PolicyDecision::Ask) => ResolvedAction::Deny {
            reason: "Mutating actions not permitted in Plan mode".to_string(),
        },
        (AutonomyMode::Safe | AutonomyMode::Assisted, PolicyDecision::Ask) => {
            ResolvedAction::PauseForApproval {
                reason: reason.to_string(),
            }
        }
        (AutonomyMode::Autonomous, PolicyDecision::Ask) => {
            if has_escalation_channel {
                ResolvedAction::Escalate {
                    reason: reason.to_string(),
                }
            } else {
                ResolvedAction::Deny {
                    reason: "Non-interactive Autonomous mode denied unresolved ASK without escalation channel"
                        .to_string(),
                }
            }
        }
        // D-13 & SEC-06: Unattended NEVER converts ASK to ALLOW
        (AutonomyMode::Unattended, PolicyDecision::Ask) => {
            if has_escalation_channel {
                ResolvedAction::Escalate {
                    reason: reason.to_string(),
                }
            } else {
                ResolvedAction::Deny {
                    reason: "Unattended mode converts unresolved ASK to DENY".to_string(),
                }
            }
        }
        (_, PolicyDecision::Escalate) => ResolvedAction::Escalate {
            reason: reason.to_string(),
        },
    }
}

/// Authoritative minimal policy decision contract containing matched rule and audit provenance (POL-01, D-01).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyDecisionContract {
    pub decision: PolicyDecision,
    pub matched_rule_id: Option<String>,
    pub matched_layer: Option<String>,
    pub precedence_rank: Option<u32>,
    pub authority_source: String,
    pub explanation: String,
    pub policy_version_or_hash: String,
}

impl PolicyDecisionContract {
    pub fn new(
        decision: PolicyDecision,
        matched_rule_id: Option<String>,
        matched_layer: Option<String>,
        precedence_rank: Option<u32>,
        authority_source: impl Into<String>,
        explanation: impl Into<String>,
        policy_version_or_hash: impl Into<String>,
    ) -> Self {
        Self {
            decision,
            matched_rule_id,
            matched_layer,
            precedence_rank,
            authority_source: authority_source.into(),
            explanation: explanation.into(),
            policy_version_or_hash: policy_version_or_hash.into(),
        }
    }
}

#[async_trait]
pub trait PolicyGate: Send + Sync {
    async fn evaluate(&self, req: PolicyEvaluationRequest) -> Result<PolicyDecision, PolicyError>;

    async fn evaluate_record(
        &self,
        req: PolicyEvaluationRequest,
    ) -> Result<(PolicyDecision, Option<PolicyDecisionContract>), PolicyError> {
        let decision = self.evaluate(req).await?;
        Ok((decision, None))
    }

    /// Active policy hash when the gate is backed by a compiled policy.
    /// `None` for test doubles that carry no compiled rule set.
    fn policy_hash(&self) -> Option<String> {
        None
    }
}

/// Explicit test-only allow-all policy gate. Never use on production
/// paths: missing authoritative policy must fail closed, never silently
/// substitute allow-all.
#[derive(Debug, Clone, Default)]
pub struct AllowAllTestPolicy;

#[async_trait]
impl PolicyGate for AllowAllTestPolicy {
    async fn evaluate(&self, _req: PolicyEvaluationRequest) -> Result<PolicyDecision, PolicyError> {
        Ok(PolicyDecision::Allow)
    }
}

/// Legacy alias kept only for existing test contracts.
/// New code must use `AllowAllTestPolicy` explicitly (test-only) or a
/// compiled `EffectivePolicy` on production paths.
pub type DefaultPolicyGate = AllowAllTestPolicy;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_resolution_unattended_never_allows_ask() {
        // Without escalation channel -> Deny
        let action1 = resolve_decision(
            AutonomyMode::Unattended,
            PolicyDecision::Ask,
            "Needs human approval",
            false,
        );
        assert_eq!(
            action1,
            ResolvedAction::Deny {
                reason: "Unattended mode converts unresolved ASK to DENY".to_string()
            }
        );

        // With escalation channel -> Escalate
        let action2 = resolve_decision(
            AutonomyMode::Unattended,
            PolicyDecision::Ask,
            "Needs human approval",
            true,
        );
        assert_eq!(
            action2,
            ResolvedAction::Escalate {
                reason: "Needs human approval".to_string()
            }
        );

        // Invariant: Unattended on Ask NEVER evaluates to Proceed
        assert_ne!(action1, ResolvedAction::Proceed);
        assert_ne!(action2, ResolvedAction::Proceed);
    }

    #[test]
    fn test_policy_resolution_all_five_modes() {
        let modes = [
            AutonomyMode::Safe,
            AutonomyMode::Plan,
            AutonomyMode::Assisted,
            AutonomyMode::Autonomous,
            AutonomyMode::Unattended,
        ];

        for mode in modes {
            // Allow always proceeds
            assert_eq!(
                resolve_decision(mode, PolicyDecision::Allow, "ok", true),
                ResolvedAction::Proceed
            );
            // Deny always denies
            assert_eq!(
                resolve_decision(mode, PolicyDecision::Deny, "denied", true),
                ResolvedAction::Deny {
                    reason: "denied".to_string()
                }
            );
            // Escalate always escalates
            assert_eq!(
                resolve_decision(mode, PolicyDecision::Escalate, "esc", true),
                ResolvedAction::Escalate {
                    reason: "esc".to_string()
                }
            );
        }

        // Plan mode always denies Ask
        assert_eq!(
            resolve_decision(AutonomyMode::Plan, PolicyDecision::Ask, "ask", true),
            ResolvedAction::Deny {
                reason: "Mutating actions not permitted in Plan mode".to_string()
            }
        );

        // Safe and Assisted always PauseForApproval on Ask
        assert_eq!(
            resolve_decision(AutonomyMode::Safe, PolicyDecision::Ask, "ask", true),
            ResolvedAction::PauseForApproval {
                reason: "ask".to_string()
            }
        );
        assert_eq!(
            resolve_decision(AutonomyMode::Assisted, PolicyDecision::Ask, "ask", true),
            ResolvedAction::PauseForApproval {
                reason: "ask".to_string()
            }
        );

        // Autonomous escalates when channel present, denies when not
        assert_eq!(
            resolve_decision(AutonomyMode::Autonomous, PolicyDecision::Ask, "ask", true),
            ResolvedAction::Escalate {
                reason: "ask".to_string()
            }
        );
        assert_eq!(
            resolve_decision(AutonomyMode::Autonomous, PolicyDecision::Ask, "ask", false),
            ResolvedAction::Deny {
                reason: "Non-interactive Autonomous mode denied unresolved ASK without escalation channel".to_string()
            }
        );
    }

    #[test]
    fn test_policy_request_and_decision_serde_roundtrip() {
        let req = PolicyEvaluationRequest::new(MissionId::new(), TaskId::new(), "git_push")
            .with_role(crate::state_machine::agent::AgentRole::implementer())
            .with_autonomy_mode(AutonomyMode::Autonomous)
            .with_workspace(std::path::PathBuf::from("/tmp/ws"))
            .with_target_paths(vec![std::path::PathBuf::from("/tmp/ws/file.rs")])
            .with_arguments(serde_json::json!({"path": "/tmp/ws/file.rs"}));

        let json = serde_json::to_string(&req).unwrap();
        let parsed: PolicyEvaluationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, parsed);

        let decisions = [
            (PolicyDecision::Allow, "\"allow\""),
            (PolicyDecision::Deny, "\"deny\""),
            (PolicyDecision::Ask, "\"ask\""),
            (PolicyDecision::Escalate, "\"escalate\""),
        ];
        for (dec, expected_str) in decisions {
            let s = serde_json::to_string(&dec).unwrap();
            assert_eq!(s, expected_str);
            let parsed_dec: PolicyDecision = serde_json::from_str(&s).unwrap();
            assert_eq!(dec, parsed_dec);
        }
    }
}
