//! Dedicated integration tests for `m31a-shell` (src/pty.rs).
//!
//! Validates:
//! - POSIX pseudo-terminal lifecycle: master/slave allocation, window resizing, FD closure
//! - Real one-off command execution through `PtyRunner`
//! - Exit code propagation on failures
//! - Real execution timeout enforcement and SIGINT interrupt signal generation

use m31a_shell::pty::{PtyRunner, PtySession, PtySize};
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_posix_pty_session_allocation_and_resizing() {
    let size = PtySize { rows: 25, cols: 90 };
    let mut session = PtySession::open(size).expect("Allocate POSIX PTY session");

    assert!(session.master_fd >= 0);
    assert!(session.slave_fd.is_some());
    assert!(session.slave_path.starts_with("/dev/pts"));

    // Resize window
    let new_size = PtySize {
        rows: 40,
        cols: 120,
    };
    session.set_size(new_size).expect("Resize PTY window");

    // Close slave explicitly
    session.close_slave();
    assert!(session.slave_fd.is_none());
}

#[tokio::test]
async fn test_pty_runner_executes_real_command() {
    let temp = tempdir().unwrap();
    let env_vars = vec![("TEST_PTY_ENV".to_string(), "m31a_active".to_string())];

    let output = PtyRunner::run_command(
        "echo \"var=$TEST_PTY_ENV\"",
        temp.path(),
        &env_vars,
        Duration::from_secs(5),
    )
    .await
    .expect("Execute PTY command");

    assert_eq!(output.exit_code, 0);
    assert!(!output.timed_out);
    assert!(output.stdout.contains("var=m31a_active"));
}

#[tokio::test]
async fn test_pty_runner_non_zero_exit_code() {
    let temp = tempdir().unwrap();

    let output =
        PtyRunner::run_command("sh -c 'exit 42'", temp.path(), &[], Duration::from_secs(5))
            .await
            .expect("Execute failing command");

    assert_eq!(output.exit_code, 42);
    assert!(!output.timed_out);
}

#[tokio::test]
async fn test_pty_runner_timeout_and_interrupt() {
    let temp = tempdir().unwrap();

    // Command that sleeps for 10 seconds, but timeout is 100ms
    let output = PtyRunner::run_command("sleep 10", temp.path(), &[], Duration::from_millis(150))
        .await
        .expect("Command execution should complete with timeout status");

    assert!(output.timed_out, "Command must be marked as timed out");
    assert_eq!(
        output.exit_code, 130,
        "SIGINT exit code expected on timeout"
    );
}
