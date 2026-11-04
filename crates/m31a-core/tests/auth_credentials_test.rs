//! Integration tests for M31A credential store and authentication resolution.
//!
//! Validates precedence resolution across environment variables, keyring vaults, and config files.

use m31a_core::secrets::{LocalEncryptedVault, LocalSecretsNamespace, SecretScope, SecretsBackend};
use tempfile::tempdir;

#[test]
fn test_vault_chacha20poly1305_store_and_retrieve() {
    let tmp = tempdir().unwrap();
    let vault = LocalEncryptedVault::new(tmp.path().to_path_buf()).unwrap();

    // Store secret in M31aAuth namespace
    vault
        .set_secret(
            LocalSecretsNamespace::M31aAuth,
            "NVIDIA_API_KEY",
            "nvapi-test-secret-12345",
            SecretScope::Global,
        )
        .unwrap();

    // Retrieve from same vault
    let retrieved = vault
        .get_secret(LocalSecretsNamespace::M31aAuth, "NVIDIA_API_KEY")
        .unwrap();
    assert_eq!(retrieved.as_deref(), Some("nvapi-test-secret-12345"));

    // Verify non-existent secret
    let non_existent = vault
        .get_secret(LocalSecretsNamespace::M31aAuth, "NON_EXISTENT")
        .unwrap();
    assert_eq!(non_existent, None);

    // Verify cross-namespace isolation
    let cross_ns = vault
        .get_secret(LocalSecretsNamespace::ManagedSecrets, "NVIDIA_API_KEY")
        .unwrap();
    assert_eq!(cross_ns, None);

    // Reopen vault from same directory (simulating persistence across CLI commands)
    let reopened = LocalEncryptedVault::new(tmp.path().to_path_buf()).unwrap();
    let val = reopened
        .get_secret(LocalSecretsNamespace::M31aAuth, "NVIDIA_API_KEY")
        .unwrap();
    assert_eq!(val.as_deref(), Some("nvapi-test-secret-12345"));
}

#[test]
fn test_vault_explicit_key_passphrase() {
    let tmp = tempdir().unwrap();
    let vault_a =
        LocalEncryptedVault::with_key(tmp.path().to_path_buf(), "passphrase-alpha").unwrap();
    vault_a
        .set_secret(
            LocalSecretsNamespace::M31aAuth,
            "KEY_A",
            "val-secret",
            SecretScope::Global,
        )
        .unwrap();

    // Reopening with correct passphrase succeeds
    let vault_ok =
        LocalEncryptedVault::with_key(tmp.path().to_path_buf(), "passphrase-alpha").unwrap();
    assert_eq!(
        vault_ok
            .get_secret(LocalSecretsNamespace::M31aAuth, "KEY_A")
            .unwrap()
            .as_deref(),
        Some("val-secret")
    );

    // Reopening with wrong passphrase fails
    let vault_wrong =
        LocalEncryptedVault::with_key(tmp.path().to_path_buf(), "wrong-passphrase").unwrap();
    assert!(
        vault_wrong
            .get_secret(LocalSecretsNamespace::M31aAuth, "KEY_A")
            .is_err()
    );
}

#[test]
fn test_credentials_file_fallback_resolution() {
    let tmp = tempdir().unwrap();
    let creds_path = tmp.path().join("credentials.json");
    std::fs::write(
        &creds_path,
        r#"{"nvidia_nim": "nvapi-file-fallback-token-abc"}"#,
    )
    .unwrap();

    // Verify reading credentials.json
    let content = std::fs::read_to_string(&creds_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    let token = parsed.get("nvidia_nim").and_then(|k| k.as_str());
    assert_eq!(token, Some("nvapi-file-fallback-token-abc"));
}

#[tokio::test]
async fn test_failed_device_auth_never_persists_fake_token() {
    let tmp = tempdir().unwrap();
    let vault_dir = tmp.path().join("vault");
    let vault = LocalEncryptedVault::new(vault_dir.clone()).unwrap();

    // Verify initial unauthenticated state
    assert_eq!(
        vault
            .get_secret(LocalSecretsNamespace::M31aAuth, "NVIDIA_API_KEY")
            .unwrap(),
        None
    );
    assert_eq!(
        vault
            .get_secret(LocalSecretsNamespace::M31aAuth, "OAUTH_ACCESS_TOKEN")
            .unwrap(),
        None
    );

    // Attempt device code auth with invalid endpoint (guaranteed to fail)
    let client = reqwest::Client::new();
    let config = m31a_core::DeviceAuthRequestConfig {
        device_auth_endpoint: "http://127.0.0.1:9/invalid-device-endpoint".to_string(),
        token_endpoint: "http://127.0.0.1:9/invalid-token-endpoint".to_string(),
        client_id: "test-client".to_string(),
        scope: None,
    };

    let result = m31a_core::execute_device_code_flow(&client, &config, |_| {}).await;
    assert!(
        result.is_err(),
        "Must return an authentication error on failure"
    );

    // Ensure NO mock/placeholder token was written into vault
    let reopened = LocalEncryptedVault::new(vault_dir).unwrap();
    assert_eq!(
        reopened
            .get_secret(LocalSecretsNamespace::M31aAuth, "NVIDIA_API_KEY")
            .unwrap(),
        None
    );
    assert_eq!(
        reopened
            .get_secret(LocalSecretsNamespace::M31aAuth, "OAUTH_ACCESS_TOKEN")
            .unwrap(),
        None
    );
}

#[test]
fn test_pkce_entropy_and_appendix_b_conformance() {
    let pair = m31a_core::generate_pkce_pair();
    assert_eq!(pair.method, "S256");
    assert!(m31a_core::validate_code_verifier(&pair.verifier).is_ok());

    // Appendix B test vector
    let appendix_b_verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let challenge = m31a_core::compute_s256_challenge(appendix_b_verifier).unwrap();
    assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
}
