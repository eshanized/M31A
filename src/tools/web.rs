//! Web search and URL fetching tools (CTL-02, PRD §04).

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::family::CapabilityFamily;
use crate::capability::permissions::RiskClass;
use crate::capability::traits::web::WebSearchResult;
use crate::tools::definition::{ToolExecutionContext, TypedTool};
use crate::tools::error::ToolError;

fn default_max_results() -> usize {
    5
}

fn default_timeout_secs() -> u64 {
    30
}

/// Tool input for web search.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchInput {
    /// Search query string.
    pub query: String,
    /// Maximum number of search results (1-20, default 5).
    #[serde(default = "default_max_results")]
    pub max_results: usize,
}

/// Tool output for web search.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebSearchOutput {
    pub results: Vec<WebSearchResult>,
    pub count: usize,
}

/// Model-facing web search tool.
#[derive(Debug, Clone, Copy, Default)]
pub struct WebSearchTool;

#[async_trait]
impl TypedTool for WebSearchTool {
    type Input = WebSearchInput;
    type Output = WebSearchOutput;

    fn id(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for technical documentation, library APIs, and research information."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Web]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let web_service = ctx
            .capability_registry
            .web()
            .ok_or_else(|| ToolError::resource_not_found("web service is not configured", None))?;

        let hits = web_service
            .search_web(&input.query, input.max_results)
            .await
            .map_err(|e| ToolError::execution_failed(e.to_string(), None, None))?;

        let count = hits.len();
        Ok(WebSearchOutput {
            results: hits,
            count,
        })
    }
}

/// Tool input for fetching URL contents.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebFetchInput {
    /// Valid external HTTP or HTTPS URL to fetch.
    pub url: String,
    /// Optional timeout in seconds (default 30).
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

/// Tool output for fetching URL contents.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WebFetchOutput {
    pub status_code: u16,
    pub body: String,
    pub content_length: usize,
}

/// Model-facing web fetch tool.
#[derive(Debug, Clone, Copy, Default)]
pub struct WebFetchTool;

#[async_trait]
impl TypedTool for WebFetchTool {
    type Input = WebFetchInput;
    type Output = WebFetchOutput;

    fn id(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch text content of an external web page or documentation resource via HTTP/HTTPS."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Web]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let web_service = ctx
            .capability_registry
            .web()
            .ok_or_else(|| ToolError::resource_not_found("web service is not configured", None))?;

        let res = web_service
            .fetch_url(&input.url, input.timeout_secs)
            .await
            .map_err(|e| ToolError::execution_failed(e.to_string(), None, None))?;

        let content_length = res.body.len();
        Ok(WebFetchOutput {
            status_code: res.status_code,
            body: res.body,
            content_length,
        })
    }
}
