use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::{MissionId, TaskId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskVerificationRequest {
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub execution_output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VerificationOutcome {
    Passed,
    Failed { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CompletionGateOutcome {
    Satisfied,
    Deficient { violations: Vec<String> },
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum VerificationError {
    #[error("verification failed: {0}")]
    Failed(String),
}

#[async_trait]
pub trait VerificationEngine: Send + Sync {
    async fn verify_task(
        &self,
        req: TaskVerificationRequest,
    ) -> Result<VerificationOutcome, VerificationError>;

    async fn verify_completion_gate(
        &self,
        mission_id: MissionId,
    ) -> Result<CompletionGateOutcome, VerificationError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verifier_dto_serde() {
        let outcome = VerificationOutcome::Failed {
            reason: "assertion failed".to_string(),
        };
        let serialized = serde_json::to_string(&outcome).unwrap();
        let deserialized: VerificationOutcome = serde_json::from_str(&serialized).unwrap();
        assert_eq!(outcome, deserialized);
    }
}
