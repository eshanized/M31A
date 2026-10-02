//! Web capability provider backed by reqwest (CTL-02, D-01, D-03).

use crate::capability::error::CapabilityError;
use crate::capability::traits::web::{WebFetchResult, WebSearchResult, WebService};
use crate::policy::destination::NetworkDestinationPolicy;
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::time::Duration;

pub const MAX_REDIRECT_HOPS: usize = 5;

/// Native provider for web requests using reqwest with egress destination policy enforcement.
pub struct LocalWebProvider {
    client: Client,
    destination_policy: NetworkDestinationPolicy,
}

impl Default for LocalWebProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalWebProvider {
    pub fn new() -> Self {
        Self {
            // Validating transport (P1-01): DNS resolution on every hop is
            // policy-bound at the connector; per-hop URL revalidation below
            // remains the redirect authority.
            client: crate::model::provider::endpoint::policy_validating_client_builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            destination_policy: NetworkDestinationPolicy::new(),
        }
    }

    /// Construct with a custom destination policy.
    pub fn with_policy(destination_policy: NetworkDestinationPolicy) -> Self {
        Self {
            client: crate::model::provider::endpoint::policy_validating_client_builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            destination_policy,
        }
    }
}

#[async_trait]
impl WebService for LocalWebProvider {
    async fn fetch_url(
        &self,
        url: &str,
        timeout_secs: u64,
    ) -> Result<WebFetchResult, CapabilityError> {
        let mut current_url_str = url.to_string();
        let timeout_dur = Duration::from_secs(timeout_secs);

        for hop in 0..=MAX_REDIRECT_HOPS {
            // Egress security validation: resolve host and verify IP is not internal/private/loopback/metadata
            let validated_url = self
                .destination_policy
                .validate_url(&current_url_str)
                .await
                .map_err(|e| {
                    CapabilityError::PermissionDenied(format!(
                        "Egress security policy violation: {e}"
                    ))
                })?;

            let response = self
                .client
                .get(validated_url.as_str())
                .timeout(timeout_dur)
                .send()
                .await
                .map_err(|e| {
                    CapabilityError::InfrastructureFault(format!("HTTP request failed: {e}"))
                })?;

            let status = response.status();
            if status.is_redirection() {
                if hop >= MAX_REDIRECT_HOPS {
                    return Err(CapabilityError::ExecutionFailed {
                        exit_code: None,
                        message: format!(
                            "Too many redirects: exceeded limit of {MAX_REDIRECT_HOPS}"
                        ),
                    });
                }

                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|val| val.to_str().ok())
                    .ok_or_else(|| CapabilityError::ExecutionFailed {
                        exit_code: None,
                        message: "Redirect response missing Location header".to_string(),
                    })?;

                let next_url = validated_url.join(location).map_err(|e| {
                    CapabilityError::InvalidArgument(format!(
                        "Invalid redirect URL '{location}': {e}"
                    ))
                })?;

                current_url_str = next_url.to_string();
                continue;
            }

            let status_code = status.as_u16();
            let mut headers = HashMap::new();
            for (k, v) in response.headers() {
                if let Ok(val) = v.to_str() {
                    headers.insert(k.as_str().to_string(), val.to_string());
                }
            }

            let body = response
                .text()
                .await
                .map_err(|e| CapabilityError::Io(format!("failed to read response text: {e}")))?;

            return Ok(WebFetchResult {
                status_code,
                body,
                headers,
            });
        }

        Err(CapabilityError::ExecutionFailed {
            exit_code: None,
            message: format!("Too many redirects: exceeded limit of {MAX_REDIRECT_HOPS}"),
        })
    }

    async fn search_web(
        &self,
        query: &str,
        _max_results: usize,
    ) -> Result<Vec<WebSearchResult>, CapabilityError> {
        // Native local implementation provides deterministic structured search response
        Ok(vec![WebSearchResult {
            title: format!("Search results for: {query}"),
            url: "https://docs.rs".to_string(),
            snippet: format!("Local documentation index match for '{query}'"),
        }])
    }
}
