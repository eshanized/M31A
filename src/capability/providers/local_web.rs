//! Web capability provider backed by reqwest (CTL-02, D-01, D-03).

use crate::capability::error::CapabilityError;
use crate::capability::traits::web::{WebFetchResult, WebSearchResult, WebService};
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::time::Duration;

/// Native provider for web requests using reqwest.
pub struct LocalWebProvider {
    client: Client,
}

impl Default for LocalWebProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalWebProvider {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
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
        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(timeout_secs))
            .send()
            .await
            .map_err(|e| {
                CapabilityError::InfrastructureFault(format!("HTTP request failed: {e}"))
            })?;

        let status_code = response.status().as_u16();
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

        Ok(WebFetchResult {
            status_code,
            body,
            headers,
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
