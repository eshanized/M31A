//! Diagnostic context extraction from HTTP transport responses.
//!
//! Captures request IDs, provider headers, rate limit status, and server latency markers.

use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const REQUEST_ID_HEADER: &str = "x-request-id";
const OAI_REQUEST_ID_HEADER: &str = "x-oai-request-id";
const ANTHROPIC_REQUEST_ID_HEADER: &str = "x-anthropic-request-id";
const GOOG_REQUEST_ID_HEADER: &str = "x-goog-request-id";
const AMZN_REQUEST_ID_HEADER: &str = "x-amzn-requestid";
const GROQ_REQUEST_ID_HEADER: &str = "x-groq-request-id";
const CF_RAY_HEADER: &str = "cf-ray";
const RETRY_AFTER_HEADER: &str = "retry-after";
const AUTH_ERROR_HEADER: &str = "x-openai-authorization-error";
const NVIDIA_REQUEST_ID_HEADER: &str = "nv-request-id";

/// Diagnostic metadata extracted from HTTP transport responses and error envelopes.
/// Diagnostic metadata extracted from HTTP transport responses and error envelopes across all supported providers.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseDebugContext {
    pub request_id: Option<String>,
    pub cf_ray: Option<String>,
    pub retry_after_secs: Option<u64>,
    pub auth_error: Option<String>,
    pub http_status: Option<u16>,
    pub latency_ms: u64,
    pub rate_limit_remaining_requests: Option<u64>,
    pub rate_limit_remaining_tokens: Option<u64>,
    pub headers_sample: HashMap<String, String>,
}

impl ResponseDebugContext {
    /// Extracts diagnostic tracking identifiers and metadata from HTTP headers.
    pub fn extract(headers: &HeaderMap, http_status: Option<u16>, latency_ms: u64) -> Self {
        let extract_str = |key: &str| -> Option<String> {
            headers
                .get(key)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };

        let request_id = extract_str(NVIDIA_REQUEST_ID_HEADER)
            .or_else(|| extract_str(ANTHROPIC_REQUEST_ID_HEADER))
            .or_else(|| extract_str(GOOG_REQUEST_ID_HEADER))
            .or_else(|| extract_str(AMZN_REQUEST_ID_HEADER))
            .or_else(|| extract_str(GROQ_REQUEST_ID_HEADER))
            .or_else(|| extract_str(REQUEST_ID_HEADER))
            .or_else(|| extract_str(OAI_REQUEST_ID_HEADER));

        let cf_ray = extract_str(CF_RAY_HEADER);
        let auth_error = extract_str(AUTH_ERROR_HEADER);

        let retry_after_secs =
            extract_str(RETRY_AFTER_HEADER).and_then(|val| val.parse::<u64>().ok());

        let rate_limit_remaining_requests = extract_str("x-ratelimit-remaining-requests")
            .or_else(|| extract_str("x-ratelimit-remaining"))
            .and_then(|val| val.parse::<u64>().ok());

        let rate_limit_remaining_tokens =
            extract_str("x-ratelimit-remaining-tokens").and_then(|val| val.parse::<u64>().ok());

        let mut sample = HashMap::new();
        for key in [
            "content-type",
            "server",
            "x-ratelimit-remaining",
            "x-ratelimit-remaining-requests",
            "x-ratelimit-remaining-tokens",
            "x-ratelimit-reset",
        ] {
            if let Some(v) = extract_str(key) {
                sample.insert(key.to_string(), v);
            }
        }

        Self {
            request_id,
            cf_ray,
            retry_after_secs,
            auth_error,
            http_status,
            latency_ms,
            rate_limit_remaining_requests,
            rate_limit_remaining_tokens,
            headers_sample: sample,
        }
    }

    /// Returns a concise, single-line telemetry diagnostic summary.
    pub fn format_summary(&self) -> String {
        let mut parts = Vec::new();
        if let Some(status) = self.http_status {
            parts.push(format!("status={status}"));
        }
        parts.push(format!("latency={}ms", self.latency_ms));
        if let Some(ref req_id) = self.request_id {
            parts.push(format!("req_id={req_id}"));
        }
        if let Some(ref ray) = self.cf_ray {
            parts.push(format!("cf_ray={ray}"));
        }
        if let Some(retry) = self.retry_after_secs {
            parts.push(format!("retry_after={}s", retry));
        }
        if let Some(ref auth) = self.auth_error {
            parts.push(format!("auth_err={auth}"));
        }
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn test_extract_debug_context_full() {
        let mut headers = HeaderMap::new();
        headers.insert(REQUEST_ID_HEADER, HeaderValue::from_static("req-abc-123"));
        headers.insert(CF_RAY_HEADER, HeaderValue::from_static("8f1234abcd-IAD"));
        headers.insert(RETRY_AFTER_HEADER, HeaderValue::from_static("15"));
        headers.insert("content-type", HeaderValue::from_static("application/json"));

        let ctx = ResponseDebugContext::extract(&headers, Some(429), 245);
        assert_eq!(ctx.request_id.as_deref(), Some("req-abc-123"));
        assert_eq!(ctx.cf_ray.as_deref(), Some("8f1234abcd-IAD"));
        assert_eq!(ctx.retry_after_secs, Some(15));
        assert_eq!(ctx.http_status, Some(429));
        assert_eq!(ctx.latency_ms, 245);
        assert_eq!(
            ctx.headers_sample.get("content-type").map(String::as_str),
            Some("application/json")
        );

        let summary = ctx.format_summary();
        assert!(summary.contains("status=429"));
        assert!(summary.contains("latency=245ms"));
        assert!(summary.contains("req_id=req-abc-123"));
        assert!(summary.contains("cf_ray=8f1234abcd-IAD"));
        assert!(summary.contains("retry_after=15s"));
    }

    #[test]
    fn test_extract_debug_context_empty() {
        let headers = HeaderMap::new();
        let ctx = ResponseDebugContext::extract(&headers, Some(200), 50);
        assert_eq!(ctx.request_id, None);
        assert_eq!(ctx.cf_ray, None);
        assert_eq!(ctx.http_status, Some(200));
        assert_eq!(ctx.latency_ms, 50);
        assert_eq!(ctx.format_summary(), "status=200 latency=50ms");
    }

    #[test]
    fn test_extract_debug_context_multi_provider() {
        let mut headers = HeaderMap::new();
        headers.insert(
            ANTHROPIC_REQUEST_ID_HEADER,
            HeaderValue::from_static("req_anthropic_999"),
        );
        headers.insert(
            "x-ratelimit-remaining-requests",
            HeaderValue::from_static("4950"),
        );
        headers.insert(
            "x-ratelimit-remaining-tokens",
            HeaderValue::from_static("98000"),
        );

        let ctx = ResponseDebugContext::extract(&headers, Some(200), 120);
        assert_eq!(ctx.request_id.as_deref(), Some("req_anthropic_999"));
        assert_eq!(ctx.rate_limit_remaining_requests, Some(4950));
        assert_eq!(ctx.rate_limit_remaining_tokens, Some(98000));
    }
}
