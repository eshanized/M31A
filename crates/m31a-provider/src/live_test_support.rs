//! Centralized credential loading and environment verification for live NVIDIA tests.
//!
//! Enforces:
//! 1. Explicit opt-in via `M31A_RUN_LIVE_NVIDIA_TESTS=1`.
//! 2. Secure loading of `NVIDIA_API_KEY` from process environment or workspace root `.env`.
//! 3. Zero credential leakage: keys are never printed, logged, or recorded in error messages.
//! 4. Exact target constants for the NVIDIA Nemotron 3 Ultra integration.

use std::path::PathBuf;

/// The exact model target for real NVIDIA integration testing.
pub const NVIDIA_NEMOTRON_3_ULTRA: &str = "nvidia/nemotron-3-ultra-550b-a55b";

/// Base URL for NVIDIA NIM inference.
pub const NVIDIA_NIM_BASE_URL: &str = "https://integrate.api.nvidia.com/v1";

/// Verifies that live testing has been explicitly requested via `M31A_RUN_LIVE_NVIDIA_TESTS=1`.
/// Panics with a clear diagnostic if the opt-in environment variable is missing or empty.
pub fn ensure_live_test_opt_in() {
    match std::env::var("M31A_RUN_LIVE_NVIDIA_TESTS") {
        Ok(val) if val.trim() == "1" => {}
        _ => {
            panic!(
                "\n====================================================================\n\
                 [M31A LIVE TEST SAFETY REJECTION]\n\
                 Live NVIDIA NIM tests require explicit opt-in via environment variable:\n\
                     M31A_RUN_LIVE_NVIDIA_TESTS=1\n\
                 Refusing to make external network requests without explicit opt-in.\n\
                 ====================================================================\n"
            );
        }
    }
}

/// Resolves the repository workspace root by searching upwards from `CARGO_MANIFEST_DIR`.
pub fn find_workspace_root() -> Option<PathBuf> {
    let manifest_dir = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => std::env::current_dir().ok()?,
    };

    let mut cur = manifest_dir.clone();
    for _ in 0..10 {
        let cargo_toml = cur.join("Cargo.toml");
        if cargo_toml.is_file()
            && let Ok(content) = std::fs::read_to_string(&cargo_toml)
            && content.contains("[workspace]")
        {
            return Some(cur);
        }
        if !cur.pop() {
            break;
        }
    }

    if manifest_dir.join(".env").is_file() {
        return Some(manifest_dir);
    }

    None
}

/// Parses `NVIDIA_API_KEY` from a `.env` file content without exposing keys.
fn parse_env_content(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let stripped = line.strip_prefix("export ").unwrap_or(line).trim();
        if let Some((k, v)) = stripped.split_once('=')
            && k.trim() == "NVIDIA_API_KEY"
        {
            let val = v.trim().trim_matches('"').trim_matches('\'').trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Centralized loader for the real NVIDIA NIM API key.
///
/// Precedence:
/// 1. Already-set process environment variable `NVIDIA_API_KEY`.
/// 2. Root `.env` file located from `CARGO_MANIFEST_DIR`.
///
/// Enforces:
/// - Explicit opt-in verification.
/// - Never prints or leaks the secret in error messages.
/// - Injects the resolved key into the process environment if loaded from `.env`.
pub fn load_live_test_api_key() -> String {
    ensure_live_test_opt_in();

    // 1. Process environment variable has precedence
    if let Ok(val) = std::env::var("NVIDIA_API_KEY") {
        let trimmed = val.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    // 2. Load from workspace root .env without process environment mutation
    if let Some(root) = find_workspace_root() {
        let env_path = root.join(".env");
        if env_path.is_file()
            && let Ok(content) = std::fs::read_to_string(&env_path)
            && let Some(key) = parse_env_content(&content)
        {
            return key;
        }
    }

    panic!(
        "\n====================================================================\n\
         [M31A CREDENTIAL ERROR]\n\
         NVIDIA_API_KEY was not found in the process environment or root .env.\n\
         Please ensure a valid NVIDIA NIM API credential is provided before running\n\
         live provider integration tests.\n\
         ====================================================================\n"
    );
}

/// Helper for live test configuration options.
#[derive(Debug, Clone)]
pub struct LiveTestConfig {
    pub model: &'static str,
    pub api_base: &'static str,
    pub api_key: String,
    pub short_output_budget: u32,
    pub medium_reasoning_budget: i32,
    pub agent_reasoning_budget: i32,
    pub reasoning_effort: &'static str,
    pub temperature: f32,
    pub top_p: f32,
}

impl LiveTestConfig {
    pub fn load() -> Self {
        let api_key = load_live_test_api_key();
        Self {
            model: NVIDIA_NEMOTRON_3_ULTRA,
            api_base: NVIDIA_NIM_BASE_URL,
            api_key,
            short_output_budget: 512,
            medium_reasoning_budget: 256,
            agent_reasoning_budget: 512,
            reasoning_effort: "medium",
            temperature: 1.0,
            top_p: 0.95,
        }
    }
}
