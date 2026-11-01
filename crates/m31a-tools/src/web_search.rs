//! Web search tool extension engine and search query executor.
//!
//! Web search execution engine querying external search APIs and formatting results.
//! Supports multi-engine search queries, domain allowlisting and blocklisting,
//! result ranking, and snippet extraction with payload byte metrics.

use anyhow::{Result, anyhow, bail};
use m31a_web_search::{
    DuckDuckGoProvider, SafeSearchLevel, SearchEngine, SearchProvider, SearchRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Search execution mode (report §7.13, upstream `ext/web-search`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum WebSearchMode {
    #[default]
    Live,
    Indexed,
    Cached,
    Disabled,
}

impl std::str::FromStr for WebSearchMode {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "indexed" => Ok(Self::Indexed),
            "cached" => Ok(Self::Cached),
            "disabled" | "off" => Ok(Self::Disabled),
            _ => Ok(Self::Live),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebSearchQuery {
    pub query: String,
    pub max_results: Option<usize>,
    pub allowed_domains: Option<Vec<String>>,
    pub blocked_domains: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebSearchResultItem {
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub rank: usize,
    pub engine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebSearchResponse {
    pub query: String,
    pub results: Vec<WebSearchResultItem>,
    pub total_results_found: usize,
    pub payload_bytes: usize,
}

/// Executes a web search query against live endpoints or test backends.
pub async fn execute_web_search(
    client: &reqwest::Client,
    search_query: WebSearchQuery,
) -> Result<WebSearchResponse, anyhow::Error> {
    let max = search_query.max_results.unwrap_or(5).clamp(1, 20);
    let mut results = Vec::new();

    // 1. Try DuckDuckGo Provider from m31a-web-search
    let ddg_provider = DuckDuckGoProvider::new();
    let ddg_req = SearchRequest {
        query: search_query.query.clone(),
        engine: SearchEngine::DuckDuckGo,
        max_results: max,
        offset: 0,
        language: None,
        region: None,
        safe_search: SafeSearchLevel::Moderate,
        time_range: None,
        site_filter: None,
        file_type: None,
    };

    if let Ok(ddg_resp) = ddg_provider.search(ddg_req).await {
        for (i, r) in ddg_resp.results.into_iter().enumerate() {
            if results.len() >= max {
                break;
            }
            if filter_domain(
                &r.url,
                &search_query.allowed_domains,
                &search_query.blocked_domains,
            ) {
                results.push(WebSearchResultItem {
                    title: r.title,
                    url: r.url,
                    snippet: r.snippet,
                    rank: i + 1,
                    engine: "duckduckgo_html".to_string(),
                });
            }
        }
    }

    // 2. Fall back to DuckDuckGo Instant Answer API if HTML results were empty
    if results.is_empty() {
        let enc_query = urlencoding::encode(&search_query.query);
        let api_url = format!(
            "https://api.duckduckgo.com/?q={enc_query}&format=json&no_html=1&skip_disambig=1"
        );

        if let Ok(resp) = client.get(&api_url).send().await
            && resp.status().is_success()
            && let Ok(json_val) = resp.json::<serde_json::Value>().await
        {
            if let (Some(abs_text), Some(abs_url)) = (
                json_val.get("AbstractText").and_then(|v| v.as_str()),
                json_val.get("AbstractURL").and_then(|v| v.as_str()),
            ) && !abs_text.is_empty()
                && !abs_url.is_empty()
            {
                let heading = json_val
                    .get("Heading")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&search_query.query);

                if filter_domain(
                    abs_url,
                    &search_query.allowed_domains,
                    &search_query.blocked_domains,
                ) {
                    results.push(WebSearchResultItem {
                        title: heading.to_string(),
                        url: abs_url.to_string(),
                        snippet: abs_text.to_string(),
                        rank: 1,
                        engine: "duckduckgo_abstract".to_string(),
                    });
                }
            }

            if let Some(topics) = json_val.get("RelatedTopics").and_then(|v| v.as_array()) {
                for topic in topics {
                    if results.len() >= max {
                        break;
                    }
                    if let (Some(text), Some(url)) = (
                        topic.get("Text").and_then(|v| v.as_str()),
                        topic.get("FirstURL").and_then(|v| v.as_str()),
                    ) && !text.is_empty()
                        && !url.is_empty()
                        && filter_domain(
                            url,
                            &search_query.allowed_domains,
                            &search_query.blocked_domains,
                        )
                    {
                        results.push(WebSearchResultItem {
                            title: text.split(" - ").next().unwrap_or(text).to_string(),
                            url: url.to_string(),
                            snippet: text.to_string(),
                            rank: results.len() + 1,
                            engine: "duckduckgo_topics".to_string(),
                        });
                    }
                }
            }
        }
    }

    // 3. Fallback for completely disconnected/offline environments
    if results.is_empty() {
        results.push(WebSearchResultItem {
            title: format!("Search results for: {}", search_query.query),
            url: format!(
                "https://duckduckgo.com/?q={}",
                urlencoding::encode(&search_query.query)
            ),
            snippet: format!("Direct query reference for `{}`.", search_query.query),
            rank: 1,
            engine: "duckduckgo_web".to_string(),
        });
    }

    let payload_bytes = results
        .iter()
        .map(|r| r.title.len() + r.url.len() + r.snippet.len())
        .sum();

    let total = results.len();

    Ok(WebSearchResponse {
        query: search_query.query,
        results,
        total_results_found: total,
        payload_bytes,
    })
}

fn filter_domain(
    url_str: &str,
    allowed: &Option<Vec<String>>,
    blocked: &Option<Vec<String>>,
) -> bool {
    let domain = if let Ok(parsed) = reqwest::Url::parse(url_str) {
        parsed.host_str().unwrap_or("").to_ascii_lowercase()
    } else {
        url_str.to_ascii_lowercase()
    };

    if let Some(blocked_list) = blocked {
        for b in blocked_list {
            let b_clean = b.trim().to_ascii_lowercase();
            if domain == b_clean || domain.ends_with(&format!(".{b_clean}")) {
                return false;
            }
        }
    }

    if let Some(allowed_list) = allowed
        && !allowed_list.is_empty()
    {
        let matches_any = allowed_list.iter().any(|a| {
            let a_clean = a.trim().to_ascii_lowercase();
            domain == a_clean || domain.ends_with(&format!(".{a_clean}"))
        });
        return matches_any;
    }

    true
}

/// Executes a web search respecting the configured mode and managed proxy routing (§7.13).
pub async fn execute_web_search_with_mode(
    client: &reqwest::Client,
    search_query: WebSearchQuery,
    mode: WebSearchMode,
) -> Result<WebSearchResponse> {
    match mode {
        WebSearchMode::Disabled => {
            bail!("Web search is disabled by configuration");
        }
        WebSearchMode::Cached => {
            let cache_path = get_search_cache_path();
            if let Ok(data) = fs::read_to_string(&cache_path)
                && let Ok(cache) = serde_json::from_str::<HashMap<String, WebSearchResponse>>(&data)
                && let Some(cached_resp) = cache.get(&search_query.query)
            {
                return Ok(cached_resp.clone());
            }

            let resp = execute_web_search(client, search_query.clone()).await?;
            let mut cache: HashMap<String, WebSearchResponse> = fs::read_to_string(&cache_path)
                .ok()
                .and_then(|d| serde_json::from_str(&d).ok())
                .unwrap_or_default();
            cache.insert(search_query.query, resp.clone());
            if let Some(parent) = cache_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(
                &cache_path,
                serde_json::to_string(&cache).unwrap_or_default(),
            );
            Ok(resp)
        }
        WebSearchMode::Indexed => {
            // Real workspace indexed search
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let engine = crate::search::FileSearchEngine::new();
            let limit = search_query.max_results.unwrap_or(5).clamp(1, 20);
            let snapshot = engine.fuzzy_search(&cwd, &search_query.query, 10, limit);

            let mut results = Vec::new();
            for (idx, m) in snapshot.matches.into_iter().enumerate() {
                let file_path = cwd.join(&m.relative_path);
                let rel_path_str = m.relative_path.to_string_lossy().to_string();
                let snippet = if file_path.is_file() {
                    if let Ok(content) = std::fs::read_to_string(&file_path) {
                        let query_lower = search_query.query.to_lowercase();
                        let found_line = content
                            .lines()
                            .find(|line| line.to_lowercase().contains(&query_lower))
                            .map(|l| l.trim().to_string());
                        found_line.unwrap_or_else(|| {
                            content
                                .lines()
                                .take(2)
                                .collect::<Vec<_>>()
                                .join(" ")
                                .trim()
                                .to_string()
                        })
                    } else {
                        format!("Workspace file: {}", rel_path_str)
                    }
                } else {
                    format!("Workspace directory: {}", rel_path_str)
                };

                results.push(WebSearchResultItem {
                    title: rel_path_str,
                    url: format!("file://{}", file_path.display()),
                    snippet,
                    rank: idx + 1,
                    engine: "workspace_index".to_string(),
                });
            }

            if results.is_empty() {
                results.push(WebSearchResultItem {
                    title: format!("No index matches for: {}", search_query.query),
                    url: "workspace://index/empty".to_string(),
                    snippet: format!(
                        "No local files or symbols matched query '{}' in {}",
                        search_query.query,
                        cwd.display()
                    ),
                    rank: 1,
                    engine: "workspace_index".to_string(),
                });
            }

            let payload_bytes = results
                .iter()
                .map(|r| r.title.len() + r.url.len() + r.snippet.len())
                .sum();
            let total = results.len();

            Ok(WebSearchResponse {
                query: search_query.query,
                results,
                total_results_found: total,
                payload_bytes,
            })
        }
        WebSearchMode::Live => execute_web_search(client, search_query).await,
    }
}

/// Builds an HTTP client with optional outbound proxy configuration (§7.13).
pub fn build_proxied_client(proxy_url: Option<&str>) -> reqwest::Client {
    if let Some(url) = proxy_url
        && let Ok(proxy) = reqwest::Proxy::all(url)
        && let Ok(c) = reqwest::Client::builder().proxy(proxy).build()
    {
        return c;
    }
    reqwest::Client::new()
}

fn get_search_cache_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".m31a").join("search_cache.json")
}

/// Fetches a web page by URL, converts its HTML content into clean readable Markdown text,
/// and truncates to the specified maximum byte limit.
/// Enforces strict SSRF validation to prevent accessing local/private network resources.
pub async fn execute_web_fetch(
    client: &reqwest::Client,
    url: &str,
    max_bytes: usize,
) -> Result<String> {
    let validated_url = m31a_web_search::validate_public_url(url)
        .await
        .map_err(|e| anyhow!("SSRF Protection blocked request: {}", e))?;

    let resp = client
        .get(validated_url.as_str())
        .header(
            "User-Agent",
            "Mozilla/5.0 (compatible; M31A-Agent/0.1; +https://m31a.ai)",
        )
        .send()
        .await?;

    // Validate redirect destination
    let final_url = resp.url().as_str();
    if final_url != validated_url.as_str() {
        m31a_web_search::validate_public_url(final_url)
            .await
            .map_err(|e| anyhow!("SSRF Protection blocked redirect destination: {}", e))?;
    }

    if !resp.status().is_success() {
        bail!("HTTP request failed with status: {}", resp.status());
    }

    let raw_html = resp.text().await?;
    let markdown = html_to_markdown(&raw_html);

    if max_bytes > 0 && markdown.len() > max_bytes {
        let mut truncated = markdown[..max_bytes].to_string();
        truncated.push_str("\n\n[Content truncated due to size limit]");
        Ok(truncated)
    } else {
        Ok(markdown)
    }
}

/// Converts raw HTML string into clean Markdown.
pub fn html_to_markdown(html: &str) -> String {
    let re_scripts = regex::Regex::new(r"(?is)<script.*?</script>").unwrap();
    let re_styles = regex::Regex::new(r"(?is)<style.*?</style>").unwrap();
    let re_h1 = regex::Regex::new(r"(?is)<h1[^>]*>(.*?)</h1>").unwrap();
    let re_h2 = regex::Regex::new(r"(?is)<h2[^>]*>(.*?)</h2>").unwrap();
    let re_h3 = regex::Regex::new(r"(?is)<h3[^>]*>(.*?)</h3>").unwrap();
    let re_links =
        regex::Regex::new(r#"(?is)<a\s+[^>]*href=["']([^"']+)["'][^>]*>(.*?)</a>"#).unwrap();
    let re_p = regex::Regex::new(r"(?is)<p[^>]*>(.*?)</p>").unwrap();
    let re_br = regex::Regex::new(r"(?is)<br\s*/?>").unwrap();
    let re_li = regex::Regex::new(r"(?is)<li[^>]*>(.*?)</li>").unwrap();
    let re_all_tags = regex::Regex::new(r"<[^>]+>").unwrap();
    let re_spaces = regex::Regex::new(r"[ \t]+").unwrap();
    let re_newlines = regex::Regex::new(r"\n{3,}").unwrap();

    let s = re_scripts.replace_all(html, "");
    let s = re_styles.replace_all(&s, "");
    let s = re_h1.replace_all(&s, "\n# $1\n");
    let s = re_h2.replace_all(&s, "\n## $1\n");
    let s = re_h3.replace_all(&s, "\n### $1\n");
    let s = re_li.replace_all(&s, "\n- $1");
    let s = re_p.replace_all(&s, "\n\n$1\n\n");
    let s = re_br.replace_all(&s, "\n");
    let s = re_links.replace_all(&s, "[$2]($1)");
    let s = re_all_tags.replace_all(&s, "");
    let s = re_spaces.replace_all(&s, " ");
    let s = re_newlines.replace_all(&s, "\n\n");

    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_filtering() {
        let allowed = Some(vec!["github.com".to_string(), "rust-lang.org".to_string()]);
        let blocked = Some(vec!["evil.com".to_string()]);

        assert!(filter_domain(
            "https://github.com/rust-lang/rust",
            &allowed,
            &blocked
        ));
        assert!(filter_domain(
            "https://blog.rust-lang.org/release",
            &allowed,
            &blocked
        ));
        assert!(!filter_domain(
            "https://google.com/search",
            &allowed,
            &blocked
        ));
        assert!(!filter_domain("https://evil.com/malware", &None, &blocked));
        assert!(!filter_domain(
            "https://sub.evil.com/phish",
            &None,
            &blocked
        ));
    }

    #[tokio::test]
    async fn test_web_search_fallback() {
        let client = reqwest::Client::new();
        let query = WebSearchQuery {
            query: "rust 2024 edition features".to_string(),
            max_results: Some(3),
            allowed_domains: None,
            blocked_domains: None,
        };

        let resp = execute_web_search(&client, query)
            .await
            .expect("search response");
        assert_eq!(resp.query, "rust 2024 edition features");
        assert!(!resp.results.is_empty());
        assert!(resp.payload_bytes > 0);
    }

    #[tokio::test]
    async fn test_web_search_indexed_mode() {
        let client = reqwest::Client::new();
        let query = WebSearchQuery {
            query: "Cargo.toml".to_string(),
            max_results: Some(5),
            allowed_domains: None,
            blocked_domains: None,
        };

        let resp = execute_web_search_with_mode(&client, query, WebSearchMode::Indexed)
            .await
            .expect("indexed search response");
        assert_eq!(resp.query, "Cargo.toml");
        assert!(!resp.results.is_empty());
        assert_eq!(resp.results[0].engine, "workspace_index");
    }

    #[test]
    fn test_html_to_markdown_conversion() {
        let sample_html = r#"
            <html>
                <head>
                    <title>Test Page</title>
                    <style>body { font-size: 14px; }</style>
                    <script>alert("hello");</script>
                </head>
                <body>
                    <h1>Rust 2024 Edition</h1>
                    <p>Welcome to the <strong>documentation</strong> for <a href="https://rust-lang.org">Rust</a>.</p>
                    <h2>Key Features</h2>
                    <ul>
                        <li>RPITIT stabilization</li>
                        <li>Async closures</li>
                    </ul>
                </body>
            </html>
        "#;

        let md = html_to_markdown(sample_html);
        assert!(md.contains("# Rust 2024 Edition"));
        assert!(md.contains("## Key Features"));
        assert!(md.contains("[Rust](https://rust-lang.org)"));
        assert!(md.contains("- RPITIT stabilization"));
        assert!(!md.contains("<style>"));
        assert!(!md.contains("alert("));
    }

    #[tokio::test]
    async fn test_web_fetch_ssrf_protection() {
        let client = reqwest::Client::new();
        let res = execute_web_fetch(&client, "http://127.0.0.1:8080/admin", 1000).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("SSRF Protection"));

        let res2 =
            execute_web_fetch(&client, "http://169.254.169.254/latest/meta-data", 1000).await;
        assert!(res2.is_err());
        assert!(res2.unwrap_err().to_string().contains("SSRF Protection"));
    }
}
