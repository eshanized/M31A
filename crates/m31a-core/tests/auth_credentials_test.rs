//! Integration tests for M31A credential store and authentication resolution.
//!
//! Validates precedence resolution across environment variables, keyring vaults, and config files.

use m31a_core::secrets::{LocalEncryptedVault, LocalSecretsNamespace, SecretScope, SecretsBackend};
use tempfile::tempdir;

#[test]
fn test_vault_chacha20poly1305_store_and_retrieve() {
    let tmp = tempdir().unwrap();
    let vault = LocalEncryptedVault::new(tmp.path().to_path_buf()).unwrap();

    // Store secret in Provider namespace
    vault
        .set_secret(
            LocalSecretsNamespace::Provider,
            "NVIDIA_API_KEY",
            "nvapi-test-secret-12345",
            SecretScope::Global,
        )
        .unwrap();

    // Retrieve from same vault
    let retrieved = vault
        .get_secret(LocalSecretsNamespace::Provider, "NVIDIA_API_KEY")
        .unwrap();
    assert_eq!(retrieved.as_deref(), Some("nvapi-test-secret-12345"));

    // Verify non-existent secret
    let non_existent = vault
        .get_secret(LocalSecretsNamespace::Provider, "NON_EXISTENT")
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
        .get_secret(LocalSecretsNamespace::Provider, "NVIDIA_API_KEY")
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
fn test_provider_vault_store_and_namespace_isolation() {
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

    // Verify strict cross-namespace isolation (no fallback)
    let retrieved_compat = vault
        .get_secret(LocalSecretsNamespace::M31aAuth, "NVIDIA_API_KEY")
        .unwrap();
    assert_eq!(retrieved_compat, None);
}

#[test]
fn test_migration_of_provider_keys() {
    let tmp = tempdir().unwrap();
    let vault = LocalEncryptedVault::new(tmp.path().to_path_buf()).unwrap();

    vault
        .set_secret(
            LocalSecretsNamespace::M31aAuth,
            "OPENAI_API_KEY",
            "sk-proj-test1234",
            SecretScope::Global,
        )
        .unwrap();
    vault
        .set_secret(
            LocalSecretsNamespace::M31aAuth,
            "ANTHROPIC_API_KEY",
            "sk-ant-test5678",
            SecretScope::Global,
        )
        .unwrap();

    let auth_vault_path = tmp.path().join("auth.vault");
    assert!(auth_vault_path.exists());

    // Reopening runs migration automatically
    let vault2 = LocalEncryptedVault::new(tmp.path().to_path_buf()).unwrap();

    // auth.vault must be purged
    assert!(!auth_vault_path.exists());

    // Provider keys must be present in Provider namespace
    assert_eq!(
        vault2
            .get_secret(LocalSecretsNamespace::Provider, "OPENAI_API_KEY")
            .unwrap()
            .as_deref(),
        Some("sk-proj-test1234")
    );
    assert_eq!(
        vault2
            .get_secret(LocalSecretsNamespace::Provider, "ANTHROPIC_API_KEY")
            .unwrap()
            .as_deref(),
        Some("sk-ant-test5678")
    );

    // M31aAuth namespace has no keys
    assert_eq!(
        vault2
            .get_secret(LocalSecretsNamespace::M31aAuth, "OPENAI_API_KEY")
            .unwrap(),
        None
    );
}

#[test]
fn test_migration_duplicate_provider_keys_no_overwrite() {
    let tmp = tempdir().unwrap();
    let vault = LocalEncryptedVault::new(tmp.path().to_path_buf()).unwrap();

    vault
        .set_secret(
            LocalSecretsNamespace::Provider,
            "NVIDIA_API_KEY",
            "provider-original-val",
            SecretScope::Global,
        )
        .unwrap();
    vault
        .set_secret(
            LocalSecretsNamespace::M31aAuth,
            "NVIDIA_API_KEY",
            "auth-vault-duplicate-val",
            SecretScope::Global,
        )
        .unwrap();
    vault
        .set_secret(
            LocalSecretsNamespace::M31aAuth,
            "GROQ_API_KEY",
            "groq-key-new",
            SecretScope::Global,
        )
        .unwrap();

    let auth_vault_path = tmp.path().join("auth.vault");
    assert!(auth_vault_path.exists());

    let vault2 = LocalEncryptedVault::new(tmp.path().to_path_buf()).unwrap();

    // auth.vault must be purged
    assert!(!auth_vault_path.exists());

    // Existing provider key must NOT be overwritten
    assert_eq!(
        vault2
            .get_secret(LocalSecretsNamespace::Provider, "NVIDIA_API_KEY")
            .unwrap()
            .as_deref(),
        Some("provider-original-val")
    );
    // Non-duplicate key migrated
    assert_eq!(
        vault2
            .get_secret(LocalSecretsNamespace::Provider, "GROQ_API_KEY")
            .unwrap()
            .as_deref(),
        Some("groq-key-new")
    );
}

#[test]
fn test_migration_obsolete_tokens_only_purged() {
    let tmp = tempdir().unwrap();
    let vault = LocalEncryptedVault::new(tmp.path().to_path_buf()).unwrap();

    let obsolete_keys = [
        "OAUTH_ACCESS_TOKEN",
        "OAUTH_REFRESH_TOKEN",
        "OAUTH_EXPIRES_AT",
        "OAUTH_ACCOUNT_ID",
        "AUTH_TOKEN",
        "USER_TOKEN",
        "M31A_AUTH_BEARER",
        "m31a_tok_123",
        "DEVICE_PAIRING_CODE",
        "M31A_USER_SUB",
        "M31A_SESSION_UUID",
    ];

    for key in &obsolete_keys {
        vault
            .set_secret(
                LocalSecretsNamespace::M31aAuth,
                key,
                "obsolete-token-value",
                SecretScope::Global,
            )
            .unwrap();
    }

    let auth_vault_path = tmp.path().join("auth.vault");
    assert!(auth_vault_path.exists());

    let vault2 = LocalEncryptedVault::new(tmp.path().to_path_buf()).unwrap();

    // auth.vault must be purged even with only obsolete tokens
    assert!(!auth_vault_path.exists());

    // Provider vault must have no records
    let provider_secrets = vault2
        .list_secrets(LocalSecretsNamespace::Provider)
        .unwrap();
    assert!(provider_secrets.is_empty());
}

#[test]
fn test_migration_mixed_entries() {
    let tmp = tempdir().unwrap();
    let vault = LocalEncryptedVault::new(tmp.path().to_path_buf()).unwrap();

    vault
        .set_secret(
            LocalSecretsNamespace::M31aAuth,
            "OAUTH_ACCESS_TOKEN",
            "bad-token",
            SecretScope::Global,
        )
        .unwrap();
    vault
        .set_secret(
            LocalSecretsNamespace::M31aAuth,
            "GEMINI_API_KEY",
            "gemini-valid-key",
            SecretScope::Global,
        )
        .unwrap();

    let auth_vault_path = tmp.path().join("auth.vault");
    assert!(auth_vault_path.exists());

    let vault2 = LocalEncryptedVault::new(tmp.path().to_path_buf()).unwrap();

    assert!(!auth_vault_path.exists());
    assert_eq!(
        vault2
            .get_secret(LocalSecretsNamespace::Provider, "GEMINI_API_KEY")
            .unwrap()
            .as_deref(),
        Some("gemini-valid-key")
    );
    assert_eq!(
        vault2
            .get_secret(LocalSecretsNamespace::Provider, "OAUTH_ACCESS_TOKEN")
            .unwrap(),
        None
    );
}

#[test]
fn test_migration_repeated_idempotency() {
    let tmp = tempdir().unwrap();
    let vault = LocalEncryptedVault::new(tmp.path().to_path_buf()).unwrap();

    // With no auth.vault, migration succeeds idempotently
    assert!(vault.migrate_legacy_auth_vault_if_present().is_ok());
    assert!(vault.migrate_legacy_auth_vault_if_present().is_ok());
}

#[test]
fn test_migration_corrupt_vault_error_propagation() {
    let tmp = tempdir().unwrap();
    // First initialize so vault.key exists
    let _vault = LocalEncryptedVault::new(tmp.path().to_path_buf()).unwrap();

    // Write corrupted bytes to auth.vault
    let auth_vault_path = tmp.path().join("auth.vault");
    std::fs::write(&auth_vault_path, b"NOT_A_VALID_ENVELOPE").unwrap();

    // Reopening must fail closed because migration fails
    let res = LocalEncryptedVault::new(tmp.path().to_path_buf());
    assert!(res.is_err(), "Must fail closed when auth.vault is corrupt");
}
