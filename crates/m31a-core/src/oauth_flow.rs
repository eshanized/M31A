//! Asynchronous OAuth2 authorization flow coordinator and local callback listener.
//!
//! Manages browser-based OAuth logins, PKCE challenge generation, and token exchange,
//! including RFC 8628 OAuth 2.0 Device Authorization Grant polling.

use anyhow::{Context, Result, anyhow, bail};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant, SystemTime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::sleep;

pub const DEFAULT_DEVICE_POLL_INTERVAL_SECS: u64 = 5;
pub const DEFAULT_AUTH_URL: &str = "https://auth.m31a.org";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
    pub method: &'static str,
}

/// Validates that a PKCE code verifier conforms to RFC 7636 Section 4.1.
/// - Minimum length: 43 characters
/// - Maximum length: 128 characters
/// - Characters allowed: unreserved = [A-Z], [a-z], [0-9], "-", ".", "_", "~"
pub fn validate_code_verifier(verifier: &str) -> Result<()> {
    let len = verifier.len();
    if !(43..=128).contains(&len) {
        bail!(
            "RFC 7636 invalid code verifier length: expected between 43 and 128 characters, got {}",
            len
        );
    }

    for b in verifier.bytes() {
        let is_unreserved =
            b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_' || b == b'~';
        if !is_unreserved {
            bail!(
                "RFC 7636 invalid character in code verifier: character '{}' (byte 0x{:02x}) is not in unreserved set [A-Z, a-z, 0-9, '-', '.', '_', '~']",
                b as char,
                b
            );
        }
    }

    Ok(())
}

/// Computes the S256 code challenge for a given code verifier according to RFC 7636 Section 4.2:
/// code_challenge = BASE64URL-ENCODE(SHA256(ASCII(code_verifier))) without padding.
pub fn compute_s256_challenge(verifier: &str) -> Result<String> {
    validate_code_verifier(verifier)?;
    let hash = Sha256::digest(verifier.as_bytes());
    Ok(urlsafe_base64_encode(&hash))
}

/// Generates an RFC 7636 compliant PKCE verifier and S256 challenge
/// using an operating-system-backed CSPRNG (`OsRng`).
pub fn generate_pkce_pair() -> PkcePair {
    // 64 cryptographically secure random bytes base64url-encoded yields 86 characters,
    // comfortably within the RFC 7636 [43, 128] bounds.
    let mut random_bytes = [0u8; 64];
    rand::rng().fill_bytes(&mut random_bytes);
    let verifier = urlsafe_base64_encode(&random_bytes);
    let challenge = compute_s256_challenge(&verifier).expect("generated verifier is valid");

    PkcePair {
        verifier,
        challenge,
        method: "S256",
    }
}

pub fn urlsafe_base64_encode(bytes: &[u8]) -> String {
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

/// Endpoint and client configuration for RFC 7636 Authorization Code flow with PKCE.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthCodeFlowConfig {
    pub auth_endpoint: String,
    pub token_endpoint: String,
    pub revocation_endpoint: Option<String>,
    pub client_id: String,
    pub scope: Option<String>,
}

impl Default for AuthCodeFlowConfig {
    fn default() -> Self {
        let base = std::env::var("M31A_AUTH_URL").unwrap_or_else(|_| DEFAULT_AUTH_URL.to_string());
        let trimmed = base.trim_end_matches('/');
        Self {
            auth_endpoint: format!("{trimmed}/oauth/authorize"),
            token_endpoint: format!("{trimmed}/oauth/token"),
            revocation_endpoint: Some(format!("{trimmed}/oauth/revoke")),
            client_id: "m31a-cli".to_string(),
            scope: Some("openid profile email offline_access".to_string()),
        }
    }
}

/// Executes an authorization-code flow with PKCE using a local loopback callback server.
/// Binds an ephemeral local port on `127.0.0.1`, generates a CSPRNG PKCE pair, opens the
/// authorization URL, awaits the browser redirect, verifies CSRF state, and exchanges
/// the authorization code for an `OAuthToken`.
pub async fn execute_browser_pkce_flow<F>(
    client: &reqwest::Client,
    config: &AuthCodeFlowConfig,
    on_auth_url: F,
    timeout_duration: Duration,
) -> Result<OAuthToken>
where
    F: FnOnce(&str),
{
    // Bind local TCP listener to receive callback on loopback
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("Failed to bind local loopback listener for OAuth callback")?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    // Generate PKCE pair and CSRF state using OS CSPRNG
    let pkce = generate_pkce_pair();
    let mut state_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut state_bytes);
    let state = urlsafe_base64_encode(&state_bytes);

    let mut auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&code_challenge={}&code_challenge_method={}&state={}",
        config.auth_endpoint.trim_end_matches('/'),
        urlencoding_simple(&config.client_id),
        urlencoding_simple(&redirect_uri),
        urlencoding_simple(&pkce.challenge),
        pkce.method,
        urlencoding_simple(&state)
    );
    if let Some(ref scope) = config.scope {
        auth_url.push_str(&format!("&scope={}", urlencoding_simple(scope)));
    }

    on_auth_url(&auth_url);

    // Wait for redirect connection with timeout
    let accept_future = listener.accept();
    let (mut stream, _) = tokio::time::timeout(timeout_duration, accept_future)
        .await
        .map_err(|_| {
            anyhow!(
                "Browser authorization timed out after {}s",
                timeout_duration.as_secs()
            )
        })?
        .context("Failed to accept callback connection")?;

    let mut buffer = [0u8; 4096];
    let bytes_read = stream
        .read(&mut buffer)
        .await
        .context("Failed to read HTTP callback request")?;
    let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);

    // Parse GET line
    let first_line = request_str.lines().next().unwrap_or_default();
    let request_path = first_line.split_whitespace().nth(1).unwrap_or("/");

    // Extract query parameters
    let query_str = request_path.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut code_opt = None;
    let mut state_opt = None;
    let mut error_opt = None;

    for param in query_str.split('&') {
        if let Some((k, v)) = param.split_once('=') {
            match k {
                "code" => code_opt = Some(v.to_string()),
                "state" => state_opt = Some(v.to_string()),
                "error" => error_opt = Some(v.to_string()),
                _ => {}
            }
        }
    }

    if let Some(err) = error_opt {
        let resp = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body><h2>Authentication Denied</h2><p>Provider returned an error.</p></body></html>";
        let _ = stream.write_all(resp.as_bytes()).await;
        let _ = stream.shutdown().await;
        bail!("OAuth authorization failed from provider: {}", err);
    }

    let returned_state = state_opt.context("Missing 'state' in OAuth callback")?;
    if returned_state != state {
        let resp = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body><h2>CSRF Warning</h2><p>OAuth state parameter mismatch.</p></body></html>";
        let _ = stream.write_all(resp.as_bytes()).await;
        let _ = stream.shutdown().await;
        bail!("OAuth state parameter mismatch: potential CSRF detected");
    }

    let code = code_opt.context("Missing 'code' in OAuth callback")?;

    // Return success page to user's browser
    let ok_resp = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<!DOCTYPE html><html><body><h1>M31A Authentication Successful</h1><p>You may now close this window and return to your terminal.</p></body></html>";
    let _ = stream.write_all(ok_resp.as_bytes()).await;
    let _ = stream.flush().await;
    let _ = stream.shutdown().await;

    // Exchange auth code + verifier for token
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", redirect_uri.as_str()),
        ("client_id", config.client_id.as_str()),
        ("code_verifier", pkce.verifier.as_str()),
    ];

    let resp = client
        .post(&config.token_endpoint)
        .form(&params)
        .send()
        .await
        .context("Failed to exchange authorization code for token")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        bail!("Token exchange failed with HTTP {status}: {body}");
    }

    #[derive(Deserialize)]
    struct TokenSuccess {
        access_token: String,
        refresh_token: Option<String>,
        expires_in: Option<u64>,
        account_id: Option<String>,
    }

    let body: TokenSuccess = resp
        .json()
        .await
        .context("Failed to parse token response")?;
    let expires_in = body.expires_in.unwrap_or(3600);

    Ok(OAuthToken::new(
        body.access_token,
        body.refresh_token,
        expires_in,
        body.account_id,
    ))
}

/// Refreshes an expired OAuth access token using a stored refresh token according to RFC 6749 §6.
/// Implements refresh token rotation if a new refresh token is issued.
pub async fn refresh_oauth_token(
    client: &reqwest::Client,
    token_endpoint: &str,
    client_id: &str,
    refresh_token: &str,
) -> Result<OAuthToken> {
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", client_id),
    ];

    let resp = client
        .post(token_endpoint)
        .form(&params)
        .send()
        .await
        .context("Failed to send token refresh request")?;

    if !resp.status().is_success() {
        let status = resp.status();
        bail!("OAuth token refresh rejected with HTTP {}", status);
    }

    #[derive(Deserialize)]
    struct TokenSuccess {
        access_token: String,
        refresh_token: Option<String>,
        expires_in: Option<u64>,
        account_id: Option<String>,
    }

    let body: TokenSuccess = resp
        .json()
        .await
        .context("Failed to parse refresh token response")?;
    let expires_in = body.expires_in.unwrap_or(3600);
    let rotated_refresh = body
        .refresh_token
        .or_else(|| Some(refresh_token.to_string()));

    Ok(OAuthToken::new(
        body.access_token,
        rotated_refresh,
        expires_in,
        body.account_id,
    ))
}

/// Revokes an OAuth token according to RFC 7009 upon logout.
pub async fn revoke_oauth_token(
    client: &reqwest::Client,
    revocation_endpoint: &str,
    client_id: &str,
    token: &str,
    token_type_hint: Option<&str>,
) -> Result<()> {
    let mut params = vec![("client_id", client_id), ("token", token)];
    if let Some(hint) = token_type_hint {
        params.push(("token_type_hint", hint));
    }

    let resp = client
        .post(revocation_endpoint)
        .form(&params)
        .send()
        .await
        .context("Failed to send token revocation request")?;

    if !resp.status().is_success() {
        bail!("Token revocation failed with HTTP {}", resp.status());
    }

    Ok(())
}

fn urlencoding_simple(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_rfc7636_appendix_b_vector() {
        // Known test vector from RFC 7636 Appendix B:
        // Verifier: dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk
        // S256 Challenge: E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = compute_s256_challenge(verifier).expect("valid Appendix B verifier");
        assert_eq!(
            challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
            "Challenge must match RFC 7636 Appendix B test vector"
        );
    }

    #[test]
    fn test_pkce_verifier_constraints() {
        // Too short (< 43)
        let too_short = "a".repeat(42);
        assert!(validate_code_verifier(&too_short).is_err());

        // Valid minimum length (43)
        let min_valid = "a".repeat(43);
        assert!(validate_code_verifier(&min_valid).is_ok());

        // Valid maximum length (128)
        let max_valid = "z".repeat(128);
        assert!(validate_code_verifier(&max_valid).is_ok());

        // Too long (> 128)
        let too_long = "a".repeat(129);
        assert!(validate_code_verifier(&too_long).is_err());

        // Invalid characters (spaces, quotes, symbols outside unreserved)
        let with_space = format!("{} abc", "a".repeat(40));
        assert!(validate_code_verifier(&with_space).is_err());

        let with_plus = format!("{}+abc", "a".repeat(40));
        assert!(validate_code_verifier(&with_plus).is_err());

        let with_slash = format!("{}/abc", "a".repeat(40));
        assert!(validate_code_verifier(&with_slash).is_err());
    }

    #[test]
    fn test_generate_pkce_pair_entropy_and_uniqueness() {
        let mut verifiers = HashSet::new();
        let mut challenges = HashSet::new();

        for _ in 0..100 {
            let pair = generate_pkce_pair();
            assert_eq!(pair.method, "S256");
            assert!(validate_code_verifier(&pair.verifier).is_ok());
            assert_eq!(
                pair.challenge,
                compute_s256_challenge(&pair.verifier).unwrap()
            );
            assert!(!pair.verifier.is_empty());
            assert!(!pair.challenge.is_empty());
            assert_ne!(pair.verifier, pair.challenge);

            assert!(
                verifiers.insert(pair.verifier),
                "Verifier collision detected!"
            );
            assert!(
                challenges.insert(pair.challenge),
                "Challenge collision detected!"
            );
        }

        // Check character entropy across generated verifiers:
        // Combined characters should span lowercase, uppercase, digits, and unreserved symbols.
        let all_chars: HashSet<char> = verifiers.iter().flat_map(|v| v.chars()).collect();
        assert!(all_chars.iter().any(|c| c.is_ascii_lowercase()));
        assert!(all_chars.iter().any(|c| c.is_ascii_uppercase()));
        assert!(all_chars.iter().any(|c| c.is_ascii_digit()));
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

    #[test]
    fn test_auth_code_config_defaults() {
        let cfg = AuthCodeFlowConfig::default();
        assert!(cfg.auth_endpoint.contains("/oauth/authorize"));
        assert!(cfg.token_endpoint.contains("/oauth/token"));
        assert_eq!(cfg.client_id, "m31a-cli");
    }
}
