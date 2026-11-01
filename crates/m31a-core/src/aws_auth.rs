//! AWS SigV4 authentication signer for Amazon Bedrock and cloud services.
//!
//! Computes HMAC-SHA256 request signatures for authenticating against AWS API endpoints.

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;

#[derive(Clone, PartialEq, Eq)]
pub struct AwsAccessKeys {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub session_token: Option<String>,
}

impl fmt::Debug for AwsAccessKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AwsAccessKeys")
            .field("access_key_id", &"<redacted>")
            .field("secret_access_key", &"<redacted>")
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwsProfile {
    pub name: String,
    pub region: Option<String>,
    pub role_arn: Option<String>,
    pub source_profile: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwsAuthConfig {
    pub profile: Option<String>,
    pub region: Option<String>,
    pub service: String,
}

/// Discovers and parses all AWS profiles declared in `~/.aws/config` and `~/.aws/credentials`.
pub fn discover_aws_profiles(home_dir: &Path) -> Result<HashMap<String, AwsProfile>> {
    let mut profiles: HashMap<String, AwsProfile> = HashMap::new();

    let aws_dir = home_dir.join(".aws");
    let config_file = aws_dir.join("config");
    let credentials_file = aws_dir.join("credentials");

    // 1. Parse ~/.aws/config
    if config_file.exists()
        && let Ok(content) = fs::read_to_string(&config_file)
    {
        parse_ini_sections(&content, &mut profiles, true);
    }

    // 2. Parse ~/.aws/credentials
    if credentials_file.exists()
        && let Ok(content) = fs::read_to_string(&credentials_file)
    {
        parse_ini_sections(&content, &mut profiles, false);
    }

    Ok(profiles)
}

fn parse_ini_sections(content: &str, profiles: &mut HashMap<String, AwsProfile>, is_config: bool) {
    let mut current_profile_name = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = &trimmed[1..trimmed.len() - 1].trim();
            current_profile_name = if is_config && section.starts_with("profile ") {
                section.trim_start_matches("profile ").trim().to_string()
            } else {
                section.to_string()
            };

            profiles
                .entry(current_profile_name.clone())
                .or_insert_with(|| AwsProfile {
                    name: current_profile_name.clone(),
                    region: None,
                    role_arn: None,
                    source_profile: None,
                    access_key_id: None,
                    secret_access_key: None,
                });
            continue;
        }

        if !current_profile_name.is_empty()
            && let Some((k, v)) = trimmed.split_once('=')
        {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim().to_string();

            if let Some(prof) = profiles.get_mut(&current_profile_name) {
                match key.as_str() {
                    "region" => prof.region = Some(val),
                    "role_arn" => prof.role_arn = Some(val),
                    "source_profile" => prof.source_profile = Some(val),
                    "aws_access_key_id" => prof.access_key_id = Some(val),
                    "aws_secret_access_key" => prof.secret_access_key = Some(val),
                    _ => {}
                }
            }
        }
    }
}

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let hash = Sha256::digest(key);
        k[..32].copy_from_slice(&hash);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut i_key_pad = [0x36u8; 64];
    let mut o_key_pad = [0x5cu8; 64];
    for i in 0..64 {
        i_key_pad[i] ^= k[i];
        o_key_pad[i] ^= k[i];
    }

    let mut inner = Sha256::new();
    inner.update(i_key_pad);
    inner.update(data);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(o_key_pad);
    outer.update(inner_hash);
    let outer_hash = outer.finalize();

    let mut res = [0u8; 32];
    res.copy_from_slice(&outer_hash);
    res
}

pub fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Computes an AWS SigV4 HMAC-SHA256 signing key from credentials and scope.
pub fn derive_sigv4_signing_key(
    secret_key: &str,
    date_stamp: &str, // YYYYMMDD
    region: &str,
    service: &str,
) -> Result<Vec<u8>> {
    let k_secret = format!("AWS4{}", secret_key);
    let k_date = hmac_sha256(k_secret.as_bytes(), date_stamp.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    let k_signing = hmac_sha256(&k_service, b"aws4_request");
    Ok(k_signing.to_vec())
}

/// Computes the final hex signature for a SigV4 string-to-sign.
pub fn compute_sigv4_signature(signing_key: &[u8], string_to_sign: &str) -> Result<String> {
    let sig = hmac_sha256(signing_key, string_to_sign.as_bytes());
    Ok(to_hex(&sig))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_aws_access_keys_redacted_debug() {
        let keys = AwsAccessKeys {
            access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: Some("token123".to_string()),
        };
        let debug_str = format!("{:?}", keys);
        assert!(!debug_str.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(!debug_str.contains("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"));
        assert!(debug_str.contains("<redacted>"));
    }

    #[test]
    fn test_discover_aws_profiles() {
        let dir = tempdir().unwrap();
        let aws_dir = dir.path().join(".aws");
        fs::create_dir_all(&aws_dir).unwrap();

        let config_data = r#"
[default]
region = us-west-2

[profile staging]
region = us-east-1
role_arn = arn:aws:iam::123456789012:role/StagingRole
"#;
        fs::write(aws_dir.join("config"), config_data).unwrap();

        let creds_data = r#"
[default]
aws_access_key_id = AKIADEFAULT
aws_secret_access_key = SECRETDEFAULT
"#;
        fs::write(aws_dir.join("credentials"), creds_data).unwrap();

        let profiles = discover_aws_profiles(dir.path()).unwrap();
        assert_eq!(profiles.len(), 2);

        let default_prof = profiles.get("default").unwrap();
        assert_eq!(default_prof.region, Some("us-west-2".to_string()));
        assert_eq!(default_prof.access_key_id, Some("AKIADEFAULT".to_string()));

        let staging_prof = profiles.get("staging").unwrap();
        assert_eq!(staging_prof.region, Some("us-east-1".to_string()));
        assert_eq!(
            staging_prof.role_arn,
            Some("arn:aws:iam::123456789012:role/StagingRole".to_string())
        );
    }

    #[test]
    fn test_sigv4_key_derivation() {
        let secret = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
        let date = "20130524";
        let region = "us-east-1";
        let service = "s3";

        let key = derive_sigv4_signing_key(secret, date, region, service).unwrap();
        assert_eq!(key.len(), 32);

        let string_to_sign =
            "AWS4-HMAC-SHA256\n20130524T000000Z\n20130524/us-east-1/s3/aws4_request\nexample_hash";
        let signature = compute_sigv4_signature(&key, string_to_sign).unwrap();
        assert_eq!(signature.len(), 64);
    }
}
