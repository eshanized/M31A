//! Dedicated integration tests for `m31a-proc` (src/process.rs).
//!
//! Validates:
//! - ProcessStatus variant states, predicates (`is_running`), and UI display badges
//! - ManagedProcess model to ProcessInfo conversion
//! - Serde JSON fidelity across ProcessInfo, ProcessOutput, and ProcessStopResult

use chrono::Utc;
use m31a_proc::buffer::CircularOutputBuffer;
use m31a_proc::process::{
    ManagedProcess, ProcessInfo, ProcessOutput, ProcessStatus, ProcessStopResult,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[test]
fn test_process_status_predicates_and_badges() {
    let running = ProcessStatus::Running { pid: 1234 };
    assert!(running.is_running());
    assert_eq!(running.display_badge(), ("RUNNING", "green"));

    let exited_success = ProcessStatus::Exited { exit_code: 0 };
    assert!(!exited_success.is_running());
    assert_eq!(exited_success.display_badge(), ("EXITED(0)", "cyan"));

    let exited_err = ProcessStatus::Exited { exit_code: 1 };
    assert!(!exited_err.is_running());
    assert_eq!(exited_err.display_badge(), ("EXITED(ERR)", "red"));

    let terminated = ProcessStatus::Terminated { signal: 9 };
    assert!(!terminated.is_running());
    assert_eq!(terminated.display_badge(), ("TERMINATED", "yellow"));

    let failed = ProcessStatus::Failed {
        error: "Exec format error".to_string(),
    };
    assert!(!failed.is_running());
    assert_eq!(failed.display_badge(), ("FAILED", "red"));
}

#[tokio::test]
async fn test_managed_process_to_info_conversion() {
    let started_at = Utc::now();
    let managed = ManagedProcess {
        id: "proc-test-1".to_string(),
        label: "build worker".to_string(),
        command: "cargo".to_string(),
        args: vec!["build".to_string()],
        cwd: PathBuf::from("/workspace"),
        pid: Some(9999),
        started_at,
        status: Arc::new(RwLock::new(ProcessStatus::Running { pid: 9999 })),
        buffer: Arc::new(RwLock::new(CircularOutputBuffer::new(100))),
        cancel_tx: None,
    };

    let info = managed.to_info().await;
    assert_eq!(info.id, "proc-test-1");
    assert_eq!(info.label, "build worker");
    assert_eq!(info.command, "cargo");
    assert_eq!(info.args, vec!["build".to_string()]);
    assert_eq!(info.cwd, PathBuf::from("/workspace"));
    assert_eq!(info.pid, Some(9999));
    assert!(info.status.is_running());
    assert_eq!(info.total_lines_logged, 0);
}

#[test]
fn test_process_models_serde_roundtrip() {
    let info = ProcessInfo {
        id: "proc-serde".to_string(),
        label: "api server".to_string(),
        command: "node".to_string(),
        args: vec!["server.js".to_string()],
        cwd: PathBuf::from("/app"),
        pid: Some(4567),
        status: ProcessStatus::Running { pid: 4567 },
        started_at: Utc::now(),
        uptime_seconds: 42,
        total_lines_logged: 150,
    };

    let json = serde_json::to_string(&info).expect("serialize info");
    let deserialized: ProcessInfo = serde_json::from_str(&json).expect("deserialize info");
    assert_eq!(deserialized.id, info.id);
    assert_eq!(deserialized.pid, Some(4567));

    let stop_res = ProcessStopResult {
        id: "proc-stop".to_string(),
        label: "worker".to_string(),
        stopped: true,
        message: "Process terminated gracefully".to_string(),
    };
    let stop_json = serde_json::to_string(&stop_res).unwrap();
    let deserialized_stop: ProcessStopResult = serde_json::from_str(&stop_json).unwrap();
    assert!(deserialized_stop.stopped);

    let output = ProcessOutput {
        process_id: "proc-1".to_string(),
        label: "worker".to_string(),
        is_running: false,
        offset: 0,
        total_lines: 1,
        lines: Vec::new(),
        text: "completed".to_string(),
    };
    let out_json = serde_json::to_string(&output).unwrap();
    let deserialized_out: ProcessOutput = serde_json::from_str(&out_json).unwrap();
    assert_eq!(deserialized_out.text, "completed");
}
