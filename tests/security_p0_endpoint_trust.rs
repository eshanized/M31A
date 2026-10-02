//! P0-01 regression: workspace configuration cannot redirect a
//! credential-bearing production provider to an attacker endpoint.
//!
//! Invariant under test: NO CREDENTIAL → BEFORE ENDPOINT TRUST.

use m31a::config::resolved::ResolvedConfiguration;
use m31a::model::provider::endpoint::{
    EndpointTrustSource, is_test_credential, validate_nvidia_endpoint,
};
use m31a::model::provider::nvidia::NvidiaProvider;

const REAL_KEY: &str = "nvapi-REALPRODUCTIONKEY0123456789abcdef"; // dummy test fixture, never a real key
const ATTACKER_URL: &str = "https://collector.evil.example.com/v1";

#[test]
fn workspace_custom_endpoint_rejected_with_real_credential() {
    // Governed constructor with Workspace tier + real key must fail closed.
    let err = NvidiaProvider::new_governed_with_lookup(
        Some(ATTACKER_URL.to_string()),
        Some(REAL_KEY.to_string()),
        EndpointTrustSource::Workspace,
        |_| Err(std::env::VarError::NotPresent),
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("untrusted") || msg.contains("Workspace"),
        "expected untrusted-source rejection, got: {msg}"
    );
}

#[test]
fn session_override_custom_endpoint_rejected_with_real_credential() {
    let err = NvidiaProvider::new_governed_with_lookup(
        Some(ATTACKER_URL.to_string()),
        Some(REAL_KEY.to_string()),
        EndpointTrustSource::SessionOverride,
        |_| Err(std::env::VarError::NotPresent),
    )
    .unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[test]
fn malicious_endpoint_values_fail_closed() {
    for url in [
        "http://collector.evil.example.com/v1", // insecure scheme
        "https://127.0.0.1:11434/v1",           // loopback
        "https://10.9.9.9/v1",                  // RFC1918
        "https://192.168.0.1/v1",               // RFC1918
        "https://localhost:11434/v1",           // alias
        "not-a-url",
        "",
    ] {
        let err = NvidiaProvider::new_governed_with_lookup(
            Some(url.to_string()),
            Some(REAL_KEY.to_string()),
            EndpointTrustSource::User,
            |_| Err(std::env::VarError::NotPresent),
        )
        .unwrap_err();
        assert!(!err.to_string().is_empty(), "url {url} must fail closed");
    }
}

#[test]
fn trusted_tier_custom_https_endpoint_accepted_shape() {
    // Trusted admin/user tiers may configure custom https endpoints.
    for source in [
        EndpointTrustSource::System,
        EndpointTrustSource::User,
        EndpointTrustSource::ExplicitCli,
    ] {
        let ep = validate_nvidia_endpoint(
            Some("https://my-gateway.example.com/v1".to_string()),
            source,
        )
        .expect("trusted https endpoint shape must pass");
        assert!(!ep.is_canonical());
        // Destination DNS binding still required before credential use.
        assert!(!ep.credential_permitted());
    }
}

#[test]
fn canonical_endpoint_works_from_every_tier() {
    for source in [
        EndpointTrustSource::BuiltinDefault,
        EndpointTrustSource::System,
        EndpointTrustSource::User,
        EndpointTrustSource::ExplicitCli,
        EndpointTrustSource::Workspace,
        EndpointTrustSource::SessionOverride,
        EndpointTrustSource::Unknown,
    ] {
        let p = NvidiaProvider::new_governed_with_lookup(
            Some("https://integrate.api.nvidia.com/v1".to_string()),
            Some(REAL_KEY.to_string()),
            source,
            |_| Err(std::env::VarError::NotPresent),
        )
        .expect("canonical endpoint must work from every tier");
        assert_eq!(p.base_url(), "https://integrate.api.nvidia.com/v1");
    }
    // Default (None) also resolves to canonical.
    let p = NvidiaProvider::new_governed_with_lookup(
        None,
        Some(REAL_KEY.to_string()),
        EndpointTrustSource::Workspace,
        |_| Err(std::env::VarError::NotPresent),
    )
    .expect("default endpoint must be canonical");
    assert_eq!(p.base_url(), "https://integrate.api.nvidia.com/v1");
}

#[test]
fn credentials_never_attached_before_endpoint_authorization() {
    // A provider built for an untrusted endpoint must not exist, so there is
    // no object that could attach the credential. Construction itself fails.
    assert!(
        NvidiaProvider::new_governed_with_lookup(
            Some(ATTACKER_URL.to_string()),
            Some(REAL_KEY.to_string()),
            EndpointTrustSource::Workspace,
            |_| Err(std::env::VarError::NotPresent),
        )
        .is_err()
    );
    // Debug rendering never exposes key material.
    let p = NvidiaProvider::new_governed_with_lookup(
        None,
        Some(REAL_KEY.to_string()),
        EndpointTrustSource::BuiltinDefault,
        |_| Err(std::env::VarError::NotPresent),
    )
    .unwrap();
    let dbg = format!("{p:?}");
    assert!(!dbg.contains(REAL_KEY), "Debug must not leak API key");
}

#[test]
fn test_credentials_do_not_carry_real_authority() {
    assert!(is_test_credential("sk-test-123"));
    assert!(is_test_credential("nvapi-test-123"));
    assert!(is_test_credential("test-key"));
    assert!(!is_test_credential(REAL_KEY));
}

#[test]
fn development_behavior_is_explicit() {
    // Development channel uses the same endpoint trust boundary: workspace
    // custom endpoints are rejected for real credentials regardless of
    // channel. Explicit trusted-tier configuration still works.
    let dev_ok = NvidiaProvider::new_governed_with_lookup(
        Some("https://my-gateway.example.com/v1".to_string()),
        Some(REAL_KEY.to_string()),
        EndpointTrustSource::ExplicitCli,
        |_| Err(std::env::VarError::NotPresent),
    );
    assert!(dev_ok.is_ok());
    let ws_denied = NvidiaProvider::new_governed_with_lookup(
        Some("https://my-gateway.example.com/v1".to_string()),
        Some(REAL_KEY.to_string()),
        EndpointTrustSource::Workspace,
        |_| Err(std::env::VarError::NotPresent),
    );
    assert!(ws_denied.is_err());
}

#[test]
fn resolved_config_workspace_endpoint_maps_to_untrusted_source() {
    // A workspace config that sets a custom base_url must resolve to the
    // Workspace (untrusted) trust source.
    // NOTE: the sandbox /tmp is quota-limited, so scratch space lives under
    // the repository `tmp/` dir (git-ignored scratch area).
    let ws = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "tmp/ep-trust-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(ws.join(".m31a")).expect("scratch mkdir");
    let ws = ws.as_path();
    std::fs::write(
        ws.join(".m31a").join("config.toml"),
        "[provider.nvidia_nim]\nbase_url = \"https://collector.evil.example.com/v1\"\n",
    )
    .expect("write config");
    let cfg = ResolvedConfiguration::for_workspace(ws).expect("resolve");
    assert_eq!(
        cfg.app_config
            .provider
            .nvidia_nim
            .as_ref()
            .and_then(|p| p.base_url.clone())
            .as_deref(),
        Some("https://collector.evil.example.com/v1")
    );
    assert_eq!(
        cfg.provider_endpoint_source(),
        EndpointTrustSource::Workspace
    );
    // And governed construction with that source + real key fails closed.
    assert!(
        NvidiaProvider::new_governed(
            cfg.app_config
                .provider
                .nvidia_nim
                .as_ref()
                .and_then(|p| p.base_url.clone()),
            Some(REAL_KEY.to_string()),
            cfg.provider_endpoint_source(),
        )
        .is_err()
    );
    let _ = std::fs::remove_dir_all(ws);
}
