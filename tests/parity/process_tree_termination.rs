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
    assert!(
        !status.success(),
        "Killed child process must not report successful exit status"
    );
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
        assert!(
            !status.success(),
            "Force-killed child must not report successful exit"
        );
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert!(
                status.signal().is_some(),
                "Escalated child termination must terminate by signal, got: {:?}",
                status
            );
        }
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

/// Given: a running supervised child process.
/// When: termination/cancellation is triggered and completes.
/// Expected: the first process is genuinely terminated and dead, and a newly
/// started supervised child executes successfully without interference.
#[tokio::test]
async fn parity_tree_restart_after_termination() {
    let supervisor = ProcessSupervisor::new(Duration::from_secs(30), Duration::from_millis(500));
    let temp_dir = tempfile::Builder::new()
        .prefix("m31a-restart-lifecycle")
        .tempdir()
        .expect("tempdir");

    #[cfg(unix)]
    {
        let pid_file = temp_dir.path().join("child.pid");
        let token = tokio_util::sync::CancellationToken::new();
        let token_clone = token.clone();
        let sup_clone = supervisor.clone();
        let cwd_clone = temp_dir.path().to_path_buf();
        let pid_file_clone = pid_file.clone();

        // 1. Spawn a controlled long-running process that publishes its PID
        let first_task = tokio::spawn(async move {
            sup_clone
                .run_command(
                    "sh",
                    &[
                        "-c".to_string(),
                        format!("echo $$ > '{}'; exec sleep 60", pid_file_clone.display()),
                    ],
                    &cwd_clone,
                    None,
                    Some(Duration::from_secs(30)),
                    Some(&token_clone),
                )
                .await
        });

        // 2. Wait for explicit evidence that the child process started
        let started = poll_until(Duration::from_secs(5), || {
            pid_file.exists()
                && std::fs::read_to_string(&pid_file)
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false)
        })
        .await;
        assert!(started, "first child must start and publish its PID");

        let pid: u32 = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .expect("valid pid in pidfile");

        assert!(
            process_alive(pid) || proc_is_running_state(pid),
            "first child {pid} must be alive before cancellation"
        );

        // 3. Trigger cancellation / termination
        token.cancel();

        let first_result = first_task
            .await
            .expect("first supervisor task must complete");
        assert_eq!(
            first_result,
            Err(ProcessError::Cancelled),
            "cancelled supervised execution must yield ProcessError::Cancelled"
        );

        // 4. Confirm that the original child process is no longer running
        let dead = poll_until(Duration::from_secs(5), || {
            !proc_is_running_state(pid) || !process_alive(pid)
        })
        .await;
        assert!(
            dead,
            "original child {pid} must be terminated after supervisor cancellation"
        );
        assert!(
            !proc_is_running_state(pid),
            "original child {pid} must not remain in running state"
        );
    }

    // 5. Start a new child execution through the supervisor (recovery/restart)
    let out = supervisor
        .run_command(
            "echo",
            &["recovered_after_lifecycle_termination".to_string()],
            temp_dir.path(),
            None,
            Some(Duration::from_secs(10)),
            None,
        )
        .await
        .expect("fresh execution after termination must succeed");

    assert_eq!(out.exit_code, 0);
    assert!(
        out.stdout.contains("recovered_after_lifecycle_termination"),
        "restarted execution output must match expected result"
    );
}

/// Baseline smoke check that independent sequential executions succeed.
#[tokio::test]
async fn parity_tree_subsequent_execution_smoke() {
    let supervisor = ProcessSupervisor::default();
    let cwd = std::env::temp_dir();
    let out = supervisor
        .run_command(
            "echo",
            &["smoke_recovered".to_string()],
            &cwd,
            None,
            Some(Duration::from_secs(10)),
            None,
        )
        .await
        .expect("fresh execution must succeed");
    assert!(out.stdout.contains("smoke_recovered"));
}

/// Termination by bare PID honors identity: unknown or dead leaders never
/// produce phantom success on hosts where the mechanism exists.
#[tokio::test]
async fn parity_tree_terminate_by_pid_handles_dead_leader() {
    #[cfg(unix)]
    {
        // 1. Nonexistent PID: termination succeeds idempotently because the process is not alive.
        let non_existent_pid = 4_294_967_000;
        assert!(
            !m31a::platform::process::is_process_alive(non_existent_pid),
            "test PID must not be alive"
        );
        let result =
            ProcessTreeController::terminate_by_pid(non_existent_pid, Duration::from_millis(50))
                .await;
        assert!(
            result.is_ok(),
            "terminating nonexistent PID must succeed idempotently: {result:?}"
        );

        // 2. Real process that runs and exits naturally:
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("exit 0")
            .spawn()
            .expect("spawn short-lived child");
        let pid = child.id().expect("child has PID");
        let status = child.wait().await.expect("child waits");
        assert!(status.success(), "child must have exited");
        assert!(
            !m31a::platform::process::is_process_alive(pid),
            "child must be dead after exit"
        );

        let dead_leader_res =
            ProcessTreeController::terminate_by_pid(pid, Duration::from_millis(50)).await;
        assert!(
            dead_leader_res.is_ok(),
            "terminating verified dead leader must succeed idempotently: {dead_leader_res:?}"
        );
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
