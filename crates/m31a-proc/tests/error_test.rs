//! Dedicated integration tests for `m31a-proc` (src/error.rs).
//!
//! Validates:
//! - ProcessError enum variants and display formatting
//! - Error trait implementations and clone ergonomics

use m31a_proc::error::ProcessError;

#[test]
fn test_process_error_display_formatting() {
    let err_not_found = ProcessError::NotFound("proc-123".to_string());
    assert_eq!(
        err_not_found.to_string(),
        "Process 'proc-123' was not found"
    );

    let err_spawn =
        ProcessError::SpawnFailed("proc-456".to_string(), "Executable not found".to_string());
    assert_eq!(
        err_spawn.to_string(),
        "Failed to spawn process 'proc-456': Executable not found"
    );

    let err_policy =
        ProcessError::PolicyViolation("rm -rf /".to_string(), "Dangerous command".to_string());
    assert_eq!(
        err_policy.to_string(),
        "Execution policy violation for process 'rm -rf /': Dangerous command"
    );

    let err_term = ProcessError::AlreadyTerminated("proc-789".to_string());
    assert_eq!(
        err_term.to_string(),
        "Process 'proc-789' is already terminated"
    );

    let err_term_fail =
        ProcessError::TerminationFailed("proc-999".to_string(), "Permission denied".to_string());
    assert_eq!(
        err_term_fail.to_string(),
        "Failed to terminate process 'proc-999': Permission denied"
    );

    let err_io = ProcessError::Io("proc-000".to_string(), "Broken pipe".to_string());
    assert_eq!(
        err_io.to_string(),
        "I/O error for process 'proc-000': Broken pipe"
    );
}

#[test]
fn test_process_error_clone_and_debug() {
    let err = ProcessError::NotFound("proc-abc".to_string());
    let cloned = err.clone();
    assert_eq!(format!("{:?}", err), format!("{:?}", cloned));
}
