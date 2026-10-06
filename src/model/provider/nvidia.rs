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

use crate::model::provider::endpoint::{
    EndpointTrustError, EndpointTrustSource, is_test_credential, validate_nvidia_endpoint,
};
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

/// Extract model context capacity and provenance from a `/models` response item.
///
/// Order of precedence (MDL-01, D-05):
/// 1. `max_model_len` - authoritative NVIDIA NIM / vLLM serving sequence length
/// 2. `context_window` - standard OpenAI-compatible context window
/// 3. `context_length` - common alternative provider field
/// 4. `max_sequence_length` - transformer / HuggingFace model sequence length
/// 5. `max_position_embeddings` - transformer positional embedding limit
/// 6. `max_tokens` - legacy fallback (accepted ONLY if no context field exists; commonly an output limit)
pub fn extract_context_capacity(item: &serde_json::Value) -> (usize, &'static str) {
    let parse_num = |val: &serde_json::Value| -> Option<usize> {
        if let Some(n) = val.as_u64() {
            if n > 0 { Some(n as usize) } else { None }
        } else if let Some(s) = val.as_str() {
            s.trim().parse::<usize>().ok().filter(|&n| n > 0)
        } else {
            None
        }
    };

    let lookup = |key: &str| -> Option<usize> {
        item.get(key)
            .or_else(|| item.get("metadata").and_then(|m| m.get(key)))
            .or_else(|| item.get("extra").and_then(|e| e.get(key)))
            .and_then(parse_num)
    };

    if let Some(cap) = lookup("max_model_len") {
        return (cap, "provider:max_model_len");
    }
    if let Some(cap) = lookup("context_window") {
        return (cap, "provider:context_window");
    }
    if let Some(cap) = lookup("context_length") {
        return (cap, "provider:context_length");
    }
    if let Some(cap) = lookup("max_sequence_length") {
        return (cap, "provider:max_sequence_length");
    }
    if let Some(cap) = lookup("max_position_embeddings") {
        return (cap, "provider:max_position_embeddings");
    }
    if let Some(cap) = lookup("max_tokens") {
        return (cap, "provider:max_tokens");
    }

    (0, "unknown")
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
    metadata_resolver: Arc<crate::model::provider::nvidia_metadata::NvidiaModelMetadataResolver>,
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
    /// Parse a single model JSON object from a `/v1/models` response into a ModelCandidate.
    pub fn parse_model_candidate(
        item: &Value,
        now_secs: u64,
    ) -> Option<crate::model::router::resolver::ModelCandidate> {
        let id = item.get("id").and_then(|v| v.as_str())?.trim();
        if id.is_empty() {
            return None;
        }

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

        let (context_capacity, context_provenance) = extract_context_capacity(item);

        // Tool calling capability:
        // Invariant: Do NOT assume true when omitted. Raw /v1/models omits tool calling,
        // so it must be CapabilitySupport::Unknown unless explicitly provided.
        let tool_cap = if let Some(val) = item
            .get("supports_tools")
            .or_else(|| item.get("tool_calling"))
            .or_else(|| {
                item.get("capabilities")
                    .and_then(|c| c.get("tools").or_else(|| c.get("tool_calling")))
            })
            .and_then(|v| v.as_bool())
        {
            crate::model::router::resolver::CapabilitySupport::from(val)
        } else {
            crate::model::router::resolver::CapabilitySupport::Unknown
        };

        let mut candidate = crate::model::router::resolver::ModelCandidate::new(
            id,
            "nvidia",
            tier,
            context_capacity,
        )
        .with_model_kind(crate::model::router::resolver::ModelKind::Unknown)
        .with_tool_capability(tool_cap)
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
        candidate = candidate.with_context_provenance(context_provenance);

        Some(candidate)
    }

    /// Construct a new NvidiaProvider.
    ///
    /// Endpoint-trust ordering (P0-01): the endpoint is validated BEFORE any
    /// credential is loaded or attached. Legacy direct construction treats the
    /// endpoint as operator-supplied; test/placeholder credentials may use any
    /// endpoint shape, but a REAL credential with a custom endpoint must be
    /// `https://` and must not target a blocked literal. Production wiring
    /// must prefer [`Self::new_governed`] with the configuration tier so
    /// workspace-supplied endpoints can never carry production credentials.
    /// If `base_url` is None, defaults to `https://integrate.api.nvidia.com/v1`.
    /// If `api_key` is None, loads from `NVIDIA_API_KEY` or `API_KEY_NVIDIA` environment variable.
    /// Fails with `ModelError::AuthenticationFailed` if no key is provided or found.
    pub fn new(base_url: Option<String>, api_key: Option<String>) -> Result<Self, ModelError> {
        Self::new_with_lookup(base_url, api_key, |k| std::env::var(k))
    }

    /// Governed production constructor with explicit endpoint trust source.
    ///
    /// `source` must reflect the configuration tier that supplied `base_url`
    /// (see `ResolvedConfiguration::provider_endpoint_source`). A custom
    /// endpoint from [`EndpointTrustSource::Workspace`],
    /// [`EndpointTrustSource::SessionOverride`], or
    /// [`EndpointTrustSource::Unknown`] is rejected when a real credential
    /// would be attached — fail closed against repository-driven exfiltration.
    pub fn new_governed(
        base_url: Option<String>,
        api_key: Option<String>,
        source: EndpointTrustSource,
    ) -> Result<Self, ModelError> {
        Self::new_governed_with_lookup(base_url, api_key, source, |k| std::env::var(k))
    }

    pub(crate) fn new_with_lookup<F>(
        base_url: Option<String>,
        api_key: Option<String>,
        env_lookup: F,
    ) -> Result<Self, ModelError>
    where
        F: Fn(&str) -> Result<String, std::env::VarError>,
    {
        // Legacy path = operator-supplied endpoint (explicit construction).
        Self::construct(
            base_url,
            api_key,
            EndpointTrustSource::ExplicitCli,
            &env_lookup,
        )
    }

    pub fn new_governed_with_lookup<F>(
        base_url: Option<String>,
        api_key: Option<String>,
        source: EndpointTrustSource,
        env_lookup: F,
    ) -> Result<Self, ModelError>
    where
        F: Fn(&str) -> Result<String, std::env::VarError>,
    {
        Self::construct(base_url, api_key, source, &env_lookup)
    }

    /// Shared construction: ENDPOINT TRUST FIRST, credential second.
    fn construct<F>(
        base_url: Option<String>,
        api_key: Option<String>,
        source: EndpointTrustSource,
        env_lookup: &F,
    ) -> Result<Self, ModelError>
    where
        F: Fn(&str) -> Result<String, std::env::VarError>,
    {
        // 1. Peek at the credential kind WITHOUT attaching it anywhere, so
        //    endpoint validation can distinguish test doubles from real keys.
        let key_preview: Option<String> = match &api_key {
            Some(k) if !k.trim().is_empty() => Some(k.trim().to_string()),
            _ => env_lookup("NVIDIA_API_KEY")
                .or_else(|_| env_lookup("API_KEY_NVIDIA"))
                .map(|k| k.trim().to_string())
                .ok()
                .filter(|k| !k.is_empty()),
        };
        let uses_real_credential = key_preview
            .as_deref()
            .is_some_and(|k| !is_test_credential(k));

        // 2. Validate endpoint BEFORE consuming the credential.
        if uses_real_credential {
            // Real credential: full tier + shape authority.
            validate_nvidia_endpoint(base_url.clone(), source).map_err(|e| match e {
                EndpointTrustError::UntrustedSource(_)
                | EndpointTrustError::InsecureScheme(_)
                | EndpointTrustError::BlockedDestination(_) => {
                    ModelError::MissingConfiguration(e.to_string())
                }
                EndpointTrustError::InvalidUrl(msg) => ModelError::MissingConfiguration(msg),
                EndpointTrustError::CredentialBeforeTrust => {
                    ModelError::MissingConfiguration(e.to_string())
                }
            })?;
        } else {
            // Test/placeholder credential or missing key: validate URL shape
            // only (fail obvious on garbage); a real credential is never
            // attached on this path. Destination binding is enforced
            // per-request for real credentials (see authorized_endpoint_url).
            let raw = base_url
                .clone()
                .unwrap_or_else(|| super::endpoint::CANONICAL_NVIDIA_BASE_URL.to_string());
            let trimmed = raw.trim().trim_end_matches('/').to_string();
            if trimmed.is_empty() {
                return Err(ModelError::MissingConfiguration(
                    "provider endpoint URL is invalid: empty provider base_url".to_string(),
                ));
            }
            reqwest::Url::parse(&trimmed).map_err(|e| {
                ModelError::MissingConfiguration(format!("invalid provider endpoint: {e}"))
            })?;
        }

        // 3. Now load the credential (endpoint trust established).
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

        // Centralized validating transport (P0-01, P1-01): every connection
        // this client opens resolves through ValidatingDnsResolver (policy
        // bound to destination, Host/TLS-SNI preserved) and never follows
        // redirects implicitly (redirect hops require explicit revalidation).
        let client = super::endpoint::policy_validating_client_builder()
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
            metadata_resolver: Arc::new(
                crate::model::provider::nvidia_metadata::NvidiaModelMetadataResolver::new(),
            ),
        })
    }

    /// Attach an outbound request trace hook to observe payloads sent to the provider.
    pub fn with_request_tracer(mut self, tracer: RequestTraceHook) -> Self {
        self.request_tracer = Some(tracer);
        self
    }

    /// Override the metadata resolver used for enriching discovered model capabilities.
    pub fn with_metadata_resolver(
        mut self,
        resolver: Arc<crate::model::provider::nvidia_metadata::NvidiaModelMetadataResolver>,
    ) -> Self {
        self.metadata_resolver = resolver;
        self
    }

    /// Access the metadata resolver.
    pub fn metadata_resolver(
        &self,
    ) -> &Arc<crate::model::provider::nvidia_metadata::NvidiaModelMetadataResolver> {
        &self.metadata_resolver
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

    /// Authorize the full request URL against the egress destination policy
    /// BEFORE attaching the bearer credential (P0-01, P1-01).
    ///
    /// Test/placeholder credentials skip DNS binding (no exfiltration risk,
    /// keeps unit tests hermetic). Real credentials require scheme + DNS
    /// validation of every resolved address on every request (rebinding
    /// defense); failures fail closed with `ModelError::Configuration`.
    async fn credential_endpoint_url(&self, path: &str) -> Result<String, ModelError> {
        let url = format!("{}{}", self.base_url, path);
        if is_test_credential(&self.api_key) {
            return Ok(url);
        }
        crate::policy::destination::NetworkDestinationPolicy::new()
            .validate_url(&url)
            .await
            .map(|_| url)
            .map_err(|e| {
                ModelError::MissingConfiguration(format!("provider endpoint blocked: {e}"))
            })
    }

    /// Perform a lightweight connectivity and authentication probe against /models (WS-I §8).
    pub async fn probe(&self) -> Result<Duration, ModelError> {
        let endpoint = self.credential_endpoint_url("/models").await?;
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

    /// Perform an authenticated minimal chat completion request to verify provider and credentials (MDL-01, WS-I §8).
    ///
    /// Sends a minimal single-token non-streaming chat request to `/chat/completions`.
    /// Enforces destination and policy validation before attaching the credential.
    /// Returns actual measured round-trip latency on success.
    pub async fn verify_chat_completion(
        &self,
        model_name: &str,
    ) -> Result<Duration, ModelError> {
        let endpoint = self.credential_endpoint_url("/chat/completions").await?;
        let payload = json!({
            "model": model_name,
            "messages": [
                {
                    "role": "user",
                    "content": "ping"
                }
            ],
            "max_tokens": 1,
            "stream": false
        });

        if let Some(ref tracer) = self.request_tracer {
            tracer(model_name, &payload);
        }

        let start = std::time::Instant::now();
        let resp = self
            .client
            .post(&endpoint)
            .bearer_auth(&self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ModelError::Network(e.to_string()))?;

        let elapsed = start.elapsed();
        let status = resp.status();

        if status.is_success() {
            Ok(elapsed)
        } else if status.as_u16() == 401 || status.as_u16() == 403 {
            Err(ModelError::AuthenticationFailed)
        } else {
            let status_code = status.as_u16();
            let msg = resp.text().await.unwrap_or_default();
            if status_code == 404 || (status_code == 400 && msg.to_lowercase().contains("model")) {
                Err(ModelError::ModelUnavailable(format!(
                    "Model '{model_name}' is not available at endpoint: {msg}"
                )))
            } else {
                Err(normalize_http_error(status_code, &msg))
            }
        }
    }

    /// Discover raw inventory of models from the provider endpoint without capability enrichment.
    pub async fn discover_inventory(
        &self,
    ) -> Result<Vec<crate::model::router::resolver::ModelCandidate>, ModelError> {
        let endpoint = self.credential_endpoint_url("/models").await?;
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
            if let Some(candidate) = Self::parse_model_candidate(item, now_secs) {
                candidates.push(candidate);
            }
        }

        Ok(candidates)
    }

    /// Discover models from the provider endpoint and enrich them with authoritative metadata.
    ///
    /// Pipeline:
    ///   /v1/models (Inventory) -> raw candidates -> metadata enrichment -> normalized candidates.
    ///
    /// Preserves raw inventory in degraded mode if metadata enrichment fails.
    pub async fn discover_models(
        &self,
    ) -> Result<Vec<crate::model::router::resolver::ModelCandidate>, ModelError> {
        use crate::model::provider::nvidia_metadata::ProviderModelMetadataSource;
        let mut candidates = self.discover_inventory().await?;

        if let Err(e) = self
            .metadata_resolver
            .enrich_candidates(&mut candidates)
            .await
        {
            tracing::warn!(
                "Failed to enrich discovered model metadata: {e}; proceeding with un-enriched inventory"
            );
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
            && candidate.is_context_known()
            && candidate.supports_tools
            && candidate.tool_support
                == crate::model::router::resolver::CapabilitySupport::Supported
            && !candidate.is_embedding()
            && !candidate.is_image_generation()
            && !candidate.is_safety_guard()
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
        self.call_model_with_messages_streaming(model_name, messages, tools, cancellation, None)
            .await
    }

    async fn call_model_with_messages_streaming(
        &self,
        model_name: &str,
        messages: &[crate::model::types::ChatMessage],
        tools: Vec<Value>,
        cancellation: &CancellationToken,
        chunk_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::model::types::StreamChunk>>,
    ) -> Result<(ModelProposal, TokenUsage), ModelError> {
        if cancellation.is_cancelled() {
            return Err(ModelError::Cancelled);
        }

        let endpoint = self.credential_endpoint_url("/chat/completions").await?;
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
                            let chunks = accumulator.process_event(&event.data)?;
                            if let Some(ref tx) = chunk_tx {
                                for c in chunks {
                                    let _ = tx.send(c);
                                }
                            }
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

        let endpoint = self.credential_endpoint_url("/chat/completions").await?;
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

        let endpoint = self.credential_endpoint_url("/chat/completions").await?;
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

    async fn stream_model_with_messages(
        &self,
        model_name: &str,
        messages: &[crate::model::types::ChatMessage],
        tools: Vec<Value>,
        cancellation: &CancellationToken,
    ) -> Result<BoxStreamChunk, ModelError> {
        if cancellation.is_cancelled() {
            return Err(ModelError::Cancelled);
        }

        let endpoint = self.credential_endpoint_url("/chat/completions").await?;
        let payload = Self::build_chat_request_payload_with_messages(model_name, messages, &tools);

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

    #[test]
    fn test_extract_context_capacity_nvidia_max_model_len() {
        let item = json!({
            "id": "meta/llama-3.1-70b-instruct",
            "max_model_len": 65536
        });
        let (cap, prov) = extract_context_capacity(&item);
        assert_eq!(cap, 65536);
        assert_eq!(prov, "provider:max_model_len");
    }

    #[test]
    fn test_extract_context_capacity_distinct_across_models() {
        let models = vec![
            (json!({ "id": "a", "max_model_len": 131072 }), 131072),
            (json!({ "id": "b", "max_model_len": 65536 }), 65536),
            (json!({ "id": "c", "max_model_len": 32768 }), 32768),
            (json!({ "id": "d", "max_model_len": 16384 }), 16384),
        ];

        for (item, expected) in models {
            let (cap, _) = extract_context_capacity(&item);
            assert_eq!(cap, expected);
        }
    }

    #[test]
    fn test_extract_context_capacity_precedence_max_model_len_over_context_window() {
        let item = json!({
            "id": "test-model",
            "max_model_len": 65536,
            "context_window": 131072
        });
        let (cap, prov) = extract_context_capacity(&item);
        assert_eq!(cap, 65536);
        assert_eq!(prov, "provider:max_model_len");
    }

    #[test]
    fn test_extract_context_capacity_generic_context_window() {
        let item = json!({
            "id": "test-model",
            "context_window": 32768
        });
        let (cap, prov) = extract_context_capacity(&item);
        assert_eq!(cap, 32768);
        assert_eq!(prov, "provider:context_window");
    }

    #[test]
    fn test_extract_context_capacity_generic_context_length() {
        let item = json!({
            "id": "test-model",
            "context_length": 32768
        });
        let (cap, prov) = extract_context_capacity(&item);
        assert_eq!(cap, 32768);
        assert_eq!(prov, "provider:context_length");
    }

    #[test]
    fn test_extract_context_capacity_max_sequence_length() {
        let item = json!({
            "id": "test-model",
            "max_sequence_length": 8192
        });
        let (cap, prov) = extract_context_capacity(&item);
        assert_eq!(cap, 8192);
        assert_eq!(prov, "provider:max_sequence_length");
    }

    #[test]
    fn test_extract_context_capacity_max_position_embeddings() {
        let item = json!({
            "id": "test-model",
            "max_position_embeddings": 4096
        });
        let (cap, prov) = extract_context_capacity(&item);
        assert_eq!(cap, 4096);
        assert_eq!(prov, "provider:max_position_embeddings");
    }

    #[test]
    fn test_extract_context_capacity_max_tokens_does_not_override_context_field() {
        let item = json!({
            "id": "test-model",
            "max_model_len": 65536,
            "max_tokens": 8192
        });
        let (cap, prov) = extract_context_capacity(&item);
        assert_eq!(cap, 65536);
        assert_eq!(prov, "provider:max_model_len");
    }

    #[test]
    fn test_extract_context_capacity_legacy_max_tokens_fallback() {
        let item = json!({
            "id": "test-model",
            "max_tokens": 8192
        });
        let (cap, prov) = extract_context_capacity(&item);
        assert_eq!(cap, 8192);
        assert_eq!(prov, "provider:max_tokens");
    }

    #[test]
    fn test_extract_context_capacity_unknown_when_no_field_present() {
        let item = json!({
            "id": "test-model"
        });
        let (cap, prov) = extract_context_capacity(&item);
        assert_eq!(cap, 0);
        assert_eq!(prov, "unknown");
    }

    #[test]
    fn test_parse_model_candidate_realistic_nim_fixture() {
        let fixture = json!({
            "data": [
                {
                    "id": "model-a",
                    "max_model_len": 131072,
                    "owned_by": "nvidia",
                    "supports_tools": true
                },
                {
                    "id": "model-b",
                    "max_model_len": 65536,
                    "owned_by": "meta",
                    "supports_tools": true
                },
                {
                    "id": "model-c",
                    "max_model_len": 32768,
                    "owned_by": "mistralai",
                    "supports_tools": true
                },
                {
                    "id": "model-d",
                    "max_model_len": 16384,
                    "owned_by": "01-ai",
                    "supports_tools": true
                },
                {
                    "id": "model-e-unknown",
                    "owned_by": "custom"
                }
            ]
        });

        let data = fixture["data"].as_array().unwrap();
        let parsed: Vec<_> = data
            .iter()
            .filter_map(|item| NvidiaProvider::parse_model_candidate(item, 1700000000))
            .collect();

        assert_eq!(parsed.len(), 5);

        assert_eq!(parsed[0].model_id, "model-a");
        assert_eq!(parsed[0].context_capacity, 131072);
        assert_eq!(
            parsed[0].context_provenance(),
            Some("provider:max_model_len")
        );
        assert!(parsed[0].is_context_known());
        assert!(parsed[0].supports_tools);
        assert_eq!(
            parsed[0].tool_support,
            crate::model::router::resolver::CapabilitySupport::Supported
        );

        assert_eq!(parsed[1].model_id, "model-b");
        assert_eq!(parsed[1].context_capacity, 65536);
        assert_eq!(
            parsed[1].context_provenance(),
            Some("provider:max_model_len")
        );
        assert!(parsed[1].is_context_known());

        assert_eq!(parsed[2].model_id, "model-c");
        assert_eq!(parsed[2].context_capacity, 32768);
        assert_eq!(
            parsed[2].context_provenance(),
            Some("provider:max_model_len")
        );
        assert!(parsed[2].is_context_known());

        assert_eq!(parsed[3].model_id, "model-d");
        assert_eq!(parsed[3].context_capacity, 16384);
        assert_eq!(
            parsed[3].context_provenance(),
            Some("provider:max_model_len")
        );
        assert!(parsed[3].is_context_known());

        assert_eq!(parsed[4].model_id, "model-e-unknown");
        assert_eq!(parsed[4].context_capacity, 0);
        assert_eq!(parsed[4].context_provenance(), Some("unknown"));
        assert!(!parsed[4].is_context_known());
        assert!(!parsed[4].supports_tools);
        assert_eq!(
            parsed[4].tool_support,
            crate::model::router::resolver::CapabilitySupport::Unknown
        );

        // Eligibility validation fails closed for unknown context and unknown tool support:
        let provider = NvidiaProvider::new(None, Some("test-key".to_string())).unwrap();
        assert!(provider.validate_candidate(&parsed[0]));
        assert!(provider.validate_candidate(&parsed[1]));
        assert!(provider.validate_candidate(&parsed[2]));
        assert!(provider.validate_candidate(&parsed[3]));
        assert!(!provider.validate_candidate(&parsed[4]));
    }

    #[tokio::test]
    async fn test_pipeline_enrichment_offline_canonical_references() {
        use crate::model::provider::nvidia_metadata::{
            NvidiaModelMetadataResolver, ProviderModelMetadataSource,
        };
        let offline_resolver = Arc::new(NvidiaModelMetadataResolver::new_offline());
        let provider = NvidiaProvider::new(None, Some("test-key".to_string()))
            .unwrap()
            .with_metadata_resolver(offline_resolver);

        // Simulate raw /v1/models response without context and without tool calling
        let raw_item_gemma = json!({
            "id": "google/gemma-4-31b-it",
            "owned_by": "google"
        });
        let raw_item_nemotron = json!({
            "id": "nvidia/nemotron-3-super-120b-a12b",
            "owned_by": "nvidia"
        });

        let mut candidate_gemma =
            NvidiaProvider::parse_model_candidate(&raw_item_gemma, 1700000000).unwrap();
        let mut candidate_nemotron =
            NvidiaProvider::parse_model_candidate(&raw_item_nemotron, 1700000000).unwrap();

        // Before enrichment: context is 0 (unknown), tool support is unknown
        assert_eq!(candidate_gemma.context_capacity, 0);
        assert!(!candidate_gemma.is_context_known());
        assert_eq!(
            candidate_gemma.tool_support,
            crate::model::router::resolver::CapabilitySupport::Unknown
        );

        // Enrich candidates using provider's metadata resolver
        provider
            .metadata_resolver()
            .enrich_candidate(&mut candidate_gemma)
            .await
            .unwrap();
        provider
            .metadata_resolver()
            .enrich_candidate(&mut candidate_nemotron)
            .await
            .unwrap();

        // After enrichment: Gemma 4 = 256K, Nemotron 3 Super = 1M
        assert_eq!(candidate_gemma.context_capacity, 256_000);
        assert_eq!(
            candidate_gemma.context_provenance(),
            Some("nvidia:model_reference")
        );
        assert_eq!(
            candidate_gemma.tool_support,
            crate::model::router::resolver::CapabilitySupport::Supported
        );
        assert!(candidate_gemma.supports_tools);
        assert!(provider.validate_candidate(&candidate_gemma));

        assert_eq!(candidate_nemotron.context_capacity, 1_000_000);
        assert_eq!(
            candidate_nemotron.context_provenance(),
            Some("nvidia:model_reference")
        );
        assert_eq!(
            candidate_nemotron.tool_support,
            crate::model::router::resolver::CapabilitySupport::Supported
        );
        assert!(candidate_nemotron.supports_tools);
        assert!(provider.validate_candidate(&candidate_nemotron));
    }
}
