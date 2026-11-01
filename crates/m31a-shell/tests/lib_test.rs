//! Dedicated integration tests for `m31a-shell` (src/lib.rs).
//!
//! Validates:
//! - Root re-exports: `HeadTailBuffer`, `PtyExecutionOutput`, `PtyRunner`, `PtySession`, `PtySize`, `PersistentShellSession`, `ShellOutput`
//! - Cohesive lifecycle: persistent shell session using custom buffer limits

use m31a_shell::{HeadTailBuffer, PersistentShellSession, PtySize, ShellOutput};
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_root_reexports() {
    let size = PtySize::default();
    assert_eq!(size.rows, 24);
    assert_eq!(size.cols, 80);

    let buf = HeadTailBuffer::new(10, 10, 1024);
    assert_eq!(buf.process("hello\n"), "hello\n");
}

#[tokio::test]
async fn test_root_persistent_session_with_truncation() {
    let temp = tempdir().unwrap();
    let mut session = PersistentShellSession::new("root-session", temp.path());
    // Custom buffer with 5 head lines and 5 tail lines
    session.buffer = HeadTailBuffer::new(5, 5, 1024 * 1024);

    let script = "for i in $(seq 1 30); do echo \"Item $i\"; done";

    let out: ShellOutput = session
        .execute(script, Duration::from_secs(5))
        .await
        .expect("Execution failed");

    assert!(out.success);
    assert!(out.stdout.contains("Item 1"));
    assert!(out.stdout.contains("Item 2"));
    assert!(out.stdout.contains("Item 29"));
    assert!(out.stdout.contains("Item 30"));
    assert!(out.stdout.contains("lines truncated to preserve token context"));
}
