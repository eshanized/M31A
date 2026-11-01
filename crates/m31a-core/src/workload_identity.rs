//! Workload identity federation and cloud service account token exchange.
//!
//! Exchanges local cryptographic identities for short-lived cloud credentials (GCP, AWS, Azure).

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const MAX_ASSERTION_SIZE_BYTES: u64 = 16 * 1024; // 16 KiB

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkloadIdentityToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in_seconds: u64,
}

#[derive(Clone, Debug)]
pub struct WorkloadIdentityConfig {
    pub federation_rule_id: String,
    pub assertion_file: PathBuf,
    pub workload_identity_context: Option<String>,
}

impl WorkloadIdentityConfig {
    pub fn new(
        federation_rule_id: String,
        assertion_file: PathBuf,
        workload_identity_context: Option<String>,
    ) -> Result<Self> {
        let trimmed_id = federation_rule_id.trim();
        if trimmed_id.is_empty() {
            bail!("Workload identity federation rule ID must not be empty");
        }
        if !assertion_file.is_absolute() {
            bail!("Workload identity assertion file path must be absolute");
        }
        Ok(Self {
            federation_rule_id: trimmed_id.to_string(),
            assertion_file,
            workload_identity_context,
        })
    }
}

#[derive(Clone)]
pub struct WorkloadIdentityExchange {
    config: WorkloadIdentityConfig,
    cached_token: Arc<Mutex<Option<(WorkloadIdentityToken, Instant)>>>,
}

impl WorkloadIdentityExchange {
    pub fn new(config: WorkloadIdentityConfig) -> Self {
        Self {
            config,
            cached_token: Arc::new(Mutex::new(None)),
        }
    }

    /// Reads and validates the raw assertion token from disk with 16 KiB upper limit.
    pub fn read_assertion(&self) -> Result<String> {
        let path = &self.config.assertion_file;
        let metadata = fs::metadata(path)
            .with_context(|| format!("Could not read assertion file metadata at {:?}", path))?;

        if metadata.len() > MAX_ASSERTION_SIZE_BYTES {
            bail!(
                "Workload identity assertion file {:?} exceeds maximum limit of 16 KiB (size: {} bytes)",
                path,
                metadata.len()
            );
        }

        let content = fs::read_to_string(path).with_context(|| {
            format!(
                "Failed to read workload identity assertion file at {:?}",
                path
            )
        })?;

        let trimmed = content.trim();
        if trimmed.is_empty() {
            bail!("Workload identity assertion file is empty");
        }

        Ok(trimmed.to_string())
    }

    /// Composes the token exchange request payload for the identity provider.
    pub fn compose_exchange_request(&self) -> Result<serde_json::Value> {
        let assertion = self.read_assertion()?;
        let mut map = serde_json::Map::new();
        map.insert(
            "grant_type".to_string(),
            serde_json::json!("urn:ietf:params:oauth:grant-type:token-exchange"),
        );
        map.insert(
            "subject_token_type".to_string(),
            serde_json::json!("urn:ietf:params:oauth:token-type:jwt"),
        );
        map.insert("subject_token".to_string(), serde_json::json!(assertion));
        map.insert(
            "federation_rule_id".to_string(),
            serde_json::json!(self.config.federation_rule_id),
        );

        if let Some(ref ctx) = self.config.workload_identity_context {
            map.insert("context".to_string(), serde_json::json!(ctx));
        }

        Ok(serde_json::Value::Object(map))
    }

    /// Stores a freshly resolved token in the memory cache.
    pub fn store_token(&self, token: WorkloadIdentityToken) {
        let mut lock = self.cached_token.lock().unwrap();
        *lock = Some((token, Instant::now()));
    }

    /// Returns the currently cached token if still within validity duration.
    pub fn get_valid_token(&self) -> Option<WorkloadIdentityToken> {
        let lock = self.cached_token.lock().unwrap();
        if let Some((ref tok, ref acquired_at)) = *lock {
            let ttl = Duration::from_secs(tok.expires_in_seconds.saturating_sub(60));
            if acquired_at.elapsed() < ttl {
                return Some(tok.clone());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_workload_identity_config_validation() {
        assert!(
            WorkloadIdentityConfig::new("".to_string(), PathBuf::from("/etc/token"), None).is_err()
        );
        assert!(
            WorkloadIdentityConfig::new(
                "rule-1".to_string(),
                PathBuf::from("relative/token"),
                None
            )
            .is_err()
        );
        assert!(
            WorkloadIdentityConfig::new(
                "rule-1".to_string(),
                PathBuf::from("/absolute/token"),
                None
            )
            .is_ok()
        );
    }

    #[test]
    fn test_assertion_reading_and_payload() {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(b"eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.dummy_token")
            .unwrap();

        let cfg = WorkloadIdentityConfig::new(
            "rule-openai-k8s".to_string(),
            tmp.path().to_path_buf(),
            Some("prod-us-east".to_string()),
        )
        .unwrap();
        let exchange = WorkloadIdentityExchange::new(cfg);

        let assertion = exchange.read_assertion().unwrap();
        assert_eq!(
            assertion,
            "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.dummy_token"
        );

        let req = exchange.compose_exchange_request().unwrap();
        assert_eq!(
            req.get("federation_rule_id").and_then(|v| v.as_str()),
            Some("rule-openai-k8s")
        );
        assert_eq!(
            req.get("context").and_then(|v| v.as_str()),
            Some("prod-us-east")
        );
    }

    #[test]
    fn test_token_caching_lifecycle() {
        let cfg = WorkloadIdentityConfig::new("rule-1".to_string(), PathBuf::from("/dummy"), None)
            .unwrap();
        let exchange = WorkloadIdentityExchange::new(cfg);

        assert!(exchange.get_valid_token().is_none());

        let tok = WorkloadIdentityToken {
            access_token: "test_token_abc".to_string(),
            token_type: "Bearer".to_string(),
            expires_in_seconds: 3600,
        };
        exchange.store_token(tok);

        let cached = exchange.get_valid_token();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().access_token, "test_token_abc");
    }
}
