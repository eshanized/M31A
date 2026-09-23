//! Production NvidiaProvider targeting NVIDIA Build / NIM OpenAI-compatible API surface (D-01, MDL-01, MDL-03).
//!
//! Connects to NVIDIA NIM endpoints via async SSE streaming.
//! Enforces side-effect prohibition: returns validated ModelProposal for runtime decision.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};
use reqwest::Client;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::model::provider::sse::{StreamAccumulator, normalize_http_error};
use crate::model::provider::{BoxStreamChunk, ModelProvider};
use crate::model::types::{ModelError, ModelProposal, TokenUsage};

pub const DEFAULT_NVIDIA_BASE_URL: &str = "https://integrate.api.nvidia.com/v1";

/// Read an explicitly provider-declared capability tier from a `/models`
/// response item, if the provider supplies one.
///
/// Accepts `tier`, `capability_tier`, or `capabilities.tier` string fields
/// with values `fast` / `standard` / `reasoning`. Returns `None` when the
/// provider omits capability metadata — callers must then apply a
/// documented fallback rather than guessing from the model name.
fn explicit_tier(
    item: &serde_json::Value,
) -> Option<(crate::model::router::resolver::ModelTier, &'static str)> {
    use crate::model::router::resolver::ModelTier;
    let raw = item
        .get("tier")
        .or_else(|| item.get("capability_tier"))
        .or_else(|| item.get("capabilities").and_then(|c| c.get("tier")))
        .and_then(|v| v.as_str())?;
    let tier = match raw.trim().to_lowercase().as_str() {
        "fast" => ModelTier::Fast,
        "standard" | "std" => ModelTier::Standard,
        "reasoning" | "reasoner" => ModelTier::Reasoning,
        _ => return None,
    };
    Some((tier, "api: provider-declared tier field"))
}

/// Production model provider for NVIDIA Build and NIM OpenAI-compatible services (D-01).
///
/// Possesses NO capability to execute filesystem, shell, git, network, or sandbox actions (MDL-03).
/// It strictly produces structured proposals for the M31A runtime PolicyGate.
pub type RequestTraceHook = Arc<dyn Fn(&str, &Value) + Send + Sync>;

/// Production model provider for NVIDIA Build and NIM OpenAI-compatible services (D-01).
///
/// Possesses NO capability to execute filesystem, shell, git, network, or sandbox actions (MDL-03).
/// It strictly produces structured proposals for the M31A runtime PolicyGate.
#[derive(Clone)]
pub struct NvidiaProvider {
    client: Client,
    base_url: String,
    api_key: String,
    request_tracer: Option<RequestTraceHook>,
}

impl std::fmt::Debug for NvidiaProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NvidiaProvider")
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
            .field("has_tracer", &self.request_tracer.is_some())
            .finish()
    }
}

impl NvidiaProvider {
    /// Construct a new NvidiaProvider.
    ///
    /// If `base_url` is None, defaults to `https://integrate.api.nvidia.com/v1`.
    /// If `api_key` is None, loads from `NVIDIA_API_KEY` or `API_KEY_NVIDIA` environment variable.
    /// Fails with `ModelError::AuthenticationFailed` if no key is provided or found.
    pub fn new(base_url: Option<String>, api_key: Option<String>) -> Result<Self, ModelError> {
        Self::new_with_lookup(base_url, api_key, |k| std::env::var(k))
    }

    pub(crate) fn new_with_lookup<F>(
        base_url: Option<String>,
        api_key: Option<String>,
        env_lookup: F,
    ) -> Result<Self, ModelError>
    where
        F: Fn(&str) -> Result<String, std::env::VarError>,
    {
        let key = match api_key {
            Some(k) if !k.trim().is_empty() => k.trim().to_string(),
            _ => env_lookup("NVIDIA_API_KEY")
                .or_else(|_| env_lookup("API_KEY_NVIDIA"))
                .map_err(|_| ModelError::AuthenticationFailed)?
                .trim()
                .to_string(),
        };

        if key.is_empty() {
            return Err(ModelError::AuthenticationFailed);
        }

        let base = base_url
            .unwrap_or_else(|| DEFAULT_NVIDIA_BASE_URL.to_string())
            .trim_end_matches('/')
            .to_string();

        let client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|e| ModelError::Network(e.to_string()))?;

        Ok(Self {
            client,
            base_url: base,
            api_key: key,
            request_tracer: None,
        })
    }

    /// Attach an outbound request trace hook to observe payloads sent to the provider.
    pub fn with_request_tracer(mut self, tracer: RequestTraceHook) -> Self {
        self.request_tracer = Some(tracer);
        self
    }

    /// Construct provider with a custom reqwest client (useful for tests and custom pooling).
    pub fn with_client(
        client: Client,
        base_url: Option<String>,
        api_key: Option<String>,
    ) -> Result<Self, ModelError> {
        let mut provider = Self::new(base_url, api_key)?;
        provider.client = client;
        Ok(provider)
    }

    /// Base URL configured for this provider.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Perform a lightweight connectivity and authentication probe against /models (WS-I §8).
    pub async fn probe(&self) -> Result<Duration, ModelError> {
        let endpoint = format!("{}/models", self.base_url);
        let start = std::time::Instant::now();
        let resp = self
            .client
            .get(&endpoint)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| ModelError::Network(e.to_string()))?;

        if resp.status().is_success() {
            Ok(start.elapsed())
        } else if resp.status().as_u16() == 401 || resp.status().as_u16() == 403 {
            Err(ModelError::AuthenticationFailed)
        } else {
            let status = resp.status().as_u16();
            let msg = resp.text().await.unwrap_or_default();
            Err(normalize_http_error(status, &msg))
        }
    }

    /// Discover models from the provider endpoint with explicit validation (WS-I §9, Dynamic Discovery).
    ///
    /// Invariant: Discovered models cannot automatically become production candidates
    /// without explicit capability validation (context limits, tool calling support).
    pub async fn discover_models(
        &self,
    ) -> Result<Vec<crate::model::router::resolver::ModelCandidate>, ModelError> {
        let endpoint = format!("{}/models", self.base_url);
        let resp = self
            .client
            .get(&endpoint)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| ModelError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let msg = resp.text().await.unwrap_or_default();
            return Err(normalize_http_error(status, &msg));
        }

        let body: Value = resp
            .json()
            .await
            .map_err(|e| ModelError::InvalidResponse(e.to_string()))?;

        let data = body.get("data").and_then(|v| v.as_array()).ok_or_else(|| {
            ModelError::InvalidResponse("missing 'data' array in models response".to_string())
        })?;

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let mut candidates = Vec::new();
        for item in data {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                // Declarative authority boundary: capability data
                // belongs to the provider API response, not to name-sniffing
                // heuristics in Rust. Prefer explicit provider fields; when
                // absent, fall back to a documented Standard tier with
                // heuristic provenance recorded in metadata — never present
                // a substring guess as discovered capability.
                let (tier, tier_provenance) = explicit_tier(item).unwrap_or((
                    crate::model::router::resolver::ModelTier::Standard,
                    "heuristic-fallback: provider response omitted capability fields",
                ));

                let display_name = item
                    .get("name")
                    .or_else(|| item.get("display_name"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let context_capacity = item
                    .get("context_window")
                    .or_else(|| item.get("max_tokens"))
                    .or_else(|| item.get("context_length"))
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(131_072);

                let supports_tools = item
                    .get("supports_tools")
                    .or_else(|| item.get("tool_calling"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let mut candidate = crate::model::router::resolver::ModelCandidate::new(
                    id,
                    "nvidia",
                    tier,
                    context_capacity,
                )
                .with_tool_support(supports_tools)
                .with_discovered_at(now_secs)
                .with_source("nvidia_discovery")
                .with_availability(crate::model::types::ProviderCapabilityStatus::Available);

                if let Some(name) = display_name {
                    candidate = candidate.with_display_name(name);
                }

                if let Some(owned_by) = item.get("owned_by").and_then(|v| v.as_str()) {
                    candidate = candidate.with_metadata("owned_by", owned_by);
                }
                candidate = candidate.with_metadata("tier_provenance", tier_provenance);

                if self.validate_candidate(&candidate) {
                    candidates.push(candidate);
                }
            }
        }

        Ok(candidates)
    }

    /// Validate that a candidate meets minimum requirements for production routing (WS-I §9).
    pub fn validate_candidate(
        &self,
        candidate: &crate::model::router::resolver::ModelCandidate,
    ) -> bool {
        !candidate.model_id.trim().is_empty()
            && candidate.context_capacity >= 4096
            && candidate.supports_tools
    }

    /// Build the OpenAI-compatible chat completions JSON request payload.
    pub fn build_chat_request_payload(
        model_name: &str,
        system_prompt: &str,
        tools: &[Value],
    ) -> Value {
        let mut payload = json!({
            "model": model_name,
            "messages": [
                {
                    "role": "system",
                    "content": system_prompt
                }
            ],
            "max_tokens": 32768,
            "stream": true,
            "stream_options": {
                "include_usage": true
            }
        });

        if !tools.is_empty() {
            payload["tools"] = json!(tools);
            payload["tool_choice"] = json!("auto");
        }

        payload
    }

    /// Build the OpenAI-compatible chat completions JSON request payload with multi-turn messages (D-02, D-05).
    pub fn build_chat_request_payload_with_messages(
        model_name: &str,
        messages: &[crate::model::types::ChatMessage],
        tools: &[Value],
    ) -> Value {
        let msgs_json: Vec<Value> = messages
            .iter()
            .map(|m| match m {
                crate::model::types::ChatMessage::System { content } => json!({
                    "role": "system",
                    "content": content,
                }),
                crate::model::types::ChatMessage::User { content } => json!({
                    "role": "user",
                    "content": content,
                }),
                crate::model::types::ChatMessage::Assistant {
                    content,
                    tool_calls,
                } => {
                    let mut obj = json!({ "role": "assistant" });
                    obj["content"] = match content {
                        Some(c) => json!(c),
                        None => Value::Null,
                    };
                    if !tool_calls.is_empty() {
                        let calls: Vec<Value> = tool_calls
                            .iter()
                            .map(|tc| {
                                json!({
                                    "id": tc.id,
                                    "type": "function",
                                    "function": {
                                        "name": tc.name,
                                        "arguments": match &tc.arguments {
                                            Value::String(s) => s.clone(),
                                            other => other.to_string(),
                                        },
                                    }
                                })
                            })
                            .collect();
                        obj["tool_calls"] = json!(calls);
                    }
                    obj
                }
                crate::model::types::ChatMessage::Tool {
                    tool_call_id,
                    content,
                } => json!({
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": content,
                }),
            })
            .collect();

        let mut payload = json!({
            "model": model_name,
            "messages": msgs_json,
            "max_tokens": 32768,
            "stream": true,
            "stream_options": {
                "include_usage": true
            }
        });

        if !tools.is_empty() {
            payload["tools"] = json!(tools);
            payload["tool_choice"] = json!("auto");
        }

        payload
    }
}

#[async_trait]
impl ModelProvider for NvidiaProvider {
    async fn call_model_with_messages(
        &self,
        model_name: &str,
        messages: &[crate::model::types::ChatMessage],
        tools: Vec<Value>,
        cancellation: &CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), ModelError> {
        if cancellation.is_cancelled() {
            return Err(ModelError::Cancelled);
        }

        let endpoint = format!("{}/chat/completions", self.base_url);
        let payload = Self::build_chat_request_payload_with_messages(model_name, messages, &tools);

        if let Some(ref tracer) = self.request_tracer {
            tracer(model_name, &payload);
        }

        let request = self
            .client
            .post(&endpoint)
            .bearer_auth(&self.api_key)
            .json(&payload);

        let response = tokio::select! {
            _ = cancellation.cancelled() => return Err(ModelError::Cancelled),
            res = request.send() => {
                res.map_err(|e| ModelError::Network(e.to_string()))?
            }
        };

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let err_text = response.text().await.unwrap_or_default();
            return Err(normalize_http_error(status, &err_text));
        }

        let mut stream = response.bytes_stream().eventsource();
        let mut accumulator = StreamAccumulator::new();

        loop {
            tokio::select! {
                _ = cancellation.cancelled() => return Err(ModelError::Cancelled),
                chunk = tokio::time::timeout(Duration::from_secs(120), stream.next()) => {
                    match chunk {
                        Ok(Some(Ok(event))) => {
                            if event.data.trim() == "[DONE]" {
                                break;
                            }
                            let _ = accumulator.process_event(&event.data)?;
                        }
                        Ok(Some(Err(err))) => {
                            return Err(ModelError::StreamInterrupted(err.to_string()));
                        }
                        Ok(None) => break,
                        Err(_) => {
                            return Err(ModelError::Network("SSE stream read timed out after 120s of silence".to_string()));
                        }
                    }
                }
            }
        }

        accumulator.finalize()
    }
    async fn call_model(
        &self,
        model_name: &str,
        system_prompt: &str,
        tools: Vec<Value>,
        cancellation: &CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), ModelError> {
        if cancellation.is_cancelled() {
            return Err(ModelError::Cancelled);
        }

        let endpoint = format!("{}/chat/completions", self.base_url);
        let payload = Self::build_chat_request_payload(model_name, system_prompt, &tools);

        if let Some(ref tracer) = self.request_tracer {
            tracer(model_name, &payload);
        }

        let request = self
            .client
            .post(&endpoint)
            .bearer_auth(&self.api_key)
            .json(&payload);

        let response = tokio::select! {
            _ = cancellation.cancelled() => return Err(ModelError::Cancelled),
            res = request.send() => {
                res.map_err(|e| ModelError::Network(e.to_string()))?
            }
        };

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let err_text = response.text().await.unwrap_or_default();
            return Err(normalize_http_error(status, &err_text));
        }

        let mut stream = response.bytes_stream().eventsource();
        let mut accumulator = StreamAccumulator::new();

        loop {
            tokio::select! {
                _ = cancellation.cancelled() => return Err(ModelError::Cancelled),
                chunk = tokio::time::timeout(Duration::from_secs(120), stream.next()) => {
                    match chunk {
                        Ok(Some(Ok(event))) => {
                            if event.data.trim() == "[DONE]" {
                                break;
                            }
                            let _ = accumulator.process_event(&event.data)?;
                        }
                        Ok(Some(Err(err))) => {
                            return Err(ModelError::StreamInterrupted(err.to_string()));
                        }
                        Ok(None) => break,
                        Err(_) => {
                            return Err(ModelError::Network("SSE stream read timed out after 120s of silence".to_string()));
                        }
                    }
                }
            }
        }

        accumulator.finalize()
    }

    async fn stream_model(
        &self,
        model_name: &str,
        system_prompt: &str,
        tools: Vec<Value>,
        cancellation: &CancellationToken,
    ) -> Result<BoxStreamChunk, ModelError> {
        if cancellation.is_cancelled() {
            return Err(ModelError::Cancelled);
        }

        let endpoint = format!("{}/chat/completions", self.base_url);
        let payload = Self::build_chat_request_payload(model_name, system_prompt, &tools);

        let request = self
            .client
            .post(&endpoint)
            .bearer_auth(&self.api_key)
            .json(&payload);

        let response = tokio::select! {
            _ = cancellation.cancelled() => return Err(ModelError::Cancelled),
            res = request.send() => {
                res.map_err(|e| ModelError::Network(e.to_string()))?
            }
        };

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let err_text = response.text().await.unwrap_or_default();
            return Err(normalize_http_error(status, &err_text));
        }

        let (mut tx, rx) = mpsc::channel(64);
        let cancellation_clone = cancellation.clone();
        let mut stream = response.bytes_stream().eventsource();

        tokio::spawn(async move {
            let mut accumulator = StreamAccumulator::new();
            loop {
                tokio::select! {
                    _ = cancellation_clone.cancelled() => {
                        let _ = tx.send(Err(ModelError::Cancelled)).await;
                        break;
                    }
                    item = stream.next() => {
                        match item {
                            Some(Ok(event)) => {
                                if event.data.trim() == "[DONE]" {
                                    break;
                                }
                                match accumulator.process_event(&event.data) {
                                    Ok(chunks) => {
                                        for chunk in chunks {
                                            if tx.send(Ok(chunk)).await.is_err() {
                                                return;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let _ = tx.send(Err(e)).await;
                                        return;
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                let _ = tx.send(Err(ModelError::StreamInterrupted(e.to_string()))).await;
                                return;
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        Ok(Box::pin(rx))
    }

    async fn discover_models(
        &self,
    ) -> Result<Vec<crate::model::router::resolver::ModelCandidate>, ModelError> {
        self.discover_models().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_provider_instantiation_with_defaults() {
        let provider = NvidiaProvider::new(None, Some("nvapi-test-key-12345".to_string())).unwrap();
        assert_eq!(provider.base_url(), DEFAULT_NVIDIA_BASE_URL);
    }

    #[test]
    fn test_provider_instantiation_trims_trailing_slash() {
        let provider = NvidiaProvider::new(
            Some("https://custom.nim.endpoint/v1/".to_string()),
            Some("test-key".to_string()),
        )
        .unwrap();
        assert_eq!(provider.base_url(), "https://custom.nim.endpoint/v1");
    }

    #[test]
    fn test_provider_instantiation_fails_without_key() {
        let err =
            NvidiaProvider::new_with_lookup(None, None, |_| Err(std::env::VarError::NotPresent))
                .unwrap_err();
        assert_eq!(err, ModelError::AuthenticationFailed);

        let err_empty = NvidiaProvider::new_with_lookup(None, Some("   ".to_string()), |_| {
            Err(std::env::VarError::NotPresent)
        })
        .unwrap_err();
        assert_eq!(err_empty, ModelError::AuthenticationFailed);

        // Also test successful env lookup
        let provider =
            NvidiaProvider::new_with_lookup(None, None, |_| Ok("nvapi-from-env".to_string()))
                .unwrap();
        assert_eq!(provider.base_url(), DEFAULT_NVIDIA_BASE_URL);
    }

    #[test]
    fn test_build_chat_request_payload_without_tools() {
        let payload =
            NvidiaProvider::build_chat_request_payload("meta/llama-3.1-8b", "test system", &[]);
        assert_eq!(payload["model"], "meta/llama-3.1-8b");
        assert_eq!(payload["messages"][0]["role"], "system");
        assert_eq!(payload["messages"][0]["content"], "test system");
        assert_eq!(payload["stream"], true);
        assert_eq!(payload["stream_options"]["include_usage"], true);
        assert!(payload.get("tools").is_none());
        assert!(payload.get("tool_choice").is_none());
    }

    #[test]
    fn test_build_chat_request_payload_with_tools() {
        let tool = json!({
            "type": "function",
            "function": {
                "name": "cargo_test",
                "description": "Run tests",
                "parameters": {
                    "type": "object"
                }
            }
        });

        let payload = NvidiaProvider::build_chat_request_payload(
            "meta/llama-3.3-70b",
            "system prompt",
            std::slice::from_ref(&tool),
        );

        assert_eq!(payload["tools"], json!([tool]));
        assert_eq!(payload["tool_choice"], "auto");
    }

    #[tokio::test]
    async fn test_cancellation_aborts_before_network_call() {
        let provider = NvidiaProvider::new(
            Some("http://127.0.0.1:9".to_string()), // non-existent unreachable port
            Some("test-key".to_string()),
        )
        .unwrap();

        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let err = provider
            .call_model("test-model", "prompt", vec![], &cancellation)
            .await
            .unwrap_err();
        assert_eq!(err, ModelError::Cancelled);

        let stream_err = match provider
            .stream_model("test-model", "prompt", vec![], &cancellation)
            .await
        {
            Ok(_) => panic!("expected cancellation error"),
            Err(e) => e,
        };
        assert_eq!(stream_err, ModelError::Cancelled);
    }
}
