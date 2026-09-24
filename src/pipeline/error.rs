//! Strongly typed model-facing tool error taxonomy with bounded correction hints (TL-04, per D-10).

use serde::{Deserialize, Serialize};
use std::fmt;

/// 6 canonical failure categories for model-facing tool errors (TL-04, D-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolErrorCategory {
    Validation,
    ResourceNotFound,
    PreconditionFailed,
    PermissionDenied,
    ExecutionFailed,
    ResourceExhausted,
}

impl ToolErrorCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::ResourceNotFound => "resource_not_found",
            Self::PreconditionFailed => "precondition_failed",
            Self::PermissionDenied => "permission_denied",
            Self::ExecutionFailed => "execution_failed",
            Self::ResourceExhausted => "resource_exhausted",
        }
    }
}

/// Strongly typed failure contract with codes, diagnostics, and bounded hints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolError {
    pub category: ToolErrorCategory,
    pub code: String,
    pub message: String,
    pub provenance: Option<String>,
    pub retryable: bool,
    pub correction_hint: Option<String>,
}

impl ToolError {
    /// Create a new ToolError with bounded hint sanitization.
    pub fn new(
        category: ToolErrorCategory,
        code: impl Into<String>,
        message: impl Into<String>,
        provenance: Option<String>,
        retryable: bool,
        correction_hint: Option<String>,
    ) -> Self {
        let hint = correction_hint.map(|h| sanitize_hint(&h));
        Self {
            category,
            code: code.into(),
            message: message.into(),
            provenance,
            retryable,
            correction_hint: hint,
        }
    }

    /// Construct a validation error (malformed JSON, schema violation, invalid parameter bounds).
    pub fn validation(
        code: impl Into<String>,
        message: impl Into<String>,
        hint: Option<String>,
    ) -> Self {
        Self::new(
            ToolErrorCategory::Validation,
            code,
            message,
            None,
            false,
            hint,
        )
    }

    /// Construct a resource not found error (missing file, non-existent branch, unknown symbol).
    pub fn not_found(
        code: impl Into<String>,
        message: impl Into<String>,
        hint: Option<String>,
    ) -> Self {
        Self::new(
            ToolErrorCategory::ResourceNotFound,
            code,
            message,
            None,
            false,
            hint,
        )
    }

    /// Construct a precondition failed error (file modified externally, merge conflict, dirty worktree).
    pub fn precondition_failed(
        code: impl Into<String>,
        message: impl Into<String>,
        hint: Option<String>,
    ) -> Self {
        Self::new(
            ToolErrorCategory::PreconditionFailed,
            code,
            message,
            None,
            true,
            hint,
        )
    }

    /// Construct a permission denied error (forbidden by envelope, policy denial, out-of-scope resource).
    pub fn permission_denied(
        code: impl Into<String>,
        message: impl Into<String>,
        hint: Option<String>,
    ) -> Self {
        Self::new(
            ToolErrorCategory::PermissionDenied,
            code,
            message,
            None,
            false,
            hint,
        )
    }

    /// Construct an execution failed error (non-zero exit code, process crash, command failure).
    pub fn execution_failed(
        code: impl Into<String>,
        message: impl Into<String>,
        hint: Option<String>,
    ) -> Self {
        Self::new(
            ToolErrorCategory::ExecutionFailed,
            code,
            message,
            None,
            false,
            hint,
        )
    }

    /// Construct a resource exhausted error (memory limit, timeout exceeded, disk quota).
    pub fn resource_exhausted(
        code: impl Into<String>,
        message: impl Into<String>,
        hint: Option<String>,
    ) -> Self {
        Self::new(
            ToolErrorCategory::ResourceExhausted,
            code,
            message,
            None,
            false,
            hint,
        )
    }

    /// Attach provenance information (e.g. tool name, stage name).
    pub fn with_provenance(mut self, provenance: impl Into<String>) -> Self {
        self.provenance = Some(provenance.into());
        self
    }

    /// Set whether the error is safely retryable.
    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    /// Format the error cleanly for model consumption in `ActionResult.error` (TL-04, D-10).
    pub fn to_model_diagnostic(&self) -> String {
        let mut diag = format!(
            "[{}] {}: {}",
            self.category.as_str(),
            self.code,
            self.message
        );
        if let Some(ref prov) = self.provenance {
            diag.push_str(&format!(" (source: {})", prov));
        }
        if let Some(ref hint) = self.correction_hint {
            diag.push_str(&format!("\nHint: {}", hint));
        }
        diag
    }
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_model_diagnostic())
    }
}

impl std::error::Error for ToolError {}

/// Sanitize correction hints to guarantee they never suggest policy bypass, privilege escalation,
/// or forbidden capabilities (D-10, Law 9).
pub fn sanitize_hint(hint: &str) -> String {
    let lower = hint.to_lowercase();
    let forbidden_patterns = [
        "bypass",
        "sudo",
        "root",
        "chmod 777",
        "disable policy",
        "override policy",
        "escalate privileges",
        "escalate privilege",
        "ignore error",
        "turn off security",
        "run as admin",
        "disable security",
    ];

    for pattern in forbidden_patterns {
        if lower.contains(pattern) {
            return "Action not permitted. Please review requested arguments and execute strictly within authorized capabilities.".to_string();
        }
    }
    hint.to_string()
}

impl From<crate::tools::error::ToolError> for ToolError {
    fn from(err: crate::tools::error::ToolError) -> Self {
        match err {
            crate::tools::error::ToolError::Validation { message, hint } => {
                ToolError::validation("VALIDATION_ERROR", message, hint)
            }
            crate::tools::error::ToolError::ResourceNotFound { resource, hint } => {
                ToolError::not_found(
                    "RESOURCE_NOT_FOUND",
                    format!("Resource '{}' not found", resource),
                    hint,
                )
            }
            crate::tools::error::ToolError::PreconditionFailed { message, hint } => {
                ToolError::precondition_failed("PRECONDITION_FAILED", message, hint)
            }
            crate::tools::error::ToolError::PermissionDenied { message, hint } => {
                ToolError::permission_denied("PERMISSION_DENIED", message, hint)
            }
            crate::tools::error::ToolError::ExecutionFailed {
                message,
                exit_code,
                hint,
            } => {
                let code = if let Some(ec) = exit_code {
                    format!("EXIT_CODE_{}", ec)
                } else {
                    "EXECUTION_FAILED".to_string()
                };
                ToolError::execution_failed(code, message, hint)
            }
            crate::tools::error::ToolError::ResourceExhausted { message, hint } => {
                ToolError::resource_exhausted("RESOURCE_EXHAUSTED", message, hint)
            }
            crate::tools::error::ToolError::CapabilityUnavailable { family, hint } => {
                ToolError::permission_denied(
                    "CAPABILITY_UNAVAILABLE",
                    format!("Capability '{}' is currently unavailable", family),
                    hint,
                )
            }
        }
    }
}
