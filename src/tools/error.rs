//! Strongly typed tool execution error contract (TL-04, D-10).

use crate::capability::error::CapabilityError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Canonical tool error classifications (TL-04, D-10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, thiserror::Error)]
#[serde(tag = "category", content = "details")]
pub enum ToolError {
    #[error("Validation error: {message}")]
    Validation {
        message: String,
        hint: Option<String>,
    },
    #[error("Resource not found: {resource}")]
    ResourceNotFound {
        resource: String,
        hint: Option<String>,
    },
    #[error("Precondition failed: {message}")]
    PreconditionFailed {
        message: String,
        hint: Option<String>,
    },
    #[error("Permission denied: {message}")]
    PermissionDenied {
        message: String,
        hint: Option<String>,
    },
    #[error("Execution failed: {message}")]
    ExecutionFailed {
        message: String,
        exit_code: Option<i32>,
        hint: Option<String>,
    },
    #[error("Resource exhausted: {message}")]
    ResourceExhausted {
        message: String,
        hint: Option<String>,
    },
    #[error("Capability unavailable: {family}")]
    CapabilityUnavailable {
        family: String,
        hint: Option<String>,
    },
}

impl ToolError {
    pub fn validation(message: impl Into<String>, hint: Option<String>) -> Self {
        Self::Validation {
            message: message.into(),
            hint,
        }
    }

    pub fn resource_not_found(resource: impl Into<String>, hint: Option<String>) -> Self {
        Self::ResourceNotFound {
            resource: resource.into(),
            hint,
        }
    }

    pub fn precondition_failed(message: impl Into<String>, hint: Option<String>) -> Self {
        Self::PreconditionFailed {
            message: message.into(),
            hint,
        }
    }

    pub fn permission_denied(message: impl Into<String>, hint: Option<String>) -> Self {
        Self::PermissionDenied {
            message: message.into(),
            hint,
        }
    }

    pub fn execution_failed(
        message: impl Into<String>,
        exit_code: Option<i32>,
        hint: Option<String>,
    ) -> Self {
        Self::ExecutionFailed {
            message: message.into(),
            exit_code,
            hint,
        }
    }

    pub fn capability_unavailable(family: impl Into<String>, hint: Option<String>) -> Self {
        Self::CapabilityUnavailable {
            family: family.into(),
            hint,
        }
    }
}

impl From<CapabilityError> for ToolError {
    fn from(err: CapabilityError) -> Self {
        match err {
            CapabilityError::PermissionDenied(msg) => ToolError::PermissionDenied {
                message: msg,
                hint: None,
            },
            CapabilityError::PathOutOfBounds { path, workspace } => ToolError::PermissionDenied {
                message: format!("path '{path}' is out of workspace bounds '{workspace}'"),
                hint: Some("Keep operations within the workspace root".to_string()),
            },
            CapabilityError::NotFound(res) => ToolError::ResourceNotFound {
                resource: res,
                hint: None,
            },
            CapabilityError::Unavailable(msg) => ToolError::CapabilityUnavailable {
                family: msg,
                hint: None,
            },
            CapabilityError::ExecutionFailed { exit_code, message } => ToolError::ExecutionFailed {
                message,
                exit_code,
                hint: None,
            },
            CapabilityError::Timeout(msg) => ToolError::ResourceExhausted {
                message: msg,
                hint: Some("Operation timed out".to_string()),
            },
            other => ToolError::ExecutionFailed {
                message: other.to_string(),
                exit_code: None,
                hint: None,
            },
        }
    }
}
