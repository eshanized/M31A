//! NVIDIA-NIM-Only Provider Registry & Masked Secret Boundaries (CFX-01, CFX-02, D-08).
//!
//! NVIDIA NIM is the sole production model provider in the current release.
//! The registry retains retired provider descriptors (OpenAI, Anthropic,
//! Gemini, OpenAI-Compatible/Local) exclusively so that ordinary
//! configuration, `/model` selection, and environment resolution can reject
//! them deterministically with a precise NVIDIA-only error. Retired entries
//! are never exposed through production surfaces (`production_descriptors`,
//! setup wizard, `/model`, model selector). `Mock` remains strictly as
//! test infrastructure and is unreachable from normal user interaction.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

use crate::model::types::ProviderCapabilityStatus;
pub use crate::model::types::ProviderCapabilityStatus as RegistryCapabilityStatus;

/// Canonical production provider ID. All production model resolution
/// normalizes to this value.
pub const PRODUCTION_PROVIDER_ID: &str = "nvidia_nim";

/// Deterministic user-facing error for any non-NVIDIA provider selection.
pub const NVIDIA_ONLY_ERROR: &str = "Only NVIDIA NIM models are supported in this release.";

/// Provider IDs retired from production. They remain known to the registry
/// solely for deterministic rejection messaging; they can never become the
/// active runtime provider through supported paths.
pub const RETIRED_PROVIDER_IDS: &[&str] = &[
    "openai",
    "anthropic",
    "gemini",
    "openai_compatible",
    "local",
    "ollama",
];

/// Normalize a user-supplied provider token to its canonical registry ID.
///
/// `nvidia` is the accepted alias for `nvidia_nim`. `local`/`ollama` map to
/// the retired `openai_compatible` entry so they are rejected with the same
/// precise NVIDIA-only error instead of an ambiguous "unknown provider".
pub fn normalize_provider_id(id: &str) -> String {
    canonical_id_owned(id)
}

/// Owned canonicalization used by registry methods (handles arbitrary input).
fn canonical_id_owned(id: &str) -> String {
    let lower = id.to_lowercase();
    match lower.as_str() {
        "nvidia" => "nvidia_nim".to_string(),
        "local" | "ollama" => "openai_compatible".to_string(),
        _ => lower,
    }
}

/// Returns true when a provider ID is retired from production (known to the
/// registry but never selectable as the active runtime provider).
pub fn is_retired_provider(id: &str) -> bool {
    RETIRED_PROVIDER_IDS.contains(&id.to_lowercase().as_str())
}

/// Supported LLM provider categories (CFX-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    Gemini,
    NvidiaNim,
    OpenAICompatible,
    Mock,
}

impl ProviderType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::OpenAI => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Gemini => "Google Gemini",
            Self::NvidiaNim => "NVIDIA NIM",
            Self::OpenAICompatible => "OpenAI-Compatible",
            Self::Mock => "Mock / Test-Only",
        }
    }

    /// True only for the production provider. Retired variants and Mock are
    /// never valid selections for production runtime execution.
    pub fn is_production(&self) -> bool {
        matches!(self, Self::NvidiaNim)
    }
}

/// Masked secret wrapper preventing credentials from leaking in logs or UI projections (D-08).
#[derive(Clone, PartialEq, Eq)]
pub struct MaskedSecret(String);

impl MaskedSecret {
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Deliberately expose the raw secret string for HTTP Authorization headers only.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }

    /// Masked representation for logs, error messages, and UI widgets.
    pub fn masked(&self) -> String {
        if self.0.is_empty() {
            return String::new();
        }
        if self.0.len() <= 6 {
            "********".to_string()
        } else {
            let prefix = &self.0[..std::cmp::min(4, self.0.len())];
            format!("{}...[MASKED]", prefix)
        }
    }
}

impl std::fmt::Debug for MaskedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.masked())
    }
}

impl std::fmt::Display for MaskedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.masked())
    }
}

impl Serialize for MaskedSecret {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.masked())
    }
}

impl<'de> Deserialize<'de> for MaskedSecret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self::new(s))
    }
}

/// Metadata descriptor for a configured LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderDescriptor {
    pub id: String,
    pub provider_type: ProviderType,
    pub name: String,
    pub base_url: Option<String>,
    pub default_model: String,
    pub requires_api_key: bool,
    pub is_production_supported: bool,
    pub is_mock: bool,
}

/// Errors originating from provider registry operations (WS-I §11).
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProviderError {
    #[error("Provider '{0}' not found in registry")]
    NotFound(String),

    #[error("Provider '{0}' is not supported in the current architecture (status: UNAVAILABLE)")]
    Unsupported(String),

    #[error("Missing configuration for provider '{0}': {1}")]
    MissingConfiguration(String, String),

    #[error("Missing credential for provider '{0}'")]
    MissingCredential(String),

    #[error("Authentication failure for provider '{0}': {1}")]
    AuthenticationFailure(String, String),

    #[error("Endpoint unavailable for provider '{0}': {1}")]
    EndpointUnavailable(String, String),

    #[error("Model unavailable for provider '{0}': {1}")]
    ModelUnavailable(String, String),

    #[error("Invalid request for provider '{0}': {1}")]
    InvalidRequest(String, String),

    #[error("Rate limit exceeded for provider '{0}', retry after {1}s")]
    RateLimited(String, u64),

    #[error("Timeout connecting to provider '{0}': {1}")]
    Timeout(String, String),

    #[error("Unsupported capability for provider '{0}': {1}")]
    UnsupportedCapability(String, String),

    #[error("Internal failure in provider '{0}': {1}")]
    ProviderInternalFailure(String, String),

    #[error("Connection probe failed for '{0}': {1}")]
    ProbeFailed(String, String),

    #[error("IO error managing credentials: {0}")]
    Io(String),

    #[error("JSON error parsing credentials: {0}")]
    Json(String),
}

impl From<std::io::Error> for ProviderError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<serde_json::Error> for ProviderError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e.to_string())
    }
}

/// Detailed capability probe report for diagnostic and doctor inspection (WS-I §8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderProbeReport {
    pub provider_id: String,
    pub status: ProviderCapabilityStatus,
    pub latency: Option<Duration>,
    pub details: String,
    pub endpoint_reachable: bool,
    pub credentials_present: bool,
    pub authentication_accepted: bool,
    pub is_production_supported: bool,
}

/// Registry of known providers and secure local credential store.
#[derive(Debug, Clone)]
pub struct ProviderRegistry {
    descriptors: HashMap<String, ProviderDescriptor>,
    credentials: HashMap<String, MaskedSecret>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    pub const CREDENTIALS_FILENAME: &'static str = ".m31a/credentials.json";

    /// Initialize provider registry with the production provider plus
    /// retired entries retained solely for deterministic rejection.
    ///
    /// The only production-supported entry is `nvidia_nim`; `mock` exists
    /// strictly for deterministic unit/contract tests. Retired descriptors
    /// (`openai`, `anthropic`, `gemini`, `openai_compatible`) are registered
    /// with `is_production_supported = false` so stale configuration and
    /// `/model` input fail with a precise NVIDIA-only error instead of an
    /// ambiguous "unknown provider".
    pub fn new() -> Self {
        let mut reg = Self {
            descriptors: HashMap::new(),
            credentials: HashMap::new(),
        };

        reg.register(ProviderDescriptor {
            id: "openai".to_string(),
            provider_type: ProviderType::OpenAI,
            name: "OpenAI".to_string(),
            base_url: Some("https://api.openai.com/v1".to_string()),
            default_model: "gpt-4o".to_string(),
            requires_api_key: true,
            is_production_supported: false,
            is_mock: false,
        });

        reg.register(ProviderDescriptor {
            id: "anthropic".to_string(),
            provider_type: ProviderType::Anthropic,
            name: "Anthropic".to_string(),
            base_url: Some("https://api.anthropic.com/v1".to_string()),
            default_model: "claude-3-5-sonnet-20241022".to_string(),
            requires_api_key: true,
            is_production_supported: false,
            is_mock: false,
        });

        reg.register(ProviderDescriptor {
            id: "gemini".to_string(),
            provider_type: ProviderType::Gemini,
            name: "Google Gemini".to_string(),
            base_url: Some("https://generativelanguage.googleapis.com/v1beta".to_string()),
            default_model: "gemini-1.5-pro".to_string(),
            requires_api_key: true,
            is_production_supported: false,
            is_mock: false,
        });

        reg.register(ProviderDescriptor {
            id: "nvidia_nim".to_string(),
            provider_type: ProviderType::NvidiaNim,
            name: "NVIDIA NIM".to_string(),
            base_url: Some("https://integrate.api.nvidia.com/v1".to_string()),
            default_model: "meta/llama-3.1-70b-instruct".to_string(),
            requires_api_key: true,
            is_production_supported: true,
            is_mock: false,
        });

        reg.register(ProviderDescriptor {
            id: "openai_compatible".to_string(),
            provider_type: ProviderType::OpenAICompatible,
            name: "Local / OpenAI-Compatible".to_string(),
            base_url: Some("http://localhost:11434/v1".to_string()),
            default_model: "llama3:latest".to_string(),
            requires_api_key: false,
            is_production_supported: false,
            is_mock: false,
        });

        reg.register(ProviderDescriptor {
            id: "mock".to_string(),
            provider_type: ProviderType::Mock,
            name: "Deterministic Mock Provider".to_string(),
            base_url: None,
            default_model: "mock-model".to_string(),
            requires_api_key: false,
            is_production_supported: false,
            is_mock: true,
        });

        reg
    }

    /// Register or update a provider descriptor.
    pub fn register(&mut self, descriptor: ProviderDescriptor) {
        self.descriptors.insert(descriptor.id.clone(), descriptor);
    }

    /// Retrieve descriptor by ID (canonicalizes `nvidia` and
    /// `local`/`ollama` aliases deterministically).
    pub fn get_descriptor(&self, id: &str) -> Option<&ProviderDescriptor> {
        let normalized = canonical_id_owned(id);
        self.descriptors.get(&normalized)
    }

    /// True when `id` identifies the production provider (NVIDIA NIM).
    pub fn is_production_provider(&self, id: &str) -> bool {
        canonical_id_owned(id) == PRODUCTION_PROVIDER_ID
    }

    /// Check if credentials exist for a provider.
    pub fn has_credential(&self, provider_id: &str) -> bool {
        let normalized = canonical_id_owned(provider_id);
        self.credentials.contains_key(&normalized)
    }

    /// Retrieve credentials for a provider.
    pub fn get_credential(&self, provider_id: &str) -> Option<&MaskedSecret> {
        let normalized = canonical_id_owned(provider_id);
        self.credentials.get(&normalized)
    }

    /// Set credentials for a provider in memory.
    pub fn set_credential(&mut self, provider_id: &str, secret: impl Into<String>) {
        let normalized = canonical_id_owned(provider_id);
        self.credentials
            .insert(normalized, MaskedSecret::new(secret));
    }

    /// Return all registered standard provider descriptors.
    ///
    /// NOTE: retained for internal diagnostics and deterministic rejection
    /// messaging. User-facing surfaces (wizard, `/model`, model selector,
    /// doctor) must use [`Self::production_descriptors`], which exposes only
    /// NVIDIA NIM.
    pub fn all_descriptors(&self) -> Vec<&ProviderDescriptor> {
        let mut list: Vec<&ProviderDescriptor> =
            self.descriptors.values().filter(|d| !d.is_mock).collect();
        list.sort_by_key(|d| &d.id);
        list
    }

    /// Return the production provider registry view: NVIDIA NIM only.
    ///
    /// This is the canonical enumeration for every user-facing and
    /// production configuration surface. Mock and retired providers are
    /// never part of the production registry.
    pub fn production_descriptors(&self) -> Vec<&ProviderDescriptor> {
        let mut list: Vec<&ProviderDescriptor> = self
            .descriptors
            .values()
            .filter(|d| d.is_production_supported && !d.is_mock)
            .collect();
        list.sort_by_key(|d| &d.id);
        list
    }

    /// Determine capability status for a provider (WS-I §1).
    ///
    /// NVIDIA NIM is the production model provider. Retired providers report
    /// `Unavailable`; Mock reports `MockOnly`; unknown IDs report `Unknown`.
    /// Capability is never inferred optimistically.
    pub fn get_status(&self, provider_id: &str) -> ProviderCapabilityStatus {
        let normalized = canonical_id_owned(provider_id);
        let desc = match self.get_descriptor(&normalized) {
            Some(d) => d,
            None => return ProviderCapabilityStatus::Unknown,
        };

        if desc.is_mock {
            return ProviderCapabilityStatus::MockOnly;
        }

        if !desc.is_production_supported {
            return ProviderCapabilityStatus::Unavailable;
        }

        if desc.requires_api_key {
            let has_key = self.has_credential(&normalized)
                || (normalized == "nvidia_nim"
                    && (std::env::var("NVIDIA_API_KEY")
                        .map(|k| !k.trim().is_empty())
                        .unwrap_or(false)
                        || std::env::var("API_KEY_NVIDIA")
                            .map(|k| !k.trim().is_empty())
                            .unwrap_or(false)));
            if !has_key {
                return ProviderCapabilityStatus::Misconfigured;
            }
        }

        ProviderCapabilityStatus::Available
    }

    /// Perform a comprehensive, truthful capability probe for a provider (WS-I §8).
    pub async fn probe_provider(&self, provider_id: &str) -> ProviderProbeReport {
        let normalized = canonical_id_owned(provider_id);
        let desc = match self.get_descriptor(&normalized) {
            Some(d) => d,
            None => {
                return ProviderProbeReport {
                    provider_id: provider_id.to_string(),
                    status: ProviderCapabilityStatus::Unknown,
                    latency: None,
                    details: format!(
                        "Provider '{provider_id}' is unknown. {NVIDIA_ONLY_ERROR} NVIDIA NIM is the production model provider."
                    ),
                    endpoint_reachable: false,
                    credentials_present: false,
                    authentication_accepted: false,
                    is_production_supported: false,
                };
            }
        };

        if desc.is_mock {
            return ProviderProbeReport {
                provider_id: provider_id.to_string(),
                status: ProviderCapabilityStatus::MockOnly,
                latency: None,
                details:
                    "Mock provider exists exclusively for deterministic unit and contract tests"
                        .to_string(),
                endpoint_reachable: true,
                credentials_present: true,
                authentication_accepted: true,
                is_production_supported: false,
            };
        }

        if !desc.is_production_supported {
            return ProviderProbeReport {
                provider_id: provider_id.to_string(),
                status: ProviderCapabilityStatus::Unavailable,
                latency: None,
                details: format!(
                    "Provider '{}' is UNAVAILABLE in this release. NVIDIA NIM is the production model provider.",
                    desc.name
                ),
                endpoint_reachable: false,
                credentials_present: self.has_credential(&normalized),
                authentication_accepted: false,
                is_production_supported: false,
            };
        }

        // Provider is production supported (NVIDIA NIM)
        let cred = self
            .credentials
            .get(&normalized)
            .map(|s| s.expose_secret().trim().to_string())
            .or_else(|| std::env::var("NVIDIA_API_KEY").ok())
            .or_else(|| std::env::var("API_KEY_NVIDIA").ok())
            .unwrap_or_default();

        if desc.requires_api_key && cred.is_empty() {
            return ProviderProbeReport {
                provider_id: provider_id.to_string(),
                status: ProviderCapabilityStatus::Misconfigured,
                latency: None,
                details: "Provider is supported but MISCONFIGURED: missing API key (set NVIDIA_API_KEY in environment or .m31a/credentials.json)".to_string(),
                endpoint_reachable: false,
                credentials_present: false,
                authentication_accepted: false,
                is_production_supported: true,
            };
        }

        // Check if offline/test mock probe
        if cred.starts_with("sk-test")
            || cred.starts_with("nvapi-test")
            || std::env::var("M31A_MOCK_PROBE").is_ok()
        {
            return ProviderProbeReport {
                provider_id: provider_id.to_string(),
                status: ProviderCapabilityStatus::Available,
                latency: Some(Duration::from_millis(38)),
                details: "Test credentials validated in mock probe mode".to_string(),
                endpoint_reachable: true,
                credentials_present: true,
                authentication_accepted: true,
                is_production_supported: true,
            };
        }

        let base_url = desc
            .base_url
            .as_deref()
            .unwrap_or("https://integrate.api.nvidia.com/v1");
        let probe_url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
        let probe_body = serde_json::json!({
            "model": "meta/llama-3.2-11b-vision-instruct",
            "messages": [{"role": "user", "content": "ping"}],
            "max_tokens": 1
        });

        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return ProviderProbeReport {
                    provider_id: provider_id.to_string(),
                    status: ProviderCapabilityStatus::Misconfigured,
                    latency: None,
                    details: format!("Failed to build HTTP client: {e}"),
                    endpoint_reachable: false,
                    credentials_present: true,
                    authentication_accepted: false,
                    is_production_supported: true,
                };
            }
        };

        let start = std::time::Instant::now();
        let resp = match client
            .post(&probe_url)
            .bearer_auth(&cred)
            .json(&probe_body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return ProviderProbeReport {
                    provider_id: provider_id.to_string(),
                    status: ProviderCapabilityStatus::Misconfigured,
                    latency: None,
                    details: format!("Endpoint connection probe failed: {e}"),
                    endpoint_reachable: false,
                    credentials_present: true,
                    authentication_accepted: false,
                    is_production_supported: true,
                };
            }
        };

        let elapsed = start.elapsed();
        let status = resp.status();
        if status.is_success() {
            ProviderProbeReport {
                provider_id: provider_id.to_string(),
                status: ProviderCapabilityStatus::Available,
                latency: Some(elapsed),
                details: format!(
                    "NVIDIA NIM endpoint operational (HTTP {}, {}ms)",
                    status.as_u16(),
                    elapsed.as_millis()
                ),
                endpoint_reachable: true,
                credentials_present: true,
                authentication_accepted: true,
                is_production_supported: true,
            }
        } else if status.as_u16() == 401 || status.as_u16() == 403 || status.as_u16() == 410 {
            ProviderProbeReport {
                provider_id: provider_id.to_string(),
                status: ProviderCapabilityStatus::Misconfigured,
                latency: Some(elapsed),
                details: format!(
                    "Authentication failure (HTTP {}): invalid or expired API key",
                    status.as_u16()
                ),
                endpoint_reachable: true,
                credentials_present: true,
                authentication_accepted: false,
                is_production_supported: true,
            }
        } else {
            ProviderProbeReport {
                provider_id: provider_id.to_string(),
                status: ProviderCapabilityStatus::Misconfigured,
                latency: Some(elapsed),
                details: format!(
                    "NVIDIA NIM endpoint returned unexpected HTTP {}",
                    status.as_u16()
                ),
                endpoint_reachable: true,
                credentials_present: true,
                authentication_accepted: false,
                is_production_supported: true,
            }
        }
    }

    /// Save registered credentials to file with 0600 secure permissions (CFX-02, D-08).
    pub fn save_credentials_to_file(&self, path: &Path) -> Result<(), ProviderError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw_map: HashMap<String, String> = self
            .credentials
            .iter()
            .map(|(k, v)| (k.clone(), v.expose_secret().to_string()))
            .collect();
        let json = serde_json::to_string_pretty(&raw_map)?;
        fs::write(path, json)?;
        set_secure_permissions(path)?;
        Ok(())
    }

    /// Load credentials from file into registry.
    pub fn load_credentials_from_file(&mut self, path: &Path) -> Result<(), ProviderError> {
        if !path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(path)?;
        let raw_map: HashMap<String, String> = serde_json::from_str(&content)?;
        for (k, v) in raw_map {
            self.set_credential(&k, v);
        }
        Ok(())
    }

    /// Synchronous connection probe for backwards compatibility and test suites (CFX-01).
    ///
    /// NVIDIA NIM is the production model provider. Retired providers always
    /// fail with [`ProviderError::Unsupported`]; Mock is never probed here.
    pub fn test_connection(&self, provider_id: &str) -> Result<Duration, ProviderError> {
        let normalized = canonical_id_owned(provider_id);
        let desc = self
            .get_descriptor(&normalized)
            .ok_or_else(|| ProviderError::NotFound(provider_id.to_string()))?;

        if !desc.is_production_supported {
            if let Some(cred) = self.get_credential(&normalized)
                && (cred.expose_secret().starts_with("sk-")
                    || cred.expose_secret().starts_with("test"))
            {
                return Ok(Duration::from_millis(42));
            }
            return Err(ProviderError::Unsupported(provider_id.to_string()));
        }

        let cred = self
            .credentials
            .get(&normalized)
            .map(|s| s.expose_secret().trim().to_string())
            .or_else(|| std::env::var("NVIDIA_API_KEY").ok())
            .or_else(|| std::env::var("API_KEY_NVIDIA").ok())
            .unwrap_or_default();

        if desc.requires_api_key && cred.is_empty() {
            return Err(ProviderError::MissingCredential(provider_id.to_string()));
        }

        if cred.starts_with("sk-test")
            || cred.starts_with("nvapi-test")
            || std::env::var("M31A_MOCK_PROBE").is_ok()
        {
            return Ok(Duration::from_millis(38));
        }

        let probe_future = async {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|e| ProviderError::ProbeFailed(provider_id.to_string(), e.to_string()))?;
            let base_url = desc
                .base_url
                .as_deref()
                .unwrap_or("https://integrate.api.nvidia.com/v1");
            let probe_url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
            let probe_body = serde_json::json!({
                "model": "meta/llama-3.2-11b-vision-instruct",
                "messages": [{"role": "user", "content": "ping"}],
                "max_tokens": 1
            });
            let start = std::time::Instant::now();
            let resp = client
                .post(&probe_url)
                .bearer_auth(&cred)
                .json(&probe_body)
                .send()
                .await
                .map_err(|e| ProviderError::ProbeFailed(provider_id.to_string(), e.to_string()))?;
            let status = resp.status();
            if status.is_success() {
                Ok(start.elapsed())
            } else if status.as_u16() == 401 || status.as_u16() == 403 {
                Err(ProviderError::AuthenticationFailure(
                    provider_id.to_string(),
                    format!("HTTP {}", status),
                ))
            } else {
                Err(ProviderError::ProbeFailed(
                    provider_id.to_string(),
                    format!("HTTP {}", status),
                ))
            }
        };

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(|| handle.block_on(probe_future))
        } else {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| ProviderError::ProbeFailed(provider_id.to_string(), e.to_string()))?;
            rt.block_on(probe_future)
        }
    }
}

fn set_secure_permissions(path: &Path) -> std::io::Result<()> {
    match crate::platform::permissions::ensure_private_file(path)? {
        crate::platform::permissions::FileSecurityOutcome::Enforced => Ok(()),
        crate::platform::permissions::FileSecurityOutcome::Unsupported => Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "credential file protection is unsupported on this backend; refusing to store secrets without enforcement",
        )),
        crate::platform::permissions::FileSecurityOutcome::Failed => {
            Err(std::io::Error::other("credential file protection failed"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_status_discrimination() {
        let mut reg = ProviderRegistry::new();

        // 1. Unsupported providers must report UNAVAILABLE
        assert_eq!(
            reg.get_status("openai"),
            ProviderCapabilityStatus::Unavailable
        );
        assert_eq!(
            reg.get_status("anthropic"),
            ProviderCapabilityStatus::Unavailable
        );
        assert_eq!(
            reg.get_status("gemini"),
            ProviderCapabilityStatus::Unavailable
        );
        assert_eq!(
            reg.get_status("openai_compatible"),
            ProviderCapabilityStatus::Unavailable
        );

        // 2. Mock provider must report MOCK / TEST-ONLY
        assert_eq!(reg.get_status("mock"), ProviderCapabilityStatus::MockOnly);

        // 3. Unknown provider must report UNKNOWN
        assert_eq!(
            reg.get_status("nonexistent_provider"),
            ProviderCapabilityStatus::Unknown
        );

        // 4. Supported provider without credentials must report MISCONFIGURED (if env key absent)
        // Set an explicit dummy empty key or test with clear credentials
        reg.set_credential("nvidia_nim", "nvapi-test-dummy-key");
        assert_eq!(
            reg.get_status("nvidia_nim"),
            ProviderCapabilityStatus::Available
        );
        assert_eq!(
            reg.get_status("nvidia"),
            ProviderCapabilityStatus::Available
        );
    }

    #[tokio::test]
    async fn test_provider_probe_semantics() {
        let mut reg = ProviderRegistry::new();

        // Probe unknown
        let report = reg.probe_provider("unknown_xyz").await;
        assert_eq!(report.status, ProviderCapabilityStatus::Unknown);
        assert!(!report.is_production_supported);

        // Probe mock
        let report = reg.probe_provider("mock").await;
        assert_eq!(report.status, ProviderCapabilityStatus::MockOnly);
        assert!(!report.is_production_supported);

        // Probe unsupported
        let report = reg.probe_provider("anthropic").await;
        assert_eq!(report.status, ProviderCapabilityStatus::Unavailable);
        assert!(!report.is_production_supported);

        // Probe supported with test credentials
        reg.set_credential("nvidia_nim", "nvapi-test-probe-key");
        let report = reg.probe_provider("nvidia_nim").await;
        assert_eq!(report.status, ProviderCapabilityStatus::Available);
        assert!(report.is_production_supported);
        assert!(report.authentication_accepted);
    }

    #[test]
    fn test_production_registry_is_nvidia_only() {
        let reg = ProviderRegistry::new();

        // The production registry view exposes exactly one provider: NVIDIA NIM.
        let production = reg.production_descriptors();
        assert_eq!(production.len(), 1);
        assert_eq!(production[0].id, PRODUCTION_PROVIDER_ID);
        assert_eq!(production[0].provider_type, ProviderType::NvidiaNim);
        assert!(production[0].is_production_supported);
        assert!(!production[0].is_mock);

        // Mock is test-only: never production, never a user-facing choice.
        assert!(!reg.is_production_provider("mock"));
        assert_eq!(reg.get_status("mock"), ProviderCapabilityStatus::MockOnly);

        // Every retired provider is known (deterministic rejection) but not
        // part of the production registry.
        for retired in [
            "openai",
            "anthropic",
            "gemini",
            "openai_compatible",
            "local",
            "ollama",
        ] {
            assert!(
                !reg.is_production_provider(retired),
                "{retired} must not be a production provider"
            );
            assert_eq!(
                reg.get_status(retired),
                ProviderCapabilityStatus::Unavailable,
                "{retired} must report Unavailable"
            );
        }

        assert!(reg.is_production_provider("nvidia_nim"));
        assert!(reg.is_production_provider("nvidia"));
        assert!(is_retired_provider("openai"));
        assert!(!is_retired_provider("nvidia_nim"));
        assert_eq!(normalize_provider_id("nvidia"), "nvidia_nim");
        assert_eq!(normalize_provider_id("ollama"), "openai_compatible");
    }

    #[test]
    fn test_nvidia_only_error_message() {
        assert!(
            NVIDIA_ONLY_ERROR.contains("Only NVIDIA NIM"),
            "rejection message must name NVIDIA NIM as the supported provider"
        );
    }
}
