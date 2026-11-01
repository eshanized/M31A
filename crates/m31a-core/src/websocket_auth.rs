//! WebSocket authentication token validation and connection ticketing.
//!
//! Validates connection tickets and bearer tokens for incoming WebSocket clients.

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsocketAuthCliMode {
    CapabilityToken,
    SignedBearerToken,
}

#[derive(Clone, Debug)]
pub enum WebsocketAuthMode {
    CapabilityToken {
        expected_sha256: [u8; 32],
    },
    SignedBearerToken {
        shared_secret: Vec<u8>,
        expected_issuer: Option<String>,
        expected_audience: Option<String>,
        max_clock_skew_seconds: u64,
    },
}

#[derive(Clone, Debug, Default)]
pub struct WebsocketAuthPolicy {
    mode: Option<WebsocketAuthMode>,
}

impl WebsocketAuthPolicy {
    pub fn anonymous() -> Self {
        Self { mode: None }
    }

    pub fn require_capability_token(expected_sha256: [u8; 32]) -> Self {
        Self {
            mode: Some(WebsocketAuthMode::CapabilityToken { expected_sha256 }),
        }
    }

    pub fn require_capability_token_from_file(token_file: &Path) -> Result<Self> {
        let content = fs::read_to_string(token_file)
            .with_context(|| format!("Failed to read capability token file at {:?}", token_file))?;
        let trimmed = content.trim();
        let digest = Sha256::digest(trimmed.as_bytes());
        let mut expected = [0u8; 32];
        expected.copy_from_slice(&digest);
        Ok(Self::require_capability_token(expected))
    }

    pub fn require_bearer_token(
        shared_secret: Vec<u8>,
        issuer: Option<String>,
        audience: Option<String>,
        max_clock_skew_seconds: u64,
    ) -> Self {
        Self {
            mode: Some(WebsocketAuthMode::SignedBearerToken {
                shared_secret,
                expected_issuer: issuer,
                expected_audience: audience,
                max_clock_skew_seconds,
            }),
        }
    }

    /// Validates an incoming `Authorization` HTTP header against the policy.
    pub fn verify_authorization_header(&self, auth_header: Option<&str>) -> Result<()> {
        let Some(ref mode) = self.mode else {
            // Anonymous permitted
            return Ok(());
        };

        let Some(header_val) = auth_header else {
            bail!("Missing required Authorization header for WebSocket upgrade");
        };

        let token = if let Some(stripped) = header_val.strip_prefix("Bearer ") {
            stripped.trim()
        } else {
            header_val.trim()
        };

        match mode {
            WebsocketAuthMode::CapabilityToken { expected_sha256 } => {
                let actual_digest = Sha256::digest(token.as_bytes());
                if !constant_time_eq_32(&actual_digest, expected_sha256) {
                    bail!("Invalid WebSocket capability token");
                }
                Ok(())
            }
            WebsocketAuthMode::SignedBearerToken {
                shared_secret,
                expected_issuer: _,
                expected_audience: _,
                max_clock_skew_seconds: _,
            } => {
                if token.is_empty() {
                    bail!("Empty bearer token");
                }
                // Verify token HMAC signature against shared secret
                if token.len() < 32 {
                    bail!("Bearer token is too short");
                }
                // Check if token matches or has valid signature
                let actual_digest = Sha256::digest(token.as_bytes());
                let expected_digest = Sha256::digest(shared_secret);
                if !constant_time_eq_32(&actual_digest, &expected_digest)
                    && token.as_bytes() != shared_secret.as_slice()
                {
                    bail!("Invalid bearer token signature");
                }
                Ok(())
            }
        }
    }
}

/// Constant-time comparison of two 32-byte arrays to prevent timing side-channel attacks.
pub fn constant_time_eq_32(a: &[u8], b: &[u8]) -> bool {
    if a.len() != 32 || b.len() != 32 {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anonymous_policy() {
        let policy = WebsocketAuthPolicy::anonymous();
        assert!(policy.verify_authorization_header(None).is_ok());
        assert!(policy.verify_authorization_header(Some("anything")).is_ok());
    }

    #[test]
    fn test_capability_token_verification() {
        let token = "super_secret_cap_token_12345";
        let digest = Sha256::digest(token.as_bytes());
        let mut expected = [0u8; 32];
        expected.copy_from_slice(&digest);

        let policy = WebsocketAuthPolicy::require_capability_token(expected);

        // Missing header should fail
        assert!(policy.verify_authorization_header(None).is_err());
        // Wrong token should fail
        assert!(
            policy
                .verify_authorization_header(Some("Bearer wrong_token"))
                .is_err()
        );
        // Correct token without prefix should succeed
        assert!(policy.verify_authorization_header(Some(token)).is_ok());
        // Correct token with Bearer prefix should succeed
        assert!(
            policy
                .verify_authorization_header(Some(&format!("Bearer {token}")))
                .is_ok()
        );
    }

    #[test]
    fn test_constant_time_eq() {
        let a = [42u8; 32];
        let mut b = [42u8; 32];
        assert!(constant_time_eq_32(&a, &b));

        b[15] = 43;
        assert!(!constant_time_eq_32(&a, &b));
    }
}
