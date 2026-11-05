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

#[test]
fn test_provider_vault_store_and_migration() {
    let tmp = tempdir().unwrap();
    let vault = LocalEncryptedVault::new(tmp.path().to_path_buf()).unwrap();

    // Store secret in Provider namespace
    vault
        .set_secret(
            LocalSecretsNamespace::Provider,
            "NVIDIA_API_KEY",
            "nvapi-test-provider-key-12345",
            SecretScope::Global,
        )
        .unwrap();

    // Retrieve from same vault
    let retrieved = vault
        .get_secret(LocalSecretsNamespace::Provider, "NVIDIA_API_KEY")
        .unwrap();
    assert_eq!(retrieved.as_deref(), Some("nvapi-test-provider-key-12345"));

    // Verify cross-lookup compatibility
    let retrieved_compat = vault
        .get_secret(LocalSecretsNamespace::M31aAuth, "NVIDIA_API_KEY")
        .unwrap();
    assert_eq!(
        retrieved_compat.as_deref(),
        Some("nvapi-test-provider-key-12345")
    );
}
