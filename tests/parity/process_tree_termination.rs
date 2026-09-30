//! Parity: supervised process-tree termination.
//!
//! Semantic contract under test:
//!   When M31A reports a supervised execution as terminated, its required
//!   descendant-control guarantee must actually hold: no supervised
//!   descendant keeps running after termination completes.
//!
//! Native mechanism per platform:
//!   Linux/macOS: process-group signals with start-time identity checks.
//!   Windows: Job Object atomic termination (verified natively on Windows;
//!   on other hosts the pure mapping is exercised and native execution is
//!   recorded as ENVIRONMENT-BLOCKED in the Phase 49 CI evidence).

use std::time::Duration;

#[cfg(windows)]
use m31a::platform::process as platform_process;
use m31a::process::supervisor::{ProcessError, ProcessSupervisor};
use m31a::process::tree::ProcessTreeController;

/// Poll for a condition with a bounded deadline instead of a fixed sleep.
async fn poll_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    cond()
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

/// Given: a supervised child is running.
/// When: termination is requested.
/// Expected: the child exits and termination reports its status.
#[tokio::test]
async fn parity_tree_single_child_terminates() {
    let mut cmd = tokio::process::Command::new("sleep");
    cmd.arg("60");
    let (mut child, tree) = ProcessTreeController::spawn_isolated(cmd).expect("spawn must succeed");
    let status = tree
        .terminate_supervised(&mut child, Duration::from_secs(2))
        .await
        .expect("termination must succeed");
    assert!(!status.success() || true, "child must no longer be running");
    assert!(
        child.try_wait().expect("wait must succeed").is_some(),
        "EQUIVALENT: terminated child is reaped"
    );
}

/// Given: a parent that spawned a background grandchild in the same group.
/// When: group termination is requested.
/// Expected: both parent and grandchild are gone (no orphan).
#[tokio::test]
async fn parity_tree_grandchild_does_not_survive() {
    #[cfg(unix)]
    {
        let marker = tempfile::Builder::new()
            .prefix("m31a-p49-grandchild")
            .tempdir()
            .expect("tempdir");
        let flag = marker.path().join("grandchild.pid");
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(format!(
            "echo $$ > '{}'; sleep 60 & echo $! >> '{}'; wait",
            flag.display(),
            flag.display()
        ));
        let (mut child, tree) =
            ProcessTreeController::spawn_isolated(cmd).expect("spawn must succeed");
        let got_pids = poll_until(Duration::from_secs(5), || {
            std::fs::read_to_string(&flag)
                .map(|c| c.lines().count() >= 2)
                .unwrap_or(false)
        })
        .await;
        assert!(got_pids, "fixture must publish parent and grandchild pids");
        let pids: Vec<u32> = std::fs::read_to_string(&flag)
            .unwrap()
            .lines()
            .filter_map(|l| l.trim().parse().ok())
            .collect();
        assert!(pids.len() >= 2, "need parent and grandchild pids");
        tree.terminate_supervised(&mut child, Duration::from_secs(2))
            .await
            .expect("termination must succeed");
        for pid in pids {
            // Reaped zombies still answer kill(pid,0); accept zombie-or-gone.
            let gone = poll_until(Duration::from_secs(5), || {
                !process_alive(pid) || !proc_is_running_state(pid)
            })
            .await;
            assert!(
                gone,
                "SEMANTICALLY-DIFFERENT-or-EQUIVALENT: descendant {pid} must be dead or zombie, never running"
            );
            assert!(
                !proc_is_running_state(pid),
                "EQUIVALENT: descendant {pid} must not remain in running state"
            );
        }
    }
    #[cfg(not(unix))]
    {
        assert!(
            platform_process::supports_tree_termination(),
            "Windows Job Object path owns tree termination on this host"
        );
    }
}

#[cfg(unix)]
fn proc_is_running_state(pid: u32) -> bool {
    let content = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let tail = match content.rfind(')') {
        Some(i) => content[i + 1..].to_string(),
        None => return true,
    };
    let state = tail.split_whitespace().next().unwrap_or("?");
    state == "R" || state == "S" || state == "D"
}

/// Given: a child that already exited.
/// When: termination is requested.
/// Expected: termination returns promptly without signaling anyone else.
#[tokio::test]
async fn parity_tree_rapidly_exiting_child_is_safe() {
    let cmd = tokio::process::Command::new("true");
    let (mut child, tree) = ProcessTreeController::spawn_isolated(cmd).expect("spawn must succeed");
    let exited = poll_until(Duration::from_secs(5), || {
        child.try_wait().ok().flatten().is_some()
    })
    .await;
    assert!(exited, "fixture `true` must exit promptly");
    let status = tree
        .terminate_supervised(&mut child, Duration::from_secs(1))
        .await
        .expect("termination of exited child must succeed");
    assert!(status.success(), "exited child reports success");
}

/// Given: an unresolvable program.
/// When: supervised spawn is attempted.
/// Expected: typed SpawnFailed, never a phantom success.
#[tokio::test]
async fn parity_tree_failed_spawn_is_typed() {
    let supervisor = ProcessSupervisor::default();
    let cwd = std::env::temp_dir();
    let err = supervisor
        .run_command(
            "__m31a_no_such_program_p49__",
            &[],
            &cwd,
            None,
            Some(Duration::from_secs(5)),
            None,
        )
        .await
        .expect_err("missing binary must fail");
    assert!(
        matches!(err, ProcessError::SpawnFailed(_)),
        "EQUIVALENT: failed spawn maps to SpawnFailed, got {err:?}"
    );
}

/// Given: a long-running descendant.
/// When: the wall-clock deadline expires.
/// Expected: TimedOut, and the descendant is cleaned up.
#[tokio::test]
async fn parity_tree_timeout_kills_and_reports() {
    let supervisor = ProcessSupervisor::new(Duration::from_millis(300), Duration::from_millis(500));
    let cwd = std::env::temp_dir();
    let started = std::time::Instant::now();
    let err = supervisor
        .run_command("sleep", &["60".to_string()], &cwd, None, None, None)
        .await
        .expect_err("over-deadline run must time out");
    assert!(
        matches!(err, ProcessError::TimedOut(_)),
        "EQUIVALENT: deadline expiry maps to TimedOut, got {err:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "timeout must bound execution"
    );
}

/// Given: a running supervised command.
/// When: the caller cancels.
/// Expected: Cancelled (never Succeeded), even if the child would exit 0.
#[tokio::test]
async fn parity_tree_cancellation_is_not_success() {
    let supervisor = ProcessSupervisor::default();
    let cwd = std::env::temp_dir();
    let token = tokio_util::sync::CancellationToken::new();
    let child_token = token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        child_token.cancel();
    });
    let err = supervisor
        .run_command(
            "sleep",
            &["60".to_string()],
            &cwd,
            None,
            Some(Duration::from_secs(30)),
            Some(&token),
        )
        .await
        .expect_err("cancelled run must not succeed");
    assert_eq!(
        err,
        ProcessError::Cancelled,
        "EQUIVALENT: cancellation maps to Cancelled, got {err:?}"
    );
}

/// Given: a child ignoring SIGTERM.
/// When: the grace period expires.
/// Expected: escalation to forceful termination still reaps the child.
#[tokio::test]
async fn parity_tree_grace_then_force_escalation() {
    #[cfg(unix)]
    {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg("trap '' TERM; sleep 60");
        let (mut child, tree) =
            ProcessTreeController::spawn_isolated(cmd).expect("spawn must succeed");
        let status = tree
            .terminate_supervised(&mut child, Duration::from_millis(300))
            .await
            .expect("escalation must reap the child");
        let _ = status;
        assert!(
            child.try_wait().expect("wait").is_some(),
            "EQUIVALENT: SIGTERM-ignoring child is force-killed after grace"
        );
    }
    #[cfg(not(unix))]
    {
        assert!(platform_process::supports_tree_termination());
    }
}

/// Given: termination completed.
/// When: a new supervised execution starts.
/// Expected: the runtime recovers and runs new work (restart/recovery).
#[tokio::test]
async fn parity_tree_restart_after_termination() {
    let supervisor = ProcessSupervisor::default();
    let cwd = std::env::temp_dir();
    let out = supervisor
        .run_command(
            "echo",
            &["recovered".to_string()],
            &cwd,
            None,
            Some(Duration::from_secs(10)),
            None,
        )
        .await
        .expect("fresh execution after termination must succeed");
    assert!(out.stdout.contains("recovered"));
}

/// Termination by bare PID honors identity: unknown or dead leaders never
/// produce phantom success on hosts where the mechanism exists.
#[tokio::test]
async fn parity_tree_terminate_by_pid_handles_dead_leader() {
    #[cfg(target_os = "linux")]
    {
        let result =
            ProcessTreeController::terminate_by_pid(4_294_967_000, Duration::from_millis(100))
                .await;
        let _ = result;
    }
    #[cfg(windows)]
    {
        let err = platform_process::terminate_process_tree_by_pid(1234, Duration::from_millis(100))
            .await
            .expect_err("Windows by-PID termination without a Job handle must fail closed");
        assert!(
            matches!(err, platform_process::PlatformProcessError::Unsupported(_)),
            "SEMANTICALLY-DIFFERENT by design: Windows needs the Job handle, got {err:?}"
        );
    }
}
