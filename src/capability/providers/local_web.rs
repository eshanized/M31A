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
                .timeout(Duration::from_secs(
                    crate::config::canonical::DEFAULT_OUTBOUND_HTTP_TIMEOUT_SECS,
                ))
                .build()
                .unwrap_or_default(),
            destination_policy: NetworkDestinationPolicy::new(),
        }
    }

    /// Construct with a custom destination policy.
    pub fn with_policy(destination_policy: NetworkDestinationPolicy) -> Self {
        Self {
            client: crate::model::provider::endpoint::policy_validating_client_builder()
                .timeout(Duration::from_secs(
                    crate::config::canonical::DEFAULT_OUTBOUND_HTTP_TIMEOUT_SECS,
                ))
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

            let full_text = response
                .text()
                .await
                .map_err(|e| CapabilityError::Io(format!("failed to read response text: {e}")))?;

            let bounded_text = if full_text.len() > 2 * 1024 * 1024 {
                full_text.chars().take(2 * 1024 * 1024).collect::<String>()
            } else {
                full_text
            };

            let scrubbed_body =
                crate::telemetry::redactor::SecretRedactor::new().redact_text(&bounded_text);

            return Ok(WebFetchResult {
                status_code,
                body: scrubbed_body,
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
        max_results: usize,
    ) -> Result<Vec<WebSearchResult>, CapabilityError> {
        let trimmed_query = query.trim();
        if trimmed_query.is_empty() {
            return Ok(Vec::new());
        }

        let limit = max_results.clamp(1, 20);
        let redactor = crate::telemetry::redactor::SecretRedactor::new();
        let mut results = Vec::new();

        // 1. DuckDuckGo Instant Answer JSON API
        let ddg_json_url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            trimmed_query
        );

        if let Ok(validated_url) = self.destination_policy.validate_url(&ddg_json_url).await {
            let req = self
                .client
                .get(validated_url.as_str())
                .timeout(Duration::from_secs(10))
                .header(
                    reqwest::header::USER_AGENT,
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) M31A/0.1.5",
                )
                .send()
                .await;

            if let Ok(resp) = req {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        let abstract_text = json
                            .get("AbstractText")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        let abstract_url = json
                            .get("AbstractURL")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        let heading = json
                            .get("Heading")
                            .and_then(|v| v.as_str())
                            .unwrap_or(trimmed_query);

                        if !abstract_text.is_empty() && !abstract_url.is_empty() {
                            results.push(WebSearchResult {
                                title: redactor.redact_text(heading),
                                url: redactor.redact_text(abstract_url),
                                snippet: redactor.redact_text(abstract_text),
                            });
                        }

                        if let Some(topics) = json.get("RelatedTopics").and_then(|v| v.as_array()) {
                            for item in topics {
                                if results.len() >= limit {
                                    break;
                                }
                                if let (Some(text), Some(url)) = (
                                    item.get("Text").and_then(|v| v.as_str()),
                                    item.get("FirstURL").and_then(|v| v.as_str()),
                                ) {
                                    if !text.is_empty() && !url.is_empty() {
                                        let title: String =
                                            text.chars().take(80).collect::<String>();
                                        results.push(WebSearchResult {
                                            title: redactor.redact_text(title.trim()),
                                            url: redactor.redact_text(url),
                                            snippet: redactor.redact_text(text),
                                        });
                                    }
                                } else if let Some(subtopics) =
                                    item.get("Topics").and_then(|v| v.as_array())
                                {
                                    for sub in subtopics {
                                        if results.len() >= limit {
                                            break;
                                        }
                                        if let (Some(text), Some(url)) = (
                                            sub.get("Text").and_then(|v| v.as_str()),
                                            sub.get("FirstURL").and_then(|v| v.as_str()),
                                        ) {
                                            if !text.is_empty() && !url.is_empty() {
                                                let title: String =
                                                    text.chars().take(80).collect::<String>();
                                                results.push(WebSearchResult {
                                                    title: redactor.redact_text(title.trim()),
                                                    url: redactor.redact_text(url),
                                                    snippet: redactor.redact_text(text),
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Fallback to DuckDuckGo HTML Lite search if Instant Answer yielded no results
        if results.is_empty() {
            let ddg_html_url = format!("https://html.duckduckgo.com/html/?q={}", trimmed_query);
            if let Ok(validated_url) = self.destination_policy.validate_url(&ddg_html_url).await {
                let req = self
                    .client
                    .get(validated_url.as_str())
                    .timeout(Duration::from_secs(10))
                    .header(
                        reqwest::header::USER_AGENT,
                        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) M31A/0.1.5",
                    )
                    .send()
                    .await;

                if let Ok(resp) = req {
                    if resp.status().is_success() {
                        if let Ok(html) = resp.text().await {
                            let link_re = regex::Regex::new(
                                r#"<a\s+class="result__url"\s+href="([^"]+)"[^>]*>(.*?)</a>"#,
                            )
                            .ok();
                            let snippet_re =
                                regex::Regex::new(r#"<a\s+class="result__snippet"[^>]*>(.*?)</a>"#)
                                    .ok();
                            if let (Some(l_re), Some(s_re)) = (link_re, snippet_re) {
                                let links: Vec<String> = l_re
                                    .captures_iter(&html)
                                    .filter_map(|c| c.get(1).map(|m| clean_html(m.as_str())))
                                    .collect();
                                let snippets: Vec<String> = s_re
                                    .captures_iter(&html)
                                    .filter_map(|c| c.get(1).map(|m| clean_html(m.as_str())))
                                    .collect();
                                for (url, snippet) in
                                    links.into_iter().zip(snippets.into_iter()).take(limit)
                                {
                                    if !url.is_empty() && !snippet.is_empty() {
                                        let title: String =
                                            snippet.chars().take(80).collect::<String>();
                                        results.push(WebSearchResult {
                                            title: redactor.redact_text(title.trim()),
                                            url: redactor.redact_text(&url),
                                            snippet: redactor.redact_text(&snippet),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(results)
    }
}

fn clean_html(input: &str) -> String {
    let tag_re = regex::Regex::new(r"<[^>]*>").unwrap();
    let stripped = tag_re.replace_all(input, "");
    stripped
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .trim()
        .to_string()
}
