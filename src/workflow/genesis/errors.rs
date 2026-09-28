//! Typed error model for Project Genesis discovery, research, and synthesis.

use crate::error::M31AError;
use crate::workflow::WorkflowError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GenesisError {
    #[error("invalid genesis request: {0}")]
    InvalidRequest(String),

    #[error("environment probe failed: {0}")]
    EnvironmentProbeFailed(String),

    #[error("discovery error: {0}")]
    Discovery(String),

    #[error("charter validation error: {0}")]
    CharterValidation(String),

    #[error("research decision error: {0}")]
    ResearchDecision(String),

    #[error("research execution failed: {0}")]
    ResearchExecution(String),

    #[error("synthesis error: {0}")]
    Synthesis(String),

    #[error("brownfield analysis failed: {0}")]
    BrownfieldAnalysis(String),

    #[error("genesis quality gate failed: {0}")]
    QualityGate(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("workflow error: {0}")]
    Workflow(#[from] WorkflowError),
}

impl From<GenesisError> for WorkflowError {
    fn from(err: GenesisError) -> Self {
        match err {
            GenesisError::Workflow(w) => w,
            other => WorkflowError::InvalidState(other.to_string()),
        }
    }
}

impl From<GenesisError> for M31AError {
    fn from(err: GenesisError) -> Self {
        M31AError::internal(err.to_string())
    }
}
