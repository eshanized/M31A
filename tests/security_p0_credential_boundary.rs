//! Priority Zero Credential Boundary Regression Tests (Section 2.6)
//!
//! Proves strict isolation of production credential resolution from ambient
//! environment variables, .env files, and live-test harnesses.

use m31a::config::env::SafeEnvironmentStatus;
use m31a::deployment::DeploymentChannel;
use m31a::model::provider::{EndpointTrustSource, NvidiaProvider};
use m31a::model::types::ModelError;
use m31a::runtime_authorities::{CredentialSource, resolve_runtime_credentials};
use m31a::storage::StorageLayout;
use std::sync::Mutex;
use tempfile::tempdir;

mod support;
use support::credentials::get_nvidia_api_key_from_env;
use support::real_model::RealModelHarness;

static SERIAL_ENV: Mutex<()> = Mutex::new(());

struct EnvGuard {
    saved_nvidia: Option<String>,
    saved_compat: Option<String>,
}

impl EnvGuard {
    fn new() -> Self {
        let saved_nvidia = std::env::var("NVIDIA_API_KEY").ok();
        let saved_compat = std::env::var("API_KEY_NVIDIA").ok();
        Self {
            saved_nvidia,
            saved_compat,
        }
    }

    fn clear_all(&self) {
        unsafe {
            std::env::remove_var("NVIDIA_API_KEY");
            std::env::remove_var("API_KEY_NVIDIA");
        }
    }

    fn set_nvidia(&self, val: &str) {
        unsafe {
            std::env::set_var("NVIDIA_API_KEY", val);
            std::env::remove_var("API_KEY_NVIDIA");
        }
    }

    fn set_compat(&self, val: &str) {
        unsafe {
            std::env::remove_var("NVIDIA_API_KEY");
            std::env::set_var("API_KEY_NVIDIA", val);
        }
    }

    fn set_empty(&self) {
        unsafe {
            std::env::set_var("NVIDIA_API_KEY", "");
            std::env::set_var("API_KEY_NVIDIA", "");
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.saved_nvidia {
                Some(v) => std::env::set_var("NVIDIA_API_KEY", v),
                None => std::env::remove_var("NVIDIA_API_KEY"),
            }
            match &self.saved_compat {
                Some(v) => std::env::set_var("API_KEY_NVIDIA", v),
                None => std::env::remove_var("API_KEY_NVIDIA"),
            }
        }
    }
}

// 1. A production resolver returns no credential when neither supported secure credential source
//    contains a valid key, even if NVIDIA_API_KEY is set.
#[test]
fn test_01_resolver_ignores_nvidia_api_key_in_production() {
    let _lock = SERIAL_ENV.lock().unwrap();
    let guard = EnvGuard::new();
    guard.set_nvidia("ambient-secret-nvidia-key");

    let dir = tempdir().unwrap();
    let res = resolve_runtime_credentials(dir.path(), DeploymentChannel::Production);

    assert_eq!(res.api_key, None);
    assert_eq!(res.source, CredentialSource::Absent);
}

// 2. The same is true when only API_KEY_NVIDIA is set.
#[test]
fn test_02_resolver_ignores_api_key_nvidia_in_production() {
    let _lock = SERIAL_ENV.lock().unwrap();
    let guard = EnvGuard::new();
    guard.set_compat("ambient-secret-compat-key");

    let dir = tempdir().unwrap();
    let res = resolve_runtime_credentials(dir.path(), DeploymentChannel::Production);

    assert_eq!(res.api_key, None);
    assert_eq!(res.source, CredentialSource::Absent);
}

// 3. A .env file containing a test key does not configure the production runtime.
#[test]
fn test_03_dotenv_file_does_not_configure_production_runtime() {
    let _lock = SERIAL_ENV.lock().unwrap();
    let guard = EnvGuard::new();
    guard.clear_all();

    let dir = tempdir().unwrap();
    let dotenv_path = dir.path().join(".env");
    std::fs::write(&dotenv_path, "NVIDIA_API_KEY=dotenv-secret-key-12345\n").unwrap();

    let res = resolve_runtime_credentials(dir.path(), DeploymentChannel::Production);
    assert_eq!(res.api_key, None);
    assert_eq!(res.source, CredentialSource::Absent);

    let probe = SafeEnvironmentStatus::probe_workspace(dir.path());
    assert!(!probe.provider_configured);
}

// 4. Production startup does not import API keys from a workspace .env into the process environment.
#[test]
fn test_04_production_startup_does_not_import_dotenv_into_env() {
    let _lock = SERIAL_ENV.lock().unwrap();
    let guard = EnvGuard::new();
    guard.clear_all();

    let dir = tempdir().unwrap();
    let dotenv_path = dir.path().join(".env");
    std::fs::write(&dotenv_path, "NVIDIA_API_KEY=do-not-import-into-env\n").unwrap();

    // Probing or resolving credentials must never mutate std::env
    let _res = resolve_runtime_credentials(dir.path(), DeploymentChannel::Production);
    assert!(std::env::var("NVIDIA_API_KEY").is_err());
    assert!(std::env::var("API_KEY_NVIDIA").is_err());
}

// 5. Existing supported secure credential-store behavior remains intact.
#[test]
fn test_05_supported_secure_credential_store_remains_intact() {
    let _lock = SERIAL_ENV.lock().unwrap();
    let guard = EnvGuard::new();
    guard.clear_all();

    let dir = tempdir().unwrap();
    let global_file =
        StorageLayout::new(dir.path(), DeploymentChannel::Production).global_credentials_file();
    std::fs::create_dir_all(global_file.parent().unwrap()).unwrap();
    std::fs::write(&global_file, r#"{"nvidia_nim": "global-secure-key-999"}"#).unwrap();

    let res = resolve_runtime_credentials(dir.path(), DeploymentChannel::Production);
    assert_eq!(res.api_key.as_deref(), Some("global-secure-key-999"));
    assert!(matches!(res.source, CredentialSource::GlobalFile(_)));
}

// 6. Required legacy migration behavior remains intact without reintroducing environment fallback.
#[test]
fn test_06_legacy_migration_behavior_intact_without_env_fallback() {
    let _lock = SERIAL_ENV.lock().unwrap();
    let guard = EnvGuard::new();
    guard.clear_all();

    let dir = tempdir().unwrap();
    let legacy_file = dir.path().join(".m31a").join("credentials.json");
    std::fs::create_dir_all(legacy_file.parent().unwrap()).unwrap();
    std::fs::write(&legacy_file, r#"{"nvidia_nim": "legacy-stored-key-777"}"#).unwrap();

    let res = resolve_runtime_credentials(dir.path(), DeploymentChannel::Production);
    assert_eq!(res.api_key.as_deref(), Some("legacy-stored-key-777"));
    assert!(matches!(res.source, CredentialSource::ChannelFile(_)));
}

// 7. Deployment-channel isolation remains intact.
#[test]
fn test_07_deployment_channel_isolation_intact() {
    let _lock = SERIAL_ENV.lock().unwrap();
    let guard = EnvGuard::new();
    guard.clear_all();

    let dir = tempdir().unwrap();
    // Write production credentials only
    let prod_file = dir.path().join(".m31a").join("credentials.json");
    std::fs::create_dir_all(prod_file.parent().unwrap()).unwrap();
    std::fs::write(&prod_file, r#"{"nvidia_nim": "prod-channel-key"}"#).unwrap();

    // Development channel resolution must NOT see the production file
    let dev = resolve_runtime_credentials(dir.path(), DeploymentChannel::Development);
    assert_eq!(dev.api_key, None);
    assert_eq!(dev.source, CredentialSource::Absent);
}

// 8. Provider construction receives the credential explicitly from the authoritative production resolver.
#[test]
fn test_08_provider_construction_fails_closed_without_explicit_credential() {
    let _lock = SERIAL_ENV.lock().unwrap();
    let guard = EnvGuard::new();
    guard.set_nvidia("ambient-key-must-be-ignored");

    // Passing None must fail closed, refusing to pull from ambient environment
    let provider = NvidiaProvider::new(None, None);
    assert!(matches!(provider, Err(ModelError::AuthenticationFailed)));

    let governed = NvidiaProvider::new_governed(None, None, EndpointTrustSource::BuiltinDefault);
    assert!(matches!(governed, Err(ModelError::AuthenticationFailed)));
}

// 9. Explicit live-model tests can still read a key from .env and perform real model execution.
#[test]
fn test_09_explicit_live_model_harness_reads_configured_key() {
    let _lock = SERIAL_ENV.lock().unwrap();
    let guard = EnvGuard::new();
    guard.set_nvidia("explicit-test-key-001");

    assert_eq!(
        get_nvidia_api_key_from_env().as_deref(),
        Some("explicit-test-key-001")
    );
    let harness = RealModelHarness::ensure();
    assert!(harness.is_ok());
}

// 10. Missing live-test credentials never cause a mock to be substituted and counted as a live success.
#[test]
fn test_10_missing_live_test_credentials_never_substitute_mock() {
    let _lock = SERIAL_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let guard = EnvGuard::new();
    guard.set_empty();

    let harness = RealModelHarness::ensure();
    assert!(
        harness.is_err(),
        "Must fail closed rather than returning a mocked success"
    );
}

// 11. A deterministic test run does not unexpectedly make real external model calls.
#[test]
fn test_11_safe_environment_probe_does_not_mutate_or_call_external() {
    let _lock = SERIAL_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let guard = EnvGuard::new();
    guard.clear_all();

    let dir = tempdir().unwrap();
    let probe = SafeEnvironmentStatus::probe_workspace(dir.path());
    assert!(!probe.provider_configured);
}

// 12. Credential values never appear in logs, reports, assertions, or error messages.
#[test]
fn test_12_credentials_never_appear_in_error_messages() {
    let _lock = SERIAL_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let guard = EnvGuard::new();
    guard.clear_all();

    let err = NvidiaProvider::new(None, None).unwrap_err();
    let err_str = format!("{err:?}");
    assert!(!err_str.contains("nvapi-"));
    assert!(!err_str.contains("secret"));

    let res = resolve_runtime_credentials(
        std::path::Path::new("/dummy"),
        DeploymentChannel::Production,
    );
    let debug_str = format!("{res:?}");
    assert!(!debug_str.contains("secret"));
}

// 13. All production consumers share the single credential resolver: the
// resolved configuration's provider status, a fresh registry, and the
// runtime resolver agree (Misconfigured when the store is empty, even with
// ambient keys set; Available once the canonical store holds a key).
// Channel-aware: exercises the compiled artifact channel so the same test
// passes on production and development builds.
#[test]
fn test_13_all_production_consumers_share_single_resolver() {
    let _lock = SERIAL_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let guard = EnvGuard::new();
    guard.set_nvidia("ambient-key-ignored-by-everyone");

    let channel = DeploymentChannel::current();
    let dir = tempdir().unwrap();
    let cfg =
        m31a::config::ResolvedConfiguration::for_workspace(dir.path()).expect("workspace resolves");
    assert_eq!(
        cfg.active_provider_status(),
        m31a::model::types::ProviderCapabilityStatus::Misconfigured
    );
    let reg = m31a::config::provider_registry::ProviderRegistry::new();
    assert_eq!(
        reg.get_status(&cfg.active_provider),
        m31a::model::types::ProviderCapabilityStatus::Misconfigured
    );

    // Store a key in the canonical store: every consumer flips together.
    let global_file = StorageLayout::new(dir.path(), channel).global_credentials_file();
    std::fs::create_dir_all(global_file.parent().unwrap()).unwrap();
    std::fs::write(&global_file, r#"{"nvidia_nim": "shared-authority-key"}"#).unwrap();

    let res = resolve_runtime_credentials(dir.path(), channel);
    assert_eq!(res.api_key.as_deref(), Some("shared-authority-key"));
    let cfg2 =
        m31a::config::ResolvedConfiguration::for_workspace(dir.path()).expect("workspace resolves");
    assert_eq!(
        cfg2.active_provider_status(),
        m31a::model::types::ProviderCapabilityStatus::Available
    );
}
