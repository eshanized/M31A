//! Provider endpoint trust boundary (P0-01).
//!
//! Invariant: NO CREDENTIAL → BEFORE ENDPOINT TRUST.
//!
//! A repository-controlled workspace configuration must never be able to
//! redirect a credential-bearing production provider request to an
//! attacker-controlled URL. Endpoint authorization happens before any
//! credential is loaded or attached, and every credential-bearing HTTP
//! request site must go through a [`ValidatedProviderEndpoint`].
//!
//! Trust tiers:
//! - [`EndpointTrustSource::BuiltinDefault`] — immutable canonical default.
//! - [`EndpointTrustSource::System`] — `/etc/m31a/config.toml` (trusted admin).
//! - [`EndpointTrustSource::User`] — `~/.config/m31a/config.toml` (trusted user).
//! - [`EndpointTrustSource::ExplicitCli`] — explicit `--config <path>` (trusted operator intent).
//! - [`EndpointTrustSource::Workspace`] — `<ws>/.m31a/config.toml` (UNTRUSTED repository content).
//! - [`EndpointTrustSource::SessionOverride`] — generic session overlay (untrusted by default).
//!
//! Policy (production):
//! - Canonical `https://integrate.api.nvidia.com/v1` is always trusted.
//! - Any custom endpoint carrying a real credential MUST come from a trusted
//!   tier (System/User/ExplicitCli/BuiltinDefault). Workspace/Session custom
//!   endpoints are rejected when a real credential would be attached.
//! - Custom endpoints must be `https://` (fail closed on `http://`), must
//!   parse, and must not target blocked destinations (loopback/RFC1918/
//!   link-local/metadata/host aliases). Full DNS validation happens in
//!   [`ValidatedProviderEndpoint::authorize_for_credential`] before use.

use thiserror::Error;

/// DNS resolver that binds validation to the connection destination (P1-01).
/// Canonical home: [`crate::policy::destination::ValidatingDnsResolver`].
/// Re-exported here so provider code has a single import surface.
pub use crate::policy::destination::ValidatingDnsResolver;
/// Centralized validating HTTP client builder (P0-01, P1-01).
/// Canonical home: [`crate::policy::destination::policy_validating_client_builder`].
pub use crate::policy::destination::policy_validating_client_builder;

/// Canonical production NVIDIA endpoint. Immutable; never overridable by policy.
///
/// Single statement location is [`crate::config::canonical::CANONICAL_NVIDIA_BASE_URL`];
/// re-exported here because this module remains the endpoint *trust/security*
/// authority (validation, credential binding, egress checks).
pub use crate::config::canonical::CANONICAL_NVIDIA_BASE_URL;

/// Where an endpoint value originated. Lower-trust tiers cannot authorize
/// credential-bearing custom endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EndpointTrustSource {
    BuiltinDefault,
    System,
    User,
    ExplicitCli,
    Workspace,
    SessionOverride,
    Unknown,
}

impl EndpointTrustSource {
    /// Trusted tiers may authorize custom `https://` endpoints.
    pub const fn is_trusted_tier(self) -> bool {
        matches!(
            self,
            Self::BuiltinDefault | Self::System | Self::User | Self::ExplicitCli
        )
    }
}

/// Endpoint authorization failures. All fail closed.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EndpointTrustError {
    #[error("provider endpoint URL is invalid: {0}")]
    InvalidUrl(String),
    #[error(
        "provider endpoint scheme '{0}' rejected: custom credential-bearing endpoints must use https"
    )]
    InsecureScheme(String),
    #[error(
        "custom provider endpoint from untrusted configuration tier cannot carry production credentials (source={0:?}); use system/user config or explicit --config instead"
    )]
    UntrustedSource(EndpointTrustSource),
    #[error("provider endpoint destination blocked by egress policy: {0}")]
    BlockedDestination(String),
    #[error("credential must not be attached before endpoint trust is established")]
    CredentialBeforeTrust,
}

/// A provider endpoint that has passed trust authorization.
///
/// Construct ONLY via [`validate_nvidia_endpoint`] (sync shape checks) plus
/// [`ValidatedProviderEndpoint::authorize_for_credential`] (async DNS/destination
/// check) before any credential is attached or any HTTP request is issued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProviderEndpoint {
    url: String,
    source: EndpointTrustSource,
    is_canonical: bool,
    credential_permitted: bool,
    destination_authorized: bool,
}

impl ValidatedProviderEndpoint {
    /// Canonical endpoint URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Trust source that supplied the endpoint.
    pub fn source(&self) -> EndpointTrustSource {
        self.source
    }

    /// Whether this is the immutable canonical production endpoint.
    pub fn is_canonical(&self) -> bool {
        self.is_canonical
    }

    /// Whether a real credential may be attached to requests for this endpoint.
    pub fn credential_permitted(&self) -> bool {
        self.credential_permitted && self.destination_authorized
    }

    /// Require credential permission; fail closed otherwise.
    pub fn require_credential_permitted(&self) -> Result<&str, EndpointTrustError> {
        if self.credential_permitted && self.destination_authorized {
            Ok(&self.url)
        } else {
            Err(EndpointTrustError::CredentialBeforeTrust)
        }
    }
}

/// Returns true for placeholder/test credentials that must never reach production.
pub fn is_test_credential(key: &str) -> bool {
    let k = key.trim();
    k.is_empty()
        || k.starts_with("sk-test")
        || k.starts_with("nvapi-test")
        || k == "test-key"
        || k.contains("dummy")
        || k.contains("fake")
}

/// Normalize trailing slashes for comparison.
fn normalize(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}

/// Returns true if `url` matches the canonical production endpoint
/// (case-insensitive host, trailing-slash insensitive).
pub fn is_canonical_endpoint(url: &str) -> bool {
    normalize(url).eq_ignore_ascii_case(normalize(CANONICAL_NVIDIA_BASE_URL).as_str())
}

/// Synchronous endpoint trust validation (shape + tier).
///
/// This runs BEFORE any credential is loaded. It checks tier authority,
/// URL shape, and scheme. DNS/destination authorization must follow via
/// [`ValidatedProviderEndpoint::authorize_for_credential_async`] — the
/// sync constructor marks `destination_authorized=false` for custom endpoints
/// and `true` for the canonical endpoint whose identity is pinned.
pub fn validate_nvidia_endpoint(
    base_url: Option<String>,
    source: EndpointTrustSource,
) -> Result<ValidatedProviderEndpoint, EndpointTrustError> {
    let raw = base_url.unwrap_or_else(|| CANONICAL_NVIDIA_BASE_URL.to_string());
    let url = normalize(&raw);
    if url.is_empty() {
        return Err(EndpointTrustError::InvalidUrl(
            "empty provider base_url".to_string(),
        ));
    }
    if is_canonical_endpoint(&url) {
        return Ok(ValidatedProviderEndpoint {
            url: CANONICAL_NVIDIA_BASE_URL.to_string(),
            source,
            is_canonical: true,
            credential_permitted: true,
            // Canonical host identity is pinned; per-request DNS policy still
            // applies at request time via authorize_for_credential.
            destination_authorized: true,
        });
    }

    // Custom endpoint: parse shape synchronously.
    let parsed = reqwest::Url::parse(&url)
        .map_err(|e| EndpointTrustError::InvalidUrl(format!("{url}: {e}")))?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if scheme != "https" {
        return Err(EndpointTrustError::InsecureScheme(scheme));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| EndpointTrustError::InvalidUrl(format!("missing host: {url}")))?
        .to_string();
    // Sync blocklist for literal IPs and localhost aliases (no DNS here).
    let lower = host.to_ascii_lowercase();
    if lower == "localhost"
        || lower == "localhost.localdomain"
        || lower.ends_with(".localhost")
        || lower.ends_with(".local")
        || lower.ends_with(".internal")
    {
        return Err(EndpointTrustError::BlockedDestination(format!(
            "localhost/internal hostname alias: {host}"
        )));
    }
    if let Ok(ip) = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(&host)
        .parse::<std::net::IpAddr>()
    {
        if crate::policy::destination::NetworkDestinationPolicy::is_ip_blocked(&ip).is_some() {
            return Err(EndpointTrustError::BlockedDestination(format!(
                "blocked IP literal: {ip}"
            )));
        }
    }

    // Tier authority: untrusted tiers cannot carry production credentials.
    if !source.is_trusted_tier() {
        return Err(EndpointTrustError::UntrustedSource(source));
    }

    Ok(ValidatedProviderEndpoint {
        url,
        source,
        is_canonical: false,
        credential_permitted: true,
        // DNS binding must still authorize before credential attachment.
        destination_authorized: false,
    })
}

impl ValidatedProviderEndpoint {
    /// Async destination authorization: resolves the endpoint host and validates
    /// EVERY resolved address against the egress policy (fail closed on any
    /// blocked address). Must be called before attaching a credential or
    /// issuing a credential-bearing request for non-canonical endpoints.
    /// Canonical endpoints are revalidated as well (defense in depth).
    pub async fn authorize_for_credential(mut self) -> Result<Self, EndpointTrustError> {
        let parsed = reqwest::Url::parse(&self.url)
            .map_err(|e| EndpointTrustError::InvalidUrl(format!("{}: {e}", self.url)))?;
        let url_for_policy = if self.is_canonical {
            self.url.clone()
        } else {
            // Validate the full endpoint URL (scheme + host + DNS).
            self.url.clone()
        };
        let policy = crate::policy::destination::NetworkDestinationPolicy::new();
        policy
            .validate_url(&url_for_policy)
            .await
            .map_err(|e| match e {
                crate::policy::destination::NetworkSecurityError::BlockedDestination {
                    host,
                    ip,
                    reason,
                } => EndpointTrustError::BlockedDestination(format!(
                    "host={host} ip={ip} reason={reason}"
                )),
                other => EndpointTrustError::BlockedDestination(other.to_string()),
            })?;
        let _ = parsed;
        self.destination_authorized = true;
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_endpoint_always_trusted_from_any_tier() {
        for source in [
            EndpointTrustSource::BuiltinDefault,
            EndpointTrustSource::System,
            EndpointTrustSource::User,
            EndpointTrustSource::ExplicitCli,
            EndpointTrustSource::Workspace,
            EndpointTrustSource::SessionOverride,
            EndpointTrustSource::Unknown,
        ] {
            let ep = validate_nvidia_endpoint(Some(CANONICAL_NVIDIA_BASE_URL.to_string()), source)
                .expect("canonical must validate");
            assert!(ep.is_canonical());
            assert!(ep.credential_permitted());
        }
    }

    #[test]
    fn workspace_custom_endpoint_rejected_for_credentials() {
        let err = validate_nvidia_endpoint(
            Some("https://evil-collector.example.com/v1".to_string()),
            EndpointTrustSource::Workspace,
        )
        .unwrap_err();
        assert_eq!(
            err,
            EndpointTrustError::UntrustedSource(EndpointTrustSource::Workspace)
        );
        let err = validate_nvidia_endpoint(
            Some("https://evil-collector.example.com/v1".to_string()),
            EndpointTrustSource::SessionOverride,
        )
        .unwrap_err();
        assert!(matches!(err, EndpointTrustError::UntrustedSource(_)));
    }

    #[test]
    fn trusted_tier_custom_https_endpoint_shape_accepted_pending_dns() {
        let ep = validate_nvidia_endpoint(
            Some("https://my-gateway.example.com/v1/".to_string()),
            EndpointTrustSource::User,
        )
        .expect("trusted https custom endpoint shape must pass");
        assert!(!ep.is_canonical());
        // Destination not yet authorized → credential not yet permitted.
        assert!(!ep.credential_permitted());
        assert!(ep.require_credential_permitted().is_err());
    }

    #[test]
    fn http_custom_endpoint_rejected_even_from_trusted_tier() {
        let err = validate_nvidia_endpoint(
            Some("http://my-gateway.example.com/v1".to_string()),
            EndpointTrustSource::System,
        )
        .unwrap_err();
        assert!(matches!(err, EndpointTrustError::InsecureScheme(_)));
    }

    #[test]
    fn loopback_and_private_literals_rejected() {
        for url in [
            "https://127.0.0.1:11434/v1",
            "https://10.0.0.5/v1",
            "https://192.168.1.10/v1",
            "https://localhost:11434/v1",
        ] {
            let err = validate_nvidia_endpoint(Some(url.to_string()), EndpointTrustSource::User)
                .unwrap_err();
            assert!(
                matches!(err, EndpointTrustError::BlockedDestination(_)),
                "{url} should be blocked, got {err:?}"
            );
        }
    }

    #[tokio::test]
    async fn destination_authorization_binds_dns_before_credential() {
        // Literal public IP passes DNS binding (no DNS lookup needed... but
        // validate_host parses literals directly).
        let ep = validate_nvidia_endpoint(
            Some("https://8.8.8.8/v1".to_string()),
            EndpointTrustSource::User,
        )
        .expect("shape ok");
        let ep = ep
            .authorize_for_credential()
            .await
            .expect("8.8.8.8 allowed");
        assert!(ep.credential_permitted());
        assert_eq!(
            ep.require_credential_permitted().unwrap(),
            "https://8.8.8.8/v1"
        );
    }

    #[tokio::test]
    async fn blocked_destination_fails_closed_on_authorize() {
        let ep = ValidatedProviderEndpoint {
            url: "https://localhost/v1".to_string(),
            source: EndpointTrustSource::User,
            is_canonical: false,
            credential_permitted: true,
            destination_authorized: false,
        };
        assert!(ep.authorize_for_credential().await.is_err());
    }
}
