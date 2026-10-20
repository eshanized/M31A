//! Test-only credential helpers for explicitly opted-in live-model testing.
//!
//! This module lives under `tests/` because ONLY test code may load `.env`
//! files or obtain LLM API keys from environment variables. Production code
//! under `src/` resolves credentials exclusively through the canonical
//! channel-isolated credential store
//! (`m31a::runtime_authorities::resolve_runtime_credentials`) and must never
//! call anything in this module.
//!
//! Helpers here must never log credentials, never fabricate credentials, and
//! restore any modified process environment after execution.
//!
//! Shared across many integration-test binaries, each using only a subset:
//! per-binary dead-code lints are suppressed for this helper module.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Load test credentials from a `.env` file in the current working directory
/// or repository root into the process environment (without overwriting
/// existing vars). Test-only: production never loads `.env`.
pub fn load_dotenv_for_tests() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    load_dotenv_for_tests_from_workspace(&cwd);
}

/// Load test credentials from a `.env` file in the given workspace or its
/// parent directories. Test-only: production never loads `.env`.
pub fn load_dotenv_for_tests_from_workspace(workspace_root: &Path) {
    // 1. Check workspace_root/.env
    let candidate = workspace_root.join(".env");
    if candidate.is_file() {
        let _ = load_env_file_for_tests(&candidate);
    }

    // 2. Check current_dir/.env if different from workspace
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_env = cwd.join(".env");
        if cwd_env.is_file() && cwd_env != candidate {
            let _ = load_env_file_for_tests(&cwd_env);
        }

        // Also check parent directories of cwd
        let mut curr = cwd.as_path();
        while let Some(parent) = curr.parent() {
            let p_env = parent.join(".env");
            if p_env.is_file() {
                let _ = load_env_file_for_tests(&p_env);
                break;
            }
            curr = parent;
        }
    }
}

/// Parse a specific `.env` file into the process environment without
/// overwriting existing vars. Test-only helper.
///
/// Narrowed allowlist: ONLY the variables the live-test harness needs are
/// imported (`NVIDIA_API_KEY`, `API_KEY_NVIDIA`, `M31A_MODEL`,
/// `NVIDIA_MODEL`). Every other `.env` entry is ignored so parallel test
/// threads cannot globally mutate unrelated environment state, and secrets
/// for other services are never pulled into the test process.
fn load_env_file_for_tests(path: &Path) -> Result<(), std::io::Error> {
    const ALLOWED: &[&str] = &[
        "NVIDIA_API_KEY",
        "API_KEY_NVIDIA",
        "M31A_MODEL",
        "NVIDIA_MODEL",
    ];
    let content = std::fs::read_to_string(path)?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some((raw_key, raw_val)) = trimmed.split_once('=') {
            let key = raw_key.trim();
            if key.is_empty() {
                continue;
            }
            // Narrow: skip anything outside the live-test allowlist. Never
            // print keys or values here (no credential exposure in logs).
            if !ALLOWED.contains(&key) {
                continue;
            }

            let mut val = raw_val.trim();
            // Strip matching double or single quotes
            if ((val.starts_with('"') && val.ends_with('"'))
                || (val.starts_with('\'') && val.ends_with('\'')))
                && val.len() >= 2
            {
                val = &val[1..val.len() - 1];
            }

            // Only set if not already present in the environment
            if std::env::var(key).is_err() {
                // SAFETY: test-harness startup only; values are suite-wide
                // constants for explicitly opted-in live-model tests.
                unsafe {
                    std::env::set_var(key, val);
                }
            }
        }
    }
    Ok(())
}

/// Sanitized diagnostic for the live-test gate: reports whether a credential
/// is present and which `.env` file supplied it, WITHOUT printing the secret,
/// its length, or any environment dump.
pub fn credential_gate_status() -> String {
    load_dotenv_for_tests();
    let present = get_nvidia_api_key_from_env().is_some_and(|k| !k.trim().is_empty());
    // Locate the repository `.env` without exposing its contents.
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dotenv_path = String::from("<none>");
    for candidate in [
        cwd.join(".env"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".env"),
    ] {
        if candidate.is_file() {
            dotenv_path = candidate.display().to_string();
            break;
        }
    }
    if present {
        format!("CREDENTIAL-OK: NVIDIA key present (source: {dotenv_path})")
    } else {
        format!("CREDENTIAL-BLOCKED: no NVIDIA key in environment or {dotenv_path}")
    }
}

/// Resolve an NVIDIA API key from a caller-supplied lookup (dependency
/// injection for deterministic tests that must not touch the real
/// environment). Trims whitespace and rejects empty values; prefers
/// `NVIDIA_API_KEY` over the `API_KEY_NVIDIA` compatibility alias.
pub fn get_nvidia_api_key_from_lookup<F>(lookup: F) -> Option<String>
where
    F: Fn(&str) -> Result<String, std::env::VarError>,
{
    if let Ok(key) = lookup("NVIDIA_API_KEY") {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Ok(key) = lookup("API_KEY_NVIDIA") {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

/// Resolve the NVIDIA API key from the process environment aliases
/// (`NVIDIA_API_KEY` / `API_KEY_NVIDIA`). Test-only: production code must
/// never call this; it resolves credentials from the canonical store.
pub fn get_nvidia_api_key_from_env() -> Option<String> {
    get_nvidia_api_key_from_lookup(|k| std::env::var(k))
}

/// RAII guard that snapshots a set of environment variables on creation and
/// restores them on drop, so environment-mutating tests cannot leak state
/// into other tests in the same binary.
pub struct EnvGuard {
    saved: Vec<(String, Option<String>)>,
}

impl EnvGuard {
    /// Snapshot the current values of `keys`.
    pub fn capture(keys: &[&str]) -> Self {
        let saved = keys
            .iter()
            .map(|k| ((*k).to_string(), std::env::var(k).ok()))
            .collect();
        Self { saved }
    }

    /// Remove all `keys` from the process environment (for isolation tests).
    pub fn clear(keys: &[&str]) -> Self {
        let guard = Self::capture(keys);
        for k in keys {
            // SAFETY: test isolation only; restored on drop.
            unsafe {
                std::env::remove_var(k);
            }
        }
        guard
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.saved {
            // SAFETY: restoring the exact pre-test environment.
            unsafe {
                match value {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nvidia_api_key_primary_alias_resolution() {
        // verifies that NVIDIA_API_KEY is resolved when present
        let lookup = |k: &str| -> Result<String, std::env::VarError> {
            match k {
                "NVIDIA_API_KEY" => Ok("nvapi-primary-key-12345".to_string()),
                _ => Err(std::env::VarError::NotPresent),
            }
        };
        assert_eq!(
            get_nvidia_api_key_from_lookup(lookup),
            Some("nvapi-primary-key-12345".to_string())
        );
    }

    #[test]
    fn test_api_key_nvidia_secondary_alias_resolution() {
        // verifies that API_KEY_NVIDIA compatibility alias is resolved
        let lookup = |k: &str| -> Result<String, std::env::VarError> {
            match k {
                "API_KEY_NVIDIA" => Ok("nvapi-secondary-key-67890".to_string()),
                _ => Err(std::env::VarError::NotPresent),
            }
        };
        assert_eq!(
            get_nvidia_api_key_from_lookup(lookup),
            Some("nvapi-secondary-key-67890".to_string())
        );
    }

    #[test]
    fn test_nvidia_credential_precedence() {
        // verifies that NVIDIA_API_KEY takes precedence over API_KEY_NVIDIA
        let lookup = |k: &str| -> Result<String, std::env::VarError> {
            match k {
                "NVIDIA_API_KEY" => Ok("nvapi-preferred".to_string()),
                "API_KEY_NVIDIA" => Ok("nvapi-fallback".to_string()),
                _ => Err(std::env::VarError::NotPresent),
            }
        };
        assert_eq!(
            get_nvidia_api_key_from_lookup(lookup),
            Some("nvapi-preferred".to_string())
        );
    }

    #[test]
    fn test_missing_credentials_return_none() {
        // verifies that missing credentials return None cleanly
        let lookup = |_k: &str| -> Result<String, std::env::VarError> {
            Err(std::env::VarError::NotPresent)
        };
        assert_eq!(get_nvidia_api_key_from_lookup(lookup), None);
    }

    #[test]
    fn test_whitespace_credentials_treated_as_missing() {
        // verifies that empty and whitespace-only credentials return None
        let lookup_empty = |k: &str| -> Result<String, std::env::VarError> {
            match k {
                "NVIDIA_API_KEY" => Ok("   \t\n  ".to_string()),
                "API_KEY_NVIDIA" => Ok("".to_string()),
                _ => Err(std::env::VarError::NotPresent),
            }
        };
        assert_eq!(get_nvidia_api_key_from_lookup(lookup_empty), None);
    }

    #[test]
    fn test_env_guard_captures_and_restores_vars() {
        // verifies that EnvGuard safely restores previous state
        unsafe {
            std::env::set_var("TEST_CRED_GUARD_KEY", "initial_val");
        }
        {
            let _guard = EnvGuard::clear(&["TEST_CRED_GUARD_KEY"]);
            assert!(std::env::var("TEST_CRED_GUARD_KEY").is_err());
        }
        assert_eq!(std::env::var("TEST_CRED_GUARD_KEY").unwrap(), "initial_val");
        unsafe {
            std::env::remove_var("TEST_CRED_GUARD_KEY");
        }
    }
}
