//! Unified Model Provider Abstraction
//!
//! This module provides a trait-based abstraction over the NVIDIA NIM model provider
//! for v1, enabling reliable and verifiable model usage.

use anyhow::Result;
use async_trait::async_trait;
use m31a_core::types::{ChatMessage, TokenUsage, ToolDefinition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Chat completion request that works across all providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub tool_choice: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub stream: bool,
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub reasoning_budget: Option<i32>,
    pub extra_params: HashMap<String, serde_json::Value>,
}

/// Stream chunk from any provider
#[derive(Debug, Clone)]
pub enum StreamEvent {
    ContentDelta(String),
    ReasoningDelta(String),
    ToolCallDelta {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments: Option<String>,
    },
    Usage(TokenUsage),
    Done,
    Error(String),
}

/// Stream callback type
pub type StreamHandler = Arc<dyn Fn(StreamEvent) + Send + Sync>;

/// Model capability flags
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub supports_tools: bool,
    pub supports_reasoning: bool,
    pub supports_vision: bool,
    pub supports_parallel_tools: bool,
    pub supports_streaming: bool,
    pub max_context_tokens: u32,
    pub max_output_tokens: Option<u32>,
}

/// Model metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub capabilities: ModelCapabilities,
    pub context_window: u32,
    pub description: String,
    pub pricing: Option<ModelPricing>,
}

/// Pricing information (per 1M tokens)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub input_per_million: f64,
    pub output_per_million: f64,
    pub cache_read_per_million: Option<f64>,
    pub cache_write_per_million: Option<f64>,
}

/// Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider_type: ProviderType,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub model: String,
    pub extra: HashMap<String, serde_json::Value>,
}

/// Typed provider error hierarchy ensuring no raw secrets are ever leaked.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ProviderError {
    #[error(
        "Missing API key for provider '{provider}': remote providers require an explicit, non-empty API key"
    )]
    MissingApiKey { provider: String },
    #[error("Invalid endpoint URL for provider '{provider}': {endpoint}")]
    InvalidEndpoint { provider: String, endpoint: String },
    #[error("Unknown or ambiguous provider for model '{model}' and endpoint '{endpoint}'")]
    UnknownProvider { model: String, endpoint: String },
    #[error(
        "Model resolution failed for '{model}': custom models require explicit provider configuration"
    )]
    ModelResolutionFailed { model: String },
    #[error("Authentication failed for provider '{provider}' (HTTP {status})")]
    AuthenticationFailed { provider: String, status: u16 },
    #[error("Provider '{provider}' is unreachable: {message}")]
    Unreachable { provider: String, message: String },
    #[error("Model '{model}' is unavailable on provider '{provider}': {message}")]
    ModelUnavailable {
        provider: String,
        model: String,
        message: String,
    },
    #[error("Rate limited by provider '{provider}': {message}")]
    RateLimited { provider: String, message: String },
    #[error("Provider '{provider}' error: {message}")]
    Other { provider: String, message: String },
}

/// Validates that a provider configuration satisfies required endpoint and authentication invariants.
pub fn validate_provider_config(config: &ProviderConfig) -> Result<(), ProviderError> {
    // 1. Endpoint format validation if provided
    if let Some(ref base) = config.api_base {
        let trimmed = base.trim();
        if trimmed.is_empty()
            || (!trimmed.starts_with("http://") && !trimmed.starts_with("https://"))
        {
            return Err(ProviderError::InvalidEndpoint {
                provider: config.provider_type.display_name().to_string(),
                endpoint: base.clone(),
            });
        }
    }

    // 2. Model name validation
    if config.model.trim().is_empty() {
        return Err(ProviderError::ModelResolutionFailed {
            model: config.model.clone(),
        });
    }

    // 3. Credential validation based on provider type
    match config.provider_type {
        ProviderType::Nim => {
            let key = config.api_key.as_deref().unwrap_or("");
            if key.trim().is_empty() {
                return Err(ProviderError::MissingApiKey {
                    provider: config.provider_type.display_name().to_string(),
                });
            }
            let is_local = config
                .api_base
                .as_deref()
                .map(|b| b.contains("127.0.0.1") || b.contains("localhost"))
                .unwrap_or(false);
            if !is_local
                && crate::models::find_canonical_model(&config.model).is_none()
                && crate::models::find_model(&config.model).is_none()
                && config.model != ProviderType::Nim.default_model()
                && !config.model.starts_with("nvidia/")
                && !config.model.starts_with("meta/")
                && !config.model.starts_with("deepseek")
            {
                return Err(ProviderError::ModelUnavailable {
                    provider: "NVIDIA NIM".to_string(),
                    model: config.model.clone(),
                    message: "not a recognized model in NVIDIA NIM catalog".to_string(),
                });
            }
            Ok(())
        }
    }
}

/// Provider type enumeration. In v1, NVIDIA NIM is the single supported provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    #[serde(alias = "nvidia", alias = "nvidia-nim")]
    Nim,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nim => write!(f, "nim"),
        }
    }
}

impl ProviderType {
    pub fn default_api_base(&self) -> &'static str {
        match self {
            Self::Nim => "https://integrate.api.nvidia.com/v1",
        }
    }

    pub fn default_model(&self) -> &'static str {
        match self {
            Self::Nim => "nvidia/nemotron-3.5-lightning-30b-a3b",
        }
    }

    pub fn is_local(&self) -> bool {
        false
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Nim => "NVIDIA NIM",
        }
    }

    /// Deterministically resolves the provider type.
    /// In v1, NVIDIA NIM is the single supported provider.
    ///
    /// Precedence rules:
    /// 1. Explicit provider selection ("nim", "nvidia", "nvidia-nim") resolves to NIM.
    /// 2. Other explicit providers fail explicitly with `UnknownProvider`.
    /// 3. Canonical catalog models resolve to NIM.
    /// 4. Known endpoint domain mappings (NVIDIA NIM or local testing endpoints) resolve to NIM.
    /// 5. Ambiguous endpoints or unknown models fail closed.
    pub fn resolve(
        explicit_provider: Option<&str>,
        model: &str,
        endpoint: Option<&str>,
    ) -> Result<Self, ProviderError> {
        // 1. Explicit provider takes precedence
        if let Some(p) = explicit_provider {
            let trimmed = p.trim().to_lowercase();
            match trimmed.as_str() {
                "nim" | "nvidia" | "nvidia-nim" => return Ok(Self::Nim),
                _ => {
                    return Err(ProviderError::UnknownProvider {
                        model: model.to_string(),
                        endpoint: p.to_string(),
                    });
                }
            }
        }

        // 2. Exact registered canonical model check (for NVIDIA catalog)
        if crate::models::find_canonical_model(model).is_some()
            || crate::models::find_model(model).is_some()
            || model == Self::Nim.default_model()
            || model.starts_with("nvidia/")
            || model.starts_with("meta/")
            || model.starts_with("deepseek")
            || model.starts_with("mistralai/")
            || model.starts_with("qwen/")
        {
            return Ok(Self::Nim);
        }

        // 3. Known endpoint domain mappings
        if let Some(ep) = endpoint {
            let ep_lower = ep.to_lowercase();
            if ep_lower.contains("nvidia.com")
                || ep_lower.contains("127.0.0.1")
                || ep_lower.contains("localhost")
            {
                return Ok(Self::Nim);
            }
        }

        // Rule: In v1, only NVIDIA NIM is supported; all other models/endpoints fail closed.
        Err(ProviderError::UnknownProvider {
            model: model.to_string(),
            endpoint: endpoint.unwrap_or("none").to_string(),
        })
    }

    pub fn infer_from_model_or_endpoint(
        model: &str,
        endpoint: Option<&str>,
    ) -> Result<Self, ProviderError> {
        Self::resolve(None, model, endpoint)
    }
}

/// Core trait that all model providers must implement
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Provider type identifier
    fn provider_type(&self) -> ProviderType;

    /// Get the currently configured model
    fn current_model(&self) -> &str;

    /// Set the model for subsequent requests
    fn set_model(&mut self, model: &str);

    /// Get model capabilities for the current model
    fn capabilities(&self) -> ModelCapabilities;

    /// List all available models from this provider
    async fn list_models(&self) -> Result<Vec<ModelMetadata>>;

    /// Check if the provider is reachable
    async fn health_check(&self) -> Result<bool>;

    /// Check if the specified model is available on this provider
    async fn test_model_availability(&self, model: &str) -> Result<()>;

    /// Stream a chat completion
    async fn stream_chat(
        &self,
        request: ChatRequest,
        handler: StreamHandler,
    ) -> Result<ChatCompletionResponse>;

    /// Non-streaming chat completion (for compaction, etc.)
    async fn chat(&self, request: ChatRequest) -> Result<ChatCompletionResponse>;

    /// Estimate token count for messages (optional, uses tiktoken approximation)
    fn estimate_tokens(&self, messages: &[ChatMessage]) -> usize {
        messages
            .iter()
            .map(|m| m.content.as_deref().map_or(0, |s| s.len() / 4))
            .sum()
    }
}

/// Response from a chat completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub message: ChatMessage,
    pub usage: TokenUsage,
    pub model: String,
    pub finish_reason: Option<String>,
}

impl ChatCompletionResponse {
    /// Estimates cost in USD for this completion based on model pricing if available.
    pub fn estimate_cost_usd(&self) -> Option<f64> {
        crate::models_identity::calculate_token_cost(
            &self.model,
            self.usage.prompt_tokens,
            self.usage.completion_tokens,
        )
    }
}

/// Abstract provider for dense vector text embeddings.
#[async_trait]
pub trait EmbeddingsProvider: Send + Sync {
    /// Computes dense embeddings for a slice of input strings.
    async fn create_embeddings(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>>;
}

/// Builder for creating providers from config
pub struct ProviderBuilder;

impl ProviderBuilder {
    pub fn build(config: ProviderConfig) -> Result<Box<dyn ModelProvider>> {
        validate_provider_config(&config)?;
        match config.provider_type {
            ProviderType::Nim => {
                let api_base = config
                    .api_base
                    .unwrap_or_else(|| ProviderType::Nim.default_api_base().to_string());
                let api_key = config
                    .api_key
                    .ok_or_else(|| anyhow::anyhow!("API key required for NVIDIA NIM provider"))?;
                let mut nim = NimProvider::new(api_base, api_key, config.model);
                let mut opts = crate::client::NimRequestOptions::default();
                if let Some(b) = config
                    .extra
                    .get("reasoning_budget")
                    .and_then(|v| v.as_i64())
                {
                    opts.reasoning_budget = Some(b as i32);
                }
                if let Some(m) = config.extra.get("max_tokens").and_then(|v| v.as_u64()) {
                    opts.max_tokens = Some(m as u32);
                }
                if let Some(t) = config.extra.get("temperature").and_then(|v| v.as_f64()) {
                    opts.temperature = Some(t as f32);
                }
                if let Some(tc) = config.extra.get("tool_choice").and_then(|v| v.as_str()) {
                    opts.tool_choice = Some(tc.to_string());
                }
                nim = nim.with_options(opts);
                Ok(Box::new(nim))
            }
        }
    }

    pub fn build_client(config: ProviderConfig) -> Result<ProviderClient> {
        validate_provider_config(&config)?;
        let model = config.model.clone();
        let provider: Arc<dyn ModelProvider> = match config.provider_type {
            ProviderType::Nim => {
                let api_base = config
                    .api_base
                    .unwrap_or_else(|| ProviderType::Nim.default_api_base().to_string());
                let api_key = config
                    .api_key
                    .ok_or_else(|| anyhow::anyhow!("API key required for NVIDIA NIM provider"))?;
                let mut nim = NimProvider::new(api_base, api_key, config.model);
                let mut opts = crate::client::NimRequestOptions::default();
                if let Some(b) = config
                    .extra
                    .get("reasoning_budget")
                    .and_then(|v| v.as_i64())
                {
                    opts.reasoning_budget = Some(b as i32);
                }
                if let Some(m) = config.extra.get("max_tokens").and_then(|v| v.as_u64()) {
                    opts.max_tokens = Some(m as u32);
                }
                if let Some(t) = config.extra.get("temperature").and_then(|v| v.as_f64()) {
                    opts.temperature = Some(t as f32);
                }
                if let Some(tc) = config.extra.get("tool_choice").and_then(|v| v.as_str()) {
                    opts.tool_choice = Some(tc.to_string());
                }
                nim = nim.with_options(opts);
                Arc::new(nim)
            }
        };
        Ok(ProviderClient::new(provider).with_model(model))
    }
}

/// NVIDIA NIM Provider Implementation
pub struct NimProvider {
    inner: crate::client::NvidiaNimClient,
}

impl NimProvider {
    pub fn new(api_base: String, api_key: String, model: String) -> Self {
        Self {
            inner: crate::client::NvidiaNimClient::new(api_base, api_key, model),
        }
    }

    pub fn from_client(inner: crate::client::NvidiaNimClient) -> Self {
        Self { inner }
    }

    pub fn with_options(mut self, options: crate::client::NimRequestOptions) -> Self {
        self.inner = self.inner.with_options(options);
        self
    }
}

#[async_trait]
impl ModelProvider for NimProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Nim
    }

    fn current_model(&self) -> &str {
        self.inner.model()
    }

    fn set_model(&mut self, model: &str) {
        self.inner = self.inner.with_model(model);
    }

    fn capabilities(&self) -> ModelCapabilities {
        let model = self.inner.model();
        let info = crate::models::find_model(model);
        let canonical = crate::models::find_canonical_model(model);

        let context_window = info
            .as_ref()
            .map(|m| m.context_window)
            .or_else(|| canonical.as_ref().and_then(|c| c.context_window))
            .unwrap_or(131072);

        let max_output = info
            .as_ref()
            .map(|m| m.max_output_tokens)
            .or_else(|| canonical.as_ref().and_then(|c| c.max_output_tokens))
            .unwrap_or(8192);

        let supports_tools = info
            .as_ref()
            .map(|m| m.supports_tools)
            .or_else(|| canonical.as_ref().map(|c| c.capabilities.tool_calling))
            .unwrap_or(true);

        let supports_reasoning = info
            .as_ref()
            .map(|m| m.supports_thinking)
            .or_else(|| canonical.as_ref().map(|c| c.capabilities.reasoning))
            .unwrap_or_else(|| {
                model.contains("deepseek")
                    || model.contains("nemotron")
                    || model.contains("reasoning")
                    || model.contains("r1")
            });

        let supports_vision = info
            .as_ref()
            .map(|m| m.supports_vision)
            .or_else(|| canonical.as_ref().map(|c| c.capabilities.vision))
            .unwrap_or(false);

        ModelCapabilities {
            supports_tools,
            supports_reasoning,
            supports_vision,
            supports_parallel_tools: supports_tools,
            supports_streaming: true,
            max_context_tokens: context_window,
            max_output_tokens: Some(max_output),
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelMetadata>> {
        let catalog = crate::models::get_nvidia_model_catalog();
        Ok(catalog
            .into_iter()
            .map(|m| {
                let canonical = crate::models::find_canonical_model(&m.slug);
                let pricing = canonical.and_then(|c| {
                    if let (Some(inp), Some(out)) =
                        (c.pricing.input_per_million, c.pricing.output_per_million)
                    {
                        Some(ModelPricing {
                            input_per_million: inp,
                            output_per_million: out,
                            cache_read_per_million: None,
                            cache_write_per_million: None,
                        })
                    } else {
                        None
                    }
                });

                ModelMetadata {
                    id: m.slug.clone(),
                    display_name: m.display_name.clone(),
                    provider: "nim".to_string(),
                    capabilities: ModelCapabilities {
                        supports_tools: m.supports_tools,
                        supports_reasoning: m.supports_thinking,
                        supports_vision: m.supports_vision,
                        supports_parallel_tools: true,
                        supports_streaming: true,
                        max_context_tokens: m.context_window,
                        max_output_tokens: Some(m.max_output_tokens),
                    },
                    context_window: m.context_window,
                    description: m.description.clone(),
                    pricing,
                }
            })
            .collect())
    }

    async fn health_check(&self) -> Result<bool> {
        self.inner.check_connection().await.map(|_| true)
    }

    async fn test_model_availability(&self, model: &str) -> Result<()> {
        let client = self.inner.clone().with_model(model);
        client.test_model_availability().await
    }

    async fn stream_chat(
        &self,
        request: ChatRequest,
        handler: StreamHandler,
    ) -> Result<ChatCompletionResponse> {
        let on_content = handler.clone();
        let on_reasoning = handler.clone();

        let mut client = self.inner.clone().with_model(&request.model);
        if let Some(ref tc) = request.tool_choice {
            client = client.with_tool_choice(Some(tc.clone()));
        }
        if let Some(t) = request.temperature {
            client = client.with_temperature(Some(t));
        }
        if let Some(m) = request.max_tokens {
            client = client.with_max_tokens(Some(m));
        }
        if let Some(ref r) = request.reasoning_effort {
            client = client.with_reasoning_effort(Some(r.clone()));
        }
        if let Some(b) = request.reasoning_budget {
            client = client.with_reasoning_budget(Some(b));
        }

        let (message, usage) = client
            .stream_chat(
                &request.messages,
                request.tools.as_deref(),
                Some(Arc::new(move |chunk: &str| {
                    on_content(StreamEvent::ContentDelta(chunk.to_string()));
                })),
                Some(Arc::new(move |chunk: &str| {
                    on_reasoning(StreamEvent::ReasoningDelta(chunk.to_string()));
                })),
            )
            .await?;

        handler(StreamEvent::Done);
        Ok(ChatCompletionResponse {
            message,
            usage,
            model: request.model,
            finish_reason: Some("stop".to_string()),
        })
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatCompletionResponse> {
        let mut client = self.inner.clone().with_model(&request.model);
        if let Some(ref tc) = request.tool_choice {
            client = client.with_tool_choice(Some(tc.clone()));
        }
        if let Some(t) = request.temperature {
            client = client.with_temperature(Some(t));
        }
        if let Some(m) = request.max_tokens {
            client = client.with_max_tokens(Some(m));
        }
        if let Some(ref r) = request.reasoning_effort {
            client = client.with_reasoning_effort(Some(r.clone()));
        }
        if let Some(b) = request.reasoning_budget {
            client = client.with_reasoning_budget(Some(b));
        }

        let (message, usage) = client
            .chat(&request.messages, request.tools.as_deref())
            .await?;
        Ok(ChatCompletionResponse {
            message,
            usage,
            model: request.model,
            finish_reason: Some("stop".to_string()),
        })
    }
}

#[async_trait]
impl EmbeddingsProvider for NimProvider {
    async fn create_embeddings(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        self.inner.create_embeddings(inputs).await
    }
}

/// Universal provider client for driving agent orchestration.
/// Encapsulates model provider implementing `ModelProvider` (NVIDIA NIM in v1)
/// and exposes a stable interface for turn loops, agents, and tool loops.
#[derive(Clone)]
pub struct ProviderClient {
    provider: Arc<dyn ModelProvider>,
    model: String,
    reasoning_effort: Option<String>,
    reasoning_budget: Option<i32>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    tool_choice: Option<String>,
}

impl std::fmt::Debug for ProviderClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderClient")
            .field("model", &self.model)
            .field("reasoning_effort", &self.reasoning_effort)
            .field("reasoning_budget", &self.reasoning_budget)
            .field("max_tokens", &self.max_tokens)
            .field("temperature", &self.temperature)
            .field("tool_choice", &self.tool_choice)
            .finish()
    }
}

impl ProviderClient {
    pub fn new(provider: Arc<dyn ModelProvider>) -> Self {
        let model = provider.current_model().to_string();
        Self {
            provider,
            model,
            reasoning_effort: None,
            reasoning_budget: None,
            max_tokens: None,
            temperature: None,
            tool_choice: None,
        }
    }

    pub fn from_provider<P: ModelProvider + 'static>(provider: P) -> Self {
        Self::new(Arc::new(provider))
    }

    pub fn provider(&self) -> &Arc<dyn ModelProvider> {
        &self.provider
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn with_model(&self, model: impl Into<String>) -> Self {
        Self {
            provider: self.provider.clone(),
            model: model.into(),
            reasoning_effort: self.reasoning_effort.clone(),
            reasoning_budget: self.reasoning_budget,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            tool_choice: self.tool_choice.clone(),
        }
    }

    pub fn with_reasoning_effort(mut self, effort: Option<String>) -> Self {
        self.reasoning_effort = effort;
        self
    }

    pub fn with_reasoning_budget(mut self, budget: Option<i32>) -> Self {
        self.reasoning_budget = budget;
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: Option<u32>) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    pub fn with_temperature(mut self, temperature: Option<f32>) -> Self {
        self.temperature = temperature;
        self
    }

    pub fn with_tool_choice(mut self, tool_choice: Option<String>) -> Self {
        self.tool_choice = tool_choice;
        self
    }

    pub fn reasoning_effort(&self) -> Option<&str> {
        self.reasoning_effort.as_deref()
    }

    pub fn reasoning_budget(&self) -> Option<i32> {
        self.reasoning_budget
    }

    pub fn max_tokens(&self) -> Option<u32> {
        self.max_tokens
    }

    pub fn temperature(&self) -> Option<f32> {
        self.temperature
    }

    pub fn tool_choice(&self) -> Option<&str> {
        self.tool_choice.as_deref()
    }

    /// Returns dynamic model capabilities for this client's configured model.
    pub fn capabilities(&self) -> ModelCapabilities {
        let mut caps = self.provider.capabilities();
        if self.model != self.provider.current_model() {
            if let Some(info) = crate::models::find_model(&self.model) {
                caps.max_context_tokens = info.context_window;
                caps.max_output_tokens = Some(info.max_output_tokens);
                caps.supports_tools = info.supports_tools;
                caps.supports_reasoning = info.supports_thinking;
                caps.supports_vision = info.supports_vision;
            } else if let Some(canon) = crate::models::find_canonical_model(&self.model) {
                if let Some(cw) = canon.context_window {
                    caps.max_context_tokens = cw;
                }
                if let Some(mo) = canon.max_output_tokens {
                    caps.max_output_tokens = Some(mo);
                }
                caps.supports_tools = canon.capabilities.tool_calling;
                caps.supports_reasoning = canon.capabilities.reasoning;
                caps.supports_vision = canon.capabilities.vision;
            }
        }
        caps
    }

    pub async fn check_connection(&self) -> Result<()> {
        self.provider.health_check().await.map(|_| ())
    }

    pub async fn test_model_availability(&self) -> Result<()> {
        self.provider.test_model_availability(&self.model).await
    }

    pub async fn stream_chat(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
        on_content_chunk: Option<crate::client::StreamCallback>,
        on_reasoning_chunk: Option<crate::client::StreamCallback>,
    ) -> Result<(ChatMessage, TokenUsage)> {
        let handler: StreamHandler = Arc::new(move |event| match event {
            StreamEvent::ContentDelta(text) => {
                if let Some(ref cb) = on_content_chunk {
                    cb(&text);
                }
            }
            StreamEvent::ReasoningDelta(text) => {
                if let Some(ref cb) = on_reasoning_chunk {
                    cb(&text);
                }
            }
            _ => {}
        });

        let req = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            tools: tools.map(|t| t.to_vec()),
            tool_choice: self.tool_choice.clone(),
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            stream: true,
            reasoning_effort: self.reasoning_effort.clone(),
            reasoning_budget: self.reasoning_budget,
            extra_params: HashMap::new(),
        };

        let resp = self.provider.stream_chat(req, handler).await?;
        Ok((resp.message, resp.usage))
    }

    pub async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<(ChatMessage, TokenUsage)> {
        let req = ChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            tools: tools.map(|t| t.to_vec()),
            tool_choice: self.tool_choice.clone(),
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            stream: false,
            reasoning_effort: self.reasoning_effort.clone(),
            reasoning_budget: self.reasoning_budget,
            extra_params: HashMap::new(),
        };

        let resp = self.provider.chat(req).await?;
        Ok((resp.message, resp.usage))
    }
}

impl From<crate::client::NvidiaNimClient> for ProviderClient {
    fn from(nim: crate::client::NvidiaNimClient) -> Self {
        Self::from_provider(NimProvider::from_client(nim))
    }
}

impl<P: ModelProvider + 'static> From<Arc<P>> for ProviderClient {
    fn from(p: Arc<P>) -> Self {
        Self::new(p)
    }
}

// Types are already public via `pub struct`/`pub type` above
// Re-exports handled in lib.rs
