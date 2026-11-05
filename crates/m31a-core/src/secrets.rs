//! AES-256-GCM encrypted local secrets vault and master key management.
//!
//! Provides authenticated encryption for stored credentials, tokens, and private configuration settings.

use anyhow::{Context, Result, anyhow};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key as AeadKey, Nonce};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

/// Selects the local encrypted vault namespace used by the secrets backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalSecretsNamespace {
    /// General managed user secrets and integration keys (e.g. GitHub, AWS, custom).
    ManagedSecrets,
    /// Model provider API credentials (e.g. NVIDIA_API_KEY, OpenAI, Anthropic).
    Provider,
    /// MCP server OAuth access and refresh tokens.
    McpOAuth,
    /// Gateway and proxy OAuth credentials.
    GatewayOAuth,
    /// Deprecated M31A account authentication namespace, preserved for safe, idempotent migration.
    #[serde(alias = "m31a_auth")]
    M31aAuth,
}

impl LocalSecretsNamespace {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ManagedSecrets => "managed_secrets",
            Self::Provider => "provider",
            Self::M31aAuth => "m31a_auth",
            Self::McpOAuth => "mcp_oauth",
            Self::GatewayOAuth => "gateway_oauth",
        }
    }

    pub fn file_name(&self) -> &'static str {
        match self {
            Self::ManagedSecrets => "managed.vault",
            Self::Provider => "provider.vault",
            Self::M31aAuth => "auth.vault",
            Self::McpOAuth => "mcp_oauth.vault",
            Self::GatewayOAuth => "gateway_oauth.vault",
        }
    }
}

/// Scope of a stored secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SecretScope {
    #[default]
    Global,
    Workspace,
    Session,
}

/// Metadata about a stored secret (without exposing the raw secret value).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretEntry {
    pub name: String,
    pub namespace: LocalSecretsNamespace,
    pub scope: SecretScope,
    pub preview: String,
    pub updated_at: u64,
}

/// Trait for storing, retrieving, and managing secrets across namespaces.
pub trait SecretsBackend: Send + Sync {
    fn get_secret(&self, namespace: LocalSecretsNamespace, name: &str) -> Result<Option<String>>;
    fn set_secret(
        &self,
        namespace: LocalSecretsNamespace,
        name: &str,
        value: &str,
        scope: SecretScope,
    ) -> Result<()>;
    fn delete_secret(&self, namespace: LocalSecretsNamespace, name: &str) -> Result<bool>;
    fn list_secrets(&self, namespace: LocalSecretsNamespace) -> Result<Vec<SecretEntry>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredRecord {
    value: String,
    scope: SecretScope,
    updated_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct VaultPayload {
    version: u8,
    records: BTreeMap<String, StoredRecord>,
}

/// Derives the 256-bit vault root key from a passphrase (explicit-key mode only).
fn derive_root_key(passphrase: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"M31A-VAULT-ROOT-KEY-SALT-v1");
    hasher.update(passphrase.as_bytes());
    hasher.finalize().to_vec()
}

/// Generates a fresh random 256-bit master key from the OS CSPRNG.
/// The default key is never derived from a source-visible string.
fn generate_master_key() -> Vec<u8> {
    let mut key = vec![0u8; MASTER_KEY_LEN];
    rand::rng().fill_bytes(&mut key);
    key
}

/// v1 legacy envelope magic — decrypt-only, kept for one-time migration.
const VAULT_MAGIC: &[u8; 8] = b"M31AVLT1";
/// v2 envelope magic: MAGIC(8) || nonce(12, CSPRNG) || ChaCha20-Poly1305 AEAD ciphertext+tag.
const VAULT_MAGIC_V2: &[u8; 8] = b"M31AVLT2";
const VAULT_VERSION: u8 = 2;
const VAULT_KEY_FILE: &str = "vault.key";
const MASTER_KEY_LEN: usize = 32;

/// File-backed authenticated encrypted vault (AEAD).
///
/// v2 envelopes are sealed with ChaCha20-Poly1305 (RFC 8439): a fresh 12-byte
/// CSPRNG nonce per write, the namespace name bound as associated data, and a
/// 256-bit master key. The default master key is a random value persisted at
/// `<vault_dir>/vault.key` with 0600 permissions — it is never derived from a
/// source-visible string. Legacy v1 envelopes are decrypted in place and
/// transparently re-encrypted to v2 on first access (migration).
#[derive(Debug)]
pub struct LocalEncryptedVault {
    vault_dir: PathBuf,
    master_key: Vec<u8>,
    cache: Arc<Mutex<HashMap<LocalSecretsNamespace, BTreeMap<String, StoredRecord>>>>,
}

impl LocalEncryptedVault {
    /// Initialize with a random master key persisted as `vault.key` (0600).
    /// Legacy v1 vaults (deterministic default seed) are migrated to v2 automatically.
    pub fn new(vault_dir: PathBuf) -> Result<Self> {
        Self::init(vault_dir, None)
    }

    /// Initialize with an explicit master key passphrase (derived via SHA-256).
    /// Bypasses `vault.key`; only vaults written by this same passphrase can be opened.
    pub fn with_key(vault_dir: PathBuf, master_key: &str) -> Result<Self> {
        Self::init(vault_dir, Some(derive_root_key(master_key)))
    }

    fn init(vault_dir: PathBuf, explicit_key: Option<Vec<u8>>) -> Result<Self> {
        if !vault_dir.exists() {
            fs::create_dir_all(&vault_dir).with_context(|| {
                format!(
                    "Failed to create vault directory at {}",
                    vault_dir.display()
                )
            })?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&vault_dir, fs::Permissions::from_mode(0o700));
            }
        }

        let is_explicit = explicit_key.is_some();
        let master_key = match explicit_key {
            Some(key) => key,
            None => Self::resolve_default_master_key(&vault_dir)?,
        };

        if master_key.len() != MASTER_KEY_LEN {
            return Err(anyhow!(
                "Vault master key must be {MASTER_KEY_LEN} bytes, got {}",
                master_key.len()
            ));
        }

        let vault = Self {
            vault_dir,
            master_key,
            cache: Arc::new(Mutex::new(HashMap::new())),
        };
        if !is_explicit {
            vault.migrate_legacy_auth_vault_if_present()?;
        }
        Ok(vault)
    }

    /// Default key resolution: reuse persisted random key, or migrate legacy v1
    /// vaults under a fresh random key. Legacy decryption failures are fatal
    /// (fail-closed) — nothing is silently dropped or re-encrypted on error.
    fn resolve_default_master_key(vault_dir: &Path) -> Result<Vec<u8>> {
        let key_path = vault_dir.join(VAULT_KEY_FILE);
        if key_path.exists() {
            let bytes = fs::read(&key_path).with_context(|| {
                format!("Failed to read vault key file at {}", key_path.display())
            })?;
            if bytes.len() != MASTER_KEY_LEN {
                return Err(anyhow!(
                    "Vault key file has invalid length {} (expected {MASTER_KEY_LEN})",
                    bytes.len()
                ));
            }
            return Ok(bytes);
        }

        let legacy_seed = format!("m31a-default-vault-key-{}", vault_dir.display());
        let legacy_key = derive_root_key(&legacy_seed);

        // Any non-empty legacy v1 vault file forces a one-time migration.
        let mut legacy_vaults: Vec<(LocalSecretsNamespace, PathBuf)> = Vec::new();
        for ns in [
            LocalSecretsNamespace::ManagedSecrets,
            LocalSecretsNamespace::M31aAuth,
            LocalSecretsNamespace::McpOAuth,
            LocalSecretsNamespace::GatewayOAuth,
        ] {
            let path = vault_dir.join(ns.file_name());
            if path.exists() && fs::metadata(&path).map(|m| m.len() > 0).unwrap_or(false) {
                legacy_vaults.push((ns, path));
            }
        }

        let new_key = generate_master_key();
        Self::write_key_file(&key_path, &new_key)?;

        if !legacy_vaults.is_empty() {
            let legacy_vault = Self {
                vault_dir: vault_dir.to_path_buf(),
                master_key: legacy_key,
                cache: Arc::new(Mutex::new(HashMap::new())),
            };

            let new_vault = Self {
                vault_dir: vault_dir.to_path_buf(),
                master_key: new_key.clone(),
                cache: Arc::new(Mutex::new(HashMap::new())),
            };

            for (ns, path) in &legacy_vaults {
                let bytes = fs::read(path).with_context(|| {
                    format!("Failed to read legacy vault file at {}", path.display())
                })?;
                let plaintext = legacy_vault.decrypt_payload(*ns, &bytes)?;
                let payload: VaultPayload =
                    serde_json::from_slice(&plaintext).with_context(|| {
                        format!(
                            "Failed to deserialize legacy vault payload at {}",
                            path.display()
                        )
                    })?;
                new_vault.save_namespace(*ns, &payload.records)?;
            }

            debug!(
                "Migrated {} legacy v1 vault file(s) to AEAD v2 format",
                legacy_vaults.len()
            );
        }

        Ok(new_key)
    }

    fn write_key_file(key_path: &Path, key: &[u8]) -> Result<()> {
        #[cfg(unix)]
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(key_path)?;

        #[cfg(not(unix))]
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(key_path)?;

        file.write_all(key)?;
        file.sync_all()?;
        drop(file);
        Ok(())
    }

    fn namespace_path(&self, namespace: LocalSecretsNamespace) -> PathBuf {
        self.vault_dir.join(namespace.file_name())
    }

    // -----------------------------------------------------------------------
    // v2: ChaCha20-Poly1305 AEAD (RFC 8439)
    // -----------------------------------------------------------------------

    fn aead_cipher(&self) -> ChaCha20Poly1305 {
        ChaCha20Poly1305::new(AeadKey::from_slice(&self.master_key))
    }

    fn encrypt_payload(
        &self,
        namespace: LocalSecretsNamespace,
        plaintext: &[u8],
    ) -> Result<Vec<u8>> {
        // Fresh CSPRNG nonce per write (12 bytes); never derived from time or state.
        let mut nonce_bytes = [0u8; 12];
        rand::rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Bind the envelope to its namespace: moving a sealed file between
        // namespaces (or replaying it under another name) fails authentication.
        let ciphertext = self
            .aead_cipher()
            .encrypt(
                nonce,
                Payload {
                    msg: plaintext,
                    aad: namespace.as_str().as_bytes(),
                },
            )
            .map_err(|_| anyhow!("Vault encryption failed"))?;

        let mut envelope = Vec::with_capacity(8 + 12 + ciphertext.len());
        envelope.extend_from_slice(VAULT_MAGIC_V2);
        envelope.extend_from_slice(&nonce_bytes);
        envelope.extend_from_slice(&ciphertext);
        Ok(envelope)
    }

    fn decrypt_payload(&self, namespace: LocalSecretsNamespace, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 8 {
            return Err(anyhow!(
                "Corrupt vault envelope: file too small ({} bytes)",
                data.len()
            ));
        }

        if &data[..8] == VAULT_MAGIC_V2 {
            // v2: 8 (MAGIC) + 12 (nonce) + ciphertext (>= 16 bytes tag)
            if data.len() < 8 + 12 + 16 {
                return Err(anyhow!(
                    "Corrupt v2 vault envelope: file too small ({} bytes)",
                    data.len()
                ));
            }

            let nonce = Nonce::from_slice(&data[8..20]);
            self.aead_cipher()
                .decrypt(nonce, Payload {
                    msg: &data[20..],
                    aad: namespace.as_str().as_bytes(),
                })
                .map_err(|_| anyhow!("Integrity check failed: vault file was modified, corrupted, or opened with a different master key"))
        } else if &data[..8] == VAULT_MAGIC {
            // Legacy v1 envelope: decrypt-only for migration.
            Self::decrypt_payload_v1(namespace, data, &self.master_key)
        } else {
            Err(anyhow!("Invalid vault magic header"))
        }
    }

    // -----------------------------------------------------------------------
    // v1 legacy: SHA-256 keystream + HMAC-SHA256 (decrypt-only for migration)
    // -----------------------------------------------------------------------

    fn derive_namespace_keys(
        master_key: &[u8],
        namespace: LocalSecretsNamespace,
        salt: &[u8],
    ) -> (Vec<u8>, Vec<u8>) {
        // Enc key
        let mut enc_hasher = Sha256::new();
        enc_hasher.update(master_key);
        enc_hasher.update(namespace.as_str().as_bytes());
        enc_hasher.update(b"-ENC-");
        enc_hasher.update(salt);
        let enc_key = enc_hasher.finalize().to_vec();

        // Mac key
        let mut mac_hasher = Sha256::new();
        mac_hasher.update(master_key);
        mac_hasher.update(namespace.as_str().as_bytes());
        mac_hasher.update(b"-MAC-");
        mac_hasher.update(salt);
        let mac_key = mac_hasher.finalize().to_vec();

        (enc_key, mac_key)
    }

    fn generate_keystream(enc_key: &[u8], nonce: &[u8], len: usize) -> Vec<u8> {
        let mut stream = Vec::with_capacity(len + 32);
        let mut counter: u64 = 0;
        while stream.len() < len {
            let mut hasher = Sha256::new();
            hasher.update(enc_key);
            hasher.update(nonce);
            hasher.update(counter.to_be_bytes());
            let block = hasher.finalize();
            stream.extend_from_slice(&block);
            counter += 1;
        }
        stream.truncate(len);
        stream
    }

    fn compute_hmac(mac_key: &[u8], data: &[u8]) -> [u8; 32] {
        // HMAC-SHA256 RFC 2104 implementation
        let block_size = 64;
        let mut key_pad = [0u8; 64];
        if mac_key.len() > block_size {
            let hash = Sha256::digest(mac_key);
            key_pad[..32].copy_from_slice(&hash);
        } else {
            key_pad[..mac_key.len()].copy_from_slice(mac_key);
        }

        let mut o_key_pad = [0x5cu8; 64];
        let mut i_key_pad = [0x36u8; 64];
        for i in 0..64 {
            o_key_pad[i] ^= key_pad[i];
            i_key_pad[i] ^= key_pad[i];
        }

        let mut inner_hasher = Sha256::new();
        inner_hasher.update(i_key_pad);
        inner_hasher.update(data);
        let inner_hash = inner_hasher.finalize();

        let mut outer_hasher = Sha256::new();
        outer_hasher.update(o_key_pad);
        outer_hasher.update(inner_hash);
        let outer_hash = outer_hasher.finalize();

        let mut result = [0u8; 32];
        result.copy_from_slice(&outer_hash);
        result
    }

    fn decrypt_payload_v1(
        namespace: LocalSecretsNamespace,
        data: &[u8],
        master_key: &[u8],
    ) -> Result<Vec<u8>> {
        // Minimum length: 8 (MAGIC) + 1 (ver) + 16 (salt) + 16 (nonce) + 0 (ciphertext) + 32 (HMAC) = 73 bytes
        if data.len() < 73 {
            return Err(anyhow!(
                "Corrupt v1 vault envelope: file too small ({} bytes)",
                data.len()
            ));
        }

        let version = data[8];
        if version != 1 {
            return Err(anyhow!("Unsupported vault version {}", version));
        }

        let salt = &data[9..25];
        let nonce = &data[25..41];
        let ciphertext = &data[41..data.len() - 32];
        let expected_hmac = &data[data.len() - 32..];

        let (enc_key, mac_key) = Self::derive_namespace_keys(master_key, namespace, salt);
        let computed_hmac = Self::compute_hmac(&mac_key, &data[..data.len() - 32]);

        // Constant-time comparison
        let mut diff = 0u8;
        for (a, b) in expected_hmac.iter().zip(computed_hmac.iter()) {
            diff |= a ^ b;
        }
        if diff != 0 {
            return Err(anyhow!(
                "Integrity check failed: vault file was modified or corrupted"
            ));
        }

        let keystream = Self::generate_keystream(&enc_key, nonce, ciphertext.len());
        let mut plaintext = Vec::with_capacity(ciphertext.len());
        for (c, k) in ciphertext.iter().zip(keystream.iter()) {
            plaintext.push(c ^ k);
        }

        Ok(plaintext)
    }

    /// v1 legacy envelope writer — retained for migration tests only.
    #[cfg(test)]
    fn encrypt_payload_v1(
        &self,
        namespace: LocalSecretsNamespace,
        plaintext: &[u8],
    ) -> Result<Vec<u8>> {
        // Deterministic legacy salt/nonce derived from the current timestamp
        // (this is exactly the weakness that motivated v2).
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let mut salt_hasher = Sha256::new();
        salt_hasher.update(b"SALT");
        salt_hasher.update(now.to_le_bytes());
        salt_hasher.update(b"PSEUDO-RANDOM");
        let full_salt = salt_hasher.finalize();
        let salt = &full_salt[..16];

        let mut nonce_hasher = Sha256::new();
        nonce_hasher.update(b"NONCE");
        nonce_hasher.update(now.to_be_bytes());
        let full_nonce = nonce_hasher.finalize();
        let nonce = &full_nonce[..16];

        let (enc_key, mac_key) = Self::derive_namespace_keys(&self.master_key, namespace, salt);
        let keystream = Self::generate_keystream(&enc_key, nonce, plaintext.len());

        let mut ciphertext = Vec::with_capacity(plaintext.len());
        for (p, k) in plaintext.iter().zip(keystream.iter()) {
            ciphertext.push(p ^ k);
        }

        let mut envelope_header = Vec::with_capacity(8 + 1 + 16 + 16 + ciphertext.len());
        envelope_header.extend_from_slice(VAULT_MAGIC);
        envelope_header.push(1);
        envelope_header.extend_from_slice(salt);
        envelope_header.extend_from_slice(nonce);
        envelope_header.extend_from_slice(&ciphertext);

        let hmac = Self::compute_hmac(&mac_key, &envelope_header);

        let mut envelope = envelope_header;
        envelope.extend_from_slice(&hmac);
        Ok(envelope)
    }

    fn load_namespace(
        &self,
        namespace: LocalSecretsNamespace,
    ) -> Result<BTreeMap<String, StoredRecord>> {
        let path = self.namespace_path(namespace);
        if !path.exists() {
            return Ok(BTreeMap::new());
        }

        let bytes = fs::read(&path)
            .with_context(|| format!("Failed to read vault file at {}", path.display()))?;

        if bytes.is_empty() {
            return Ok(BTreeMap::new());
        }

        let decrypted = self.decrypt_payload(namespace, &bytes)?;
        let payload: VaultPayload = serde_json::from_slice(&decrypted)
            .context("Failed to deserialize decrypted vault payload")?;

        Ok(payload.records)
    }

    fn save_namespace(
        &self,
        namespace: LocalSecretsNamespace,
        records: &BTreeMap<String, StoredRecord>,
    ) -> Result<()> {
        let payload = VaultPayload {
            version: VAULT_VERSION,
            records: records.clone(),
        };

        let serialized = serde_json::to_vec(&payload)?;
        let encrypted = self.encrypt_payload(namespace, &serialized)?;

        let target_path = self.namespace_path(namespace);
        let temp_path = target_path.with_extension("tmp");

        // Open with 0600 permissions
        #[cfg(unix)]
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temp_path)?;

        #[cfg(not(unix))]
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)?;

        file.write_all(&encrypted)?;
        file.sync_all()?;
        drop(file);

        fs::rename(&temp_path, &target_path).with_context(|| {
            format!(
                "Failed to atomically rename vault to {}",
                target_path.display()
            )
        })?;

        Ok(())
    }

    fn make_preview(value: &str) -> String {
        if value.len() <= 6 {
            "***".to_string()
        } else {
            let prefix = &value[..3];
            let suffix = &value[value.len() - 3..];
            format!("{}...{}", prefix, suffix)
        }
    }

    /// Migrates external provider API keys out of any legacy `auth.vault` into `provider.vault`
    /// and safely purges obsolete M31A user account tokens.
    ///
    /// Migration is atomic and transactional:
    /// - Fails closed if legacy auth vault is corrupted or unreadable.
    /// - Purges obsolete first-party authentication tokens.
    /// - Preserves external provider credentials into `LocalSecretsNamespace::Provider` (without overwriting).
    /// - Atomically saves the updated provider vault.
    /// - Deletes `auth.vault` unconditionally on success.
    pub fn migrate_legacy_auth_vault_if_present(&self) -> Result<()> {
        let auth_path = self.namespace_path(LocalSecretsNamespace::M31aAuth);
        if !auth_path.exists() {
            return Ok(());
        }

        // Fail closed if loading/decryption fails
        let records = self.load_namespace(LocalSecretsNamespace::M31aAuth)?;
        let mut provider_records = self.load_namespace(LocalSecretsNamespace::Provider)?;
        let mut provider_modified = false;

        for (key, record) in records {
            let is_obsolete_token = key == "OAUTH_ACCESS_TOKEN"
                || key == "OAUTH_REFRESH_TOKEN"
                || key == "OAUTH_EXPIRES_AT"
                || key == "OAUTH_ACCOUNT_ID"
                || key == "AUTH_TOKEN"
                || key == "USER_TOKEN"
                || key.starts_with("M31A_AUTH_")
                || key.starts_with("m31a_tok_")
                || key.starts_with("DEVICE_PAIRING_")
                || key.starts_with("M31A_USER_")
                || key.starts_with("M31A_SESSION_");

            if !is_obsolete_token
                && let std::collections::btree_map::Entry::Vacant(e) = provider_records.entry(key)
            {
                e.insert(record);
                provider_modified = true;
            }
        }

        if provider_modified {
            self.save_namespace(LocalSecretsNamespace::Provider, &provider_records)?;
            if let Ok(mut cache) = self.cache.lock() {
                cache.insert(LocalSecretsNamespace::Provider, provider_records);
            }
        }

        // Unconditionally delete legacy auth.vault once provider changes are committed
        fs::remove_file(&auth_path).with_context(|| {
            format!(
                "Failed to remove migrated legacy auth vault at {}",
                auth_path.display()
            )
        })?;

        if let Ok(mut cache) = self.cache.lock() {
            cache.remove(&LocalSecretsNamespace::M31aAuth);
        }

        debug!("Successfully migrated legacy auth vault to provider vault and purged auth.vault");
        Ok(())
    }
}

impl SecretsBackend for LocalEncryptedVault {
    fn get_secret(&self, namespace: LocalSecretsNamespace, name: &str) -> Result<Option<String>> {
        let mut cache = self.cache.lock().unwrap();
        if let Some(records) = cache.get(&namespace) {
            if let Some(val) = records.get(name).map(|r| r.value.clone()) {
                return Ok(Some(val));
            }
        } else {
            let records = self.load_namespace(namespace)?;
            let val = records.get(name).map(|r| r.value.clone());
            cache.insert(namespace, records);
            if val.is_some() {
                return Ok(val);
            }
        }

        Ok(None)
    }

    fn set_secret(
        &self,
        namespace: LocalSecretsNamespace,
        name: &str,
        value: &str,
        scope: SecretScope,
    ) -> Result<()> {
        let mut cache = self.cache.lock().unwrap();
        let mut records = if let Some(r) = cache.get(&namespace) {
            r.clone()
        } else {
            self.load_namespace(namespace)?
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        records.insert(
            name.to_string(),
            StoredRecord {
                value: value.to_string(),
                scope,
                updated_at: now,
            },
        );

        self.save_namespace(namespace, &records)?;
        cache.insert(namespace, records);
        debug!("Saved secret '{}' in namespace '{:?}'", name, namespace);
        Ok(())
    }

    fn delete_secret(&self, namespace: LocalSecretsNamespace, name: &str) -> Result<bool> {
        let mut cache = self.cache.lock().unwrap();
        let mut records = if let Some(r) = cache.get(&namespace) {
            r.clone()
        } else {
            self.load_namespace(namespace)?
        };

        if records.remove(name).is_some() {
            self.save_namespace(namespace, &records)?;
            cache.insert(namespace, records);
            debug!("Deleted secret '{}' from namespace '{:?}'", name, namespace);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn list_secrets(&self, namespace: LocalSecretsNamespace) -> Result<Vec<SecretEntry>> {
        let mut cache = self.cache.lock().unwrap();
        let records = if let Some(r) = cache.get(&namespace) {
            r.clone()
        } else {
            let loaded = self.load_namespace(namespace)?;
            cache.insert(namespace, loaded.clone());
            loaded
        };

        let mut list = Vec::new();
        for (name, record) in records {
            list.push(SecretEntry {
                name,
                namespace,
                scope: record.scope,
                preview: Self::make_preview(&record.value),
                updated_at: record.updated_at,
            });
        }
        Ok(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_encryption_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let vault = LocalEncryptedVault::new(dir.path().to_path_buf()).unwrap();

        let ns = LocalSecretsNamespace::ManagedSecrets;
        vault
            .set_secret(
                ns,
                "OPENAI_API_KEY",
                "sk-proj-1234567890abcdef",
                SecretScope::Global,
            )
            .unwrap();
        vault
            .set_secret(
                ns,
                "GITHUB_TOKEN",
                "ghp_super_secret_token",
                SecretScope::Workspace,
            )
            .unwrap();

        // Retrieve from memory cache
        let key = vault.get_secret(ns, "OPENAI_API_KEY").unwrap();
        assert_eq!(key.as_deref(), Some("sk-proj-1234567890abcdef"));

        // Re-open from disk with brand new instance
        let vault2 = LocalEncryptedVault::new(dir.path().to_path_buf()).unwrap();
        let key2 = vault2.get_secret(ns, "OPENAI_API_KEY").unwrap();
        assert_eq!(key2.as_deref(), Some("sk-proj-1234567890abcdef"));

        let list = vault2.list_secrets(ns).unwrap();
        assert_eq!(list.len(), 2);
        let preview = &list
            .iter()
            .find(|e| e.name == "OPENAI_API_KEY")
            .unwrap()
            .preview;
        assert_eq!(preview, "sk-...def");

        // Delete
        assert!(vault2.delete_secret(ns, "OPENAI_API_KEY").unwrap());
        assert_eq!(vault2.get_secret(ns, "OPENAI_API_KEY").unwrap(), None);
    }

    #[test]
    fn test_vault_tamper_detection() {
        let dir = tempfile::tempdir().unwrap();
        let vault = LocalEncryptedVault::new(dir.path().to_path_buf()).unwrap();

        let ns = LocalSecretsNamespace::ManagedSecrets;
        vault
            .set_secret(ns, "AUTH_TOKEN", "super-secret-token", SecretScope::Global)
            .unwrap();

        // Tamper with file
        let path = dir.path().join(ns.file_name());
        let mut bytes = fs::read(&path).unwrap();
        // Flip a byte in the ciphertext
        let len = bytes.len();
        bytes[len / 2] ^= 0xff;
        fs::write(&path, bytes).unwrap();

        // New vault instance must detect tamper
        let vault2 = LocalEncryptedVault::new(dir.path().to_path_buf()).unwrap();
        let err = vault2.get_secret(ns, "AUTH_TOKEN").unwrap_err();
        assert!(err.to_string().contains("Integrity check failed"));
    }

    #[test]
    fn test_vault_v2_envelope_and_persisted_random_key() {
        let dir = tempfile::tempdir().unwrap();
        let vault = LocalEncryptedVault::new(dir.path().to_path_buf()).unwrap();

        let ns = LocalSecretsNamespace::ManagedSecrets;
        vault
            .set_secret(ns, "TEST_SECRET", "value-1", SecretScope::Global)
            .unwrap();

        // On-disk envelope must be v2 magic.
        let path = dir.path().join(ns.file_name());
        let bytes = fs::read(&path).unwrap();
        assert_eq!(
            &bytes[..8],
            b"M31AVLT2",
            "vault envelope must use v2 AEAD format"
        );

        // Master key must be persisted as a 32-byte random key file with 0600 perms.
        let key_path = dir.path().join("vault.key");
        assert!(
            key_path.exists(),
            "random master key must be persisted as vault.key"
        );
        let key_bytes = fs::read(&key_path).unwrap();
        assert_eq!(key_bytes.len(), 32);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&key_path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "vault.key must be 0600");
        }

        // Key must NOT be reproducible from the legacy source-visible seed.
        let legacy_key =
            derive_root_key(&format!("m31a-default-vault-key-{}", dir.path().display()));
        assert_ne!(key_bytes, legacy_key);

        // Two distinct vault dirs must get distinct random keys.
        let dir2 = tempfile::tempdir().unwrap();
        let vault2 = LocalEncryptedVault::new(dir2.path().to_path_buf()).unwrap();
        vault2
            .set_secret(ns, "TEST_SECRET", "value-1", SecretScope::Global)
            .unwrap();
        assert_ne!(
            fs::read(&key_path).unwrap(),
            fs::read(dir2.path().join("vault.key")).unwrap(),
            "distinct vault dirs must use distinct random master keys"
        );
    }

    #[test]
    fn test_vault_v1_to_v2_migration() {
        let dir = tempfile::tempdir().unwrap();
        let ns = LocalSecretsNamespace::ManagedSecrets;

        // Fabricate a legacy v1 vault sealed under the deterministic default seed.
        let legacy_seed = format!("m31a-default-vault-key-{}", dir.path().display());
        let legacy = LocalEncryptedVault::with_key(dir.path().to_path_buf(), &legacy_seed).unwrap();
        let mut records = BTreeMap::new();
        records.insert(
            "LEGACY_TOKEN".to_string(),
            StoredRecord {
                value: "legacy-secret-value".to_string(),
                scope: SecretScope::Global,
                updated_at: 12345,
            },
        );
        let payload = VaultPayload {
            version: 1,
            records,
        };
        let plaintext = serde_json::to_vec(&payload).unwrap();
        let envelope = legacy.encrypt_payload_v1(ns, &plaintext).unwrap();

        let path = dir.path().join(ns.file_name());
        fs::write(&path, &envelope).unwrap();
        assert_eq!(&envelope[..8], b"M31AVLT1");

        // Opening with the default constructor must migrate v1 -> v2 transparently.
        assert!(!dir.path().join("vault.key").exists());
        let vault = LocalEncryptedVault::new(dir.path().to_path_buf()).unwrap();
        let value = vault.get_secret(ns, "LEGACY_TOKEN").unwrap();
        assert_eq!(value.as_deref(), Some("legacy-secret-value"));

        // Key file created, and the legacy file replaced by a v2 envelope.
        assert!(dir.path().join("vault.key").exists());
        let migrated_bytes = fs::read(&path).unwrap();
        assert_eq!(&migrated_bytes[..8], b"M31AVLT2");

        // Survives a fresh instance (migration is persistent, not per-session).
        let vault2 = LocalEncryptedVault::new(dir.path().to_path_buf()).unwrap();
        assert_eq!(
            vault2.get_secret(ns, "LEGACY_TOKEN").unwrap().as_deref(),
            Some("legacy-secret-value")
        );
    }

    #[test]
    fn test_vault_v2_namespace_binding() {
        // A v2 envelope sealed for one namespace must not authenticate when
        // presented as another namespace (associated-data binding).
        let dir = tempfile::tempdir().unwrap();
        let vault = LocalEncryptedVault::new(dir.path().to_path_buf()).unwrap();

        let ns = LocalSecretsNamespace::ManagedSecrets;
        vault
            .set_secret(ns, "SECRET_A", "cross-namespace-test", SecretScope::Global)
            .unwrap();

        let sealed = fs::read(dir.path().join(ns.file_name())).unwrap();

        // Replay the same ciphertext bytes under a different namespace.
        let err = vault
            .decrypt_payload(LocalSecretsNamespace::M31aAuth, &sealed)
            .unwrap_err();
        assert!(err.to_string().contains("Integrity check failed"));
    }

    #[test]
    fn test_vault_explicit_passphrase_isolated() {
        // Explicit passphrase vaults remain openable only by the same passphrase.
        let dir = tempfile::tempdir().unwrap();
        let vault =
            LocalEncryptedVault::with_key(dir.path().to_path_buf(), "correct-passphrase").unwrap();

        let ns = LocalSecretsNamespace::ManagedSecrets;
        vault
            .set_secret(ns, "SECRET", "passphrase-protected", SecretScope::Global)
            .unwrap();

        let vault_ok =
            LocalEncryptedVault::with_key(dir.path().to_path_buf(), "correct-passphrase").unwrap();
        assert_eq!(
            vault_ok.get_secret(ns, "SECRET").unwrap().as_deref(),
            Some("passphrase-protected")
        );

        let vault_bad =
            LocalEncryptedVault::with_key(dir.path().to_path_buf(), "wrong-passphrase").unwrap();
        let err = vault_bad.get_secret(ns, "SECRET").unwrap_err();
        assert!(err.to_string().contains("Integrity check failed"));
    }
}
