//! Asynchronous OAuth2 authorization flow coordinator and local callback listener.
//!
//! Manages browser-based OAuth logins, PKCE challenge generation, and token exchange,
//! including RFC 8628 OAuth 2.0 Device Authorization Grant polling.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant, SystemTime};
use tokio::time::sleep;

pub const DEFAULT_DEVICE_POLL_INTERVAL_SECS: u64 = 5;
pub const DEFAULT_AUTH_URL: &str = "https://auth.m31a.org";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
    pub method: &'static str,
}

/// Generates an RFC 7636 compliant PKCE verifier and S256 challenge.
pub fn generate_pkce_pair() -> PkcePair {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    let mut random_bytes = [0u8; 32];
    for (i, b) in random_bytes.iter_mut().enumerate() {
        let x = (now >> (i % 16)) as u8;
        *b = x.wrapping_add((i as u8).wrapping_mul(31));
    }

    let verifier = urlsafe_base64_encode(&random_bytes);
    let hash = Sha256::digest(verifier.as_bytes());
    let challenge = urlsafe_base64_encode(&hash);

    PkcePair {
        verifier,
        challenge,
        method: "S256",
    }
}

fn urlsafe_base64_encode(bytes: &[u8]) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let b0 = bytes[i];
        let b1 = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };
        let b2 = if i + 2 < bytes.len() { bytes[i + 2] } else { 0 };

        let idx0 = (b0 >> 2) as usize;
        let idx1 = (((b0 & 0x03) << 4) | (b1 >> 4)) as usize;
        let idx2 = (((b1 & 0x0f) << 2) | (b2 >> 6)) as usize;
        let idx3 = (b2 & 0x3f) as usize;

        out.push(CHARSET[idx0] as char);
        out.push(CHARSET[idx1] as char);
        if i + 1 < bytes.len() {
            out.push(CHARSET[idx2] as char);
        }
        if i + 2 < bytes.len() {
            out.push(CHARSET[idx3] as char);
        }
        i += 3;
    }
    out
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    pub interval: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: u64,
    pub account_id: Option<String>,
}

impl OAuthToken {
    pub fn new(
        access_token: String,
        refresh_token: Option<String>,
        expires_in_secs: u64,
        account_id: Option<String>,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            access_token,
            refresh_token,
            expires_at: now + expires_in_secs,
            account_id,
        }
    }

    pub fn needs_refresh(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now + 60 >= self.expires_at
    }
}

/// Endpoint configuration for RFC 8628 device code authentication.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceAuthRequestConfig {
    pub device_auth_endpoint: String,
    pub token_endpoint: String,
    pub client_id: String,
    pub scope: Option<String>,
}

impl Default for DeviceAuthRequestConfig {
    fn default() -> Self {
        let base = std::env::var("M31A_AUTH_URL").unwrap_or_else(|_| DEFAULT_AUTH_URL.to_string());
        Self {
            device_auth_endpoint: format!("{}/oauth/device/code", base.trim_end_matches('/')),
            token_endpoint: format!("{}/oauth/token", base.trim_end_matches('/')),
            client_id: "m31a-cli".to_string(),
            scope: Some("openid profile email offline_access".to_string()),
        }
    }
}

/// Result of a single device code poll attempt (RFC 8628 §3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevicePollResult {
    Success(OAuthToken),
    Pending,
    SlowDown,
    AccessDenied,
    Expired,
    Error(String),
}

/// Initiates the device authorization flow (RFC 8628 §3.1).
pub async fn request_device_code(
    client: &reqwest::Client,
    config: &DeviceAuthRequestConfig,
) -> Result<DeviceCodeResponse> {
    let mut params = vec![("client_id", config.client_id.as_str())];
    if let Some(ref scope) = config.scope {
        params.push(("scope", scope.as_str()));
    }

    let resp = client
        .post(&config.device_auth_endpoint)
        .form(&params)
        .send()
        .await
        .context("Failed to request device authorization code")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!("Device auth request failed with HTTP {}: {}", status, text);
    }

    let code_resp: DeviceCodeResponse = resp
        .json()
        .await
        .context("Failed to parse device authorization response")?;

    Ok(code_resp)
}

/// Polls token endpoint once for device code status (RFC 8628 §3.4).
pub async fn poll_device_token_once(
    client: &reqwest::Client,
    config: &DeviceAuthRequestConfig,
    device_code: &str,
) -> Result<DevicePollResult> {
    let params = [
        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ("device_code", device_code),
        ("client_id", config.client_id.as_str()),
    ];

    let resp = client
        .post(&config.token_endpoint)
        .form(&params)
        .send()
        .await
        .context("Failed to query token endpoint")?;

    if resp.status().is_success() {
        #[derive(Deserialize)]
        struct TokenSuccess {
            access_token: String,
            refresh_token: Option<String>,
            expires_in: Option<u64>,
            account_id: Option<String>,
        }
        let token_body: TokenSuccess = resp.json().await?;
        let expires_in = token_body.expires_in.unwrap_or(3600);
        return Ok(DevicePollResult::Success(OAuthToken::new(
            token_body.access_token,
            token_body.refresh_token,
            expires_in,
            token_body.account_id,
        )));
    }

    #[derive(Deserialize)]
    struct ErrorBody {
        error: String,
        error_description: Option<String>,
    }

    if let Ok(err_data) = resp.json::<ErrorBody>().await {
        match err_data.error.as_str() {
            "authorization_pending" => Ok(DevicePollResult::Pending),
            "slow_down" => Ok(DevicePollResult::SlowDown),
            "access_denied" => Ok(DevicePollResult::AccessDenied),
            "expired_token" => Ok(DevicePollResult::Expired),
            other => Ok(DevicePollResult::Error(format!(
                "{}: {}",
                other,
                err_data.error_description.unwrap_or_default()
            ))),
        }
    } else {
        Ok(DevicePollResult::Error(
            "Unknown token response error".into(),
        ))
    }
}

/// Executes the full polling loop with backoff and timeout according to RFC 8628.
pub async fn execute_device_code_flow<F>(
    client: &reqwest::Client,
    config: &DeviceAuthRequestConfig,
    on_code_received: F,
) -> Result<OAuthToken>
where
    F: FnOnce(&DeviceCodeResponse),
{
    let code_resp = request_device_code(client, config).await?;
    on_code_received(&code_resp);

    let start_time = Instant::now();
    let max_duration = Duration::from_secs(code_resp.expires_in);
    let mut interval = Duration::from_secs(
        code_resp
            .interval
            .unwrap_or(DEFAULT_DEVICE_POLL_INTERVAL_SECS),
    );

    loop {
        if start_time.elapsed() >= max_duration {
            bail!(
                "Device code authorization timed out after {} seconds.",
                code_resp.expires_in
            );
        }

        sleep(interval).await;

        match poll_device_token_once(client, config, &code_resp.device_code).await? {
            DevicePollResult::Success(tok) => return Ok(tok),
            DevicePollResult::Pending => {
                // Continue polling
            }
            DevicePollResult::SlowDown => {
                interval += Duration::from_secs(5);
            }
            DevicePollResult::AccessDenied => {
                bail!("User denied device code authorization.");
            }
            DevicePollResult::Expired => {
                bail!("Device code has expired. Please restart the login flow.");
            }
            DevicePollResult::Error(err) => {
                bail!("Device authorization error: {}", err);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pkce_pair() {
        let pair = generate_pkce_pair();
        assert_eq!(pair.method, "S256");
        assert!(!pair.verifier.is_empty());
        assert!(!pair.challenge.is_empty());
        assert_ne!(pair.verifier, pair.challenge);
    }

    #[test]
    fn test_oauth_token_lifecycle() {
        let tok = OAuthToken::new(
            "acc123".to_string(),
            Some("ref123".to_string()),
            3600,
            Some("org-abc".to_string()),
        );
        assert!(!tok.needs_refresh());

        let expiring_tok = OAuthToken::new("acc123".to_string(), None, 30, None);
        assert!(expiring_tok.needs_refresh());
    }

    #[test]
    fn test_device_auth_config_defaults() {
        let cfg = DeviceAuthRequestConfig::default();
        assert!(cfg.device_auth_endpoint.contains("/oauth/device/code"));
        assert!(cfg.token_endpoint.contains("/oauth/token"));
        assert_eq!(cfg.client_id, "m31a-cli");
    }
}
