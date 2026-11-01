//! Unified Model Provider Abstraction
//!
//! This module provides a trait-based abstraction over different model providers
//! (NVIDIA NIM, OpenAI, Anthropic, Ollama, LM Studio, Bedrock, Vertex AI, etc.)
//! enabling vendor-independent model usage.

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
        #[cfg(any(test, feature = "test-support"))]
        ProviderType::Mock => Ok(()),

        ProviderType::Ollama | ProviderType::LmStudio => {
            // Local providers deliberately operate without an API key
            Ok(())
        }

        ProviderType::Custom => {
            // Custom endpoints must have an explicit authentication policy:
            // Either a non-empty, non-whitespace API key is provided,
            // or extra explicitly specifies allow_anonymous = true or auth_policy = "none".
            let has_key = config
                .api_key
                .as_ref()
                .map(|k| !k.trim().is_empty())
                .unwrap_or(false);
            let allow_anonymous = config
                .extra
                .get("allow_anonymous")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                || config
                    .extra
                    .get("auth_policy")
                    .and_then(|v| v.as_str())
                    .map(|s| s.eq_ignore_ascii_case("none"))
                    .unwrap_or(false);

            if !has_key && !allow_anonymous {
                return Err(ProviderError::MissingApiKey {
                    provider: format!(
                        "Custom ({})",
                        config.api_base.as_deref().unwrap_or("unspecified")
                    ),
                });
            }
            Ok(())
        }

        ProviderType::Nim => {
            let key = config.api_key.as_deref().unwrap_or("");
            if key.trim().is_empty() {
                return Err(ProviderError::MissingApiKey {
                    provider: config.provider_type.display_name().to_string(),
                });
            }
            if crate::models::find_canonical_model(&config.model).is_none()
                && crate::models::find_model(&config.model).is_none()
                && config.model != ProviderType::Nim.default_model()
                && !config.model.starts_with("nvidia/")
            {
                return Err(ProviderError::ModelUnavailable {
                    provider: "NVIDIA NIM".to_string(),
                    model: config.model.clone(),
                    message: "not a recognized model in NVIDIA NIM catalog".to_string(),
                });
            }
            Ok(())
        }

        ProviderType::OpenAi
        | ProviderType::Anthropic
        | ProviderType::Gemini
        | ProviderType::Groq
        | ProviderType::DeepSeek
        | ProviderType::Mistral
        | ProviderType::Together => {
            let key = config.api_key.as_deref().unwrap_or("");
            if key.trim().is_empty() {
                return Err(ProviderError::MissingApiKey {
                    provider: config.provider_type.display_name().to_string(),
                });
            }
            Ok(())
        }
    }
}

/// Provider type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Nim,
    Ollama,
    LmStudio,
    OpenAi,
    Anthropic,
    Gemini,
    Groq,
    DeepSeek,
    Mistral,
    Together,
    Custom,
    /// Explicit test-only provider. Never inferred from a model name or endpoint.
    #[cfg(any(test, feature = "test-support"))]
    Mock,
}

impl ProviderType {
    pub fn default_api_base(&self) -> &'static str {
        match self {
            Self::Nim => "https://integrate.api.nvidia.com/v1",
            Self::Ollama => "http://localhost:11434/v1",
            Self::LmStudio => "http://localhost:1234/v1",
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Anthropic => "https://api.anthropic.com/v1",
            Self::Gemini => "https://generativelanguage.googleapis.com/v1beta/openai",
            Self::Groq => "https://api.groq.com/openai/v1",
            Self::DeepSeek => "https://api.deepseek.com/v1",
            Self::Mistral => "https://api.mistral.ai/v1",
            Self::Together => "https://api.together.xyz/v1",
            Self::Custom => "http://localhost:8000/v1",
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock => "http://localhost:0",
        }
    }

    pub fn default_model(&self) -> &'static str {
        match self {
            Self::Nim => "meta/llama-3.3-70b-instruct",
            Self::Ollama => "llama3.2:latest",
            Self::LmStudio => "default",
            Self::OpenAi => "gpt-4o",
            Self::Anthropic => "claude-3-5-sonnet-20241022",
            Self::Gemini => "gemini-1.5-pro-latest",
            Self::Groq => "llama-3.3-70b-versatile",
            Self::DeepSeek => "deepseek-chat",
            Self::Mistral => "mistral-large-latest",
            Self::Together => "meta-llama/Llama-3.3-70B-Instruct-Turbo",
            Self::Custom => "default",
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock => "mock-model",
        }
    }

    pub fn is_local(&self) -> bool {
        matches!(self, Self::Ollama | Self::LmStudio)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Nim => "NVIDIA NIM",
            Self::Ollama => "Ollama (Local)",
            Self::LmStudio => "LM Studio (Local)",
            Self::OpenAi => "OpenAI",
            Self::Anthropic => "Anthropic",
            Self::Gemini => "Google Gemini",
            Self::Groq => "Groq",
            Self::DeepSeek => "DeepSeek",
            Self::Mistral => "Mistral AI",
            Self::Together => "Together AI",
            Self::Custom => "Custom Endpoint",
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock => "Mock Engine",
        }
    }

    /// Deterministically resolves the provider type given an optional explicit provider,
    /// model identifier, and optional endpoint.
    ///
    /// Precedence rules:
    /// 1. An explicit provider selection in configuration or model metadata takes precedence.
    /// 2. Known provider identifiers and endpoint mappings support backward-compatible migration.
    /// 3. Registered models in the canonical catalog with default or NVIDIA endpoint resolve to NIM.
    /// 4. Known model prefixes (e.g. claude-, gpt-) resolve to their respective providers.
    /// 5. Ambiguous custom endpoints or unknown models produce an explicit resolution error.
    /// 6. Model names never select MockProvider.
    /// 7. Never silently falls back to a default provider.
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
                "openai" => return Ok(Self::OpenAi),
                "anthropic" => return Ok(Self::Anthropic),
                "gemini" | "google" => return Ok(Self::Gemini),
                "groq" => return Ok(Self::Groq),
                "deepseek" => return Ok(Self::DeepSeek),
                "mistral" | "mistralai" => return Ok(Self::Mistral),
                "together" | "togetherai" => return Ok(Self::Together),
                "ollama" => return Ok(Self::Ollama),
                "lmstudio" | "lm-studio" => return Ok(Self::LmStudio),
                "custom" | "generic" | "openai-compatible" => return Ok(Self::Custom),
                #[cfg(any(test, feature = "test-support"))]
                "mock" => return Ok(Self::Mock),
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
        {
            return Ok(Self::Nim);
        }

        // 3. Known endpoint domain mappings
        if let Some(ep) = endpoint {
            let ep_lower = ep.to_lowercase();
            if ep_lower.contains("anthropic.com") {
                return Ok(Self::Anthropic);
            }
            if ep_lower.contains("api.openai.com") {
                return Ok(Self::OpenAi);
            }
            if ep_lower.contains("generativelanguage.googleapis.com") {
                return Ok(Self::Gemini);
            }
            if ep_lower.contains("groq.com") {
                return Ok(Self::Groq);
            }
            if ep_lower.contains("deepseek.com") {
                return Ok(Self::DeepSeek);
            }
            if ep_lower.contains("mistral.ai") {
                return Ok(Self::Mistral);
            }
            if ep_lower.contains("together.xyz") {
                return Ok(Self::Together);
            }
            if ep_lower.contains("11434") {
                return Ok(Self::Ollama);
            }
            if ep_lower.contains("1234") {
                return Ok(Self::LmStudio);
            }
            // Note: Do NOT match integrate.api.nvidia.com here for arbitrary unknown models.
            // If an unrecognized model is passed with the default NIM endpoint,
            // it must not be silently routed to NVIDIA NIM.
        }

        // 4. Backward-compatible prefix migrations for standard cloud providers
        if model.starts_with("claude-") {
            Ok(Self::Anthropic)
        } else if model.starts_with("gpt-") || model.starts_with("o1") || model.starts_with("o3") {
            Ok(Self::OpenAi)
        } else if model.starts_with("gemini-") {
            Ok(Self::Gemini)
        } else {
            // Rule 2.3: Unknown models and ambiguous endpoints must produce an explicit resolution error!
            // No silent fallback to NVIDIA NIM, and no MockProvider.
            Err(ProviderError::UnknownProvider {
                model: model.to_string(),
                endpoint: endpoint.unwrap_or("none").to_string(),
            })
        }
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
            #[cfg(any(test, feature = "test-support"))]
            ProviderType::Mock => Ok(Box::new(MockProvider::new(config.model))),
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
            ProviderType::Anthropic => {
                let api_base = config
                    .api_base
                    .unwrap_or_else(|| ProviderType::Anthropic.default_api_base().to_string());
                let api_key = config
                    .api_key
                    .ok_or_else(|| anyhow::anyhow!("API key required for Anthropic provider"))?;
                Ok(Box::new(AnthropicProvider::new(
                    api_base,
                    api_key,
                    config.model,
                )))
            }
            ProviderType::Ollama | ProviderType::LmStudio => {
                let api_base = config
                    .api_base
                    .unwrap_or_else(|| config.provider_type.default_api_base().to_string());
                let api_key = config.api_key.unwrap_or_else(|| "local-no-key".to_string());
                Ok(Box::new(GenericOpenAiProvider::new(
                    config.provider_type,
                    api_base,
                    api_key,
                    config.model,
                )))
            }
            ProviderType::Custom => {
                let api_base = config
                    .api_base
                    .unwrap_or_else(|| ProviderType::Custom.default_api_base().to_string());
                let api_key = config.api_key.unwrap_or_default();
                Ok(Box::new(GenericOpenAiProvider::new(
                    config.provider_type,
                    api_base,
                    api_key,
                    config.model,
                )))
            }
            other => {
                let api_base = config
                    .api_base
                    .unwrap_or_else(|| other.default_api_base().to_string());
                let api_key = config.api_key.ok_or_else(|| {
                    anyhow::anyhow!("API key required for {} provider", other.display_name())
                })?;
                Ok(Box::new(GenericOpenAiProvider::new(
                    other,
                    api_base,
                    api_key,
                    config.model,
                )))
            }
        }
    }

    pub fn build_client(config: ProviderConfig) -> Result<ProviderClient> {
        validate_provider_config(&config)?;
        let model = config.model.clone();
        let provider: Arc<dyn ModelProvider> = match config.provider_type {
            #[cfg(any(test, feature = "test-support"))]
            ProviderType::Mock => Arc::new(MockProvider::new(config.model)),
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
            ProviderType::Anthropic => {
                let api_base = config
                    .api_base
                    .unwrap_or_else(|| ProviderType::Anthropic.default_api_base().to_string());
                let api_key = config
                    .api_key
                    .ok_or_else(|| anyhow::anyhow!("API key required for Anthropic provider"))?;
                Arc::new(AnthropicProvider::new(api_base, api_key, config.model))
            }
            ProviderType::Ollama | ProviderType::LmStudio => {
                let api_base = config
                    .api_base
                    .unwrap_or_else(|| config.provider_type.default_api_base().to_string());
                let api_key = config.api_key.unwrap_or_else(|| "local-no-key".to_string());
                Arc::new(GenericOpenAiProvider::new(
                    config.provider_type,
                    api_base,
                    api_key,
                    config.model,
                ))
            }
            ProviderType::Custom => {
                let api_base = config
                    .api_base
                    .unwrap_or_else(|| ProviderType::Custom.default_api_base().to_string());
                let api_key = config.api_key.unwrap_or_default();
                Arc::new(GenericOpenAiProvider::new(
                    config.provider_type,
                    api_base,
                    api_key,
                    config.model,
                ))
            }
            other => {
                let api_base = config
                    .api_base
                    .unwrap_or_else(|| other.default_api_base().to_string());
                let api_key = config.api_key.ok_or_else(|| {
                    anyhow::anyhow!("API key required for {} provider", other.display_name())
                })?;
                Arc::new(GenericOpenAiProvider::new(
                    other,
                    api_base,
                    api_key,
                    config.model,
                ))
            }
        };
        Ok(ProviderClient::new(provider).with_model(model))
    }
}

/// Universal OpenAI-compatible Model Provider
/// Powers Ollama, LM Studio, OpenAI, Anthropic, Gemini, Groq, DeepSeek, Mistral, Together.
pub struct GenericOpenAiProvider {
    provider_type: ProviderType,
    inner: crate::client::NvidiaNimClient,
}

impl GenericOpenAiProvider {
    pub fn new(
        provider_type: ProviderType,
        api_base: String,
        api_key: String,
        model: String,
    ) -> Self {
        Self {
            provider_type,
            inner: crate::client::NvidiaNimClient::new(api_base, api_key, model),
        }
    }
}

#[async_trait]
impl ModelProvider for GenericOpenAiProvider {
    fn provider_type(&self) -> ProviderType {
        self.provider_type
    }

    fn current_model(&self) -> &str {
        self.inner.model()
    }

    fn set_model(&mut self, model: &str) {
        self.inner = self.inner.with_model(model);
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            supports_tools: true,
            supports_reasoning: self.inner.model().contains("r1")
                || self.inner.model().contains("reasoning")
                || self.inner.model().contains("o1")
                || self.inner.model().contains("o3"),
            supports_vision: self.inner.model().contains("vision")
                || self.inner.model().contains("4o")
                || self.inner.model().contains("gemini"),
            supports_parallel_tools: true,
            supports_streaming: true,
            max_context_tokens: 131072,
            max_output_tokens: Some(8192),
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelMetadata>> {
        let is_ollama = self.provider_type == ProviderType::Ollama;
        let target_url = if is_ollama {
            format!(
                "{}/api/tags",
                self.inner
                    .api_base()
                    .trim_end_matches("/v1")
                    .trim_end_matches('/')
            )
        } else {
            format!("{}/models", self.inner.api_base().trim_end_matches('/'))
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        let mut req = client.get(&target_url);
        if !self.inner.api_key().is_empty() && self.inner.api_key() != "local-no-key" {
            req = req.header("Authorization", format!("Bearer {}", self.inner.api_key()));
        }

        let mut models = Vec::new();
        if let Ok(resp) = req.send().await
            && let Ok(json) = resp.json::<serde_json::Value>().await
        {
            if is_ollama {
                if let Some(arr) = json.get("models").and_then(|v| v.as_array()) {
                    for item in arr {
                        if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                            models.push(ModelMetadata {
                                id: name.to_string(),
                                display_name: format!("Ollama: {}", name),
                                provider: self.provider_type.display_name().to_string(),
                                capabilities: self.capabilities(),
                                context_window: 131072,
                                description: format!("Local Ollama model: {}", name),
                                pricing: None,
                            });
                        }
                    }
                }
            } else if let Some(arr) = json.get("data").and_then(|v| v.as_array()) {
                for item in arr {
                    if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                        models.push(ModelMetadata {
                            id: id.to_string(),
                            display_name: format!("{}: {}", self.provider_type.display_name(), id),
                            provider: self.provider_type.display_name().to_string(),
                            capabilities: self.capabilities(),
                            context_window: 131072,
                            description: format!("Catalog model: {}", id),
                            pricing: None,
                        });
                    }
                }
            }
        }

        if models.is_empty() {
            models.push(ModelMetadata {
                id: self.current_model().to_string(),
                display_name: format!(
                    "{}: {}",
                    self.provider_type.display_name(),
                    self.current_model()
                ),
                provider: self.provider_type.display_name().to_string(),
                capabilities: self.capabilities(),
                context_window: 131072,
                description: format!(
                    "Default configured model for {}",
                    self.provider_type.display_name()
                ),
                pricing: None,
            });
        }

        Ok(models)
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

        let mut client = self.inner.clone();
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
        let mut client = self.inner.clone();
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
        // NIM models generally support tools and some support reasoning
        ModelCapabilities {
            supports_tools: true,
            supports_reasoning: self.inner.model().contains("deepseek")
                || self.inner.model().contains("nemotron"),
            supports_vision: false,
            supports_parallel_tools: true,
            supports_streaming: true,
            max_context_tokens: 131072,
            max_output_tokens: Some(8192),
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

        let mut client = self.inner.clone();
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
        let mut client = self.inner.clone();
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
/// Native Anthropic Messages API Provider
pub struct AnthropicProvider {
    api_base: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(api_base: String, api_key: String, model: String) -> Self {
        Self {
            api_base,
            api_key,
            model,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Anthropic
    }

    fn current_model(&self) -> &str {
        &self.model
    }

    fn set_model(&mut self, model: &str) {
        self.model = model.to_string();
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            supports_tools: true,
            supports_reasoning: true,
            supports_vision: true,
            supports_parallel_tools: true,
            supports_streaming: true,
            max_context_tokens: 200_000,
            max_output_tokens: Some(8192),
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelMetadata>> {
        Ok(vec![
            ModelMetadata {
                id: "claude-3-5-sonnet-20241022".to_string(),
                display_name: "Claude 3.5 Sonnet (Latest)".to_string(),
                provider: "Anthropic".to_string(),
                capabilities: self.capabilities(),
                context_window: 200_000,
                description:
                    "Frontier multimodal intelligence with high coding and reasoning precision"
                        .to_string(),
                pricing: None,
            },
            ModelMetadata {
                id: "claude-3-5-haiku-20241022".to_string(),
                display_name: "Claude 3.5 Haiku".to_string(),
                provider: "Anthropic".to_string(),
                capabilities: self.capabilities(),
                context_window: 200_000,
                description: "Fast, low-latency code and instruction model".to_string(),
                pricing: None,
            },
            ModelMetadata {
                id: "claude-3-opus-20240229".to_string(),
                display_name: "Claude 3 Opus".to_string(),
                provider: "Anthropic".to_string(),
                capabilities: self.capabilities(),
                context_window: 200_000,
                description: "Deep reasoning and analytical model".to_string(),
                pricing: None,
            },
        ])
    }

    async fn health_check(&self) -> Result<bool> {
        let base = self
            .api_base
            .trim_end_matches("/messages")
            .trim_end_matches('/');
        let endpoint = if base.ends_with("/v1") {
            format!("{}/models", base)
        } else {
            format!("{}/v1/models", base)
        };
        let resp = self
            .client
            .get(&endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            Ok(true)
        } else if status.as_u16() == 401 || status.as_u16() == 403 {
            anyhow::bail!("Anthropic authentication failed (HTTP {})", status);
        } else if status.as_u16() == 429 {
            anyhow::bail!("Anthropic rate limit reached (HTTP 429)");
        } else if status.is_server_error() {
            anyhow::bail!("Anthropic server error (HTTP {})", status);
        } else {
            anyhow::bail!("Anthropic health probe returned HTTP {}", status);
        }
    }

    async fn test_model_availability(&self, model: &str) -> Result<()> {
        let models = self.list_models().await?;
        if models
            .iter()
            .any(|m| m.id == model || m.id.eq_ignore_ascii_case(model))
        {
            Ok(())
        } else {
            anyhow::bail!("Model '{model}' is not available on Anthropic provider");
        }
    }

    async fn stream_chat(
        &self,
        request: ChatRequest,
        handler: StreamHandler,
    ) -> Result<ChatCompletionResponse> {
        let endpoint = if self.api_base.ends_with("/messages") {
            self.api_base.clone()
        } else if self.api_base.ends_with("/v1") {
            format!("{}/messages", self.api_base)
        } else {
            format!("{}/v1/messages", self.api_base.trim_end_matches('/'))
        };

        let mut system_text = String::new();
        let mut anthropic_messages = Vec::new();

        for msg in &request.messages {
            match msg.role {
                m31a_core::types::Role::System => {
                    if let Some(ref text) = msg.content {
                        if !system_text.is_empty() {
                            system_text.push('\n');
                        }
                        system_text.push_str(text);
                    }
                }
                m31a_core::types::Role::User => {
                    anthropic_messages.push(serde_json::json!({
                        "role": "user",
                        "content": msg.content.clone().unwrap_or_default(),
                    }));
                }
                m31a_core::types::Role::Assistant => {
                    if let Some(ref calls) = msg.tool_calls {
                        let mut blocks: Vec<serde_json::Value> = Vec::new();
                        if let Some(ref text) = msg.content
                            && !text.is_empty()
                        {
                            blocks.push(serde_json::json!({ "type": "text", "text": text }));
                        }
                        for call in calls {
                            let input_val: serde_json::Value =
                                serde_json::from_str(&call.function.arguments).unwrap_or_else(
                                    |_| serde_json::json!({ "input": call.function.arguments }),
                                );
                            blocks.push(serde_json::json!({
                                "type": "tool_use",
                                "id": call.id,
                                "name": call.function.name,
                                "input": input_val,
                            }));
                        }
                        anthropic_messages.push(serde_json::json!({
                            "role": "assistant",
                            "content": blocks,
                        }));
                    } else {
                        anthropic_messages.push(serde_json::json!({
                            "role": "assistant",
                            "content": msg.content.clone().unwrap_or_default(),
                        }));
                    }
                }
                m31a_core::types::Role::Tool => {
                    let tool_id = msg
                        .tool_call_id
                        .clone()
                        .unwrap_or_else(|| "call_0".to_string());
                    anthropic_messages.push(serde_json::json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": tool_id,
                            "content": msg.content.clone().unwrap_or_default(),
                        }]
                    }));
                }
            }
        }

        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": 8192,
            "messages": anthropic_messages,
            "stream": false,
        });

        if !system_text.is_empty() {
            body["system"] = serde_json::Value::String(system_text);
        }

        if let Some(ref tools) = request.tools {
            let anthropic_tools: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.function.name,
                        "description": t.function.description,
                        "input_schema": t.function.parameters,
                    })
                })
                .collect();
            body["tools"] = serde_json::Value::Array(anthropic_tools);
        }

        let resp = self
            .client
            .post(&endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let err_txt = resp.text().await.unwrap_or_default();
            anyhow::bail!("Anthropic API error: {}", err_txt);
        }

        let res_json: serde_json::Value = resp.json().await?;
        let mut text_acc = String::new();
        let mut tool_calls = Vec::new();

        if let Some(content_arr) = res_json.get("content").and_then(|v| v.as_array()) {
            for block in content_arr {
                if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                    if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                        text_acc.push_str(t);
                        handler(StreamEvent::ContentDelta(t.to_string()));
                    }
                } else if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                    let id = block
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = block
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let input = block
                        .get("input")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null);
                    tool_calls.push(m31a_core::types::ToolCall {
                        id,
                        call_type: "function".to_string(),
                        function: m31a_core::types::FunctionCall {
                            name,
                            arguments: input.to_string(),
                        },
                    });
                }
            }
        }

        let usage = TokenUsage {
            prompt_tokens: res_json
                .get("usage")
                .and_then(|u| u.get("input_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            completion_tokens: res_json
                .get("usage")
                .and_then(|u| u.get("output_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            total_tokens: 0,
            reasoning_tokens: 0,
        };

        handler(StreamEvent::Done);

        let final_msg = if tool_calls.is_empty() {
            ChatMessage::assistant(Some(text_acc), None)
        } else {
            ChatMessage {
                role: m31a_core::types::Role::Assistant,
                content: if text_acc.is_empty() {
                    None
                } else {
                    Some(text_acc)
                },
                tool_calls: Some(tool_calls),
                tool_call_id: None,
                reasoning_content: None,
            }
        };

        Ok(ChatCompletionResponse {
            message: final_msg,
            usage,
            model: self.model.clone(),
            finish_reason: Some("stop".to_string()),
        })
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatCompletionResponse> {
        let handler = Arc::new(|_| {});
        self.stream_chat(request, handler).await
    }
}

/// Mock Provider for Testing
#[cfg(any(test, feature = "test-support"))]
pub struct MockProvider {
    model: String,
    responses: Vec<String>,
    response_index: std::sync::atomic::AtomicUsize,
}

#[cfg(any(test, feature = "test-support"))]
impl MockProvider {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            responses: vec![
                "This is a mock response from the test provider.".to_string(),
                "Mock tool call response.".to_string(),
            ],
            response_index: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn with_responses(model: String, responses: Vec<String>) -> Self {
        Self {
            model,
            responses,
            response_index: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
#[async_trait]
impl ModelProvider for MockProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Mock
    }

    fn current_model(&self) -> &str {
        &self.model
    }

    fn set_model(&mut self, model: &str) {
        self.model = model.to_string();
    }

    fn capabilities(&self) -> ModelCapabilities {
        ModelCapabilities {
            supports_tools: true,
            supports_reasoning: true,
            supports_vision: true,
            supports_parallel_tools: true,
            supports_streaming: true,
            max_context_tokens: 1000000,
            max_output_tokens: Some(100000),
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelMetadata>> {
        Ok(vec![ModelMetadata {
            id: self.model.clone(),
            display_name: format!("Mock: {}", self.model),
            provider: "mock".to_string(),
            capabilities: self.capabilities(),
            context_window: 1000000,
            description: "Mock provider for testing".to_string(),
            pricing: None,
        }])
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }

    async fn test_model_availability(&self, model: &str) -> Result<()> {
        if model == "unavailable" || model.contains("unavailable") || model.contains("nonexistent")
        {
            anyhow::bail!("Model '{}' is not available on mock provider", model);
        }
        Ok(())
    }

    async fn stream_chat(
        &self,
        request: ChatRequest,
        handler: StreamHandler,
    ) -> Result<ChatCompletionResponse> {
        let idx = self
            .response_index
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            % self.responses.len();
        let response = &self.responses[idx];

        // Simulate streaming by sending chunks
        for chunk in response.chars() {
            handler(StreamEvent::ContentDelta(chunk.to_string()));
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        handler(StreamEvent::Done);

        Ok(ChatCompletionResponse {
            message: ChatMessage::assistant(Some(response.clone()), None),
            usage: TokenUsage {
                prompt_tokens: request
                    .messages
                    .iter()
                    .map(|m| m.content.as_deref().map_or(0, |s| s.len() as u64 / 4))
                    .sum(),
                completion_tokens: response.len() as u64 / 4,
                reasoning_tokens: 0,
                total_tokens: 0,
            },
            model: request.model,
            finish_reason: Some("stop".to_string()),
        })
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatCompletionResponse> {
        self.stream_chat(request, Arc::new(|_| {})).await
    }
}

#[async_trait]
impl EmbeddingsProvider for NimProvider {
    async fn create_embeddings(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        self.inner.create_embeddings(inputs).await
    }
}

#[async_trait]
impl EmbeddingsProvider for GenericOpenAiProvider {
    async fn create_embeddings(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        self.inner.create_embeddings(inputs).await
    }
}

#[cfg(any(test, feature = "test-support"))]
#[async_trait]
impl EmbeddingsProvider for MockProvider {
    async fn create_embeddings(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        // Return synthetic unit-length embedding vector per input
        Ok(inputs.iter().map(|_| vec![0.1f32; 384]).collect())
    }
}

/// Universal provider-independent client for driving agent orchestration.
/// Encapsulates any model provider implementing `ModelProvider` (NIM, OpenAI, Anthropic, Mock, Ollama, etc.)
/// and exposes a stable interface for turn loops, agents, and tool loops.
#[derive(Clone)]
pub struct ProviderClient {
    provider: Arc<dyn ModelProvider>,
    model: String,
    reasoning_effort: Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    tool_choice: Option<String>,
}

impl std::fmt::Debug for ProviderClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderClient")
            .field("model", &self.model)
            .field("reasoning_effort", &self.reasoning_effort)
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
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            tool_choice: self.tool_choice.clone(),
        }
    }

    pub fn with_reasoning_effort(mut self, effort: Option<String>) -> Self {
        self.reasoning_effort = effort;
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

    pub fn max_tokens(&self) -> Option<u32> {
        self.max_tokens
    }

    pub fn temperature(&self) -> Option<f32> {
        self.temperature
    }

    pub fn tool_choice(&self) -> Option<&str> {
        self.tool_choice.as_deref()
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
