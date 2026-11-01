//! Web Search Extension
//!
//! Provides web search capabilities through various search engines
//! and APIs (Google, Bing, DuckDuckGo, Brave, SerpAPI, etc.)

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Boxed future returned by async provider methods; keeps the trait object-safe.
pub type SearchFuture<'a> = Pin<Box<dyn Future<Output = Result<SearchResponse>> + Send + 'a>>;

/// Web search request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub engine: SearchEngine,
    pub max_results: usize,
    pub offset: usize,
    pub language: Option<String>,
    pub region: Option<String>,
    pub safe_search: SafeSearchLevel,
    pub time_range: Option<TimeRange>,
    pub site_filter: Option<String>,
    pub file_type: Option<String>,
}

/// Search engine options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SearchEngine {
    Google,
    Bing,
    DuckDuckGo,
    Brave,
    SerpApi,
    Startpage,
    Custom,
}

/// Safe search levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SafeSearchLevel {
    Off,
    Moderate,
    Strict,
}

/// Time range for results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeRange {
    PastHour,
    PastDay,
    PastWeek,
    PastMonth,
    PastYear,
    Any,
}

/// Search result item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub source: String,
    pub position: usize,
    pub metadata: Option<SearchResultMetadata>,
}

/// Additional metadata for search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultMetadata {
    pub published_date: Option<String>,
    pub author: Option<String>,
    pub domain: String,
    pub is_sponsored: bool,
    pub rich_snippet: Option<String>,
}

/// Search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub engine: SearchEngine,
    pub results: Vec<SearchResult>,
    pub total_results: Option<u64>,
    pub search_time_ms: u64,
    pub timestamp: u64,
}

/// Search provider trait
pub trait SearchProvider: Send + Sync {
    fn engine(&self) -> SearchEngine;
    fn name(&self) -> &str;
    fn supports_pagination(&self) -> bool;
    fn max_results_per_page(&self) -> usize;
    fn search(&self, request: SearchRequest) -> SearchFuture<'_>;
    fn get_rate_limit(&self) -> Option<u32>;
}

fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Brave Search API provider
pub struct BraveSearchProvider {
    api_key: String,
    api_base: String,
    client: reqwest::Client,
}

impl BraveSearchProvider {
    pub fn new(api_key: String, api_base: Option<String>) -> Self {
        Self {
            api_key,
            api_base: api_base.unwrap_or_else(|| "https://api.search.brave.com/res/v1".to_string()),
            client: reqwest::Client::new(),
        }
    }
}

impl SearchProvider for BraveSearchProvider {
    fn engine(&self) -> SearchEngine {
        SearchEngine::Brave
    }

    fn name(&self) -> &str {
        "brave"
    }

    fn supports_pagination(&self) -> bool {
        true
    }

    fn max_results_per_page(&self) -> usize {
        20
    }

    fn search(&self, request: SearchRequest) -> SearchFuture<'_> {
        Box::pin(async move {
            let start_time = std::time::Instant::now();

            // Owned values: the parameter list must outlive this statement.
            let count = request.max_results.min(20).to_string();
            let offset = request.offset.to_string();
            let mut params: Vec<(String, String)> = vec![
                ("q".to_string(), request.query.clone()),
                ("count".to_string(), count),
                ("offset".to_string(), offset),
            ];

            if let Some(lang) = &request.language {
                params.push(("search_lang".to_string(), lang.clone()));
            }
            if let Some(region) = &request.region {
                params.push(("country".to_string(), region.clone()));
            }
            params.push((
                "safesearch".to_string(),
                match request.safe_search {
                    SafeSearchLevel::Off => "off",
                    SafeSearchLevel::Moderate => "moderate",
                    SafeSearchLevel::Strict => "strict",
                }
                .to_string(),
            ));

            if let Some(range) = request.time_range {
                params.push((
                    "freshness".to_string(),
                    match range {
                        TimeRange::PastHour => "ph",
                        TimeRange::PastDay => "pd",
                        TimeRange::PastWeek => "pw",
                        TimeRange::PastMonth => "pm",
                        TimeRange::PastYear => "py",
                        TimeRange::Any => "all",
                    }
                    .to_string(),
                ));
            }

            let endpoint = format!("{}/web/search", self.api_base);
            validate_public_url(&endpoint).await?;

            let response = self
                .client
                .get(&endpoint)
                .header("Accept", "application/json")
                .header("X-Subscription-Token", &self.api_key)
                .query(&params)
                .send()
                .await
                .context("Brave Search request failed")?;

            if !response.status().is_success() {
                return Err(anyhow::anyhow!(
                    "Brave Search API error: {}",
                    response.status()
                ));
            }

            let result: serde_json::Value = response.json().await?;
            let mut results = Vec::new();

            if let Some(web_results) = result
                .get("web")
                .and_then(|web| web.get("results"))
                .and_then(|v| v.as_array())
            {
                for (i, item) in web_results.iter().enumerate() {
                    let url = item["url"].as_str().unwrap_or("").to_string();
                    let domain = url::Url::parse(&url)
                        .ok()
                        .and_then(|u| u.host_str().map(|s| s.to_string()))
                        .unwrap_or_default();
                    results.push(SearchResult {
                        title: item["title"].as_str().unwrap_or("").to_string(),
                        url,
                        snippet: item["description"].as_str().unwrap_or("").to_string(),
                        source: "brave".to_string(),
                        position: i + 1 + request.offset,
                        metadata: Some(SearchResultMetadata {
                            published_date: item["age"].as_str().map(|s| s.to_string()),
                            author: None,
                            domain,
                            is_sponsored: item["is_sponsored"].as_bool().unwrap_or(false),
                            rich_snippet: None,
                        }),
                    });
                }
            }

            Ok(SearchResponse {
                query: request.query,
                engine: SearchEngine::Brave,
                results,
                total_results: result["web"]["total_results"].as_u64(),
                search_time_ms: start_time.elapsed().as_millis() as u64,
                timestamp: now_unix_secs(),
            })
        })
    }

    fn get_rate_limit(&self) -> Option<u32> {
        Some(20) // 20 requests per second for free tier
    }
}

/// SerpAPI provider (Google, Bing, etc.)
pub struct SerpApiProvider {
    api_key: String,
    api_base: String,
    client: reqwest::Client,
}

impl SerpApiProvider {
    pub fn new(api_key: String, api_base: Option<String>) -> Self {
        Self {
            api_key,
            api_base: api_base.unwrap_or_else(|| "https://serpapi.com/search".to_string()),
            client: reqwest::Client::new(),
        }
    }
}

impl SearchProvider for SerpApiProvider {
    fn engine(&self) -> SearchEngine {
        SearchEngine::SerpApi
    }

    fn name(&self) -> &str {
        "serpapi"
    }

    fn supports_pagination(&self) -> bool {
        true
    }

    fn max_results_per_page(&self) -> usize {
        100
    }

    fn search(&self, request: SearchRequest) -> SearchFuture<'_> {
        Box::pin(async move {
            let start_time = std::time::Instant::now();

            let num = request.max_results.min(100).to_string();
            let start = request.offset.to_string();
            let mut params: Vec<(String, String)> = vec![
                ("q".to_string(), request.query.clone()),
                ("num".to_string(), num),
                ("start".to_string(), start),
                ("api_key".to_string(), self.api_key.clone()),
            ];

            if let Some(lang) = &request.language {
                params.push(("hl".to_string(), lang.clone()));
            }
            if let Some(region) = &request.region {
                params.push(("gl".to_string(), region.clone()));
            }
            params.push((
                "safe".to_string(),
                match request.safe_search {
                    SafeSearchLevel::Off => "off",
                    SafeSearchLevel::Moderate => "off",
                    SafeSearchLevel::Strict => "active",
                }
                .to_string(),
            ));

            if let Some(range) = request.time_range {
                params.push((
                    "tbs".to_string(),
                    match range {
                        TimeRange::PastHour => "qdr:h",
                        TimeRange::PastDay => "qdr:d",
                        TimeRange::PastWeek => "qdr:w",
                        TimeRange::PastMonth => "qdr:m",
                        TimeRange::PastYear => "qdr:y",
                        TimeRange::Any => "",
                    }
                    .to_string(),
                ));
            }

            if let Some(site) = &request.site_filter {
                params.push(("as_sitesearch".to_string(), site.clone()));
            }
            if let Some(file_type) = &request.file_type {
                params.push(("file_type".to_string(), file_type.clone()));
            }

            validate_public_url(&self.api_base).await?;

            let response = self
                .client
                .get(&self.api_base)
                .query(&params)
                .send()
                .await
                .context("SerpAPI request failed")?;

            if !response.status().is_success() {
                return Err(anyhow::anyhow!("SerpAPI error: {}", response.status()));
            }

            let result: serde_json::Value = response.json().await?;
            let mut results = Vec::new();

            if let Some(organic) = result.get("organic_results").and_then(|v| v.as_array()) {
                for (i, item) in organic.iter().enumerate() {
                    let url = item["link"].as_str().unwrap_or("").to_string();
                    let domain = url::Url::parse(&url)
                        .ok()
                        .and_then(|u| u.host_str().map(|s| s.to_string()))
                        .unwrap_or_default();
                    results.push(SearchResult {
                        title: item["title"].as_str().unwrap_or("").to_string(),
                        url,
                        snippet: item["snippet"].as_str().unwrap_or("").to_string(),
                        source: "serpapi".to_string(),
                        position: i + 1 + request.offset,
                        metadata: Some(SearchResultMetadata {
                            published_date: item["date"].as_str().map(|s| s.to_string()),
                            author: None,
                            domain,
                            is_sponsored: item["is_sponsored"].as_bool().unwrap_or(false),
                            rich_snippet: item["rich_snippet"].as_str().map(|s| s.to_string()),
                        }),
                    });
                }
            }

            Ok(SearchResponse {
                query: request.query,
                engine: SearchEngine::SerpApi,
                results,
                total_results: result["search_information"]["total_results"].as_u64(),
                search_time_ms: start_time.elapsed().as_millis() as u64,
                timestamp: now_unix_secs(),
            })
        })
    }

    fn get_rate_limit(&self) -> Option<u32> {
        Some(100) // Depends on plan
    }
}

/// DuckDuckGo HTML scraping provider (free, no API key)
pub struct DuckDuckGoProvider {
    client: reqwest::Client,
}

impl DuckDuckGoProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("M31A/0.1.13 (compatible; WebSearch)")
                .build()
                .unwrap(),
        }
    }
}

impl Default for DuckDuckGoProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// DuckDuckGo's HTML endpoint returns redirect links of the form
/// `//duckduckgo.com/l/?uddg=<percent-encoded target>&rut=...`; unwrap the
/// real target when present, otherwise pass the href through unchanged.
fn decode_ddg_href(href: &str) -> String {
    let candidate = if href.starts_with("//") {
        format!("https:{href}")
    } else {
        href.to_string()
    };
    if let Ok(parsed) = url::Url::parse(&candidate)
        && parsed.path() == "/l/"
    {
        for (key, value) in parsed.query_pairs() {
            if key == "uddg" {
                return value.into_owned();
            }
        }
    }
    candidate
}

impl SearchProvider for DuckDuckGoProvider {
    fn engine(&self) -> SearchEngine {
        SearchEngine::DuckDuckGo
    }

    fn name(&self) -> &str {
        "duckduckgo"
    }

    fn supports_pagination(&self) -> bool {
        true
    }

    fn max_results_per_page(&self) -> usize {
        50
    }

    fn search(&self, request: SearchRequest) -> SearchFuture<'_> {
        Box::pin(async move {
            let start_time = std::time::Instant::now();

            let params = [
                ("q", request.query.as_str()),
                ("kl", request.region.as_deref().unwrap_or("us-en")),
            ];

            let response = self
                .client
                .get("https://html.duckduckgo.com/html/")
                .query(&params)
                .send()
                .await
                .context("DuckDuckGo request failed")?;

            let html = response.text().await?;
            let mut results = Vec::new();

            // `tl` exposes query_selector on the VDom (not the Parser), it
            // returns Option, and there is no parent traversal — so pair the
            // title links and snippets by document order instead.
            let document = tl::parse(&html, tl::ParserOptions::default())?;
            let parser = document.parser();

            let mut titles_and_urls: Vec<(String, String)> = Vec::new();
            if let Some(iter) = document.query_selector(".result__a") {
                for handle in iter {
                    let Some(tag) = handle.get(parser).and_then(|node| node.as_tag()) else {
                        continue;
                    };
                    let title = tag.inner_text(parser).to_string();
                    let href = tag
                        .attributes()
                        .get("href")
                        .and_then(|v| v)
                        .map(|b| b.as_utf8_str().to_string())
                        .unwrap_or_default();
                    if !title.is_empty() && !href.is_empty() {
                        titles_and_urls.push((title, decode_ddg_href(&href)));
                    }
                }
            }

            let mut snippets: Vec<String> = Vec::new();
            if let Some(iter) = document.query_selector(".result__snippet") {
                for handle in iter {
                    if let Some(node) = handle.get(parser) {
                        snippets.push(node.inner_text(parser).to_string());
                    }
                }
            }

            for (i, (title, url)) in titles_and_urls.into_iter().enumerate() {
                if i >= request.max_results {
                    break;
                }
                let snippet = snippets.get(i).cloned().unwrap_or_default();
                let domain = url::Url::parse(&url)
                    .ok()
                    .and_then(|u| u.host_str().map(|s| s.to_string()))
                    .unwrap_or_default();
                results.push(SearchResult {
                    title,
                    url,
                    snippet,
                    source: "duckduckgo".to_string(),
                    position: i + 1,
                    metadata: Some(SearchResultMetadata {
                        published_date: None,
                        author: None,
                        domain,
                        is_sponsored: false,
                        rich_snippet: None,
                    }),
                });
            }

            Ok(SearchResponse {
                query: request.query,
                engine: SearchEngine::DuckDuckGo,
                results,
                total_results: None,
                search_time_ms: start_time.elapsed().as_millis() as u64,
                timestamp: now_unix_secs(),
            })
        })
    }

    fn get_rate_limit(&self) -> Option<u32> {
        None // No official rate limit, but be respectful
    }
}

/// Custom search provider for internal/search engines
pub struct CustomSearchProvider {
    name: String,
    base_url: String,
    client: reqwest::Client,
    auth_header: Option<String>,
}

impl CustomSearchProvider {
    pub fn new(name: String, base_url: String, auth_header: Option<String>) -> Self {
        Self {
            name,
            base_url,
            client: reqwest::Client::new(),
            auth_header,
        }
    }
}

impl SearchProvider for CustomSearchProvider {
    fn engine(&self) -> SearchEngine {
        SearchEngine::Custom
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn supports_pagination(&self) -> bool {
        true
    }

    fn max_results_per_page(&self) -> usize {
        50
    }

    fn search(&self, request: SearchRequest) -> SearchFuture<'_> {
        Box::pin(async move {
            validate_public_url(&self.base_url).await?;
            let start_time = std::time::Instant::now();

            let limit = request.max_results.to_string();
            let offset = request.offset.to_string();
            let mut req = self.client.get(&self.base_url).query(&[
                ("q", request.query.as_str()),
                ("limit", limit.as_str()),
                ("offset", offset.as_str()),
            ]);

            if let Some(auth) = &self.auth_header {
                req = req.header("Authorization", auth);
            }

            let response = req.send().await.context("Custom search request failed")?;

            if !response.status().is_success() {
                return Err(anyhow::anyhow!(
                    "Custom search error: {}",
                    response.status()
                ));
            }

            let result: serde_json::Value = response.json().await?;

            // Expect standard format: { results: [{title, url, snippet}, ...] }
            let mut results = Vec::new();
            if let Some(items) = result.get("results").and_then(|v| v.as_array()) {
                for (i, item) in items.iter().enumerate() {
                    results.push(SearchResult {
                        title: item["title"].as_str().unwrap_or("").to_string(),
                        url: item["url"].as_str().unwrap_or("").to_string(),
                        snippet: item["snippet"].as_str().unwrap_or("").to_string(),
                        source: self.name.clone(),
                        position: i + 1 + request.offset,
                        metadata: None,
                    });
                }
            }

            Ok(SearchResponse {
                query: request.query,
                engine: SearchEngine::Custom,
                results,
                total_results: result["total"].as_u64(),
                search_time_ms: start_time.elapsed().as_millis() as u64,
                timestamp: now_unix_secs(),
            })
        })
    }

    fn get_rate_limit(&self) -> Option<u32> {
        None
    }
}

/// Search manager
pub struct SearchManager {
    providers: HashMap<SearchEngine, Box<dyn SearchProvider>>,
    default_engine: SearchEngine,
    cache: Arc<RwLock<HashMap<String, CachedSearch>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedSearch {
    response: SearchResponse,
    cached_at: u64,
    ttl_seconds: u64,
}

impl SearchManager {
    pub fn new(default_engine: SearchEngine) -> Self {
        Self {
            providers: HashMap::new(),
            default_engine,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register(&mut self, engine: SearchEngine, provider: Box<dyn SearchProvider>) {
        self.providers.insert(engine, provider);
    }

    pub fn set_default(&mut self, engine: SearchEngine) {
        self.default_engine = engine;
    }

    pub async fn search(&self, mut request: SearchRequest) -> Result<SearchResponse> {
        if request.engine == SearchEngine::Custom {
            // Use default
            request.engine = self.default_engine;
        }

        let cache_key = self.cache_key(&request);

        // Check cache
        if let Some(cached) = self.cache.read().await.get(&cache_key)
            && now_unix_secs() - cached.cached_at < cached.ttl_seconds
        {
            return Ok(cached.response.clone());
        }

        let provider = self
            .providers
            .get(&request.engine)
            .with_context(|| format!("Search engine {:?} not available", request.engine))?;

        let response = provider.search(request).await?;

        // Cache result (TTL: 5 minutes)
        self.cache.write().await.insert(
            cache_key,
            CachedSearch {
                response: response.clone(),
                cached_at: now_unix_secs(),
                ttl_seconds: 300,
            },
        );

        Ok(response)
    }

    pub async fn search_all(
        &self,
        query: String,
        max_results: usize,
    ) -> Result<Vec<SearchResponse>> {
        let mut responses = Vec::new();
        for (engine, provider) in &self.providers {
            let request = SearchRequest {
                query: query.clone(),
                engine: *engine,
                max_results: max_results.min(provider.max_results_per_page()),
                offset: 0,
                language: None,
                region: None,
                safe_search: SafeSearchLevel::Moderate,
                time_range: None,
                site_filter: None,
                file_type: None,
            };

            if let Ok(response) = provider.search(request).await {
                responses.push(response);
            }
        }
        Ok(responses)
    }

    fn cache_key(&self, request: &SearchRequest) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(request.query.as_bytes());
        hasher.update(format!("{:?}", request.engine).as_bytes());
        hasher.update(request.max_results.to_string().as_bytes());
        hasher.update(request.offset.to_string().as_bytes());
        format!("{:x}", hasher.finalize())[..32].to_string()
    }

    pub fn available_engines(&self) -> Vec<SearchEngine> {
        self.providers.keys().copied().collect()
    }
}

impl Default for SearchRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            engine: SearchEngine::DuckDuckGo,
            max_results: 10,
            offset: 0,
            language: None,
            region: None,
            safe_search: SafeSearchLevel::Moderate,
            time_range: None,
            site_filter: None,
            file_type: None,
        }
    }
}

/// Validates whether an IP address is a private, loopback, link-local, or cloud metadata address.
pub fn is_private_or_restricted_ip(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            let octets = v4.octets();
            let a = octets[0];
            let b = octets[1];
            // 0.0.0.0/8
            a == 0
            // 10.0.0.0/8 (RFC 1918)
            || a == 10
            // 127.0.0.0/8 (loopback)
            || a == 127
            // 100.64.0.0/10 (carrier-grade NAT)
            || (a == 100 && (64..=127).contains(&b))
            // 169.254.0.0/16 (link-local and cloud instance metadata)
            || (a == 169 && b == 254)
            // 172.16.0.0/12 (RFC 1918)
            || (a == 172 && (16..=31).contains(&b))
            // 192.0.0.0/24 (IETF assignments)
            || (a == 192 && b == 0)
            // 192.168.0.0/16 (RFC 1918)
            || (a == 192 && b == 168)
            // 198.18.0.0/15 (benchmarking)
            || (a == 198 && (b == 18 || b == 19))
            // 224.0.0.0/4 (multicast) and 240.0.0.0/4 (reserved)
            || a >= 224
        }
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()
            || v6.is_unspecified()
            || (v6.segments()[0] & 0xfe00) == 0xfc00 // fc00::/7 unique local
            || (v6.segments()[0] & 0xffc0) == 0xfe80 // fe80::/10 link local
            || v6.to_ipv4_mapped().is_some()
        }
    }
}

/// Validates that a URL is a public HTTP/HTTPS URL and does not point to internal or private resources.
pub async fn validate_public_url(url_str: &str) -> Result<url::Url> {
    let parsed = url::Url::parse(url_str).context("Invalid URL format")?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        anyhow::bail!(
            "Only http and https protocols are permitted: {}",
            parsed.scheme()
        );
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        anyhow::bail!("URLs with embedded credentials are not allowed");
    }

    let host_str = parsed
        .host_str()
        .context("URL is missing a host")?
        .to_lowercase();
    let host_clean = host_str.trim_end_matches('.');

    if host_clean.is_empty()
        || host_clean == "localhost"
        || host_clean.ends_with(".localhost")
        || host_clean.ends_with(".local")
        || host_clean.ends_with(".internal")
        || host_clean.ends_with(".arpa")
    {
        anyhow::bail!(
            "Access to local or internal domain '{}' is blocked",
            host_clean
        );
    }

    if let Ok(ip) = host_clean.parse::<std::net::IpAddr>() {
        if is_private_or_restricted_ip(&ip) {
            anyhow::bail!(
                "Access to private/restricted IP address '{}' is blocked",
                ip
            );
        }
    } else {
        let port = parsed.port_or_known_default().unwrap_or(80);
        let host_port = format!("{}:{}", host_clean, port);
        if let Ok(addrs) = tokio::net::lookup_host(&host_port).await {
            for addr in addrs {
                if is_private_or_restricted_ip(&addr.ip()) {
                    anyhow::bail!(
                        "Host '{}' resolved to private/restricted IP address '{}', blocked",
                        host_clean,
                        addr.ip()
                    );
                }
            }
        }
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_request_defaults() {
        let req = SearchRequest::default();
        assert_eq!(req.max_results, 10);
        assert_eq!(req.engine, SearchEngine::DuckDuckGo);
        assert_eq!(req.safe_search, SafeSearchLevel::Moderate);
    }

    #[test]
    fn test_decode_ddg_href_unwraps_redirect() {
        let href = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fpage%3Fa%3D1&rut=abc";
        assert_eq!(decode_ddg_href(href), "https://example.com/page?a=1");
    }

    #[test]
    fn test_decode_ddg_href_passthrough() {
        assert_eq!(
            decode_ddg_href("https://direct.example.org/"),
            "https://direct.example.org/"
        );
    }

    #[test]
    fn test_ssrf_ip_blocking() {
        use std::net::IpAddr;
        assert!(is_private_or_restricted_ip(
            &"127.0.0.1".parse::<IpAddr>().unwrap()
        ));
        assert!(is_private_or_restricted_ip(
            &"10.0.0.1".parse::<IpAddr>().unwrap()
        ));
        assert!(is_private_or_restricted_ip(
            &"172.16.0.5".parse::<IpAddr>().unwrap()
        ));
        assert!(is_private_or_restricted_ip(
            &"192.168.1.1".parse::<IpAddr>().unwrap()
        ));
        assert!(is_private_or_restricted_ip(
            &"169.254.169.254".parse::<IpAddr>().unwrap()
        ));
        assert!(is_private_or_restricted_ip(
            &"::1".parse::<IpAddr>().unwrap()
        ));

        // Public IP should not be blocked
        assert!(!is_private_or_restricted_ip(
            &"8.8.8.8".parse::<IpAddr>().unwrap()
        ));
        assert!(!is_private_or_restricted_ip(
            &"1.1.1.1".parse::<IpAddr>().unwrap()
        ));
    }

    #[tokio::test]
    async fn test_validate_public_url_blocks_ssrf() {
        assert!(
            validate_public_url("http://127.0.0.1:8080/secret")
                .await
                .is_err()
        );
        assert!(validate_public_url("http://localhost:3000/").await.is_err());
        assert!(
            validate_public_url("http://169.254.169.254/latest/meta-data/")
                .await
                .is_err()
        );
        assert!(
            validate_public_url("http://user:pass@example.com/")
                .await
                .is_err()
        );
        assert!(validate_public_url("ftp://example.com/").await.is_err());
        assert!(
            validate_public_url("http://service.internal/")
                .await
                .is_err()
        );

        assert!(
            validate_public_url("https://example.com/test")
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn test_custom_search_provider_rejects_ssrf() {
        let provider = CustomSearchProvider::new(
            "internal_test".to_string(),
            "http://169.254.169.254/latest/meta-data".to_string(),
            None,
        );
        let req = SearchRequest {
            query: "test".to_string(),
            ..Default::default()
        };
        let res = provider.search(req).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("blocked"));
    }
}
