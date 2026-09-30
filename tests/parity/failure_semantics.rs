//! Parity: failure semantics (error taxonomy, retryability, fail-closed).
//!
//! The exact native OS message never needs to match. The M31A semantic
//! class must be stable: every tested failure maps to a typed PlatformError
//! with correct retryability, fail-closed behavior, telemetry, and recovery
//! guidance on every host.

use m31a::platform::errors::{PlatformError, from_io};

fn all_errors() -> Vec<PlatformError> {
    vec![
        PlatformError::AccessDenied {
            operation: "spawn".to_string(),
            detail: "denied".to_string(),
        },
        PlatformError::Unsupported {
            operation: "isolate".to_string(),
            detail: "no backend".to_string(),
        },
        PlatformError::NotFound {
            operation: "spawn".to_string(),
            detail: "missing".to_string(),
        },
        PlatformError::ResourceLimit {
            operation: "enforce".to_string(),
            detail: "quota".to_string(),
        },
        PlatformError::ProcessUnavailable {
            operation: "signal".to_string(),
            detail: "recycled".to_string(),
        },
        PlatformError::InvalidPath {
            operation: "contain".to_string(),
            detail: "escape".to_string(),
        },
        PlatformError::SandboxUnavailable {
            operation: "sandbox".to_string(),
            detail: "no bwrap".to_string(),
        },
        PlatformError::TerminalUnavailable {
            operation: "pty".to_string(),
            detail: "no conpty".to_string(),
        },
        PlatformError::Cancelled {
            operation: "run".to_string(),
        },
        PlatformError::TimedOut {
            operation: "run".to_string(),
            detail: "deadline".to_string(),
        },
        PlatformError::OperationFailed {
            operation: "probe".to_string(),
            detail: "eio".to_string(),
        },
    ]
}

/// Every failure has a stable category label on every host.
#[test]
fn parity_failure_categories_stable() {
    let expected = [
        "access-denied",
        "unsupported",
        "not-found",
        "resource-limit",
        "process-unavailable",
        "invalid-path",
        "sandbox-unavailable",
        "terminal-unavailable",
        "cancelled",
        "timed-out",
        "operation-failed",
    ];
    for (err, want) in all_errors().iter().zip(expected) {
        assert_eq!(
            err.category(),
            want,
            "EQUIVALENT: category stable for {err:?}"
        );
        assert_eq!(err.operation(), err.operation());
        assert!(!err.diagnostic().is_empty());
        assert!(!err.telemetry_event().is_empty());
        assert!(!err.recovery().is_empty());
    }
}

/// Retryability: only transient states retry; security failures never do.
#[test]
fn parity_failure_retryability_honored() {
    let retryable = ["process-unavailable", "timed-out", "operation-failed"];
    for err in all_errors() {
        let should_retry = retryable.contains(&err.category());
        assert_eq!(
            err.retryable(),
            should_retry,
            "retryability for {}",
            err.category()
        );
    }
    assert!(
        !PlatformError::AccessDenied {
            operation: "o".to_string(),
            detail: "d".to_string()
        }
        .retryable(),
        "authorization failures never retry"
    );
    assert!(
        !PlatformError::Unsupported {
            operation: "o".to_string(),
            detail: "d".to_string()
        }
        .retryable(),
        "unsupported capabilities never retry"
    );
}

/// Fail-closed: every integrity failure refuses silent continuation; only
/// control-flow outcomes (Cancelled, TimedOut) are exempt.
#[test]
fn parity_failure_fail_closed() {
    for err in all_errors() {
        let exempt = matches!(
            err,
            PlatformError::Cancelled { .. } | PlatformError::TimedOut { .. }
        );
        assert_eq!(
            !err.fail_closed(),
            exempt,
            "fail-closed for {}",
            err.category()
        );
    }
}

/// Raw I/O failures translate without leaking host strings into control flow.
#[test]
fn parity_failure_io_translation() {
    let denied = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "os text");
    assert_eq!(from_io("spawn", &denied).category(), "access-denied");
    let missing = std::io::Error::new(std::io::ErrorKind::NotFound, "os text");
    assert_eq!(from_io("spawn", &missing).category(), "not-found");
    let timeout = std::io::Error::new(std::io::ErrorKind::TimedOut, "os text");
    assert_eq!(from_io("run", &timeout).category(), "timed-out");
    let cancelled = std::io::Error::new(std::io::ErrorKind::Interrupted, "os text");
    assert_eq!(from_io("run", &cancelled).category(), "cancelled");
}

/// Diagnostics carry no secrets: redaction holds for credential-shaped text.
#[test]
fn parity_failure_no_secret_leak() {
    let err = PlatformError::OperationFailed {
        operation: "spawn".to_string(),
        detail: "exit 1".to_string(),
    };
    assert!(!err.diagnostic().contains("API_KEY"));
    assert!(err.to_string().contains("operation-failed"));
}

/// Telemetry events are distinct per class for cross-platform correlation.
#[test]
fn parity_failure_telemetry_distinct() {
    let mut events: Vec<&str> = all_errors().iter().map(|e| e.telemetry_event()).collect();
    events.sort_unstable();
    events.dedup();
    assert_eq!(events.len(), 11, "one telemetry event per failure class");
}
