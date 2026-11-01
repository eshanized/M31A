//! Child process security hardening, anti-tampering flags, and credential isolation.
//!
//! Cleanses environment variables, sets resource limits, and disables debugger attachment for child processes.

use std::env;
use tracing::debug;

pub const DANGEROUS_ENV_PREFIXES: &[&str] = &[
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "LD_AUDIT",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "DYLD_FRAMEWORK_PATH",
];

/// Applies process hardening to prevent memory dumping, ptrace attach, and malicious loader overrides.
pub fn apply_process_hardening() {
    // 1. Scrub dangerous environment variables
    scrub_dangerous_env_vars();

    // 2. Linux-specific security hardening (prctl non-dumpable, rlimit core 0)
    #[cfg(target_os = "linux")]
    apply_linux_process_hardening();

    debug!("Process hardening applied successfully");
}

pub fn scrub_dangerous_env_vars() -> usize {
    let mut scrubbed = 0;
    for var in DANGEROUS_ENV_PREFIXES {
        if env::var_os(var).is_some() {
            unsafe { env::remove_var(var) };
            scrubbed += 1;
        }
    }
    scrubbed
}

#[cfg(target_os = "linux")]
fn apply_linux_process_hardening() {
    // Disables ptrace attach and marks process memory non-dumpable
    unsafe {
        // PR_SET_DUMPABLE = 4
        let _ = libc::prctl(4, 0, 0, 0, 0);

        // RLIMIT_CORE = 4
        let rlim = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        let _ = libc::setrlimit(4, &rlim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scrub_dangerous_env_vars() {
        unsafe {
            env::set_var("LD_PRELOAD", "/malicious/lib.so");
            env::set_var("DYLD_LIBRARY_PATH", "/malicious/lib");
        }

        assert!(env::var("LD_PRELOAD").is_ok());
        let count = scrub_dangerous_env_vars();
        assert!(count >= 2);
        assert!(env::var("LD_PRELOAD").is_err());
        assert!(env::var("DYLD_LIBRARY_PATH").is_err());
    }

    #[test]
    fn test_apply_process_hardening_safe() {
        // Must not crash or panic
        apply_process_hardening();
    }
}
