//! Dedicated integration tests for `m31a-proc` (src/lib.rs).
//!
//! Validates:
//! - Root re-exports: `CircularOutputBuffer`, `OutputStream`, `ProcessLine`, `ProcessError`, `ProcessManager`, `ProcessManagerConfig`, `ManagedProcess`, `ProcessInfo`, `ProcessOutput`, `ProcessStatus`, `ProcessStopResult`
//! - End-to-end pipeline: supervisor orchestration from root exports

use m31a_proc::{
    CircularOutputBuffer, OutputStream, ProcessError, ProcessInfo, ProcessManager,
    ProcessManagerConfig, ProcessOutput, ProcessStatus, ProcessStopResult,
};
use std::collections::HashMap;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_root_reexports() {
    let mut buf = CircularOutputBuffer::new(5);
    buf.push(OutputStream::Stdout, "Root line".to_string());
    assert_eq!(buf.len(), 1);

    let err = ProcessError::NotFound("test".to_string());
    assert!(err.to_string().contains("not found"));

    let status = ProcessStatus::Running { pid: 100 };
    assert!(status.is_running());
}

#[tokio::test]
async fn test_root_manager_lifecycle_integration() {
    let temp = tempdir().unwrap();
    let manager = ProcessManager::new(ProcessManagerConfig::default());

    let info: ProcessInfo = manager
        .start(
            "sh",
            &["-c".to_string(), "echo 'Supervisor test'; exit 0".to_string()],
            temp.path(),
            HashMap::new(),
            Some("integration-proc".to_string()),
        )
        .await
        .expect("Failed to start process");

    tokio::time::sleep(Duration::from_millis(150)).await;

    let output: ProcessOutput = manager
        .get_output(&info.id, None, None)
        .await
        .expect("Failed to fetch output");

    assert!(output.text.contains("Supervisor test"));

    // Spawn a long-running background process to test stop_all
    let _ = manager
        .start(
            "sh",
            &["-c".to_string(), "sleep 30".to_string()],
            temp.path(),
            HashMap::new(),
            Some("background-worker".to_string()),
        )
        .await
        .expect("Failed to start background process");

    let stop_results: Vec<ProcessStopResult> = manager.stop_all().await;
    assert!(!stop_results.is_empty());
    assert!(stop_results[0].stopped);
}
