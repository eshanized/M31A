//! Web access and external search service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Response from an HTTP fetch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WebFetchResult {
    pub status_code: u16,
    pub body: String,
    pub headers: HashMap<String, String>,
}

/// Search hit from web query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Asynchronous service seam for web fetching and search.
#[async_trait]
pub trait WebService: Send + Sync + 'static {
    /// Fetch resource content from a URL under timeout.
    async fn fetch_url(
        &self,
        url: &str,
        timeout_secs: u64,
    ) -> Result<WebFetchResult, CapabilityError>;

    /// Query web search engine for top results.
    async fn search_web(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<WebSearchResult>, CapabilityError>;
}
