//! Parity: environment isolation.
//!
//! Semantic contract (identical on every host): the host environment is
//! untrusted. Children never inherit it wholesale; only an allowlist is
//! propagated, secrets and loader overrides are stripped, and a hostile
//! variable in the parent never reaches the child.

use std::time::Duration;

use m31a::process::env::EnvironmentBuilder;
use m31a::process::supervisor::ProcessSupervisor;

/// Allowlist behavior: PATH/HOME-class variables pass only via the builder.
#[test]
fn parity_env_allowlist_baseline() {
    let dir = tempfile::Builder::new()
        .prefix("m31a-p49-env")
        .tempdir()
        .unwrap();
    let builder = EnvironmentBuilder::new(dir.path());
    let map = builder.build_map();
    assert!(
        map.contains_key("PATH"),
        "PATH has a safe default even when the host provides none"
    );
    assert!(
        map.contains_key("TMPDIR"),
        "TMPDIR has a safe default on every host"
    );
}

/// Dangerous loader variables are rejected when set explicitly.
#[test]
fn parity_env_loader_overrides_rejected() {
    let dir = tempfile::Builder::new()
        .prefix("m31a-p49-env-danger")
        .tempdir()
        .unwrap();
    let mut builder = EnvironmentBuilder::new(dir.path());
    for hostile in [
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "DYLD_INSERT_LIBRARIES",
        "DYLD_LIBRARY_PATH",
        "PYTHONPATH",
        "NODE_OPTIONS",
        "RUBYLIB",
        "PERL5LIB",
        "RUSTC_WRAPPER",
    ] {
        assert!(
            builder.set_var(hostile, "/tmp/evil.so").is_err() || builder.is_forbidden_key(hostile),
            "EQUIVALENT: {hostile} must never enter the child environment"
        );
    }
}

/// Credential-like variables are stripped.
#[test]
fn parity_env_credential_vars_stripped() {
    let dir = tempfile::Builder::new()
        .prefix("m31a-p49-env-cred")
        .tempdir()
        .unwrap();
    let mut builder = EnvironmentBuilder::new(dir.path());
    for secret in [
        "OPENAI_API_KEY",
        "GITHUB_TOKEN",
        "AWS_SECRET_ACCESS_KEY",
        "DATABASE_URL",
        "MY_PASSWORD",
        "SERVICE_AUTH_HEADER",
    ] {
        assert!(
            builder.set_var(secret, "s3cr3t").is_err(),
            "EQUIVALENT: {secret} is forbidden in child environments"
        );
    }
    let map = builder.build_map();
    for k in map.keys() {
        let upper = k.to_ascii_uppercase();
        assert!(
            !upper.contains("API_KEY") && !upper.contains("SECRET") && !upper.contains("TOKEN"),
            "baseline carries no secret-like variable: {k}"
        );
    }
}

/// Hostile parent variables never reach the child: end-to-end through a
/// real child process (`env` output must not contain the poison).
#[tokio::test]
async fn parity_env_hostile_parent_does_not_leak() {
    unsafe {
        std::env::set_var("M31A_P49_POISON_MARKER", "leaked");
        std::env::set_var("LD_PRELOAD", "/tmp/m31a-p49-evil.so");
    }
    let dir = tempfile::Builder::new()
        .prefix("m31a-p49-env-leak")
        .tempdir()
        .unwrap();
    let builder = EnvironmentBuilder::new(dir.path());
    let supervisor = ProcessSupervisor::default();
    #[cfg(unix)]
    let out = supervisor
        .run_command(
            "env",
            &[],
            dir.path(),
            Some(&builder),
            Some(Duration::from_secs(10)),
            None,
        )
        .await
        .expect("env must run");
    #[cfg(not(unix))]
    let out = supervisor
        .run_command(
            "cmd.exe",
            &[
                "/D".to_string(),
                "/S".to_string(),
                "/C".to_string(),
                "set".to_string(),
            ],
            dir.path(),
            Some(&builder),
            Some(Duration::from_secs(10)),
            None,
        )
        .await
        .expect("set must run");
    assert!(
        !out.stdout.contains("M31A_P49_POISON_MARKER"),
        "EQUIVALENT: unlisted host variables never leak into the child"
    );
    assert!(
        !out.stdout.contains("m31a-p49-evil.so"),
        "EQUIVALENT: LD_PRELOAD poison from the parent is stripped"
    );
    unsafe {
        std::env::remove_var("M31A_P49_POISON_MARKER");
        std::env::remove_var("LD_PRELOAD");
    }
}

/// Safe defaults: a child started with the builder sees a usable PATH and a
/// workspace-rooted cwd.
#[tokio::test]
async fn parity_env_safe_defaults_usable() {
    let dir = tempfile::Builder::new()
        .prefix("m31a-p49-env-safe")
        .tempdir()
        .unwrap();
    let builder = EnvironmentBuilder::new(dir.path());
    let supervisor = ProcessSupervisor::default();
    let out = supervisor
        .run_command(
            "echo",
            &["ok".to_string()],
            dir.path(),
            Some(&builder),
            Some(Duration::from_secs(10)),
            None,
        )
        .await
        .expect("child with safe defaults must run");
    assert!(out.stdout.contains("ok"));
}

/// Explicit block list removes even allowlisted keys.
#[test]
fn parity_env_explicit_block_wins() {
    let dir = tempfile::Builder::new()
        .prefix("m31a-p49-env-block")
        .tempdir()
        .unwrap();
    let mut builder = EnvironmentBuilder::new(dir.path());
    builder.set_var("M31A_CUSTOM_OK", "1").unwrap();
    builder.block_key("M31A_CUSTOM_OK");
    assert!(!builder.build_map().contains_key("M31A_CUSTOM_OK"));
    assert!(builder.is_forbidden_key("GIT_DIR"));
    assert!(builder.is_forbidden_key("GIT_WORK_TREE"));
}
