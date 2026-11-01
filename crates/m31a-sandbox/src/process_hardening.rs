//! Process hardening and anti-tamper utilities for M31A sandboxes and execution runners.
//!
//! Kernel-level security hardening, anti-tampering flags, and credential isolation for child processes:
//! - Disabling memory dumping and debugger attachment via `PR_SET_DUMPABLE`.
//! - Setting core file size limit to zero (`RLIMIT_CORE = 0`) to prevent credential leakage.
//! - Cleansing dangerous dynamic linker and shell execution variables.
//! - Setting parent-death signal (`PR_SET_PDEATHSIG`) to avoid orphaned sandbox processes.
//! - Providing hardened environment map construction.

use std::collections::{HashMap, HashSet};
use tracing::{debug, warn};

/// List of dangerous environment variable names or prefixes that could allow
/// code injection, binary hijacking, or dynamic linker manipulation.
pub const DANGEROUS_ENV_PREFIXES: &[&str] = &[
    "LD_",
    "DYLD_",
    "BASH_ENV",
    "ENV",
    "PROMPT_COMMAND",
    "CDPATH",
    "IFS",
    "GCONV_PATH",
    "PYTHONPATH",
    "RUBYLIB",
    "PERL5LIB",
    "NODE_OPTIONS",
];

/// Known sensitive variable names that must not leak into sandboxed commands unless explicitly scoped.
pub const SENSITIVE_TOKEN_VARIABLES: &[&str] = &[
    "NVIDIA_API_KEY",
    "OPENAI_API_KEY",
    "ANTHROPIC_API_KEY",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "GITLAB_TOKEN",
    "M31A_VAULT_KEY",
    "SLACK_BOT_TOKEN",
];

/// Results of applying pre-execution process hardening.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardeningStatus {
    pub dumpable_disabled: bool,
    pub core_limit_zeroed: bool,
    pub removed_env_vars: Vec<String>,
}

/// Applies pre-main process hardening in the current process.
/// Disables dumpability, sets core limit to 0, and purges dangerous linker environment variables.
pub fn apply_process_hardening() -> HardeningStatus {
    let mut status = HardeningStatus {
        dumpable_disabled: false,
        core_limit_zeroed: false,
        removed_env_vars: Vec::new(),
    };

    #[cfg(target_os = "linux")]
    {
        // 1. Disable ptrace attach & memory dumping
        unsafe {
            let res = libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0);
            if res == 0 {
                status.dumpable_disabled = true;
            } else {
                warn!("prctl(PR_SET_DUMPABLE, 0) returned {}", res);
            }
        }

        // 2. Set RLIMIT_CORE to 0
        unsafe {
            let rlim = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            let res = libc::setrlimit(libc::RLIMIT_CORE, &rlim);
            if res == 0 {
                status.core_limit_zeroed = true;
            } else {
                warn!("setrlimit(RLIMIT_CORE, 0) returned {}", res);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        unsafe {
            let res = libc::ptrace(libc::PT_DENY_ATTACH, 0, std::ptr::null_mut(), 0);
            if res == 0 {
                status.dumpable_disabled = true;
            }
        }
        unsafe {
            let rlim = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            let res = libc::setrlimit(libc::RLIMIT_CORE, &rlim);
            if res == 0 {
                status.core_limit_zeroed = true;
            }
        }
    }

    // 3. Purge dangerous linker variables from current process environment
    let mut vars_to_remove = Vec::new();
    for (k, _) in std::env::vars() {
        for prefix in DANGEROUS_ENV_PREFIXES {
            if k.starts_with(prefix) {
                vars_to_remove.push(k.clone());
                break;
            }
        }
    }

    for k in &vars_to_remove {
        unsafe {
            std::env::remove_var(k);
        }
    }
    status.removed_env_vars = vars_to_remove;

    status
}

/// Configure the child process to automatically terminate when the parent exits.
/// Should be called inside `Command::pre_exec` on Unix systems.
#[cfg(target_os = "linux")]
pub fn set_parent_death_signal() -> std::io::Result<()> {
    unsafe {
        let res = libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0);
        if res == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
}

/// Non-Linux fallback for parent death signal.
#[cfg(not(target_os = "linux"))]
pub fn set_parent_death_signal() -> std::io::Result<()> {
    Ok(())
}

/// Builds a sanitized, hardened environment variable map for child processes.
/// Strips dangerous linker vars and sensitive credentials unless explicitly allowed.
pub fn build_hardened_environment(
    base_env: &HashMap<String, String>,
    allowed_credentials: &HashSet<String>,
) -> HashMap<String, String> {
    let mut sanitized = HashMap::new();

    for (k, v) in base_env {
        // Strip dangerous injection prefixes
        let is_dangerous = DANGEROUS_ENV_PREFIXES.iter().any(|p| k.starts_with(p));
        if is_dangerous {
            debug!("Hardened env stripped dangerous variable: {}", k);
            continue;
        }

        // Strip sensitive credentials unless explicitly whitelisted
        let is_sensitive = SENSITIVE_TOKEN_VARIABLES.iter().any(|s| k == s);
        if is_sensitive && !allowed_credentials.contains(k) {
            debug!("Hardened env redacted credential: {}", k);
            continue;
        }

        sanitized.insert(k.clone(), v.clone());
    }

    // Ensure standard defensive defaults
    sanitized.insert("LC_ALL".to_string(), "C.UTF-8".to_string());
    sanitized.insert("LANG".to_string(), "C.UTF-8".to_string());

    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_process_hardening() {
        let status = apply_process_hardening();
        #[cfg(target_os = "linux")]
        {
            assert!(status.dumpable_disabled);
            assert!(status.core_limit_zeroed);
        }
    }

    #[test]
    fn test_build_hardened_environment() {
        let mut raw = HashMap::new();
        raw.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        raw.insert("LD_PRELOAD".to_string(), "/tmp/inject.so".to_string());
        raw.insert("LD_LIBRARY_PATH".to_string(), "/opt/custom/lib".to_string());
        raw.insert("PROMPT_COMMAND".to_string(), "malicious_script".to_string());
        raw.insert(
            "NVIDIA_API_KEY".to_string(),
            "nvapi-secret-12345".to_string(),
        );
        raw.insert("ALLOWED_VAR".to_string(), "safe_value".to_string());

        let mut allowed = HashSet::new();
        allowed.insert("ALLOWED_VAR".to_string());

        let hardened = build_hardened_environment(&raw, &allowed);

        // Dangerous variables must be absent
        assert!(!hardened.contains_key("LD_PRELOAD"));
        assert!(!hardened.contains_key("LD_LIBRARY_PATH"));
        assert!(!hardened.contains_key("PROMPT_COMMAND"));
        assert!(!hardened.contains_key("NVIDIA_API_KEY"));

        // Safe variables must be present
        assert_eq!(hardened.get("PATH").unwrap(), "/usr/bin:/bin");
        assert_eq!(hardened.get("ALLOWED_VAR").unwrap(), "safe_value");
        assert_eq!(hardened.get("LC_ALL").unwrap(), "C.UTF-8");
    }

    #[test]
    fn test_build_hardened_environment_with_whitelisted_token() {
        let mut raw = HashMap::new();
        raw.insert("NVIDIA_API_KEY".to_string(), "nvapi-permitted".to_string());
        raw.insert("OPENAI_API_KEY".to_string(), "sk-forbidden".to_string());

        let mut allowed = HashSet::new();
        allowed.insert("NVIDIA_API_KEY".to_string());

        let hardened = build_hardened_environment(&raw, &allowed);
        assert_eq!(hardened.get("NVIDIA_API_KEY").unwrap(), "nvapi-permitted");
        assert!(!hardened.contains_key("OPENAI_API_KEY"));
    }
}
