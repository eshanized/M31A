//! Dedicated integration tests for `m31a-shell` (src/session.rs).
//!
//! Validates:
//! - Stateful environment persistence across multiple commands
//! - Automatic working directory synchronization upon `cd`
//! - Success and failure exit status reporting in `ShellOutput`
//! - Real long-lived child process reuse across agent turns

use m31a_shell::session::PersistentShellSession;
use std::fs;
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test]
async fn test_persistent_session_environment_and_cwd_tracking() {
    let temp = tempdir().unwrap();
    let initial_path = temp.path().to_path_buf();

    // Create a subfolder to test `cd`
    let subfolder = initial_path.join("build_output");
    fs::create_dir_all(&subfolder).unwrap();

    let mut session = PersistentShellSession::new("test-session-1", &initial_path);
    session.set_env("PERSISTENT_KEY", "M31A_SHELL_VALUE");

    // 1. Verify custom environment variable
    let out1 = session
        .execute("echo \"KEY=$PERSISTENT_KEY\"", Duration::from_secs(5))
        .await
        .expect("Command 1 failed");

    assert_eq!(out1.exit_code, 0);
    assert!(out1.success);
    assert!(out1.stdout.contains("KEY=M31A_SHELL_VALUE"));
    assert_eq!(out1.cwd, initial_path);

    // 2. Change directory and create a marker file
    let out2 = session
        .execute(
            "cd build_output && touch marker.txt",
            Duration::from_secs(5),
        )
        .await
        .expect("Command 2 failed");

    assert_eq!(out2.exit_code, 0);
    assert!(out2.success);
    // Session cwd must update to the canonical subfolder
    let canonical_subfolder = subfolder.canonicalize().unwrap();
    let canonical_session_cwd = session.cwd.canonicalize().unwrap();
    assert_eq!(canonical_session_cwd, canonical_subfolder);

    // 3. Verify file exists in subfolder
    assert!(subfolder.join("marker.txt").exists());

    // 4. Subsequent command should run from the new working directory
    let out3 = session
        .execute("pwd", Duration::from_secs(5))
        .await
        .expect("Command 3 failed");

    assert_eq!(out3.exit_code, 0);
    assert!(out3.stdout.contains("build_output"));
}

#[tokio::test]
async fn test_persistent_session_failure_reporting() {
    let temp = tempdir().unwrap();
    let mut session = PersistentShellSession::new("test-session-err", temp.path());

    let out = session
        .execute(
            "sh -c 'echo \"err_msg\" >&2; exit 7'",
            Duration::from_secs(5),
        )
        .await
        .expect("Command execution should complete");

    assert_eq!(out.exit_code, 7);
    assert!(!out.success);
    assert!(!out.timed_out);
}
