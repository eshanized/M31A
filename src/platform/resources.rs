//! Semantic resource budgets separated from enforcement mechanisms.
//!
//! The runtime declares what a step may consume. The host reports what it
//! actually enforced. Callers must propagate the outcome instead of
//! assuming a limit took effect.
//!
//! Linux: POSIX rlimits + cgroups v2
//! macOS: POSIX rlimits (RLIMIT_CPU, RLIMIT_AS, RLIMIT_NOFILE, RLIMIT_NPROC)
//! Windows: Job Objects (CPU rate, memory, active process count)

use serde::{Deserialize, Serialize};

/// Portable budget requested by the runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceBudget {
    pub max_cpu_seconds: Option<u64>,
    pub max_memory_bytes: Option<u64>,
    pub max_processes: Option<u64>,
    pub max_open_files: Option<u64>,
    pub max_output_bytes: Option<usize>,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            max_cpu_seconds: Some(30),
            max_memory_bytes: Some(1024 * 1024 * 1024),
            max_processes: None,
            max_open_files: Some(256),
            max_output_bytes: Some(10 * 1024 * 1024),
        }
    }
}

/// Honest enforcement result. Partial states name what was applied so the
/// caller can decide whether degraded execution is acceptable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LimitOutcome {
    Applied,
    PartiallyApplied {
        applied: Vec<String>,
        missing: Vec<String>,
    },
    Unsupported,
    Rejected {
        reason: String,
    },
}

/// Validate a budget before attempting enforcement.
pub fn validate_budget(budget: &ResourceBudget) -> Result<(), LimitOutcome> {
    if budget.max_cpu_seconds == Some(0) {
        return Err(LimitOutcome::Rejected {
            reason: "CPU time limit cannot be zero".to_string(),
        });
    }
    if budget.max_memory_bytes == Some(0) {
        return Err(LimitOutcome::Rejected {
            reason: "Memory limit cannot be zero".to_string(),
        });
    }
    if budget.max_open_files == Some(0) {
        return Err(LimitOutcome::Rejected {
            reason: "File descriptor limit cannot be zero".to_string(),
        });
    }
    if budget.max_processes == Some(0) {
        return Err(LimitOutcome::Rejected {
            reason: "Process count limit cannot be zero".to_string(),
        });
    }
    Ok(())
}

/// Enforcement profile of the executing host.
pub fn host_enforcement_profile() -> LimitOutcome {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        LimitOutcome::Applied
    }
    #[cfg(windows)]
    {
        if crate::platform::windows::job::job_objects_available() {
            LimitOutcome::Applied
        } else {
            LimitOutcome::Unsupported
        }
    }
    #[cfg(all(not(any(target_os = "linux", target_os = "macos")), not(windows), unix))]
    {
        LimitOutcome::Applied
    }
    #[cfg(all(
        not(any(target_os = "linux", target_os = "macos")),
        not(windows),
        not(unix)
    ))]
    {
        LimitOutcome::Unsupported
    }
}

/// Apply POSIX limits inside a pre-execution hook on hosts that provide
/// the mechanism (Linux, macOS, other Unix). Other hosts report unsupported
/// instead of succeeding without enforcing anything.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn apply_pre_exec_limits(budget: &ResourceBudget) -> Result<LimitOutcome, std::io::Error> {
    validate_budget(budget).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid resource budget")
    })?;
    unsafe {
        if let Some(cpu) = budget.max_cpu_seconds {
            let rlim = libc::rlimit {
                rlim_cur: cpu as libc::rlim_t,
                rlim_max: cpu as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_CPU, &rlim) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        if let Some(mem) = budget.max_memory_bytes {
            let rlim = libc::rlimit {
                rlim_cur: mem as libc::rlim_t,
                rlim_max: mem as libc::rlim_t,
            };
            // macOS uses RLIMIT_AS for virtual memory; Linux also supports it.
            if libc::setrlimit(libc::RLIMIT_AS, &rlim) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        if let Some(nofile) = budget.max_open_files {
            let rlim = libc::rlimit {
                rlim_cur: nofile as libc::rlim_t,
                rlim_max: nofile as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_NOFILE, &rlim) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        if let Some(nproc) = budget.max_processes {
            let rlim = libc::rlimit {
                rlim_cur: nproc as libc::rlim_t,
                rlim_max: nproc as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_NPROC, &rlim) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        // On macOS, document that RLIMIT_AS enforcement is best-effort.
        let _ = crate::platform::macos::validate_macos_budget(budget);
    }
    Ok(LimitOutcome::Applied)
}

/// Apply POSIX limits on other Unix hosts.
#[cfg(all(unix, not(target_os = "linux"), not(target_os = "macos")))]
pub fn apply_pre_exec_limits(budget: &ResourceBudget) -> Result<LimitOutcome, std::io::Error> {
    validate_budget(budget).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid resource budget")
    })?;
    unsafe {
        if let Some(cpu) = budget.max_cpu_seconds {
            let rlim = libc::rlimit {
                rlim_cur: cpu as libc::rlim_t,
                rlim_max: cpu as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_CPU, &rlim) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        if let Some(mem) = budget.max_memory_bytes {
            let rlim = libc::rlimit {
                rlim_cur: mem as libc::rlim_t,
                rlim_max: mem as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_AS, &rlim) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        if let Some(nofile) = budget.max_open_files {
            let rlim = libc::rlimit {
                rlim_cur: nofile as libc::rlim_t,
                rlim_max: nofile as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_NOFILE, &rlim) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        if let Some(nproc) = budget.max_processes {
            let rlim = libc::rlimit {
                rlim_cur: nproc as libc::rlim_t,
                rlim_max: nproc as libc::rlim_t,
            };
            if libc::setrlimit(libc::RLIMIT_NPROC, &rlim) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
    }
    Ok(LimitOutcome::Applied)
}

/// Windows has no pre-execution limit hook; Job Object limits are applied
/// after spawn by the supervisor. This returns Unsupported so callers
/// know no pre-exec enforcement occurred.
#[cfg(windows)]
pub fn apply_pre_exec_limits(_budget: &ResourceBudget) -> Result<LimitOutcome, std::io::Error> {
    Ok(LimitOutcome::Unsupported)
}

/// Non-Unix, non-Windows hosts have no pre-execution limit hook.
#[cfg(all(not(unix), not(windows)))]
pub fn apply_pre_exec_limits(_budget: &ResourceBudget) -> Result<LimitOutcome, std::io::Error> {
    Ok(LimitOutcome::Unsupported)
}

/// Install the resource hook on a Tokio command when the host provides one.
///
/// Concentrates the remaining system conditional in the platform layer so
/// sandbox and confinement callers stay portable.
pub fn install_pre_exec_hook(cmd: &mut tokio::process::Command, budget: ResourceBudget) {
    #[cfg(any(target_os = "linux", target_os = "macos", all(unix, not(windows))))]
    unsafe {
        cmd.pre_exec(move || match apply_pre_exec_limits(&budget) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        });
    }
    #[cfg(all(
        not(any(target_os = "linux", target_os = "macos")),
        not(unix),
        not(windows)
    ))]
    {
        let _ = (cmd, budget);
    }
    #[cfg(windows)]
    {
        // Windows Job Object limits are installed post-spawn by the
        // confinement supervisor which owns the JobHandle.
        let _ = (cmd, budget);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_zero_limits() {
        assert!(
            validate_budget(&ResourceBudget {
                max_cpu_seconds: Some(0),
                ..ResourceBudget::default()
            })
            .is_err()
        );
        assert!(
            validate_budget(&ResourceBudget {
                max_cpu_seconds: Some(1),
                max_memory_bytes: Some(0),
                ..ResourceBudget::default()
            })
            .is_err()
        );
    }

    #[test]
    fn default_budget_is_valid() {
        assert!(validate_budget(&ResourceBudget::default()).is_ok());
    }

    #[test]
    fn profile_reports_applied_on_supported_hosts() {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        assert_eq!(host_enforcement_profile(), LimitOutcome::Applied);
    }
}
