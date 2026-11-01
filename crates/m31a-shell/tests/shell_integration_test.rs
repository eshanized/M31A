//! Integration tests for stateful persistent shell sessions.
//!
//! Exercises environment variable persistence, working directory changes, and output truncation.

use m31a_shell::{HeadTailBuffer, PersistentShellSession, PtyRunner, PtySession, PtySize};
use std::fs;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_pty_session_open_and_resize() {
    let pty = PtySession::open(PtySize { rows: 24, cols: 80 }).expect("open pty");
    assert!(pty.master_fd >= 0);
    assert!(pty.slave_path.exists());

    // Resize PTY
    let res = pty.set_size(PtySize {
        rows: 50,
        cols: 120,
    });
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_pty_runner_real_command_execution() {
    let tmp = tempdir().unwrap();
    let res = PtyRunner::run_command(
        "echo 'PTY_LIVE_TEST_OUTPUT'",
        tmp.path(),
        &[],
        Duration::from_secs(5),
    )
    .await
    .expect("pty run");

    assert_eq!(res.exit_code, 0);
    assert!(!res.timed_out);
    assert!(res.stdout.contains("PTY_LIVE_TEST_OUTPUT"));
}

#[tokio::test]
async fn test_pty_runner_nonzero_exit_code() {
    let tmp = tempdir().unwrap();
    let res = PtyRunner::run_command("exit 42", tmp.path(), &[], Duration::from_secs(5))
        .await
        .expect("pty run");

    assert_eq!(res.exit_code, 42);
    assert!(!res.timed_out);
}

#[tokio::test]
async fn test_pty_runner_timeout_kill() {
    let tmp = tempdir().unwrap();
    let res = PtyRunner::run_command("sleep 5", tmp.path(), &[], Duration::from_millis(200))
        .await
        .expect("pty run");

    assert!(res.timed_out);
}

#[tokio::test]
async fn test_persistent_shell_multi_turn_cd_navigation() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let sub = root.join("nested_workspace");
    fs::create_dir_all(&sub).unwrap();

    let mut session = PersistentShellSession::new("test_sess", &root);
    assert_eq!(session.cwd, root);

    // Turn 1: cd into nested_workspace
    let out1 = session
        .execute("cd nested_workspace", Duration::from_secs(5))
        .await
        .unwrap();
    assert!(out1.success);
    assert_eq!(session.cwd, sub);

    // Turn 2: write a file in current dir and verify pwd
    let out2 = session
        .execute("pwd && touch inside.txt", Duration::from_secs(5))
        .await
        .unwrap();
    assert!(out2.success);
    assert!(out2.stdout.contains("nested_workspace"));
    assert!(sub.join("inside.txt").exists());
}

#[test]
fn test_head_tail_buffer_byte_budget_truncation() {
    let buffer = HeadTailBuffer::new(100, 100, 50); // very small byte budget
    let raw = "a".repeat(200);
    let processed = buffer.process(&raw);

    assert!(processed.contains("output truncated by byte budget"));
    assert!(processed.len() <= 120);
}

#[tokio::test]
async fn test_persistent_shell_environment_variable_persistence() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();

    let mut session = PersistentShellSession::new("env_test_sess", &root);

    // Turn 1: export variable
    let out1 = session
        .execute(
            "export SESSION_KEY='M31A_ALIVE_TOKEN'",
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert!(out1.success);

    // Turn 2: read exported variable in the persistent process
    let out2 = session
        .execute("echo \"KEY=$SESSION_KEY\"", Duration::from_secs(5))
        .await
        .unwrap();
    assert!(out2.success);
    assert!(out2.stdout.contains("KEY=M31A_ALIVE_TOKEN"));
}

#[tokio::test]
async fn test_persistent_shell_sandboxed_execution() {
    let tmp = tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();

    let mut session = PersistentShellSession::new("sandbox_sess", &root)
        .with_sandbox(m31a_core::SandboxPolicy::WorkspaceWrite);

    let out = session
        .execute("echo 'SANDBOX_OK' && pwd", Duration::from_secs(5))
        .await
        .unwrap();

    assert!(out.success);
    assert!(out.stdout.contains("SANDBOX_OK"));
}
