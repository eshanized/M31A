//! Integration tests for Linux Bubblewrap Sandbox wiring in LocalProcessProvider (AD-009).

use m31a::capability::error::CapabilityError;
use m31a::capability::providers::LocalProcessProvider;
use m31a::capability::traits::process::ProcessService;
use m31a::capability::traits::shell::ShellService;
use m31a::process::supervisor::ProcessSupervisor;
use m31a::sandbox::probe::PlatformProbe;
use m31a::sandbox::providers::{BubblewrapSandboxProvider, ProcessIsolationProvider};
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_local_process_provider_bubblewrap_sandboxed_execution() {
    let bwrap_opt = PlatformProbe::detect_bwrap_path();
    let userns = PlatformProbe::probe_user_namespaces();

    if bwrap_opt.is_none() || !userns {
        eprintln!("Skipping bwrap test: bwrap or unprivileged user namespaces not supported");
        return;
    }

    let bwrap_path = bwrap_opt.unwrap();
    let temp = tempdir().unwrap();
    let workspace = temp.path().to_path_buf();

    let supervisor = Arc::new(ProcessSupervisor::default());
    let bwrap_provider = Arc::new(BubblewrapSandboxProvider::new(bwrap_path));
    let provider = LocalProcessProvider::with_sandbox(
        workspace.clone(),
        supervisor,
        Some(bwrap_provider),
        true, // sandbox strictly required
    );

    // 1. Verify sandboxed spawn_command writes into workspace
    let output = provider
        .spawn_command(
            "sh",
            &[
                "-c".to_string(),
                "echo 'sandboxed-execution-ok' > test.txt && cat test.txt".to_string(),
            ],
            None,
            10,
        )
        .await
        .expect("Sandboxed command execution must succeed");

    assert_eq!(output.exit_code, 0);
    assert!(
        output.stdout.contains("sandboxed-execution-ok"),
        "stdout must contain echo result: {}",
        output.stdout
    );

    let written =
        std::fs::read_to_string(workspace.join("test.txt")).expect("file must exist in workspace");
    assert_eq!(written.trim(), "sandboxed-execution-ok");

    // 2. Verify execute_bounded_shell routes through sandboxing
    let shell_out = provider
        .execute_bounded_shell("echo 'shell-sandboxed' >> test.txt", None, 10)
        .await
        .expect("Sandboxed shell execution must succeed");
    assert_eq!(shell_out.exit_code, 0);

    let written_after =
        std::fs::read_to_string(workspace.join("test.txt")).expect("file must exist");
    assert!(written_after.contains("shell-sandboxed"));
}

#[tokio::test]
async fn test_local_process_provider_fail_closed_when_sandbox_required_and_unavailable() {
    let temp = tempdir().unwrap();
    let workspace = temp.path().to_path_buf();

    // Configure with fallback ProcessIsolationProvider (no filesystem isolation capability)
    let supervisor = Arc::new(ProcessSupervisor::default());
    let fallback_sandbox = Arc::new(ProcessIsolationProvider::new());
    let provider = LocalProcessProvider::with_sandbox(
        workspace.clone(),
        supervisor,
        Some(fallback_sandbox),
        true, // strictly required
    );

    let res = provider
        .spawn_command("echo", &["test".to_string()], None, 10)
        .await;

    match res {
        Err(CapabilityError::PermissionDenied(msg)) => {
            assert!(
                msg.contains("Sandbox execution is strictly required"),
                "Expected fail-closed permission denied, got: {}",
                msg
            );
        }
        other => panic!(
            "Expected PermissionDenied fail-closed error, got: {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_local_process_provider_timeout_under_sandboxing() {
    let bwrap_opt = PlatformProbe::detect_bwrap_path();
    let userns = PlatformProbe::probe_user_namespaces();

    if bwrap_opt.is_none() || !userns {
        eprintln!("Skipping bwrap timeout test: bwrap not available");
        return;
    }

    let bwrap_path = bwrap_opt.unwrap();
    let temp = tempdir().unwrap();
    let workspace = temp.path().to_path_buf();

    let supervisor = Arc::new(ProcessSupervisor::default());
    let bwrap_provider = Arc::new(BubblewrapSandboxProvider::new(bwrap_path));
    let provider = LocalProcessProvider::with_sandbox(
        workspace.clone(),
        supervisor,
        Some(bwrap_provider),
        true,
    );

    let res = provider
        .spawn_command("sleep", &["5".to_string()], None, 1)
        .await;

    match res {
        Err(CapabilityError::Timeout(_)) => {
            // Expected timeout
        }
        other => panic!("Expected Timeout error, got: {:?}", other),
    }
}
