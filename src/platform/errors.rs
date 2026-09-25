//! Stable native-operation error taxonomy.
//!
//! Raw operating-system failures vary across hosts. Callers match on this
//! type for control flow and keep the underlying message for diagnostics.
//! Secrets are never embedded in these values; display strings carry only
//! the operation, the category, and the non-sensitive detail.

use serde::{Deserialize, Serialize};

/// Stable outcome of a native platform operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformError {
    AccessDenied { operation: String, detail: String },
    Unsupported { operation: String, detail: String },
    NotFound { operation: String, detail: String },
    ResourceLimit { operation: String, detail: String },
    ProcessUnavailable { operation: String, detail: String },
    InvalidPath { operation: String, detail: String },
    SandboxUnavailable { operation: String, detail: String },
    TerminalUnavailable { operation: String, detail: String },
    Cancelled { operation: String },
    TimedOut { operation: String, detail: String },
    OperationFailed { operation: String, detail: String },
}

impl PlatformError {
    /// Machine-readable category label.
    pub fn category(&self) -> &'static str {
        match self {
            Self::AccessDenied { .. } => "access-denied",
            Self::Unsupported { .. } => "unsupported",
            Self::NotFound { .. } => "not-found",
            Self::ResourceLimit { .. } => "resource-limit",
            Self::ProcessUnavailable { .. } => "process-unavailable",
            Self::InvalidPath { .. } => "invalid-path",
            Self::SandboxUnavailable { .. } => "sandbox-unavailable",
            Self::TerminalUnavailable { .. } => "terminal-unavailable",
            Self::Cancelled { .. } => "cancelled",
            Self::TimedOut { .. } => "timed-out",
            Self::OperationFailed { .. } => "operation-failed",
        }
    }

    /// Operation name carried by the error.
    pub fn operation(&self) -> &str {
        match self {
            Self::AccessDenied { operation, .. }
            | Self::Unsupported { operation, .. }
            | Self::NotFound { operation, .. }
            | Self::ResourceLimit { operation, .. }
            | Self::ProcessUnavailable { operation, .. }
            | Self::InvalidPath { operation, .. }
            | Self::SandboxUnavailable { operation, .. }
            | Self::TerminalUnavailable { operation, .. }
            | Self::Cancelled { operation }
            | Self::TimedOut { operation, .. }
            | Self::OperationFailed { operation, .. } => operation,
        }
    }

    /// Whether retrying the same operation could plausibly succeed.
    pub fn retryable(&self) -> bool {
        match self {
            // Transient host states: process table races, momentary contention.
            Self::ProcessUnavailable { .. } | Self::OperationFailed { .. } => true,
            Self::TimedOut { .. } => true,
            // Definitive states: never retry without changing the request.
            Self::AccessDenied { .. }
            | Self::Unsupported { .. }
            | Self::NotFound { .. }
            | Self::ResourceLimit { .. }
            | Self::InvalidPath { .. }
            | Self::SandboxUnavailable { .. }
            | Self::TerminalUnavailable { .. }
            | Self::Cancelled { .. } => false,
        }
    }

    /// Whether the runtime must refuse dependent work when this error occurs.
    pub fn fail_closed(&self) -> bool {
        match self {
            // Cancellation and timeout are control-flow outcomes, not
            // integrity failures; callers translate them into typed results.
            Self::Cancelled { .. } | Self::TimedOut { .. } => false,
            // Every other native failure refuses silent continuation.
            _ => true,
        }
    }

    /// Short diagnostic line safe to surface to operators.
    pub fn diagnostic(&self) -> String {
        match self {
            Self::AccessDenied { operation, detail }
            | Self::Unsupported { operation, detail }
            | Self::NotFound { operation, detail }
            | Self::ResourceLimit { operation, detail }
            | Self::ProcessUnavailable { operation, detail }
            | Self::InvalidPath { operation, detail }
            | Self::SandboxUnavailable { operation, detail }
            | Self::TerminalUnavailable { operation, detail }
            | Self::OperationFailed { operation, detail } => {
                format!("{} [{}]: {}", operation, self.category(), detail)
            }
            Self::Cancelled { operation } => format!("{operation} [cancelled]"),
            Self::TimedOut { operation, detail } => {
                format!("{operation} [timed-out]: {detail}")
            }
        }
    }

    /// Telemetry event name for this failure class.
    pub fn telemetry_event(&self) -> &'static str {
        match self {
            Self::AccessDenied { .. } => "platform.access_denied",
            Self::Unsupported { .. } => "platform.unsupported",
            Self::NotFound { .. } => "platform.not_found",
            Self::ResourceLimit { .. } => "platform.resource_limit",
            Self::ProcessUnavailable { .. } => "platform.process_unavailable",
            Self::InvalidPath { .. } => "platform.invalid_path",
            Self::SandboxUnavailable { .. } => "platform.sandbox_unavailable",
            Self::TerminalUnavailable { .. } => "platform.terminal_unavailable",
            Self::Cancelled { .. } => "platform.cancelled",
            Self::TimedOut { .. } => "platform.timed_out",
            Self::OperationFailed { .. } => "platform.operation_failed",
        }
    }

    /// Recovery guidance for operators.
    pub fn recovery(&self) -> &'static str {
        match self {
            Self::AccessDenied { .. } => "check ownership and privilege; do not retry as-is",
            Self::Unsupported { .. } => {
                "select a backend that provides the capability or reduce requirements"
            }
            Self::NotFound { .. } => "verify the path, binary, or process identity still exists",
            Self::ResourceLimit { .. } => "raise the budget or reduce workload concurrency",
            Self::ProcessUnavailable { .. } => "re-resolve the process identity before retrying",
            Self::InvalidPath { .. } => "correct the path representation for this host",
            Self::SandboxUnavailable { .. } => {
                "install the native isolation backend or run contained-only work"
            }
            Self::TerminalUnavailable { .. } => "fall back to non-interactive execution",
            Self::Cancelled { .. } => "no recovery needed; cancellation is intentional",
            Self::TimedOut { .. } => "extend the deadline or reduce the workload",
            Self::OperationFailed { .. } => "inspect diagnostics; retry once, then escalate",
        }
    }
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.diagnostic())
    }
}

impl std::error::Error for PlatformError {}

/// Translate a raw I/O failure into the stable taxonomy without leaking
/// host-specific strings as control-flow signals.
pub fn from_io(operation: &str, err: &std::io::Error) -> PlatformError {
    use std::io::ErrorKind as K;
    match err.kind() {
        K::Unsupported => PlatformError::Unsupported {
            operation: operation.to_string(),
            detail: err.to_string(),
        },
        K::NotFound => PlatformError::NotFound {
            operation: operation.to_string(),
            detail: err.to_string(),
        },
        K::PermissionDenied => PlatformError::AccessDenied {
            operation: operation.to_string(),
            detail: "permission denied by host".to_string(),
        },
        K::InvalidInput | K::InvalidData | K::IsADirectory | K::NotADirectory => {
            PlatformError::InvalidPath {
                operation: operation.to_string(),
                detail: err.to_string(),
            }
        }
        K::TimedOut => PlatformError::TimedOut {
            operation: operation.to_string(),
            detail: err.to_string(),
        },
        K::Interrupted => PlatformError::Cancelled {
            operation: operation.to_string(),
        },
        _ => PlatformError::OperationFailed {
            operation: operation.to_string(),
            detail: err.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_is_not_retryable_and_fails_closed() {
        let e = PlatformError::Unsupported {
            operation: "tree-terminate".to_string(),
            detail: "no backend".to_string(),
        };
        assert!(!e.retryable());
        assert!(e.fail_closed());
        assert_eq!(e.category(), "unsupported");
    }

    #[test]
    fn cancelled_is_control_flow_not_integrity_failure() {
        let e = PlatformError::Cancelled {
            operation: "spawn".to_string(),
        };
        assert!(!e.retryable());
        assert!(!e.fail_closed());
    }

    #[test]
    fn io_translation_preserves_categories() {
        let denied = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "x");
        assert_eq!(from_io("op", &denied).category(), "access-denied");
        let missing = std::io::Error::new(std::io::ErrorKind::NotFound, "x");
        assert_eq!(from_io("op", &missing).category(), "not-found");
    }
}
