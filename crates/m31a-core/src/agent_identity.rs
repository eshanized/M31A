//! Agent identity metadata, cryptographic keys, and cryptographic signatures.
//!
//! Manages public/private key pairs and identity claims for authenticating agent actions and provenance.

use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentIdentity {
    pub agent_id: String,
    pub role: String,
    pub parent_agent_id: Option<String>,
    pub key_fingerprint: String,
    pub created_at: i64,
    #[serde(skip_serializing, default = "generate_secret")]
    secret: [u8; 32],
}

fn generate_secret() -> [u8; 32] {
    let mut secret = [0u8; 32];
    let seed1 = Uuid::new_v4().as_u128().to_le_bytes();
    let seed2 = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_le_bytes();
    secret[..16].copy_from_slice(&seed1);
    secret[16..].copy_from_slice(&seed2);
    secret
}

impl AgentIdentity {
    pub fn new(role: &str, parent_id: Option<&str>) -> Self {
        let agent_id = format!("agent_{}_{}", role, Uuid::new_v4().simple());
        let secret = generate_secret();

        let mut hasher = Sha256::new();
        hasher.update(secret);
        hasher.update(agent_id.as_bytes());
        let fingerprint = format!("{:x}", hasher.finalize())[..16].to_string();

        let now = chrono::Utc::now().timestamp();
        Self {
            agent_id,
            role: role.to_string(),
            parent_agent_id: parent_id.map(|s| s.to_string()),
            key_fingerprint: fingerprint,
            created_at: now,
            secret,
        }
    }

    /// Signs an outbound message or action payload using HMAC-SHA256.
    pub fn sign_message(&self, payload: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.secret);
        hasher.update(self.agent_id.as_bytes());
        hasher.update(b":");
        hasher.update(payload.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Verifies that a message payload was signed by this agent.
    pub fn verify_signature(&self, payload: &str, signature: &str) -> bool {
        let expected = self.sign_message(payload);
        expected == signature
    }

    /// Generates a structured cryptographic provenance token.
    pub fn generate_provenance_token(&self, target_channel: &str) -> String {
        let timestamp = chrono::Utc::now().timestamp();
        let body = format!("{}:{}:{}", self.agent_id, target_channel, timestamp);
        let sig = self.sign_message(&body);
        format!("{}.{}.{}", self.agent_id, timestamp, sig)
    }

    /// Verifies a provenance token against this agent's identity.
    pub fn verify_provenance_token(&self, target_channel: &str, token: &str) -> Result<bool> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow!("Malformed provenance token format"));
        }
        let (token_agent_id, timestamp_str, sig) = (parts[0], parts[1], parts[2]);
        if token_agent_id != self.agent_id {
            return Ok(false);
        }

        let body = format!("{}:{}:{}", self.agent_id, target_channel, timestamp_str);
        Ok(self.verify_signature(&body, sig))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_identity_creation_and_signing() {
        let identity = AgentIdentity::new("architect", Some("root_lead"));
        assert!(identity.agent_id.starts_with("agent_architect_"));
        assert_eq!(identity.role, "architect");
        assert_eq!(identity.parent_agent_id, Some("root_lead".to_string()));
        assert_eq!(identity.key_fingerprint.len(), 16);

        let payload = "Deploy commit f425be0";
        let sig = identity.sign_message(payload);
        assert!(!sig.is_empty());
        assert!(identity.verify_signature(payload, &sig));
        assert!(!identity.verify_signature("Tampered payload", &sig));
    }

    #[test]
    fn test_provenance_token_verification() {
        let identity = AgentIdentity::new("coder", None);
        let token = identity.generate_provenance_token("development");
        assert!(
            identity
                .verify_provenance_token("development", &token)
                .unwrap()
        );
        assert!(
            !identity
                .verify_provenance_token("wrong_channel", &token)
                .unwrap()
        );
    }
}
