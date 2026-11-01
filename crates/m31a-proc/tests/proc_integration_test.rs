//! Integration tests for background process manager and output buffers.
//!
//! Tests process spawning, streaming output buffers, exit code tracking, and signal termination.

use m31a_proc::{
    CircularOutputBuffer, OutputStream, ProcessManager, ProcessManagerConfig, ProcessStatus,
};
use std::collections::HashMap;
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test]
async fn test_process_manager_start_and_get_output() {
    let tmp = tempdir().unwrap();
    let manager = ProcessManager::default();

    let info = manager
        .start(
            "sh",
            &[
                "-c".into(),
                "echo 'STDOUT_SAMPLE'; echo 'STDERR_SAMPLE' >&2".into(),
            ],
            tmp.path(),
            HashMap::new(),
            Some("test_echo".into()),
        )
        .await
        .expect("start process");

    assert!(info.id.starts_with("proc-"));
    assert_eq!(info.label, "test_echo");

    // Wait briefly for process to execute
    tokio::time::sleep(Duration::from_millis(150)).await;

    let output = manager
        .get_output(&info.id, None, None)
        .await
        .expect("get output");
    assert!(output.text.contains("STDOUT_SAMPLE"));
    assert!(output.text.contains("STDERR_SAMPLE"));
}

#[tokio::test]
async fn test_process_manager_list_and_clear_exited() {
    let tmp = tempdir().unwrap();
    let manager = ProcessManager::default();

    let p1 = manager
        .start(
            "sh",
            &["-c".into(), "echo 'p1 finished'".into()],
            tmp.path(),
            HashMap::new(),
            Some("proc_quick".into()),
        )
        .await
        .unwrap();

    // Verify it appears in list
    let list = manager.list().await;
    assert!(list.iter().any(|p| p.id == p1.id));

    // Wait for exit
    tokio::time::sleep(Duration::from_millis(150)).await;

    let info = manager.get(&p1.id).await.unwrap();
    assert!(matches!(info.status, ProcessStatus::Exited { .. }));

    // Cleanup finished processes with 0s threshold
    tokio::time::sleep(Duration::from_millis(50)).await;
    manager.cleanup_finished(0).await;
    assert!(manager.get(&p1.id).await.is_none());
}

#[tokio::test]
async fn test_process_manager_stop_and_stop_all() {
    let tmp = tempdir().unwrap();
    let config = ProcessManagerConfig {
        default_buffer_capacity: 1000,
        termination_grace_period_ms: 100,
        ..Default::default()
    };
    let manager = ProcessManager::new(config);

    // Start 2 sleep processes
    let p1 = manager
        .start("sleep", &["10".into()], tmp.path(), HashMap::new(), None)
        .await
        .unwrap();
    let p2 = manager
        .start("sleep", &["10".into()], tmp.path(), HashMap::new(), None)
        .await
        .unwrap();

    // Stop p1 specifically
    let res = manager.stop(&p1.id, false).await.unwrap();
    assert!(res.stopped);

    let p1_info = manager.get(&p1.id).await.unwrap();
    assert!(!p1_info.status.is_running());

    // Stop all remaining (p2)
    let reports = manager.stop_all().await;
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].id, p2.id);

    let p2_info = manager.get(&p2.id).await.unwrap();
    assert!(!p2_info.status.is_running());
}

#[test]
fn test_circular_output_buffer_slice_and_overflow() {
    let mut buf = CircularOutputBuffer::new(5); // capacity = 5 lines

    for i in 1..=10 {
        buf.push(OutputStream::Stdout, format!("Line {}", i));
    }

    assert_eq!(buf.total_lines(), 10);
    assert_eq!(buf.len(), 5);

    // Buffer should now hold only lines 6 to 10
    let text = buf.as_text(10);
    assert!(!text.contains("Line 1\n"));
    assert!(text.contains("Line 6"));
    assert!(text.contains("Line 10"));

    // Slice testing
    let (slice, total) = buf.slice(0, 3);
    assert_eq!(total, 10);
    assert_eq!(slice.len(), 3);
    assert_eq!(slice[0].line, "Line 6");
}

#[tokio::test]
async fn test_process_manager_edge_cases_and_badges() {
    let tmp = tempdir().unwrap();
    let manager = ProcessManager::default();

    // Test badges
    assert_eq!(
        ProcessStatus::Running { pid: 123 }.display_badge().0,
        "RUNNING"
    );
    assert_eq!(
        ProcessStatus::Exited { exit_code: 0 }.display_badge().0,
        "EXITED(0)"
    );
    assert_eq!(
        ProcessStatus::Exited { exit_code: 1 }.display_badge().0,
        "EXITED(ERR)"
    );
    assert_eq!(
        ProcessStatus::Terminated { signal: 9 }.display_badge().0,
        "TERMINATED"
    );
    assert_eq!(
        ProcessStatus::Failed {
            error: "err".into()
        }
        .display_badge()
        .0,
        "FAILED"
    );

    // Test non-existent process operations
    assert!(manager.get("proc-nonexistent").await.is_none());
    assert!(
        manager
            .get_output("proc-nonexistent", None, None)
            .await
            .is_err()
    );
    assert!(manager.tail_output("proc-nonexistent", 10).await.is_err());
    assert!(manager.stop("proc-nonexistent", false).await.is_err());

    // Test tail_output on live process
    let proc = manager
        .start(
            "sh",
            &["-c".into(), "echo 'Line A'; echo 'Line B'".into()],
            tmp.path(),
            HashMap::new(),
            None,
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(150)).await;

    let tail = manager.tail_output(&proc.id, 2).await.unwrap();
    assert!(tail.len() >= 2);

    // Stop process when it has already exited
    let stop_result = manager.stop(&proc.id, false).await.unwrap();
    assert!(!stop_result.stopped);
    assert!(stop_result.message.contains("not running"));

    // Start a process and force stop it (SIGKILL)
    let p_long = manager
        .start("sleep", &["30".into()], tmp.path(), HashMap::new(), None)
        .await
        .unwrap();
    let force_stop = manager.stop(&p_long.id, true).await.unwrap();
    assert!(force_stop.stopped);
    assert!(force_stop.message.contains("SIGKILL"));
}

#[tokio::test]
async fn test_process_manager_exec_policy_denial() {
    let tmp = tempdir().unwrap();
    let config = ProcessManagerConfig {
        exec_policy: Some(m31a_execpolicy::ExecPolicy::standard()),
        ..Default::default()
    };
    let manager = ProcessManager::new(config);

    // Dangerous destructive pattern: rm -rf /
    let res = manager
        .start(
            "rm",
            &["-rf".into(), "/".into()],
            tmp.path(),
            HashMap::new(),
            None,
        )
        .await;

    assert!(res.is_err());
    let err = res.err().unwrap();
    assert!(matches!(
        err,
        m31a_proc::ProcessError::PolicyViolation(_, _)
    ));
}

#[tokio::test]
async fn test_process_manager_sandbox_isolation() {
    let tmp = tempdir().unwrap();
    let config = ProcessManagerConfig {
        sandbox_policy: Some(m31a_core::SandboxPolicy::WorkspaceWrite),
        ..Default::default()
    };
    let manager = ProcessManager::new(config);

    let res = manager
        .start(
            "echo",
            &["SANDBOXED_PROCESS_OK".into()],
            tmp.path(),
            HashMap::new(),
            None,
        )
        .await;

    assert!(res.is_ok());
    let proc = res.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;

    let output = manager.get_output(&proc.id, None, None).await.unwrap();
    let stdout_text: String = output
        .lines
        .into_iter()
        .map(|l| l.line)
        .collect::<Vec<_>>()
        .join(" ");
    assert!(stdout_text.contains("SANDBOXED_PROCESS_OK"));
}
