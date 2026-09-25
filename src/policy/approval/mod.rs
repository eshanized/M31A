//! Interactive approval workflow, concurrent coordinator, and persistent session grants (POL-01, D-09, D-10, D-11, D-12).

pub mod adapter;
pub mod channel;
pub mod coordinator;
pub mod explanation;
pub mod grant;

pub use adapter::ProductionEscalationChannel;
pub use channel::{ApprovalChannel, ApprovalError};
pub use coordinator::ApprovalCoordinator;
pub use explanation::{ApprovalExplanationPacket, format_explanation_packet};
pub use grant::{PolicyGrant, PolicyGrantStore};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AgentId, ApprovalRequestId, MissionId, TaskId, ToolCallId};
use crate::tools::risk::RiskClass;

/// Lifecycle states for an approval request (D-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRequestState {
    Pending,
    Approved,
    Denied,
    Expired,
    Cancelled,
    Superseded,
    Failed,
}

impl ApprovalRequestState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Denied => "denied",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
            Self::Superseded => "superseded",
            Self::Failed => "failed",
        }
    }
}

/// Scope of authority conferred by an operator resolution (D-09, D-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalResolutionScope {
    Once,
    Task,
    Mission,
    Session,
    BoundedPolicy,
}

impl ApprovalResolutionScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::Task => "task",
            Self::Mission => "mission",
            Self::Session => "session",
            Self::BoundedPolicy => "bounded_policy",
        }
    }
}

/// Concrete operator decision action returned from approval channel (D-09, D-12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalAction {
    AllowOnce,
    AllowForTask,
    AllowForMission,
    AllowForSession,
    AllowUnderBoundedPolicy,
    Deny { reason: String },
    DenyAndCancelTask { reason: String },
}

impl ApprovalAction {
    pub fn is_allowed(&self) -> bool {
        matches!(
            self,
            Self::AllowOnce
                | Self::AllowForTask
                | Self::AllowForMission
                | Self::AllowForSession
                | Self::AllowUnderBoundedPolicy
        )
    }

    pub fn resolution_scope(&self) -> Option<ApprovalResolutionScope> {
        match self {
            Self::AllowOnce => Some(ApprovalResolutionScope::Once),
            Self::AllowForTask => Some(ApprovalResolutionScope::Task),
            Self::AllowForMission => Some(ApprovalResolutionScope::Mission),
            Self::AllowForSession => Some(ApprovalResolutionScope::Session),
            Self::AllowUnderBoundedPolicy => Some(ApprovalResolutionScope::BoundedPolicy),
            Self::Deny { .. } | Self::DenyAndCancelTask { .. } => None,
        }
    }
}

/// Complete domain entity representing an interactive approval request (D-09).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: ApprovalRequestId,
    pub mission_id: MissionId,
    pub task_id: Option<TaskId>,
    pub agent_id: Option<AgentId>,
    pub tool_call_id: ToolCallId,
    pub tool_or_capability: String,
    pub normalized_args: serde_json::Value,
    pub redacted_args: serde_json::Value,
    pub affected_resources: Vec<String>,
    pub risk_classification: RiskClass,
    pub matched_rule_id: Option<String>,
    pub policy_hash: String,
    pub reason: String,
    pub state: ApprovalRequestState,
    pub resolution_scope: Option<ApprovalResolutionScope>,
    pub resolved_by: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl ApprovalRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mission_id: MissionId,
        task_id: Option<TaskId>,
        agent_id: Option<AgentId>,
        tool_call_id: ToolCallId,
        tool_or_capability: impl Into<String>,
        normalized_args: serde_json::Value,
        affected_resources: Vec<String>,
        risk_classification: RiskClass,
        matched_rule_id: Option<String>,
        policy_hash: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let redacted_args = explanation::redact_sensitive_arguments(&normalized_args);
        Self {
            id: ApprovalRequestId::new(),
            mission_id,
            task_id,
            agent_id,
            tool_call_id,
            tool_or_capability: tool_or_capability.into(),
            normalized_args,
            redacted_args,
            affected_resources,
            risk_classification,
            matched_rule_id,
            policy_hash: policy_hash.into(),
            reason: reason.into(),
            state: ApprovalRequestState::Pending,
            resolution_scope: None,
            resolved_by: None,
            expires_at: None,
            created_at: Utc::now(),
            resolved_at: None,
        }
    }
}
