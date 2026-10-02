//! Secure environment configuration loading (CFG-01, CFG-03).
//!
//! Loads environment variables from `.env` files across the workspace hierarchy
//! into the process environment while strictly preserving credential confidentiality.
//! Secrets (API keys, tokens) are NEVER logged or printed.

use std::path::{Path, PathBuf};

/// Load environment variables from `.env` in the current working directory or repository root.
pub fn load_dotenv() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    load_dotenv_from_workspace(&cwd);
}

/// Load environment variables from `.env` in the specified workspace or its parent directories.
pub fn load_dotenv_from_workspace(workspace_root: &Path) {
    // 1. Check workspace_root/.env
    let candidate = workspace_root.join(".env");
    if candidate.is_file() {
        let _ = load_env_file(&candidate);
    }

    // 2. Check current_dir/.env if different from workspace
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_env = cwd.join(".env");
        if cwd_env.is_file() && cwd_env != candidate {
            let _ = load_env_file(&cwd_env);
        }

        // Also check parent directories of cwd
        let mut curr = cwd.as_path();
        while let Some(parent) = curr.parent() {
            let p_env = parent.join(".env");
            if p_env.is_file() {
                let _ = load_env_file(&p_env);
                break;
            }
            curr = parent;
        }
    }
}

/// Parse and load a specific `.env` file into `std::env` without overwriting existing vars.
fn load_env_file(path: &Path) -> Result<(), std::io::Error> {
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
                // SAFETY: Environment loading occurs at initialization before worker tasks are spawned
                unsafe {
                    std::env::set_var(key, val);
                }
            }
        }
    }
    Ok(())
}

/// Safely resolve NVIDIA API Key from environment variable aliases (`NVIDIA_API_KEY` or `API_KEY_NVIDIA`) without process-wide mutations.
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

/// Safely resolve NVIDIA API Key from standard environment variables.
pub fn get_nvidia_api_key() -> Option<String> {
    get_nvidia_api_key_from_lookup(|k| std::env::var(k))
}

/// Safe diagnostic representation of configured model environment without exposing credentials.
#[derive(Debug, Clone)]
pub struct SafeEnvironmentStatus {
    pub provider_configured: bool,
    pub model_configured: String,
    pub api_key_configured: bool,
}

impl SafeEnvironmentStatus {
    /// Detect currently configured provider and model status safely.
    pub fn probe() -> Self {
        load_dotenv();

        let has_nvidia = get_nvidia_api_key().is_some();

        let model = std::env::var("M31A_MODEL")
            .or_else(|_| std::env::var("NVIDIA_MODEL"))
            .unwrap_or_else(|_| "meta/llama-3.2-11b-vision-instruct".to_string());

        Self {
            provider_configured: has_nvidia,
            model_configured: model,
            api_key_configured: has_nvidia,
        }
    }

    /// Log safe diagnostic status without printing any credentials.
    pub fn log_status(&self) {
        tracing::debug!(
            provider_configured = self.provider_configured,
            model_configured = %self.model_configured,
            api_key_configured = self.api_key_configured,
            "environment status"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_nvidia_api_key_from_lookup_alias_resolution() {
        // Test resolution when NVIDIA_API_KEY is present
        let key1 = get_nvidia_api_key_from_lookup(|k| match k {
            "NVIDIA_API_KEY" => Ok("nv-key-1".to_string()),
            _ => Err(std::env::VarError::NotPresent),
        });
        assert_eq!(key1, Some("nv-key-1".to_string()));

        // Test fallback resolution when only API_KEY_NVIDIA is present
        let key2 = get_nvidia_api_key_from_lookup(|k| match k {
            "API_KEY_NVIDIA" => Ok("nv-key-2".to_string()),
            _ => Err(std::env::VarError::NotPresent),
        });
        assert_eq!(key2, Some("nv-key-2".to_string()));

        // Test preference for NVIDIA_API_KEY over API_KEY_NVIDIA
        let key3 = get_nvidia_api_key_from_lookup(|k| match k {
            "NVIDIA_API_KEY" => Ok("nv-key-primary".to_string()),
            "API_KEY_NVIDIA" => Ok("nv-key-secondary".to_string()),
            _ => Err(std::env::VarError::NotPresent),
        });
        assert_eq!(key3, Some("nv-key-primary".to_string()));

        // Test empty/whitespace filtering
        let key4 = get_nvidia_api_key_from_lookup(|k| match k {
            "NVIDIA_API_KEY" => Ok("   ".to_string()),
            "API_KEY_NVIDIA" => Ok("".to_string()),
            _ => Err(std::env::VarError::NotPresent),
        });
        assert_eq!(key4, None);
    }
}
