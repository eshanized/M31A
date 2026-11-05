//! Model definitions, context window specifications, and tool capability configurations.
//!
//! Details token limits, reasoning capabilities, and supported tool formats across model families,
//! embedding the canonical 85-model registry from NVIDIA NIM and the M31A Model Registry.

use serde::{Deserialize, Serialize};
use std::sync::{OnceLock, RwLock};

static DYNAMIC_CATALOG: RwLock<Vec<ModelInfo>> = RwLock::new(Vec::new());

/// Registers custom or dynamic models into the model catalog.
pub fn register_dynamic_models(models: Vec<ModelInfo>) {
    if let Ok(mut lock) = DYNAMIC_CATALOG.write() {
        for m in models {
            if !lock.iter().any(|existing| existing.slug == m.slug) {
                lock.push(m);
            }
        }
    }
}

/// Retrieves the complete model catalog, merging static catalog with any dynamically registered models.
pub fn get_model_catalog() -> Vec<ModelInfo> {
    let mut models = get_nvidia_model_catalog();
    if let Ok(lock) = DYNAMIC_CATALOG.read() {
        for m in lock.iter() {
            if !models.iter().any(|existing| existing.slug == m.slug) {
                models.push(m.clone());
            }
        }
    }
    models
}

/// Tool calling format expected by the model (standard function call vs custom formatting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolMode {
    /// Model supports standard function calling.
    Direct,
    /// Model supports code execution via tool calls (Code Mode).
    CodeMode,
    /// Model ONLY supports code execution (no standard tools).
    CodeModeOnly,
    /// Model does not support tool calling.
    Unsupported,
}

/// Input modality support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputModality {
    Text,
    Image,
    Audio,
    Video,
    Embedding,
}

/// Truncation policy for model outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TruncationPolicy {
    /// Truncate output at token limit.
    Truncate,
    /// Return error when output exceeds limit.
    Error,
    /// Allow overflow (dangerous for context window).
    AllowOverflow,
}

/// Service tier / quality classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceTier {
    /// Standard quality, balanced latency.
    Standard,
    /// High quality, higher latency, reasoning models.
    Premium,
    /// Optimized for speed, lower quality / fast tier.
    Turbo,
    /// Free/preview tier with rate limits.
    Preview,
}

/// Canonical catalog structure matching `models.m31a.tonmoyinfrastructure.org/src/data/catalog.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalCatalog {
    pub schema_version: String,
    pub generated_at: String,
    pub last_synced_at: String,
    pub total_models: usize,
    pub publishers: Vec<String>,
    pub modalities: Vec<String>,
    pub models: Vec<CanonicalModel>,
}

/// Detailed canonical model specification from the M31A registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalModel {
    pub id: String,
    pub name: String,
    pub publisher: String,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub architecture: Option<String>,
    #[serde(default)]
    pub official_detail_url: String,
    #[serde(default)]
    pub documentation_urls: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub catalog_status: String,
    #[serde(default)]
    pub first_seen_at: Option<String>,
    #[serde(default)]
    pub last_seen_at: Option<String>,
    #[serde(default)]
    pub endpoint_type: String,
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub access_classification: String,
    #[serde(default)]
    pub input_modalities: Vec<String>,
    #[serde(default)]
    pub output_modalities: Vec<String>,
    pub capabilities: CanonicalCapabilities,
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub pricing: CanonicalPricing,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub usage_restrictions: Option<String>,
    #[serde(default)]
    pub provenance: Option<CanonicalProvenance>,
    #[serde(default)]
    pub eligibility: Option<CanonicalEligibility>,
    #[serde(default)]
    pub verified_at: Option<String>,
    #[serde(default)]
    pub deprecation_notice: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanonicalCapabilities {
    #[serde(default)]
    pub chat: bool,
    #[serde(default)]
    pub completion: bool,
    #[serde(default)]
    pub vision: bool,
    #[serde(default)]
    pub audio: bool,
    #[serde(default)]
    pub video: bool,
    #[serde(default)]
    pub embedding: bool,
    #[serde(default)]
    pub reranking: bool,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub structured_output: bool,
    #[serde(default)]
    pub tool_calling: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanonicalPricing {
    #[serde(default)]
    pub currency: String,
    pub input_per_million: Option<f64>,
    pub output_per_million: Option<f64>,
    #[serde(default)]
    pub status: String,
    pub source: Option<String>,
    pub provenance_note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanonicalProvenance {
    #[serde(default)]
    pub source_url: String,
    #[serde(default)]
    pub source_type: String,
    #[serde(default)]
    pub observed_at: String,
    #[serde(default)]
    pub confidence: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanonicalEligibility {
    #[serde(default)]
    pub probe_eligible: bool,
    pub probe_endpoint: Option<String>,
    #[serde(default)]
    pub test_messages: Option<Vec<CanonicalTestMessage>>,
    pub max_probe_tokens: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanonicalTestMessage {
    pub role: String,
    pub content: String,
}

/// Model capability specification (report §7.3, upstream `models-manager/src/models.rs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Unique model identifier (e.g., "meta/llama-3.3-70b-instruct").
    pub slug: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Description for model picker UI.
    pub description: String,
    /// Maximum context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens per completion.
    pub max_output_tokens: u32,
    /// Whether model supports standard function/tool calling.
    pub supports_tools: bool,
    /// Whether model supports reasoning/thinking tokens (e.g., DeepSeek R1).
    pub supports_thinking: bool,
    /// Whether model supports vision/image input.
    pub supports_vision: bool,
    /// Whether model supports audio input.
    pub supports_audio: bool,
    /// Default sampling temperature.
    pub default_temperature: f32,
    /// Tool mode capability (Direct, CodeMode, CodeModeOnly, Unsupported).
    pub tool_mode: ToolMode,
    /// Supported input modalities.
    pub input_modalities: Vec<InputModality>,
    /// Truncation policy for outputs exceeding max_output_tokens.
    pub truncation_policy: TruncationPolicy,
    /// Service tier classification.
    pub service_tier: ServiceTier,
    /// Multi-agent protocol version supported (0 = none).
    pub multi_agent_version: u8,
    /// Auto-compact derivation: at what % of context_window to trigger compaction.
    pub auto_compact_threshold_percent: u8,
    /// Base instructions template path (relative to model catalog dir or bundled).
    /// If present, rendered as the system prompt base for this model (§7.3).
    pub instructions_template: Option<String>,
    /// ETag for cache validation (computed from model spec hash).
    #[serde(skip)]
    pub etag: Option<String>,
    /// Whether this model requires authentication (for filter_by_auth).
    pub requires_auth: bool,
    /// Whether this model is marked as default in the picker UI.
    pub is_default: bool,
    /// Originator/vendor for per-originator partitioning.
    pub originator: String,
}

impl Default for ModelInfo {
    fn default() -> Self {
        Self {
            slug: String::new(),
            display_name: String::new(),
            description: String::new(),
            context_window: 131072,
            max_output_tokens: 4096,
            supports_tools: true,
            supports_thinking: false,
            supports_vision: false,
            supports_audio: false,
            default_temperature: 0.2,
            tool_mode: ToolMode::Direct,
            input_modalities: vec![InputModality::Text],
            truncation_policy: TruncationPolicy::Truncate,
            service_tier: ServiceTier::Standard,
            multi_agent_version: 0,
            auto_compact_threshold_percent: 75,
            instructions_template: None,
            etag: None,
            requires_auth: true,
            is_default: false,
            originator: "nvidia".to_string(),
        }
    }
}

impl ModelInfo {
    /// Computes and sets the ETag from the model spec (stable hash).
    pub fn compute_etag(&mut self) {
        use sha2::{Digest, Sha256};
        let canonical = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        self.etag = Some(format!("{:x}", hasher.finalize()));
    }

    /// Returns the base instructions template path if available, else None.
    pub fn get_instructions_template(&self) -> Option<&str> {
        self.instructions_template.as_deref()
    }
}

impl From<&CanonicalModel> for ModelInfo {
    fn from(c: &CanonicalModel) -> Self {
        let is_reasoning = c.capabilities.reasoning
            || c.id.contains("reasoning")
            || c.id.contains("r1")
            || c.id.contains("deepseek-v4")
            || c.id.contains("thinking");

        let is_vision = c.capabilities.vision
            || c.input_modalities.iter().any(|m| m == "image")
            || c.endpoint_type == "visual_language";

        let is_audio = c.capabilities.audio || c.input_modalities.iter().any(|m| m == "audio");

        let is_coder = c.id.contains("coder")
            || c.id.contains("codestral")
            || c.id.contains("codegemma")
            || c.id.contains("codellama")
            || c.id.contains("starcoder");

        let service_tier = if is_reasoning {
            ServiceTier::Premium
        } else if c.id.contains("bolt")
            || c.id.contains("flash")
            || c.context_window.unwrap_or(65536) <= 8192
        {
            ServiceTier::Turbo
        } else {
            ServiceTier::Standard
        };

        let tool_mode = if !c.capabilities.tool_calling && c.endpoint_type != "chat" {
            ToolMode::Unsupported
        } else if is_coder {
            ToolMode::CodeMode
        } else {
            ToolMode::Direct
        };

        let mut modalities = Vec::new();
        for m in &c.input_modalities {
            match m.as_str() {
                "text" => modalities.push(InputModality::Text),
                "image" => modalities.push(InputModality::Image),
                "audio" => modalities.push(InputModality::Audio),
                "video" => modalities.push(InputModality::Video),
                "embedding" => modalities.push(InputModality::Embedding),
                _ => {}
            }
        }
        if modalities.is_empty() {
            modalities.push(InputModality::Text);
        }

        let originator = match c.publisher.as_str() {
            "deepseek-ai" => "deepseek".to_string(),
            other => other.to_string(),
        };

        let is_default = c.id == "nvidia/nemotron-3.5-lightning-30b-a3b";

        let mut info = Self {
            slug: c.id.clone(),
            display_name: c.name.clone(),
            description: if c.description.is_empty() {
                format!("NVIDIA NIM Foundation Model ({})", c.publisher)
            } else {
                c.description.clone()
            },
            context_window: c.context_window.unwrap_or(65536),
            max_output_tokens: c.max_output_tokens.unwrap_or(4096),
            supports_tools: c.capabilities.tool_calling,
            supports_thinking: is_reasoning,
            supports_vision: is_vision,
            supports_audio: is_audio,
            default_temperature: if is_reasoning {
                0.6
            } else if is_coder {
                0.1
            } else {
                0.2
            },
            tool_mode,
            input_modalities: modalities,
            truncation_policy: TruncationPolicy::Truncate,
            service_tier,
            multi_agent_version: 1,
            auto_compact_threshold_percent: 75,
            instructions_template: Some("prompt.md".to_string()),
            etag: None,
            requires_auth: true,
            is_default,
            originator,
        };

        info.compute_etag();
        info
    }
}

static CANONICAL_CATALOG: OnceLock<CanonicalCatalog> = OnceLock::new();

/// Returns a reference to the embedded canonical catalog of 85 models.
pub fn get_canonical_catalog() -> &'static CanonicalCatalog {
    CANONICAL_CATALOG.get_or_init(|| {
        let raw = include_str!("../data/catalog.json");
        serde_json::from_str::<CanonicalCatalog>(raw)
            .expect("Embedded catalog.json must be valid CanonicalCatalog JSON")
    })
}

/// Retrieves the complete, canonical list of models available in M31A.
/// Contains all 85 frontier models across 23 publishers discovered on NVIDIA NIM.
pub fn get_nvidia_model_catalog() -> Vec<ModelInfo> {
    let canonical = get_canonical_catalog();
    let mut models: Vec<ModelInfo> = canonical.models.iter().map(ModelInfo::from).collect();

    // Ensure common community local/remote developer models are also represented
    if !models
        .iter()
        .any(|m| m.slug == "qwen/qwen2.5-coder-32b-instruct")
    {
        let mut qwen = ModelInfo {
            slug: "qwen/qwen2.5-coder-32b-instruct".to_string(),
            display_name: "Qwen 2.5 Coder 32B".to_string(),
            description:
                "Code-specialized model delivering exceptional syntax and multi-file reasoning."
                    .to_string(),
            context_window: 32768,
            max_output_tokens: 8192,
            supports_tools: true,
            supports_thinking: false,
            supports_vision: false,
            supports_audio: false,
            default_temperature: 0.1,
            tool_mode: ToolMode::CodeMode,
            input_modalities: vec![InputModality::Text],
            truncation_policy: TruncationPolicy::Truncate,
            service_tier: ServiceTier::Standard,
            multi_agent_version: 1,
            auto_compact_threshold_percent: 75,
            instructions_template: Some("prompt.md".to_string()),
            etag: None,
            requires_auth: true,
            is_default: false,
            originator: "qwen".to_string(),
        };
        qwen.compute_etag();
        models.push(qwen);
    }

    if !models.iter().any(|m| m.slug == "deepseek-ai/deepseek-r1") {
        let mut r1 = ModelInfo {
            slug: "deepseek-ai/deepseek-r1".to_string(),
            display_name: "DeepSeek R1 Reasoning Model".to_string(),
            description: "Frontier open reasoning model with transparent chain-of-thought traces."
                .to_string(),
            context_window: 131072,
            max_output_tokens: 8192,
            supports_tools: true,
            supports_thinking: true,
            supports_vision: false,
            supports_audio: false,
            default_temperature: 0.6,
            tool_mode: ToolMode::Direct,
            input_modalities: vec![InputModality::Text],
            truncation_policy: TruncationPolicy::Truncate,
            service_tier: ServiceTier::Premium,
            multi_agent_version: 1,
            auto_compact_threshold_percent: 75,
            instructions_template: Some("prompt.md".to_string()),
            etag: None,
            requires_auth: true,
            is_default: false,
            originator: "deepseek-ai".to_string(),
        };
        r1.compute_etag();
        models.push(r1);
    }

    if !models
        .iter()
        .any(|m| m.slug == "meta/llama-3.3-70b-instruct")
    {
        let mut llama = ModelInfo {
            slug: "meta/llama-3.3-70b-instruct".to_string(),
            display_name: "Llama 3.3 70B Instruct".to_string(),
            description: "High-performance instruction-tuned 70B parameter frontier model by Meta."
                .to_string(),
            context_window: 131072,
            max_output_tokens: 4096,
            supports_tools: true,
            supports_thinking: false,
            supports_vision: false,
            supports_audio: false,
            default_temperature: 0.2,
            tool_mode: ToolMode::Direct,
            input_modalities: vec![InputModality::Text],
            truncation_policy: TruncationPolicy::Truncate,
            service_tier: ServiceTier::Standard,
            multi_agent_version: 1,
            auto_compact_threshold_percent: 75,
            instructions_template: Some("prompt.md".to_string()),
            etag: None,
            requires_auth: true,
            is_default: false,
            originator: "meta".to_string(),
        };
        llama.compute_etag();
        models.insert(0, llama);
    }

    models
}

/// Finds a model by its slug identifier (case-insensitive and prefix-tolerant).
pub fn find_model(slug: &str) -> Option<ModelInfo> {
    let lower = slug.to_ascii_lowercase();
    get_model_catalog().into_iter().find(|m| {
        m.slug.eq_ignore_ascii_case(slug)
            || m.slug
                .split('/')
                .next_back()
                .map(|s| s.eq_ignore_ascii_case(&lower))
                .unwrap_or(false)
    })
}

/// Finds the canonical model entry by slug.
pub fn find_canonical_model(slug: &str) -> Option<&'static CanonicalModel> {
    let lower = slug.to_ascii_lowercase();
    get_canonical_catalog().models.iter().find(|m| {
        m.id.eq_ignore_ascii_case(slug)
            || m.id
                .split('/')
                .next_back()
                .map(|s| s.eq_ignore_ascii_case(&lower))
                .unwrap_or(false)
    })
}

/// Returns all unique publishers in the catalog.
pub fn list_publishers() -> Vec<String> {
    get_canonical_catalog().publishers.clone()
}

/// Lists all models originating from a given publisher.
pub fn list_models_by_publisher(publisher: &str) -> Vec<ModelInfo> {
    let lower = publisher.to_ascii_lowercase();
    get_nvidia_model_catalog()
        .into_iter()
        .filter(|m| {
            m.originator.eq_ignore_ascii_case(&lower)
                || (lower == "deepseek" && m.originator == "deepseek-ai")
        })
        .collect()
}

/// Lists all reasoning / deep thinking models.
pub fn list_reasoning_models() -> Vec<ModelInfo> {
    get_nvidia_model_catalog()
        .into_iter()
        .filter(|m| m.supports_thinking)
        .collect()
}

/// Lists all multimodal vision-capable models.
pub fn list_vision_models() -> Vec<ModelInfo> {
    get_nvidia_model_catalog()
        .into_iter()
        .filter(|m| m.supports_vision)
        .collect()
}

/// Lists all code-specialized models.
pub fn list_coding_models() -> Vec<ModelInfo> {
    get_nvidia_model_catalog()
        .into_iter()
        .filter(|m| {
            m.tool_mode == ToolMode::CodeMode
                || m.slug.contains("code")
                || m.slug.contains("coder")
                || m.slug.contains("codestral")
                || m.slug.contains("starcoder")
        })
        .collect()
}

/// Lists all embedding models from the canonical catalog.
pub fn list_embedding_models() -> Vec<&'static CanonicalModel> {
    get_canonical_catalog()
        .models
        .iter()
        .filter(|m| m.endpoint_type == "embedding" || m.capabilities.embedding)
        .collect()
}

/// Lists all guardrail / safety models from the canonical catalog.
pub fn list_guardrail_models() -> Vec<&'static CanonicalModel> {
    get_canonical_catalog()
        .models
        .iter()
        .filter(|m| {
            m.endpoint_type == "guardrail" || m.id.contains("guard") || m.id.contains("safety")
        })
        .collect()
}

/// Filters models by free-text search across slug, display name, and description.
pub fn search_models(query: &str) -> Vec<ModelInfo> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return get_nvidia_model_catalog();
    }
    get_nvidia_model_catalog()
        .into_iter()
        .filter(|m| {
            m.slug.to_ascii_lowercase().contains(&q)
                || m.display_name.to_ascii_lowercase().contains(&q)
                || m.description.to_ascii_lowercase().contains(&q)
                || m.originator.to_ascii_lowercase().contains(&q)
        })
        .collect()
}

/// Dynamically queries the provider's `/v1/models` endpoint and discovers all live models.
pub async fn fetch_remote_models(api_base: &str, api_key: &str) -> anyhow::Result<Vec<ModelInfo>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()?;
    let url = format!("{}/models", api_base.trim_end_matches('/'));
    let mut req = client.get(&url);
    if !api_key.trim().is_empty() {
        req = req.header("Authorization", format!("Bearer {api_key}"));
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("Remote model endpoint returned status: {}", resp.status());
    }
    let data = resp.json::<serde_json::Value>().await?;
    let mut discovered = Vec::new();
    if let Some(arr) = data.get("data").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                let remote_cw = item
                    .get("max_model_len")
                    .or_else(|| item.get("context_length"))
                    .or_else(|| item.get("context_window"))
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32);

                // If it already exists in our rich catalog, take the rich version!
                if let Some(mut existing) = find_model(id) {
                    if let Some(cw) = remote_cw {
                        existing.context_window = cw;
                    }
                    existing.compute_etag();
                    discovered.push(existing);
                    continue;
                }

                let originator = id.split('/').next().unwrap_or("remote").to_string();
                let lower = id.to_lowercase();
                let is_thinking = lower.contains("deepseek")
                    || lower.contains("reasoning")
                    || lower.contains("think")
                    || lower.contains("r1");
                let is_vision =
                    lower.contains("vision") || lower.contains("vl") || lower.contains("omni");
                let is_coder = lower.contains("coder") || lower.contains("code");

                let display_name = id
                    .split('/')
                    .next_back()
                    .unwrap_or(id)
                    .replace(['-', '_'], " ");

                let context_window = if let Some(cw) = remote_cw {
                    cw
                } else if lower.contains("1048576") || lower.contains("1m") {
                    1_048_576
                } else if lower.contains("128k")
                    || lower.contains("70b")
                    || lower.contains("nemotron")
                {
                    131_072
                } else if lower.contains("32k") {
                    32_768
                } else {
                    65_536
                };

                let mut model = ModelInfo {
                    slug: id.to_string(),
                    display_name,
                    description: format!("Remote discovered model on NIM ({})", originator),
                    context_window,
                    max_output_tokens: 4096,
                    supports_tools: true,
                    supports_thinking: is_thinking,
                    supports_vision: is_vision,
                    supports_audio: false,
                    default_temperature: if is_thinking {
                        0.6
                    } else if is_coder {
                        0.1
                    } else {
                        0.2
                    },
                    tool_mode: if is_coder {
                        ToolMode::CodeMode
                    } else {
                        ToolMode::Direct
                    },
                    input_modalities: if is_vision {
                        vec![InputModality::Text, InputModality::Image]
                    } else {
                        vec![InputModality::Text]
                    },
                    truncation_policy: TruncationPolicy::Truncate,
                    service_tier: ServiceTier::Standard,
                    multi_agent_version: 1,
                    auto_compact_threshold_percent: 75,
                    instructions_template: None,
                    etag: None,
                    requires_auth: true,
                    is_default: false,
                    originator,
                };
                model.compute_etag();
                discovered.push(model);
            }
        }
    }
    Ok(discovered)
}

/// Default TTL for cached models: 24 hours (86,400 seconds).
pub const DEFAULT_MODELS_CACHE_TTL_SECS: u64 = 86_400;

/// Stored format for local cached model catalog with timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedCatalogEnvelope {
    pub updated_at_unix: u64,
    pub models: Vec<ModelInfo>,
}

/// Refresh strategy for model catalog (upstream `models-manager`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshStrategy {
    /// Always fetch from remote, fail if unavailable.
    Online,
    /// Always use cached/bundled, never fetch.
    Offline,
    /// Try online first, fall back to cache on failure.
    OnlineIfUncached,
}

pub fn default_models_cache_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join(".config")
        .join("m31a")
        .join("models_cache.json")
}

/// Reads cached models from local disk cache, enabling zero-latency offline start.
/// Falls back to the bundled 85-model catalog if the cache is missing or corrupt.
pub fn load_cached_models() -> Vec<ModelInfo> {
    let path = default_models_cache_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        // Try parsing envelope with timestamp
        if let Ok(envelope) = serde_json::from_str::<CachedCatalogEnvelope>(&content)
            && !envelope.models.is_empty()
        {
            return envelope.models;
        }
        // Fall back to legacy flat Vec<ModelInfo>
        if let Ok(models) = serde_json::from_str::<Vec<ModelInfo>>(&content)
            && !models.is_empty()
        {
            return models;
        }
    }
    // Fall back to built-in catalog
    get_nvidia_model_catalog()
}

/// Persists discovered models to local disk cache for instant reuse on subsequent runs.
pub fn save_cached_models(models: &[ModelInfo]) {
    let path = default_models_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let envelope = CachedCatalogEnvelope {
        updated_at_unix: now,
        models: models.to_vec(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&envelope) {
        let _ = std::fs::write(path, json);
    }
}

/// Checks if the cached models on disk are missing or older than `ttl_secs`.
pub fn is_models_cache_stale(ttl_secs: u64) -> bool {
    let path = default_models_cache_path();
    if let Some(envelope) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str::<CachedCatalogEnvelope>(&content).ok())
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        return now.saturating_sub(envelope.updated_at_unix) >= ttl_secs;
    }
    let metadata = match std::fs::metadata(&path) {
        Ok(m) => m,
        Err(_) => return true,
    };
    let modified = match metadata.modified() {
        Ok(t) => t,
        Err(_) => return true,
    };
    match std::time::SystemTime::now().duration_since(modified) {
        Ok(duration) => duration.as_secs() >= ttl_secs,
        Err(_) => true,
    }
}

/// Refreshes the model catalog if the local disk cache is missing or older than `ttl_secs` (e.g. 24h).
/// If refreshed, updates disk cache and registers models dynamically.
pub async fn refresh_models_if_stale(api_base: &str, api_key: &str, ttl_secs: u64) -> bool {
    if !is_models_cache_stale(ttl_secs) {
        return false;
    }
    if api_key.trim().is_empty() {
        return false;
    }
    match fetch_remote_models(api_base, api_key).await {
        Ok(models) if !models.is_empty() => {
            save_cached_models(&models);
            register_dynamic_models(models);
            true
        }
        _ => false,
    }
}

/// Asynchronously spawns a non-blocking background task to refresh models daily if stale.
pub fn spawn_daily_models_refresh(api_base: String, api_key: String) {
    if api_key.trim().is_empty() {
        return;
    }
    tokio::spawn(async move {
        let _ = refresh_models_if_stale(&api_base, &api_key, DEFAULT_MODELS_CACHE_TTL_SECS).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_catalog_loads_properly() {
        let catalog = get_canonical_catalog();
        assert_eq!(catalog.total_models, 85);
        assert!(catalog.publishers.len() >= 20);
        assert!(catalog.models.iter().any(|m| m.id == "meta/codellama-70b"));
        assert!(
            catalog
                .models
                .iter()
                .any(|m| m.id == "nvidia/nemotron-3.5-lightning-30b-a3b")
        );
        assert!(
            catalog
                .models
                .iter()
                .any(|m| m.id == "google/gemma-3-12b-it")
        );
    }

    #[test]
    fn test_get_nvidia_model_catalog_has_all_models() {
        let models = get_nvidia_model_catalog();
        assert!(models.len() >= 85);
        let default_model = models.iter().find(|m| m.is_default);
        assert!(default_model.is_some());
        assert_eq!(
            default_model.unwrap().slug,
            "nvidia/nemotron-3.5-lightning-30b-a3b"
        );
    }

    #[test]
    fn test_find_model_exact_and_partial() {
        assert!(find_model("nvidia/nemotron-3.5-lightning-30b-a3b").is_some());
        assert!(find_model("nemotron-3.5-lightning-30b-a3b").is_some());
        assert!(find_model("codellama-70b").is_some());
        assert!(find_model("deepseek-v4.1-flash").is_some());
    }

    #[test]
    fn test_filter_by_capabilities() {
        let reasoning = list_reasoning_models();
        assert!(!reasoning.is_empty());
        assert!(reasoning.iter().any(|m| m.slug.contains("deepseek")));

        let vision = list_vision_models();
        assert!(!vision.is_empty());
        assert!(
            vision
                .iter()
                .any(|m| m.slug == "google/gemma-3-12b-it" || m.slug.contains("fuyu"))
        );

        let coding = list_coding_models();
        assert!(!coding.is_empty());
        assert!(
            coding
                .iter()
                .any(|m| m.slug.contains("codestral") || m.slug.contains("codellama"))
        );

        let embeddings = list_embedding_models();
        assert!(!embeddings.is_empty());

        let guardrails = list_guardrail_models();
        assert!(!guardrails.is_empty());
    }

    #[test]
    fn test_nvidia_nemotron_3_ultra_exact_catalog_metadata() {
        let canonical = find_canonical_model("nvidia/nemotron-3-ultra-550b-a55b")
            .expect("Exact model nvidia/nemotron-3-ultra-550b-a55b must exist in catalog.json");
        assert_eq!(canonical.id, "nvidia/nemotron-3-ultra-550b-a55b");
        assert_eq!(canonical.publisher, "nvidia");
        assert!(canonical.capabilities.chat);
        assert!(
            canonical.capabilities.reasoning,
            "Reasoning capability must be true for Ultra"
        );
        assert!(
            canonical.capabilities.tool_calling,
            "Tool calling capability must be true for Ultra"
        );
        assert_eq!(canonical.context_window, Some(1_048_576));
        assert_eq!(canonical.max_output_tokens, Some(16_384));
        assert_eq!(canonical.input_modalities, vec!["text"]);
        assert_eq!(canonical.output_modalities, vec!["text"]);

        let model_info = find_model("nvidia/nemotron-3-ultra-550b-a55b")
            .expect("ModelInfo for nvidia/nemotron-3-ultra-550b-a55b must be resolvable");
        assert!(
            model_info.supports_thinking,
            "supports_thinking must be true for Ultra"
        );
        assert!(
            model_info.supports_tools,
            "supports_tools must be true for Ultra"
        );
        assert_eq!(model_info.context_window, 1_048_576);
        assert_eq!(model_info.max_output_tokens, 16_384);
        assert_eq!(model_info.service_tier, ServiceTier::Premium);
        assert_eq!(model_info.input_modalities, vec![InputModality::Text]);
    }

    #[test]
    fn test_models_cache_envelope_and_ttl() {
        let dir = tempfile::tempdir().unwrap();
        let cache_file = dir.path().join("test_models_cache.json");

        let models = vec![
            ModelInfo {
                slug: "test-model-32k".to_string(),
                display_name: "Test 32k".to_string(),
                context_window: 32_768,
                ..Default::default()
            },
            ModelInfo {
                slug: "test-model-128k".to_string(),
                display_name: "Test 128k".to_string(),
                context_window: 131_072,
                ..Default::default()
            },
        ];

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let envelope = CachedCatalogEnvelope {
            updated_at_unix: now,
            models: models.clone(),
        };

        let json = serde_json::to_string_pretty(&envelope).unwrap();
        std::fs::write(&cache_file, &json).unwrap();

        let loaded: CachedCatalogEnvelope =
            serde_json::from_str(&std::fs::read_to_string(&cache_file).unwrap()).unwrap();
        assert_eq!(loaded.models.len(), 2);
        assert_eq!(loaded.models[0].context_window, 32_768);
        assert_eq!(loaded.models[1].context_window, 131_072);
        assert_eq!(loaded.updated_at_unix, now);
    }

    #[test]
    fn test_variable_model_context_windows() {
        let qwen = find_model("qwen/qwen2.5-coder-32b-instruct")
            .expect("Qwen coder must exist in catalog");
        assert_eq!(qwen.context_window, 32_768);

        let ultra =
            find_model("nvidia/nemotron-3-ultra-550b-a55b").expect("Nemotron ultra must exist");
        assert_eq!(ultra.context_window, 1_048_576);

        let llama =
            find_model("meta/llama-3.3-70b-instruct").expect("Llama 3.3 must exist in catalog");
        assert_eq!(llama.context_window, 131_072);
    }
}
