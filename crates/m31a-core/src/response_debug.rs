//! Response debug context and transport error taxonomy.
//!
//! Extracts request correlation IDs, tracing rays, provider processing latencies,
//! authentication errors, and categorizes HTTP/API transport failures with telemetry
//! messages and actionable developer remediation guidance.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const HEADER_REQUEST_ID: &str = "x-request-id";
pub const HEADER_OAI_REQUEST_ID: &str = "x-oai-request-id";
pub const HEADER_AMZN_REQUEST_ID: &str = "x-amzn-requestid";
pub const HEADER_CORRELATION_ID: &str = "x-correlation-id";
pub const HEADER_CF_RAY: &str = "cf-ray";
pub const HEADER_AUTH_ERROR: &str = "x-openai-authorization-error";
pub const HEADER_ERROR_JSON: &str = "x-error-json";
pub const HEADER_RETRY_AFTER: &str = "retry-after";
pub const HEADER_RATELIMIT_RESET: &str = "x-ratelimit-reset";
pub const HEADER_PROCESSING_MS: &str = "openai-processing-ms";
pub const HEADER_ORGANIZATION: &str = "openai-organization";

/// Metadata and correlation identifiers captured from an upstream LLM API response.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResponseDebugContext {
    pub request_id: Option<String>,
    pub cf_ray: Option<String>,
    pub auth_error: Option<String>,
    pub auth_error_code: Option<String>,
    pub rate_limit_reset_secs: Option<u64>,
    pub organization_id: Option<String>,
    pub processing_ms: Option<u64>,
}

impl ResponseDebugContext {
    /// Formats a concise debug header string for logging or UI telemetry.
    pub fn format_telemetry_summary(&self) -> String {
        let req = self.request_id.as_deref().unwrap_or("none");
        let ray = self.cf_ray.as_deref().unwrap_or("none");
        let lat = self
            .processing_ms
            .map(|ms| format!("{}ms", ms))
            .unwrap_or_else(|| "unknown".to_string());
        format!("req_id={} ray={} latency={}", req, ray, lat)
    }
}

/// Extract debug context from HTTP response headers (case-insensitive key lookup).
pub fn extract_response_debug_context(headers: &HashMap<String, String>) -> ResponseDebugContext {
    let mut context = ResponseDebugContext::default();

    let lookup = |target: &str| -> Option<String> {
        let target_lower = target.to_ascii_lowercase();
        for (k, v) in headers {
            if k.to_ascii_lowercase() == target_lower {
                return Some(v.trim().to_string());
            }
        }
        None
    };

    context.request_id = lookup(HEADER_REQUEST_ID)
        .or_else(|| lookup(HEADER_OAI_REQUEST_ID))
        .or_else(|| lookup(HEADER_AMZN_REQUEST_ID))
        .or_else(|| lookup(HEADER_CORRELATION_ID));

    context.cf_ray = lookup(HEADER_CF_RAY);
    context.auth_error = lookup(HEADER_AUTH_ERROR);
    context.organization_id = lookup(HEADER_ORGANIZATION);

    if let Some(ms_str) = lookup(HEADER_PROCESSING_MS) {
        context.processing_ms = ms_str.parse::<u64>().ok();
    }

    if let Some(reset_str) = lookup(HEADER_RETRY_AFTER).or_else(|| lookup(HEADER_RATELIMIT_RESET)) {
        context.rate_limit_reset_secs = reset_str.parse::<u64>().ok();
    }

    if let Some(encoded) = lookup(HEADER_ERROR_JSON)
        && let Some(decoded) = decode_base64(&encoded)
        && let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(&decoded)
    {
        context.auth_error_code = parsed
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(|c| c.as_str())
            .map(str::to_string);
    }

    context
}

/// Self-contained standard Base64 decoder.
pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut table = [255u8; 256];
    for (i, c) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
        .iter()
        .enumerate()
    {
        table[*c as usize] = i as u8;
    }

    let clean: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    let mut out = Vec::with_capacity((clean.len() * 3) / 4);

    let mut buf = 0u32;
    let mut bits = 0;
    for byte in clean {
        let val = table[byte as usize];
        if val == 255 {
            return None;
        }
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Categorization of provider and transport-level failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportErrorCategory {
    RateLimit { retry_after_secs: Option<u64> },
    QuotaExceeded,
    ContextLengthExceeded { token_limit: Option<u64> },
    AuthenticationFailed { reason: String },
    ServerOverloaded { backoff_hint_secs: Option<u64> },
    ContentFilterTriggered { category: Option<String> },
    NetworkTimeout,
    NetworkConnectionReset,
    HttpError { status: u16, message: String },
}

/// Classify an HTTP response status code, headers, and body into a structured error category.
pub fn classify_http_response(
    status: u16,
    headers: &HashMap<String, String>,
    body: &str,
) -> Option<TransportErrorCategory> {
    if (200..300).contains(&status) {
        return None;
    }

    let debug_ctx = extract_response_debug_context(headers);
    let body_lower = body.to_ascii_lowercase();

    if status == 429 {
        if body_lower.contains("quota")
            || body_lower.contains("insufficient_quota")
            || body_lower.contains("billing")
        {
            return Some(TransportErrorCategory::QuotaExceeded);
        }
        return Some(TransportErrorCategory::RateLimit {
            retry_after_secs: debug_ctx.rate_limit_reset_secs,
        });
    }

    if status == 401 || status == 403 {
        let reason = debug_ctx
            .auth_error
            .or(debug_ctx.auth_error_code)
            .unwrap_or_else(|| {
                if body_lower.contains("invalid api key") || body_lower.contains("unauthorized") {
                    "Invalid or expired API credential".to_string()
                } else {
                    format!("Authentication rejected with status {}", status)
                }
            });
        return Some(TransportErrorCategory::AuthenticationFailed { reason });
    }

    if status == 400 {
        if body_lower.contains("context_length_exceeded")
            || body_lower.contains("maximum context length")
            || body_lower.contains("token limit")
        {
            return Some(TransportErrorCategory::ContextLengthExceeded { token_limit: None });
        }
        if body_lower.contains("content_filter") || body_lower.contains("safety") {
            return Some(TransportErrorCategory::ContentFilterTriggered {
                category: Some("content_policy_violation".to_string()),
            });
        }
    }

    if status == 503 || status == 529 || status == 504 {
        return Some(TransportErrorCategory::ServerOverloaded {
            backoff_hint_secs: debug_ctx.rate_limit_reset_secs.or(Some(5)),
        });
    }

    Some(TransportErrorCategory::HttpError {
        status,
        message: body.chars().take(200).collect(),
    })
}

/// Telemetry string for monitoring logs and event tracking.
pub fn telemetry_message(category: &TransportErrorCategory) -> String {
    match category {
        TransportErrorCategory::RateLimit { retry_after_secs } => match retry_after_secs {
            Some(secs) => format!("rate_limit_exceeded (retry_after: {}s)", secs),
            None => "rate_limit_exceeded".to_string(),
        },
        TransportErrorCategory::QuotaExceeded => "quota_exceeded".to_string(),
        TransportErrorCategory::ContextLengthExceeded { token_limit } => match token_limit {
            Some(limit) => format!("context_window_exceeded (limit: {})", limit),
            None => "context_window_exceeded".to_string(),
        },
        TransportErrorCategory::AuthenticationFailed { reason } => {
            format!("authentication_failure: {}", reason)
        }
        TransportErrorCategory::ServerOverloaded { backoff_hint_secs } => match backoff_hint_secs {
            Some(secs) => format!("server_overloaded (backoff: {}s)", secs),
            None => "server_overloaded".to_string(),
        },
        TransportErrorCategory::ContentFilterTriggered { category } => {
            format!(
                "content_filter_triggered ({})",
                category.as_deref().unwrap_or("unspecified")
            )
        }
        TransportErrorCategory::NetworkTimeout => "network_timeout".to_string(),
        TransportErrorCategory::NetworkConnectionReset => "network_connection_reset".to_string(),
        TransportErrorCategory::HttpError { status, .. } => format!("http_error_{}", status),
    }
}

/// Remediation guidance for the developer when a specific error occurs.
pub fn remediation_guidance(category: &TransportErrorCategory) -> &'static str {
    match category {
        TransportErrorCategory::RateLimit { .. } => {
            "Rate limit reached. M31A will automatically backoff and retry, or use --model to switch to an alternative provider."
        }
        TransportErrorCategory::QuotaExceeded => {
            "Provider account quota exhausted or billing inactive. Verify account credit or configure a different provider endpoint."
        }
        TransportErrorCategory::ContextLengthExceeded { .. } => {
            "Input prompt exceeds context limit. Enable context compression or use M31A turn history trimming to free up tokens."
        }
        TransportErrorCategory::AuthenticationFailed { .. } => {
            "Invalid or missing API key. Verify NVIDIA_API_KEY, OPENAI_API_KEY, or run 'm31a login'."
        }
        TransportErrorCategory::ServerOverloaded { .. } => {
            "Provider is temporarily overloaded. Please pause briefly or fallback to a locally hosted model (e.g. Ollama or vLLM)."
        }
        TransportErrorCategory::ContentFilterTriggered { .. } => {
            "Input or response triggered provider safety filters. Review prompt guidelines or adjust model parameters."
        }
        TransportErrorCategory::NetworkTimeout => {
            "Network connection timed out. Check internet connectivity and proxy settings."
        }
        TransportErrorCategory::NetworkConnectionReset => {
            "Upstream connection abruptly closed. Retrying connection."
        }
        TransportErrorCategory::HttpError { .. } => {
            "Unexpected HTTP error received. Check endpoint configuration and response logs."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_response_debug_context_headers() {
        let mut headers = HashMap::new();
        headers.insert("X-Request-Id".to_string(), "req-abc-123".to_string());
        headers.insert("CF-RAY".to_string(), "ray-999-iad".to_string());
        headers.insert("openai-processing-ms".to_string(), "450".to_string());
        headers.insert("retry-after".to_string(), "15".to_string());
        headers.insert("openai-organization".to_string(), "org-m31a".to_string());

        let ctx = extract_response_debug_context(&headers);
        assert_eq!(ctx.request_id.as_deref(), Some("req-abc-123"));
        assert_eq!(ctx.cf_ray.as_deref(), Some("ray-999-iad"));
        assert_eq!(ctx.processing_ms, Some(450));
        assert_eq!(ctx.rate_limit_reset_secs, Some(15));
        assert_eq!(ctx.organization_id.as_deref(), Some("org-m31a"));

        let summary = ctx.format_telemetry_summary();
        assert!(summary.contains("req_id=req-abc-123"));
        assert!(summary.contains("ray=ray-999-iad"));
        assert!(summary.contains("latency=450ms"));
    }

    #[test]
    fn test_extract_error_json_base64() {
        let mut headers = HashMap::new();
        // {"error": {"code": "insufficient_funds"}}
        let encoded = "eyJlcnJvciI6IHsiY29kZSI6ICJpbnN1ZmZpY2llbnRfZnVuZHMifX0=".to_string();
        headers.insert(HEADER_ERROR_JSON.to_string(), encoded);

        let ctx = extract_response_debug_context(&headers);
        assert_eq!(ctx.auth_error_code.as_deref(), Some("insufficient_funds"));
    }

    #[test]
    fn test_decode_base64_variants() {
        assert_eq!(decode_base64(""), Some(vec![]));
        assert_eq!(decode_base64("SGVs bG8="), Some(b"Hello".to_vec()));
        assert_eq!(
            decode_base64("c3VwZXJfc2VjcmV0"),
            Some(b"super_secret".to_vec())
        );
    }

    #[test]
    fn test_classify_http_response_rate_limit() {
        let mut headers = HashMap::new();
        headers.insert("retry-after".to_string(), "30".to_string());

        let cat = classify_http_response(429, &headers, "Too many requests").unwrap();
        assert_eq!(
            cat,
            TransportErrorCategory::RateLimit {
                retry_after_secs: Some(30)
            }
        );

        let msg = telemetry_message(&cat);
        assert_eq!(msg, "rate_limit_exceeded (retry_after: 30s)");
        assert!(remediation_guidance(&cat).contains("Rate limit reached"));
    }

    #[test]
    fn test_classify_http_response_quota() {
        let headers = HashMap::new();
        let cat = classify_http_response(429, &headers, "insufficient_quota error").unwrap();
        assert_eq!(cat, TransportErrorCategory::QuotaExceeded);
        assert_eq!(telemetry_message(&cat), "quota_exceeded");
    }

    #[test]
    fn test_classify_http_response_context_length() {
        let headers = HashMap::new();
        let cat = classify_http_response(400, &headers, "maximum context length is 131072 tokens")
            .unwrap();
        assert_eq!(
            cat,
            TransportErrorCategory::ContextLengthExceeded { token_limit: None }
        );
        assert!(remediation_guidance(&cat).contains("context compression"));
    }

    #[test]
    fn test_classify_http_response_auth_failed() {
        let mut headers = HashMap::new();
        headers.insert(
            "x-openai-authorization-error".to_string(),
            "token_expired".to_string(),
        );

        let cat = classify_http_response(401, &headers, "unauthorized").unwrap();
        match cat {
            TransportErrorCategory::AuthenticationFailed { reason } => {
                assert_eq!(reason, "token_expired");
            }
            other => panic!("Expected AuthenticationFailed, got {:?}", other),
        }
    }
}
