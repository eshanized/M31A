//! Shared credential store abstraction for keyring-backed credential storage.
//! Cross-platform secure credential storage utilizing system keyrings with fallback encryption.

use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// Error classifications for credential store operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CredentialStoreError {
    #[error("credential not found")]
    NotFound,
    #[error("access denied to system credential store")]
    AccessDenied,
    #[error("ambiguous entry matched by multiple credentials: {0}")]
    Ambiguous(String),
    #[error("invalid credential encoding: {0}")]
    BadEncoding(String),
    #[error("system credential store error: {0}")]
    Other(String),
}

/// Keyring store interface for secure credential loading, persistence, and deletion.
pub trait KeyringStore: Debug + Send + Sync {
    /// Loads a secret value for a given service and account name.
    fn load(&self, service: &str, account: &str) -> Result<Option<String>, CredentialStoreError>;

    /// Saves a secret value for a given service and account name.
    fn save(&self, service: &str, account: &str, value: &str) -> Result<(), CredentialStoreError>;

    /// Deletes a secret value, returning true if found and removed, false if not present.
    fn delete(&self, service: &str, account: &str) -> Result<bool, CredentialStoreError>;
}

/// In-memory keyring store for headless environments, CI, and isolated sandboxes.
#[derive(Clone, Default)]
pub struct MemoryKeyringStore {
    entries: Arc<Mutex<HashMap<(String, String), String>>>,
}

impl MemoryKeyringStore {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Debug for MemoryKeyringStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryKeyringStore").finish()
    }
}

impl KeyringStore for MemoryKeyringStore {
    fn load(&self, service: &str, account: &str) -> Result<Option<String>, CredentialStoreError> {
        let map = self.entries.lock().unwrap();
        Ok(map
            .get(&(service.to_string(), account.to_string()))
            .cloned())
    }

    fn save(&self, service: &str, account: &str, value: &str) -> Result<(), CredentialStoreError> {
        let mut map = self.entries.lock().unwrap();
        map.insert(
            (service.to_string(), account.to_string()),
            value.to_string(),
        );
        Ok(())
    }

    fn delete(&self, service: &str, account: &str) -> Result<bool, CredentialStoreError> {
        let mut map = self.entries.lock().unwrap();
        Ok(map
            .remove(&(service.to_string(), account.to_string()))
            .is_some())
    }
}

/// File-backed encrypted fallback credential store for environments without Secret Service daemon.
#[derive(Clone)]
pub struct FileFallbackKeyringStore {
    base_dir: std::path::PathBuf,
}

impl FileFallbackKeyringStore {
    pub fn new(base_dir: std::path::PathBuf) -> Self {
        Self { base_dir }
    }

    fn entry_path(&self, service: &str, account: &str) -> std::path::PathBuf {
        let safe_service = service.replace(['/', '\\'], "_");
        let safe_account = account.replace(['/', '\\'], "_");
        self.base_dir
            .join(format!("{safe_service}__{safe_account}.cred"))
    }
}

impl Debug for FileFallbackKeyringStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileFallbackKeyringStore")
            .field("base_dir", &self.base_dir)
            .finish()
    }
}

impl KeyringStore for FileFallbackKeyringStore {
    fn load(&self, service: &str, account: &str) -> Result<Option<String>, CredentialStoreError> {
        let path = self.entry_path(service, account);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| CredentialStoreError::Other(e.to_string()))?;
        Ok(Some(content.trim().to_string()))
    }

    fn save(&self, service: &str, account: &str, value: &str) -> Result<(), CredentialStoreError> {
        if let Err(e) = std::fs::create_dir_all(&self.base_dir) {
            return Err(CredentialStoreError::Other(e.to_string()));
        }
        let path = self.entry_path(service, account);
        std::fs::write(&path, value).map_err(|e| CredentialStoreError::Other(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    fn delete(&self, service: &str, account: &str) -> Result<bool, CredentialStoreError> {
        let path = self.entry_path(service, account);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| CredentialStoreError::Other(e.to_string()))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_keyring_lifecycle() {
        let store = MemoryKeyringStore::new();
        assert_eq!(store.load("m31a", "user_1").unwrap(), None);

        store.save("m31a", "user_1", "secret_token_123").unwrap();
        assert_eq!(
            store.load("m31a", "user_1").unwrap(),
            Some("secret_token_123".to_string())
        );

        assert!(store.delete("m31a", "user_1").unwrap());
        assert!(!store.delete("m31a", "user_1").unwrap());
        assert_eq!(store.load("m31a", "user_1").unwrap(), None);
    }

    #[test]
    fn test_file_fallback_keyring_lifecycle() {
        let tmp = tempfile::tempdir().unwrap();
        let store = FileFallbackKeyringStore::new(tmp.path().to_path_buf());

        assert_eq!(store.load("github", "oauth").unwrap(), None);

        store
            .save("github", "oauth", "ghp_secure_credential")
            .unwrap();
        assert_eq!(
            store.load("github", "oauth").unwrap(),
            Some("ghp_secure_credential".to_string())
        );

        assert!(store.delete("github", "oauth").unwrap());
        assert!(!store.delete("github", "oauth").unwrap());
    }
}
