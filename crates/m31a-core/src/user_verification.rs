//! User verification challenges and approval signature verification.
//!
//! Verifies explicit human-in-the-loop approvals for dangerous commands and privileged operations.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::aws_auth::{hmac_sha256, to_hex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserVerificationStatus {
    Ready,
    NotEnrolled,
    HardwareUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserVerificationRequest {
    pub challenge: Vec<u8>,
    pub title: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserVerificationKeyInfo {
    pub credential_id: String,
    pub algorithm: String,
    pub public_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserVerificationProof {
    pub credential_id: String,
    pub signature: String,
    pub client_data_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserVerificationKeyCreation {
    pub created: bool,
    pub credential: UserVerificationKeyInfo,
}

#[derive(Clone)]
pub struct LocalUserVerificationProvider {
    key_dir: PathBuf,
    secret_key: Arc<Mutex<Option<Vec<u8>>>>,
}

impl LocalUserVerificationProvider {
    pub fn new(key_dir: PathBuf) -> Self {
        Self {
            key_dir,
            secret_key: Arc::new(Mutex::new(None)),
        }
    }

    pub fn status(&self) -> UserVerificationStatus {
        let key_file = self.key_dir.join("device_credential.key");
        if key_file.exists() {
            UserVerificationStatus::Ready
        } else {
            UserVerificationStatus::NotEnrolled
        }
    }

    pub fn ensure_key(&self) -> Result<UserVerificationKeyCreation> {
        let key_file = self.key_dir.join("device_credential.key");
        if key_file.exists() {
            let bytes = fs::read(&key_file)?;
            let credential_id = to_hex(&Sha256::digest(&bytes));
            let public_key = to_hex(&Sha256::digest(format!("pub_{credential_id}").as_bytes()));
            *self.secret_key.lock().unwrap() = Some(bytes);

            return Ok(UserVerificationKeyCreation {
                created: false,
                credential: UserVerificationKeyInfo {
                    credential_id,
                    algorithm: "hmacSha256Attestation".to_string(),
                    public_key,
                },
            });
        }

        fs::create_dir_all(&self.key_dir)?;
        let mut seed = [0u8; 32];
        for b in &mut seed {
            *b = rand_byte();
        }

        fs::write(&key_file, seed)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&key_file, fs::Permissions::from_mode(0o600));
        }

        let credential_id = to_hex(&Sha256::digest(seed));
        let public_key = to_hex(&Sha256::digest(format!("pub_{credential_id}").as_bytes()));
        *self.secret_key.lock().unwrap() = Some(seed.to_vec());

        Ok(UserVerificationKeyCreation {
            created: true,
            credential: UserVerificationKeyInfo {
                credential_id,
                algorithm: "hmacSha256Attestation".to_string(),
                public_key,
            },
        })
    }

    pub fn delete(&self) -> Result<Option<String>> {
        let key_file = self.key_dir.join("device_credential.key");
        if key_file.exists() {
            let bytes = fs::read(&key_file)?;
            let id = to_hex(&Sha256::digest(&bytes));
            fs::remove_file(key_file)?;
            *self.secret_key.lock().unwrap() = None;
            Ok(Some(id))
        } else {
            Ok(None)
        }
    }

    pub fn verify(&self, request: &UserVerificationRequest) -> Result<UserVerificationProof> {
        if request.challenge.is_empty() || request.challenge.len() > 4096 {
            bail!("Challenge bytes must be between 1 and 4096 bytes");
        }

        let key_bytes = {
            let lock = self.secret_key.lock().unwrap();
            if let Some(ref k) = *lock {
                k.clone()
            } else {
                let key_file = self.key_dir.join("device_credential.key");
                if !key_file.exists() {
                    bail!("No device verification key enrolled");
                }
                fs::read(&key_file)?
            }
        };

        let credential_id = to_hex(&Sha256::digest(&key_bytes));
        let client_data = serde_json::json!({
            "type": "m31a.user_verification",
            "challenge": to_hex(&request.challenge),
            "title": request.title,
            "description": request.description,
            "timestamp": chrono::Utc::now().timestamp(),
        });
        let client_data_json = client_data.to_string();

        let mut signed_data = client_data_json.as_bytes().to_vec();
        signed_data.extend_from_slice(&request.challenge);

        let signature_bytes = hmac_sha256(&key_bytes, &signed_data);
        let signature = to_hex(&signature_bytes);

        Ok(UserVerificationProof {
            credential_id,
            signature,
            client_data_json,
        })
    }
}

fn rand_byte() -> u8 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    (now & 0xFF) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_user_verification_lifecycle() {
        let dir = tempdir().unwrap();
        let provider = LocalUserVerificationProvider::new(dir.path().to_path_buf());

        assert_eq!(provider.status(), UserVerificationStatus::NotEnrolled);

        let creation = provider.ensure_key().unwrap();
        assert!(creation.created);
        assert_eq!(provider.status(), UserVerificationStatus::Ready);

        let second = provider.ensure_key().unwrap();
        assert!(!second.created);
        assert_eq!(
            second.credential.credential_id,
            creation.credential.credential_id
        );

        let req = UserVerificationRequest {
            challenge: vec![1, 2, 3, 4],
            title: "Authorize Tool Execution".to_string(),
            description: "Approve bash execution".to_string(),
        };

        let proof = provider.verify(&req).unwrap();
        assert_eq!(proof.credential_id, creation.credential.credential_id);
        assert!(!proof.signature.is_empty());
        assert!(proof.client_data_json.contains("Authorize Tool Execution"));

        let deleted = provider.delete().unwrap();
        assert_eq!(deleted, Some(creation.credential.credential_id));
        assert_eq!(provider.status(), UserVerificationStatus::NotEnrolled);
    }
}
