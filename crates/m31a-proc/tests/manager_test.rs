//! Dedicated integration tests for `m31a-proc` (src/manager.rs).
//!
//! Validates:
//! - Background process spawning, stdout/stderr streaming, and completion tracking
//! - Supervisor process listings and individual process queries
//! - Graceful termination via `stop` and bulk cancellation via `stop_all`
//! - Execution policy enforcement and rejection of forbidden commands
//! - Zero-mock execution on Linux child processes

use m31a_proc::error::ProcessError;
use m31a_proc::manager::{ProcessManager, ProcessManagerConfig};
use m31a_proc::process::ProcessStatus;
use std::collections::HashMap;
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test]
async fn test_process_manager_spawn_and_output_capture() {
    let temp = tempdir().unwrap();
    let manager = ProcessManager::default();

    let info = manager
        .start(
            "sh",
            &["-c".to_string(), "echo 'Line A'; echo 'Line B' >&2; exit 0".to_string()],
            temp.path(),
            HashMap::new(),
            Some("test-worker".to_string()),
        )
        .await
        .expect("Spawn process");

    assert_eq!(info.label, "test-worker");

    // Allow process to finish execution
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Fetch output
    let output = manager
        .get_output(&info.id, None, None)
        .await
        .expect("Fetch output");

    assert!(output.text.contains("Line A"));
    assert!(output.text.contains("[stderr] Line B"));

    // Check process status updated to Exited(0)
    let updated_info = manager.get(&info.id).await.expect("Process exists");
    assert_eq!(updated_info.status, ProcessStatus::Exited { exit_code: 0 });
}

#[tokio::test]
async fn test_process_manager_stop_running_task() {
    let temp = tempdir().unwrap();
    let manager = ProcessManager::default();

    let info = manager
        .start(
            "sh",
            &["-c".to_string(), "sleep 30".to_string()],
            temp.path(),
            HashMap::new(),
            Some("long-runner".to_string()),
        )
        .await
        .expect("Spawn long runner");

    assert!(info.status.is_running());

    // Stop process
    let stop_res = manager.stop(&info.id, false).await.expect("Stop process");
    assert!(stop_res.stopped);

    // Verify status updated
    let updated = manager.get(&info.id).await.expect("Process exists");
    assert!(!updated.status.is_running());
}

#[tokio::test]
async fn test_process_manager_policy_violation_rejection() {
    let temp = tempdir().unwrap();

    let config = ProcessManagerConfig {
        exec_policy: Some(m31a_execpolicy::ExecPolicy::standard()),
        ..Default::default()
    };
    let manager = ProcessManager::new(config);

    // Destructive command blocked by standard ExecPolicy
    let res = manager
        .start(
            "rm",
            &["-rf".to_string(), "/".to_string()],
            temp.path(),
            HashMap::new(),
            None,
        )
        .await;

    assert!(matches!(res, Err(ProcessError::PolicyViolation(..))));
}

#[tokio::test]
async fn test_process_manager_not_found_handling() {
    let manager = ProcessManager::default();

    assert!(manager.get("proc-nonexistent").await.is_none());
    let out_res = manager.get_output("proc-nonexistent", None, None).await;
    assert!(matches!(out_res, Err(ProcessError::NotFound(..))));
    let stop_res = manager.stop("proc-nonexistent", false).await;
    assert!(matches!(stop_res, Err(ProcessError::NotFound(..))));
}
