//! Architecture enforcement: production credential isolation, dotenv
//! prohibition, and silent-fallback prohibition (M31A production-credential
//! remediation).
//!
//! Static part: walks every production Rust file under `src/` and fails on:
//! - dotenv-loading functions, parser implementations, or dotenv
//!   dependencies used for credential loading;
//! - reads/writes of LLM API-key environment variables (`NVIDIA_API_KEY`,
//!   `API_KEY_NVIDIA`), directly or through helpers, INCLUDING `#[cfg(test)]`
//!   modules inside `src/` (a unit-test module embedded in `src/` is not an
//!   exception to the directory boundary);
//! - references from production modules into test-only credential-loading
//!   helpers (`tests/support/`);
//! - silent hardcoded production model-ID fallbacks that could replace a
//!   missing explicit or discovered model;
//! - production-reachable mock-provider execution paths.
//!
//! Legitimate exceptions (understood, not hidden): comment lines, trailing
//! unit-test modules for *literal* scans, and generic secret-redaction /
//! forbidden-key enforcement lines (matched by marker, never by aliasing
//! the prohibited behavior).

use std::path::{Path, PathBuf};

fn src_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("src dir must be readable") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(path);
        }
    }
}

/// Byte offset (line index) where the trailing `mod tests` unit-test module
/// starts, if any. Literal scans exempt normative fixtures past this marker;
/// environment-*operation* scans do not (no exception in `src/`).
fn trailing_test_mod_start(lines: &[&str], content: &str) -> Option<usize> {
    if content.contains("#[cfg(test)]") {
        lines
            .iter()
            .position(|l| l.trim_start().starts_with("mod tests"))
    } else {
        None
    }
}

fn is_comment_line(trimmed: &str) -> bool {
    trimmed.starts_with("//")
}

/// Generic secret-redaction / forbidden-key enforcement markers. A code line
/// carrying an LLM API-key literal is legitimate ONLY when it enforces
/// exclusion (never discovery).
fn is_redaction_enforcement_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    [
        "forbidden",
        "redact",
        "mask",
        "secret",
        "secur",
        "sensitive",
        "sanitiz",
        "scrub",
        "exclud",
        "prohibit",
        "denied",
        "reject",
        "strip",
        "is_secret",
        "is_forbidden",
    ]
    .iter()
    .any(|m| lower.contains(m))
}

// ── 1. No dotenv loading in production ──────────────────────────────────────

#[test]
fn production_src_has_no_dotenv_credential_loading() {
    let mut files = Vec::new();
    collect_rs_files(&src_dir(), &mut files);
    let mut violations = Vec::new();
    for path in &files {
        let content = std::fs::read_to_string(path).expect("read src file");
        for line in content.lines() {
            let trimmed = line.trim();
            if is_comment_line(trimmed) {
                continue;
            }
            // `load_dotenv*` / `load_env_file` free functions, dotenv crates.
            // (Test-support loaders live under `tests/support/` and are
            // deliberately named `*_for_tests` so this scan catches any
            // production reintroduction by name.)
            if trimmed.contains("load_dotenv")
                || trimmed.contains("load_env_file")
                || trimmed.contains("dotenv::")
                || trimmed.contains("dotenvy::")
                || trimmed.contains("load_dotenv_for_tests")
            {
                violations.push(format!("{} :: {}", path.display(), trimmed));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "production src/ must not load .env files:\n{}",
        violations.join("\n")
    );

    // No dotenv dependency in the production dependency table either.
    let manifest =
        std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
            .expect("read Cargo.toml");
    let in_deps = manifest
        .split("[dev-dependencies]")
        .next()
        .unwrap_or(&manifest);
    assert!(
        !in_deps.contains("dotenv"),
        "Cargo dependencies must not include a dotenv crate"
    );
}

// ── 2. No LLM API-key environment operations (zero tolerance, incl. tests) ──

#[test]
fn production_src_never_touches_credential_environment() {
    let mut files = Vec::new();
    collect_rs_files(&src_dir(), &mut files);
    let mut violations = Vec::new();
    for path in &files {
        let content = std::fs::read_to_string(path).expect("read src file");
        for line in content.lines() {
            let trimmed = line.trim();
            if is_comment_line(trimmed) {
                continue;
            }
            // Direct process-environment operations naming a credential var.
            let names = trimmed.contains("NVIDIA_API_KEY") || trimmed.contains("API_KEY_NVIDIA");
            let ops = trimmed.contains("std::env::var")
                || trimmed.contains("std::env::var_os")
                || trimmed.contains("std::env::set_var")
                || trimmed.contains("std::env::remove_var")
                || trimmed.contains("env::var(")
                || trimmed.contains("env::set_var")
                || trimmed.contains("env::remove_var");
            if names && ops {
                violations.push(format!("{} :: {}", path.display(), trimmed));
                continue;
            }
            // Credential-discovery helpers (production must use the canonical
            // resolver; env-backed getters live under tests/support/).
            if trimmed.contains("get_nvidia_api_key") {
                violations.push(format!("{} :: {}", path.display(), trimmed));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "production src/ (including #[cfg(test)] modules) must never read/write LLM API-key environment variables:\n{}",
        violations.join("\n")
    );
}

// ── 3. No stray credential literals outside redaction enforcement ───────────

#[test]
fn production_src_has_no_credential_literals_outside_redaction() {
    let mut files = Vec::new();
    collect_rs_files(&src_dir(), &mut files);
    let mut violations = Vec::new();
    for path in &files {
        let content = std::fs::read_to_string(path).expect("read src file");
        let lines: Vec<&str> = content.lines().collect();
        let test_start = trailing_test_mod_start(&lines, &content);
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if is_comment_line(trimmed) {
                continue;
            }
            if let Some(start) = test_start
                && idx >= start
            {
                continue;
            }
            if (trimmed.contains("NVIDIA_API_KEY") || trimmed.contains("API_KEY_NVIDIA"))
                && !is_redaction_enforcement_line(trimmed)
            {
                violations.push(format!("{} :: {}", path.display(), trimmed));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "LLM API-key literals in production src/ must enforce exclusion (redaction/forbidden lists), never discovery:\n{}",
        violations.join("\n")
    );
}

// ── 4. No production references into test-only credential helpers ──────────

#[test]
fn production_does_not_reference_test_credential_helpers() {
    let mut files = Vec::new();
    collect_rs_files(&src_dir(), &mut files);
    let mut violations = Vec::new();
    for path in &files {
        let content = std::fs::read_to_string(path).expect("read src file");
        for line in content.lines() {
            let trimmed = line.trim();
            if is_comment_line(trimmed) {
                continue;
            }
            if trimmed.contains("testing::real_model")
                || trimmed.contains("support::credentials")
                || trimmed.contains("support::real_model")
                || trimmed.contains("probe_live_test")
            {
                violations.push(format!("{} :: {}", path.display(), trimmed));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "production src/ must not reference test-only credential helpers:\n{}",
        violations.join("\n")
    );
}

// ── 5. No silent hardcoded production model fallbacks ───────────────────────

#[test]
fn production_has_no_silent_hardcoded_model_fallback() {
    let mut files = Vec::new();
    collect_rs_files(&src_dir(), &mut files);
    let mut violations = Vec::new();
    for path in &files {
        let content = std::fs::read_to_string(path).expect("read src file");
        let lines: Vec<&str> = content.lines().collect();
        let test_start = trailing_test_mod_start(&lines, &content);
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if is_comment_line(trimmed) {
                continue;
            }
            if let Some(start) = test_start
                && idx >= start
            {
                continue;
            }
            // Removed test-only registry symbols must never return.
            if trimmed.contains("CANONICAL_REAL_MODEL_")
                || trimmed.contains("resolve_real_model_id")
            {
                violations.push(format!("{} :: {}", path.display(), trimmed));
                continue;
            }
            // Bare model-ID fallbacks outside the resolver/catalog/wizard
            // selection authorities. (Tier-0 documented defaults in
            // config/canonical.rs + schema.rs, descriptor metadata, and
            // validation bounds are the explicit authority, not silent
            // fallbacks, and are exempt by file below.)
            let p = path.to_string_lossy().replace('\\', "/");
            let is_authority = p.ends_with("config/canonical.rs")
                || p.ends_with("config/schema.rs")
                || p.ends_with("config/resolved.rs")
                || p.ends_with("config/provider_registry.rs")
                || p.ends_with("config/mod.rs")
                || p.ends_with("agent/profile.rs")
                || p.ends_with("agent/registry/builtins.rs");
            if !is_authority
                && (trimmed.contains("unwrap_or(\"meta/")
                    || trimmed.contains("unwrap_or_else(|| \"meta/")
                    || trimmed.contains("unwrap_or(\"nvidia/")
                    || trimmed.contains("unwrap_or_else(|| \"nvidia/"))
            {
                violations.push(format!("{} :: {}", path.display(), trimmed));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "production src/ must not silently fall back to hardcoded model IDs:\n{}",
        violations.join("\n")
    );
}

// ── 6. No mock-provider execution path in production ────────────────────────

#[test]
fn production_has_no_mock_provider_execution_path() {
    let mut files = Vec::new();
    collect_rs_files(&src_dir(), &mut files);
    let mut violations = Vec::new();
    for path in &files {
        let p = path.to_string_lossy().replace('\\', "/");
        // The mock provider module itself is legitimate test infrastructure.
        if p.ends_with("model/provider/mock.rs") || p.contains("src/testing/") {
            continue;
        }
        let content = std::fs::read_to_string(path).expect("read src file");
        let lines: Vec<&str> = content.lines().collect();
        let test_start = trailing_test_mod_start(&lines, &content);
        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if is_comment_line(trimmed) {
                continue;
            }
            if let Some(start) = test_start
                && idx >= start
            {
                continue;
            }
            if (trimmed.contains("MockProvider")
                || trimmed.contains("mock::MockProvider")
                || trimmed.contains("ScriptedAutonomousRepairModel"))
                && !is_redaction_enforcement_line(trimmed)
            {
                // Introspection that merely *names* the test-double type for
                // truthful reporting (e.g. `model_provider_is_test_double`
                // docs handled above as comments) is not an execution path;
                // construction or dispatch through it is.
                if trimmed.contains("MockProvider::new")
                    || trimmed.contains("Arc::new(MockProvider")
                    || trimmed.contains("ScriptedAutonomousRepairModel::new")
                {
                    violations.push(format!("{} :: {}", path.display(), trimmed));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "production src/ must not construct mock providers:\n{}",
        violations.join("\n")
    );
}

// ── 7. Dynamic cross-checks: implementation (not just scan) is clean ────────

static SERIAL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct RestoreGuard {
    saved: Vec<(String, Option<String>)>,
}

impl RestoreGuard {
    fn capture() -> Self {
        let saved = ["NVIDIA_API_KEY", "API_KEY_NVIDIA"]
            .iter()
            .map(|k| ((*k).to_string(), std::env::var(k).ok()))
            .collect();
        Self { saved }
    }
}

impl Drop for RestoreGuard {
    fn drop(&mut self) {
        for (key, value) in &self.saved {
            unsafe {
                match value {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}

#[test]
fn resolver_ignores_ambient_env_and_dotenv() {
    let _lock = SERIAL_ENV.lock().unwrap();
    let _restore = RestoreGuard::capture();
    unsafe {
        std::env::set_var("NVIDIA_API_KEY", "ambient-must-be-ignored");
        std::env::remove_var("API_KEY_NVIDIA");
    }

    let dir = tempfile::tempdir().expect("tempdir");
    // A .env file with a key must not configure production either.
    std::fs::write(dir.path().join(".env"), "NVIDIA_API_KEY=dotenv-key\n").expect("write .env");
    let res = m31a::runtime_authorities::resolve_runtime_credentials(
        dir.path(),
        m31a::deployment::DeploymentChannel::current(),
    );
    assert_eq!(res.api_key, None);
    assert_eq!(
        res.source,
        m31a::runtime_authorities::CredentialSource::Absent
    );
    // Probing must not import the .env into the process environment.
    let _ = m31a::config::env::SafeEnvironmentStatus::probe_workspace(dir.path());
    assert_eq!(
        std::env::var("NVIDIA_API_KEY").as_deref(),
        Ok("ambient-must-be-ignored"),
        "probe must leave the ambient environment untouched"
    );
}

#[test]
fn provider_construction_fails_closed_despite_ambient_env() {
    let _lock = SERIAL_ENV.lock().unwrap();
    let _restore = RestoreGuard::capture();
    unsafe {
        std::env::set_var("NVIDIA_API_KEY", "ambient-fake-key-12345");
        std::env::set_var("API_KEY_NVIDIA", "ambient-fake-key-67890");
    }

    assert_eq!(
        m31a::model::provider::nvidia::NvidiaProvider::new(None, None).unwrap_err(),
        m31a::model::types::ModelError::AuthenticationFailed
    );
    assert_eq!(
        m31a::model::provider::nvidia::NvidiaProvider::new_governed(
            None,
            None,
            m31a::model::provider::endpoint::EndpointTrustSource::BuiltinDefault,
        )
        .unwrap_err(),
        m31a::model::types::ModelError::AuthenticationFailed
    );
    // Registry status with no bound credential is Misconfigured even when
    // ambient keys exist (no silent ambient recovery).
    let reg = m31a::config::provider_registry::ProviderRegistry::new();
    assert_eq!(
        reg.get_status("nvidia_nim"),
        m31a::model::types::ProviderCapabilityStatus::Misconfigured
    );
}
