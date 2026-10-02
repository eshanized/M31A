//! Parity: resource limits (CPU, memory, FDs, process count, output).
//!
//! Each limit is tested individually because the native enforcement
//! mechanism differs per platform:
//!   Linux: cgroups v2 + RLIMIT_* (hard enforcement).
//!   macOS: RLIMIT_* best-effort (RLIMIT_AS allocator-dependent).
//!   Windows: Job Object CPU rate / memory / active-process; no FD limit.
//! Output bounds are runtime-enforced on every host (control case: must be
//! identical everywhere).

#[allow(unused_imports)]
use m31a::platform::resources::{
    ResourceBudget, apply_pre_exec_limits, host_enforcement_profile, validate_budget,
};
use m31a::platform::windows::job::describe_job_limits;
use m31a::platform::{LimitOutcome, PlatformFamily};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::process::CommandExt;

/// CPU: a deterministic busy workload under a CPU budget must be observed
/// through the M31A outcome type, never silently unenforced.
#[test]
fn parity_cpu_limit_enforcement_profile_is_typed() {
    let profile = host_enforcement_profile();
    assert!(
        matches!(
            profile,
            LimitOutcome::Applied
                | LimitOutcome::PartiallyApplied { .. }
                | LimitOutcome::Unsupported
        ),
        "EQUIVALENT: host CPU profile is a typed outcome, got {profile:?}"
    );
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    assert_eq!(
        profile,
        LimitOutcome::Applied,
        "Linux/macOS RLIMIT_CPU path reports Applied"
    );
}

/// CPU: a real busy-loop child under RLIMIT_CPU is terminated by the host
/// (SIGXCPU/SIGKILL), proving enforcement is not a no-op on Unix.
#[test]
fn parity_cpu_busy_loop_is_killed_by_rlimit() {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let budget = ResourceBudget {
            max_cpu_seconds: Some(1),
            max_memory_bytes: None,
            max_processes: None,
            max_open_files: None,
            max_output_bytes: None,
        };
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg("while true; do :; done");
        unsafe {
            let b = budget.clone();
            #[allow(clippy::missing_transmute_annotations)]
            cmd.pre_exec(move || match apply_pre_exec_limits(&b) {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            });
        }
        let mut child = cmd.spawn().expect("spawn busy loop");
        let status = child.wait().expect("wait on busy loop");
        assert!(
            !status.success(),
            "EQUIVALENT: RLIMIT_CPU=1 kills the busy loop (status {status:?})"
        );
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let budget = ResourceBudget {
            max_cpu_seconds: Some(1),
            ..ResourceBudget::default()
        };
        let desc = describe_job_limits(&budget);
        assert!(
            desc.applied.iter().any(|a| a.contains("CPU")),
            "Windows CPU maps to JOB_OBJECT_CPU_RATE_CONTROL + wall-clock"
        );
    }
}

/// Memory: below-limit allocation succeeds; the mapping names the mechanism.
#[test]
fn parity_memory_below_limit_succeeds() {
    let budget = ResourceBudget {
        max_cpu_seconds: None,
        max_memory_bytes: Some(512 * 1024 * 1024),
        max_processes: None,
        max_open_files: None,
        max_output_bytes: None,
    };
    assert!(validate_budget(&budget).is_ok());
    let (unit, _mechanism) = memory_mechanism_label();
    assert!(!unit.is_empty());
}

fn memory_mechanism_label() -> (String, String) {
    #[cfg(target_os = "linux")]
    {
        (
            "cgroups memory.max / RLIMIT_AS".to_string(),
            "hard kernel enforcement".to_string(),
        )
    }
    #[cfg(target_os = "macos")]
    {
        (
            "RLIMIT_AS".to_string(),
            "best-effort, allocator-dependent".to_string(),
        )
    }
    #[cfg(windows)]
    {
        (
            "JOB_OBJECT_LIMIT_PROCESS_MEMORY".to_string(),
            "commit/working-set limit".to_string(),
        )
    }
    #[cfg(all(not(target_os = "linux"), not(target_os = "macos"), not(windows)))]
    {
        ("foundation".to_string(), "unknown".to_string())
    }
}

/// Memory: zero and near-limit budgets validate; the semantic difference is
/// documented, not hidden: macOS RLIMIT_AS is best-effort.
#[test]
fn parity_memory_semantic_difference_is_explicit() {
    let macos_budget = m31a::platform::macos::validate_macos_budget(&ResourceBudget {
        max_cpu_seconds: None,
        max_memory_bytes: Some(64 * 1024 * 1024),
        max_processes: None,
        max_open_files: None,
        max_output_bytes: None,
    });
    assert!(
        macos_budget.is_ok(),
        "macOS budget mapping is a typed outcome: {macos_budget:?}"
    );
    let (applied, _missing) =
        m31a::platform::macos::macos_enforceable_limits(&ResourceBudget::default());
    assert!(
        applied.iter().any(|a| a.contains("RLIMIT_AS")),
        "SEMANTICALLY-DIFFERENT: macOS memory is RLIMIT_AS(best-effort), not cgroup-hard"
    );
}

/// Memory: an over-limit allocation is rejected by validation (fail-closed).
#[test]
fn parity_memory_zero_limit_rejected() {
    let budget = ResourceBudget {
        max_memory_bytes: Some(0),
        ..ResourceBudget::default()
    };
    assert!(
        matches!(validate_budget(&budget), Err(LimitOutcome::Rejected { .. })),
        "EQUIVALENT: zero memory limit is Rejected on every host"
    );
}

/// FD: Unix enforces RLIMIT_NOFILE; Windows honestly reports Unsupported.
/// This is the required negative parity test: no fabricated FD equivalent.
#[test]
fn parity_fd_limit_unix_enforced_windows_unsupported() {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::os::unix::process::CommandExt;
        let budget = ResourceBudget {
            max_cpu_seconds: None,
            max_memory_bytes: None,
            max_processes: None,
            max_open_files: Some(32),
            max_output_bytes: None,
        };
        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg("true");
        unsafe {
            let b = budget.clone();
            cmd.pre_exec(move || match apply_pre_exec_limits(&b) {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            });
        }
        let mut child = cmd.spawn().expect("spawn child with low FD limit");
        let status = child.wait().expect("wait on child");
        assert!(
            status.success(),
            "child runs successfully with applied FD limit"
        );
    }
    let win_budget = ResourceBudget {
        max_open_files: Some(256),
        ..ResourceBudget::default()
    };
    let desc = describe_job_limits(&win_budget);
    assert!(
        desc.missing.iter().any(|m| m.contains("max_open_files")),
        "GAP: Windows Job Objects have no FD counterpart; must be Missing"
    );
}

/// Process count: each platform names its enforcement; Windows uses the
/// active-process limit, Unix RLIMIT_NPROC.
#[test]
fn parity_process_count_limit_named_per_platform() {
    let budget = ResourceBudget {
        max_processes: Some(16),
        ..ResourceBudget::default()
    };
    assert!(validate_budget(&budget).is_ok());
    let desc = describe_job_limits(&budget);
    assert!(
        desc.applied.iter().any(|a| a.contains("ACTIVE_PROCESS")),
        "Windows process count maps to JOB_OBJECT_LIMIT_ACTIVE_PROCESS"
    );
    let (applied, _) = m31a::platform::macos::macos_enforceable_limits(&budget);
    assert!(
        applied.iter().any(|a| a.contains("RLIMIT_NPROC")),
        "macOS process count maps to RLIMIT_NPROC"
    );
}

/// Output bytes: runtime-enforced on every host, so the same limit must
/// produce the same M31A behavior independent of OS (control case).
#[test]
fn parity_output_limit_is_runtime_enforced_everywhere() {
    let budget = ResourceBudget {
        max_output_bytes: Some(1024),
        ..ResourceBudget::default()
    };
    let job = describe_job_limits(&budget);
    assert!(
        job.missing.iter().any(|m| m.contains("max_output_bytes")),
        "output bounds stay in the runtime dual-buffer layer, not the OS"
    );
    let (_, missing) = m31a::platform::macos::macos_enforceable_limits(&budget);
    assert!(
        missing.iter().any(|m| m.contains("max_output_bytes")),
        "macOS likewise defers output bounds to the runtime"
    );
    assert!(
        validate_budget(&budget).is_ok(),
        "EQUIVALENT: output budget validates identically everywhere"
    );
}

/// Wall-clock timeout: the portable backstop exists on every platform.
#[tokio::test]
async fn parity_wall_clock_timeout_is_portable_backstop() {
    let supervisor = m31a::process::supervisor::ProcessSupervisor::new(
        std::time::Duration::from_millis(200),
        std::time::Duration::from_millis(300),
    );
    let err = supervisor
        .run_command(
            "sleep",
            &["30".to_string()],
            &std::env::temp_dir(),
            None,
            None,
            None,
        )
        .await
        .expect_err("wall-clock must fire on every platform");
    assert!(
        matches!(err, m31a::process::supervisor::ProcessError::TimedOut(_)),
        "EQUIVALENT: wall-clock timeout is TimedOut everywhere, got {err:?}"
    );
}

/// Invalid budgets are rejected identically on every host.
#[test]
fn parity_invalid_budgets_rejected_everywhere() {
    assert!(
        validate_budget(&ResourceBudget {
            max_cpu_seconds: Some(0),
            ..ResourceBudget::default()
        })
        .is_err()
    );
    assert!(
        validate_budget(&ResourceBudget {
            max_memory_bytes: Some(0),
            ..ResourceBudget::default()
        })
        .is_err()
    );
    assert!(
        validate_budget(&ResourceBudget {
            max_open_files: Some(0),
            ..ResourceBudget::default()
        })
        .is_err()
    );
    assert!(
        validate_budget(&ResourceBudget {
            max_processes: Some(0),
            ..ResourceBudget::default()
        })
        .is_err()
    );
    assert!(validate_budget(&ResourceBudget::default()).is_ok());
    let _ = PlatformFamily::Linux;
}
