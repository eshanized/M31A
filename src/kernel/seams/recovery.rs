use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::{MissionId, TaskId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FailureClassificationRequest {
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub error_message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FailureClassification {
    #[serde(alias = "upstream_service")]
    Transient,
    Timeout,
    #[serde(alias = "sandbox_escaping")]
    Permission,
    #[serde(alias = "policy_violation")]
    Policy,
    Environment,
    Dependency,
    Compilation,
    Test,
    #[serde(alias = "schema_violation")]
    ToolContract,
    #[serde(alias = "model_refusal")]
    Model,
    Context,
    #[serde(alias = "resource_exhaustion")]
    ResourceLimit,
    #[serde(alias = "stale_state")]
    RepositoryState,
    #[serde(alias = "loop_detected", alias = "deadlock")]
    Architecture,
    Unknown,

    // Backward compatibility variants
    Permanent,
    FatalViolation,
    Configuration,
    /// Corrupt persisted failure-class value. Never produced
    /// by live classification; only by fail-closed durable reads. Always
    /// non-retryable with zero retries.
    Corrupt,
}

impl FailureClassification {
    pub const UPSTREAM_SERVICE: Self = Self::Transient;
    pub const MODEL_REFUSAL: Self = Self::Model;
    pub const SCHEMA_VIOLATION: Self = Self::ToolContract;
    pub const POLICY_VIOLATION: Self = Self::Policy;
    pub const SANDBOX_ESCAPING: Self = Self::Permission;
    pub const RESOURCE_EXHAUSTION: Self = Self::ResourceLimit;
    pub const STALE_STATE: Self = Self::RepositoryState;
    pub const LOOP_DETECTED: Self = Self::Architecture;
    pub const DEADLOCK: Self = Self::Architecture;

    /// Whether this failure class can be retried under normal budgets.
    pub fn is_retryable(&self) -> bool {
        !matches!(
            self,
            Self::Permission
                | Self::Policy
                | Self::ResourceLimit
                | Self::Permanent
                | Self::FatalViolation
                | Self::Corrupt
        )
    }

    /// Default class retry limit under D-06.
    pub fn default_retry_limit(&self) -> usize {
        match self {
            Self::Permission | Self::Policy | Self::ResourceLimit | Self::FatalViolation => 0,
            Self::Corrupt => 0,
            Self::Timeout
            | Self::Environment
            | Self::Context
            | Self::RepositoryState
            | Self::Architecture
            | Self::Unknown
            | Self::Permanent
            | Self::Configuration => 1,
            Self::Dependency | Self::ToolContract | Self::Model => 2,
            Self::Transient | Self::Compilation | Self::Test => 3,
        }
    }
}

impl std::fmt::Display for FailureClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transient => write!(f, "transient"),
            Self::Timeout => write!(f, "timeout"),
            Self::Permission => write!(f, "permission"),
            Self::Policy => write!(f, "policy"),
            Self::Environment => write!(f, "environment"),
            Self::Dependency => write!(f, "dependency"),
            Self::Compilation => write!(f, "compilation"),
            Self::Test => write!(f, "test"),
            Self::ToolContract => write!(f, "tool_contract"),
            Self::Model => write!(f, "model"),
            Self::Context => write!(f, "context"),
            Self::ResourceLimit => write!(f, "resource_limit"),
            Self::RepositoryState => write!(f, "repository_state"),
            Self::Architecture => write!(f, "architecture"),
            Self::Unknown => write!(f, "unknown"),
            Self::Permanent => write!(f, "permanent"),
            Self::FatalViolation => write!(f, "fatal_violation"),
            Self::Configuration => write!(f, "configuration"),
            Self::Corrupt => write!(f, "corrupt"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryStrategyRequest {
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub failure_class: FailureClassification,
    pub retry_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl RecoveryStrategyRequest {
    pub fn new(
        mission_id: MissionId,
        task_id: TaskId,
        failure_class: FailureClassification,
        retry_count: usize,
    ) -> Self {
        Self {
            mission_id,
            task_id,
            failure_class,
            retry_count,
            error_message: None,
        }
    }

    pub fn with_error_message(mut self, msg: impl Into<String>) -> Self {
        self.error_message = Some(msg.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAction {
    Retry {
        delay_ms: u64,
    },
    Replan {
        reason: String,
    },
    SkipTask,
    AbortMission {
        reason: String,
    },
    Repair {
        proposal: Box<crate::kernel::change::ChangeProposal>,
        reason: String,
    },
    Rollback {
        reason: String,
    },
    Escalate {
        reason: String,
    },
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RecoveryError {
    #[error("recovery error: {0}")]
    Failed(String),
}

#[async_trait]
pub trait RecoveryEngine: Send + Sync {
    async fn classify_failure(
        &self,
        req: FailureClassificationRequest,
    ) -> Result<FailureClassification, RecoveryError>;

    async fn determine_recovery(
        &self,
        req: RecoveryStrategyRequest,
    ) -> Result<RecoveryAction, RecoveryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_dto_serde() {
        let action = RecoveryAction::Retry { delay_ms: 500 };
        let serialized = serde_json::to_string(&action).unwrap();
        let deserialized: RecoveryAction = serde_json::from_str(&serialized).unwrap();
        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_failure_classification_serde_all_15_classes() {
        let classes = [
            FailureClassification::Transient,
            FailureClassification::Timeout,
            FailureClassification::Permission,
            FailureClassification::Policy,
            FailureClassification::Environment,
            FailureClassification::Dependency,
            FailureClassification::Compilation,
            FailureClassification::Test,
            FailureClassification::ToolContract,
            FailureClassification::Model,
            FailureClassification::Context,
            FailureClassification::ResourceLimit,
            FailureClassification::RepositoryState,
            FailureClassification::Architecture,
            FailureClassification::Unknown,
        ];

        for class in classes {
            let serialized = serde_json::to_string(&class).unwrap();
            let deserialized: FailureClassification = serde_json::from_str(&serialized).unwrap();
            assert_eq!(class, deserialized);
        }

        // Test serde aliases
        let de: FailureClassification = serde_json::from_str("\"upstream_service\"").unwrap();
        assert_eq!(de, FailureClassification::Transient);
        let de: FailureClassification = serde_json::from_str("\"policy_violation\"").unwrap();
        assert_eq!(de, FailureClassification::Policy);
    }

    #[test]
    fn test_non_retryable_classes_have_zero_budget() {
        assert_eq!(FailureClassification::Permission.default_retry_limit(), 0);
        assert_eq!(FailureClassification::Policy.default_retry_limit(), 0);
        assert!(!FailureClassification::Permission.is_retryable());
        assert!(!FailureClassification::Policy.is_retryable());
    }
}
