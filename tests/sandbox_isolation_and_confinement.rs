use tempfile::tempdir;

use m31a::sandbox::capabilities::SandboxCapabilities;
use m31a::sandbox::limits::{ResourceLimits, apply_pre_exec_limits};
use m31a::sandbox::plan::{NetworkConfinement, SandboxError, SandboxPlan};
use m31a::sandbox::probe::PlatformProbe;
use m31a::sandbox::provider::SandboxProvider;
use m31a::sandbox::providers::{BubblewrapSandboxProvider, ProcessIsolationProvider};

// ============================================================================
// Task 09-03-01: SandboxCapabilities Matrix, PlatformProbe & Fail-Closed Plan
// ============================================================================

#[test]
fn test_sandbox_capabilities_fail_closed() {
    // 1. Explicit 11-property matrix inspection
    let none = SandboxCapabilities::none();
    assert!(!none.fs_read_only_root);
    assert!(!none.fs_isolated_workspace_rw);
    assert!(!none.fs_private_tempdir);
    assert!(!none.fs_masked_credentials);
    assert!(!none.net_deny_all);
    assert!(!none.net_allowlist);
    assert!(!none.proc_isolated_pid_ns);
    assert!(!none.proc_group_isolation);
    assert!(!none.proc_cgroup_v2);
    assert!(!none.proc_rlimit);
    assert!(!none.user_namespaces);
    assert!(!none.supports_fs_isolation());
    assert!(!none.supports_net_isolation());

    let all = SandboxCapabilities::all();
    assert!(all.supports_fs_isolation());
    assert!(all.supports_net_isolation());
    assert!(all.supports_process_isolation());

    // 2. Probe host capabilities
    let probed = PlatformProbe::probe_platform_capabilities();
    assert!(probed.proc_group_isolation);
    assert!(probed.proc_rlimit);

    // 3. Fail-Closed Validation (SND-03, D-05)
    let temp = tempdir().unwrap();
    let plan_fs = SandboxPlan::new(temp.path().to_path_buf())
        .with_fs_isolation(true)
        .with_net_isolation(false);

    // Fails closed on capabilities missing fs isolation
    let err_fs = plan_fs.validate_against(&none).unwrap_err();
    match err_fs {
        SandboxError::CapabilitiesUnsatisfied(msg) => {
            assert!(msg.contains("Filesystem isolation required"));
        }
        other => panic!("Expected CapabilitiesUnsatisfied, got: {:?}", other),
    }

    // Fails closed on capabilities missing network isolation
    let plan_net = SandboxPlan::new(temp.path().to_path_buf())
        .with_fs_isolation(false)
        .with_net_isolation(true);
    let err_net = plan_net.validate_against(&none).unwrap_err();
    match err_net {
        SandboxError::CapabilitiesUnsatisfied(msg) => {
            assert!(msg.contains("Network isolation required"));
        }
        other => panic!("Expected CapabilitiesUnsatisfied, got: {:?}", other),
    }

    // Succeeds when capabilities satisfy requirements
    assert!(plan_fs.validate_against(&all).is_ok());
    assert!(plan_net.validate_against(&all).is_ok());
}

// ============================================================================
// Task 09-03-02: Bubblewrap Linux Filesystem Isolation & Network Confinement
// ============================================================================

#[tokio::test]
async fn test_bubblewrap_filesystem_isolation() {
    let bwrap_path = match PlatformProbe::detect_bwrap_path() {
        Some(path) => path,
        None => {
            eprintln!("Skipping bwrap test: bwrap binary not found on host");
            return;
        }
    };

    if !PlatformProbe::probe_user_namespaces() {
        eprintln!("Skipping bwrap test: unprivileged user namespaces not supported");
        return;
    }

    let provider = BubblewrapSandboxProvider::new(bwrap_path);
    let temp = tempdir().unwrap();
    let workspace = temp.path().to_path_buf();

    let plan = SandboxPlan::new(workspace.clone())
        .with_fs_isolation(true)
        .with_net_isolation(true);

    let handle = provider
        .prepare(&plan)
        .await
        .expect("prepare bwrap sandbox");

    // 1. Workspace is read-write: write a file and read it back
    let mut write_cmd = provider
        .build_command(
            &plan,
            &handle,
            "sh",
            &[
                "-c".to_string(),
                "echo 'sandboxed' > /workspace/test.txt".to_string(),
            ],
        )
        .expect("build write command");

    let output = write_cmd
        .output()
        .await
        .expect("execute write command inside sandbox");
    assert!(
        output.status.success(),
        "Writing to /workspace failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(workspace.join("test.txt"))
            .unwrap()
            .trim(),
        "sandboxed"
    );

    // 2. /usr is read-only: writing to /usr must fail
    let mut ro_cmd = provider
        .build_command(
            &plan,
            &handle,
            "sh",
            &["-c".to_string(), "touch /usr/forbidden.txt".to_string()],
        )
        .expect("build ro command");

    let ro_output = ro_cmd.output().await.expect("execute touch inside /usr");
    assert!(
        !ro_output.status.success(),
        "Writing to /usr should fail due to read-only mount"
    );

    // 3. User home directory is masked / invisible: /home must not exist or be empty
    let mut home_cmd = provider
        .build_command(
            &plan,
            &handle,
            "sh",
            &["-c".to_string(), "ls -la /home".to_string()],
        )
        .expect("build home check command");

    let home_output = home_cmd.output().await.expect("execute ls /home");
    // Either /home does not exist or has 0 user files
    let home_stderr = String::from_utf8_lossy(&home_output.stderr);
    let home_stdout = String::from_utf8_lossy(&home_output.stdout);
    assert!(
        !home_output.status.success() || !home_stdout.contains("snigdha"),
        "Host user home directory must be masked: stdout={}, stderr={}",
        home_stdout,
        home_stderr
    );

    provider.cleanup(&handle).await.expect("cleanup sandbox");
}

#[tokio::test]
async fn test_network_isolation_enforcement() {
    let bwrap_path = match PlatformProbe::detect_bwrap_path() {
        Some(path) => path,
        None => {
            eprintln!("Skipping bwrap network test: bwrap binary not found");
            return;
        }
    };

    if !PlatformProbe::probe_user_namespaces() {
        eprintln!("Skipping bwrap network test: user namespaces not supported");
        return;
    }

    let provider = BubblewrapSandboxProvider::new(bwrap_path);
    let temp = tempdir().unwrap();
    let workspace = temp.path().to_path_buf();

    let plan = SandboxPlan::new(workspace)
        .with_fs_isolation(true)
        .with_net_isolation(true)
        .with_network_confinement(NetworkConfinement::Isolated);

    let handle = provider.prepare(&plan).await.expect("prepare sandbox");

    // Outbound TCP connection to 1.1.1.1:80 must fail with Network is unreachable
    let mut net_cmd = provider
        .build_command(
            &plan,
            &handle,
            "python3",
            &[
                "-c".to_string(),
                "import socket; s = socket.socket(); s.settimeout(0.5); s.connect(('1.1.1.1', 80))"
                    .to_string(),
            ],
        )
        .expect("build net command");

    let output = net_cmd.output().await.expect("execute net check command");
    assert!(
        !output.status.success(),
        "Network access must be denied inside isolated sandbox"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Network is unreachable") || stderr.contains("Errno 101"),
        "Expected ENETUNREACH / Network is unreachable error, got: {}",
        stderr
    );

    provider.cleanup(&handle).await.expect("cleanup sandbox");
}

// ============================================================================
// Task 09-03-03: ProcessIsolationProvider Fallback & Resource Limits Enforcement
// ============================================================================

#[tokio::test]
async fn test_process_provider_rejects_fs_isolation() {
    let provider = ProcessIsolationProvider::new();
    let caps = provider.capabilities();
    assert!(!caps.fs_isolated_workspace_rw);
    assert!(!caps.net_deny_all);
    assert!(caps.proc_group_isolation);

    let temp = tempdir().unwrap();
    let workspace = temp.path().to_path_buf();

    // 1. Plan requiring filesystem isolation MUST fail closed
    let plan_fs = SandboxPlan::new(workspace.clone())
        .with_fs_isolation(true)
        .with_net_isolation(false);

    let err_fs = provider.prepare(&plan_fs).await.unwrap_err();
    match err_fs {
        SandboxError::CapabilitiesUnsatisfied(msg) => {
            assert!(msg.contains("Filesystem isolation required"));
        }
        other => panic!("Expected CapabilitiesUnsatisfied, got: {:?}", other),
    }

    // 2. Plan requiring network isolation MUST fail closed
    let plan_net = SandboxPlan::new(workspace.clone())
        .with_fs_isolation(false)
        .with_net_isolation(true);

    let err_net = provider.prepare(&plan_net).await.unwrap_err();
    match err_net {
        SandboxError::CapabilitiesUnsatisfied(msg) => {
            assert!(msg.contains("Network isolation required"));
        }
        other => panic!("Expected CapabilitiesUnsatisfied, got: {:?}", other),
    }

    // 3. Plan with no FS/net isolation requirement succeeds
    let plan_ok = SandboxPlan::new(workspace)
        .with_fs_isolation(false)
        .with_net_isolation(false);

    let handle = provider
        .prepare(&plan_ok)
        .await
        .expect("prepare succeeds without fs/net isolation");
    provider.cleanup(&handle).await.expect("cleanup succeeds");
}

#[test]
fn test_resource_limits_validation() {
    let zero_cpu = ResourceLimits {
        timeout_ms: 1000,
        cpu_time_secs: Some(0),
        memory_bytes: None,
        max_open_files: None,
        max_processes: None,
        max_output_bytes: 1024,
    };

    let res = apply_pre_exec_limits(&zero_cpu);
    assert!(res.is_err(), "Zero CPU limit must be rejected");

    let zero_mem = ResourceLimits {
        timeout_ms: 1000,
        cpu_time_secs: None,
        memory_bytes: Some(0),
        max_open_files: None,
        max_processes: None,
        max_output_bytes: 1024,
    };

    let res_mem = apply_pre_exec_limits(&zero_mem);
    assert!(res_mem.is_err(), "Zero memory limit must be rejected");
}
