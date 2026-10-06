//! NVIDIA Model Metadata Enrichment Pipeline (Section 1–15).
//!
//! Provides authoritative per-model capability normalization, official NVIDIA
//! model reference metadata enrichment, and capability provenance tracking.
//!
//! Architecture:
//! ```text
//!    /v1/models (Inventory)
//!         ↓
//!    canonical model inventory
//!         ↓
//!    metadata enrichment (NvidiaModelMetadataResolver)
//!         ↓
//!    capability normalization
//!         ↓
//!    ModelCandidate (with explicit ContextLimits & CapabilitySupport)
//!         ↓
//!    ModelCatalog
//!         ↓
//!    Router / Wizard / TUI
//! ```

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Semaphore};

use crate::model::router::resolver::{
    CapabilitySupport, ContextLimits, ModelCandidate, ModelKind, ModelTier,
};
use crate::model::types::ModelError;

/// Trait for provider-specific model metadata enrichment (Section 6).
#[async_trait]
pub trait ProviderModelMetadataSource: Send + Sync {
    /// Enrich a single candidate with authoritative metadata.
    async fn enrich_candidate(&self, candidate: &mut ModelCandidate) -> Result<(), ModelError>;

    /// Enrich multiple candidates with bounded concurrency.
    async fn enrich_candidates(&self, candidates: &mut [ModelCandidate]) -> Result<(), ModelError>;
}

/// Official NVIDIA featured model entry structure from assets.ngc.nvidia.com.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeaturedModelEntry {
    pub model: String,
    #[serde(rename = "model-name", default)]
    pub model_name: Option<String>,
    #[serde(default)]
    pub context: usize,
    #[serde(rename = "max-output", default)]
    pub max_output: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FeaturedModelsResponse {
    #[serde(rename = "featured-models", default)]
    featured_models: Vec<FeaturedModelEntry>,
}

/// Authoritative official reference specification for an NVIDIA model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NvidiaModelReferenceRecord {
    pub model_id: String,
    pub display_name: Option<String>,
    pub context_length: usize,
    pub max_output_tokens: Option<usize>,
    pub model_kind: ModelKind,
    pub modalities: Vec<String>,
    pub tool_calling: CapabilitySupport,
    pub structured_output: CapabilitySupport,
    pub tier: ModelTier,
}

impl NvidiaModelReferenceRecord {
    pub fn new(
        model_id: impl Into<String>,
        context_length: usize,
        model_kind: ModelKind,
        tool_calling: CapabilitySupport,
        tier: ModelTier,
    ) -> Self {
        Self {
            model_id: model_id.into(),
            display_name: None,
            context_length,
            max_output_tokens: None,
            model_kind,
            modalities: vec!["text".to_string()],
            tool_calling,
            structured_output: CapabilitySupport::Supported,
            tier,
        }
    }

    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    pub fn with_max_output(mut self, max_tokens: usize) -> Self {
        self.max_output_tokens = Some(max_tokens);
        self
    }

    pub fn with_modalities(mut self, modalities: Vec<String>) -> Self {
        self.modalities = modalities;
        self
    }
}

/// Cache container for remote featured models with discovery timestamp.
pub type CachedFeaturedModels = Arc<RwLock<Option<(u64, HashMap<String, FeaturedModelEntry>)>>>;

/// Production metadata resolver for NVIDIA-hosted models.
pub struct NvidiaModelMetadataResolver {
    client: Client,
    featured_models_url: String,
    reference_registry: HashMap<String, NvidiaModelReferenceRecord>,
    cached_remote_featured: CachedFeaturedModels,
    concurrency_limit: usize,
    remote_enrichment_enabled: bool,
}

impl NvidiaModelMetadataResolver {
    /// Default public endpoint for NVIDIA featured models.
    pub const DEFAULT_FEATURED_MODELS_URL: &'static str =
        "https://assets.ngc.nvidia.com/products/api-catalog/featured-models.json";

    /// TTL for cached remote featured models (e.g. 24 hours as documented by NVIDIA).
    pub const REMOTE_CACHE_TTL_SECS: u64 =
        crate::config::canonical::DEFAULT_REMOTE_METADATA_TTL_SECS;

    /// Construct a new resolver with the canonical reference registry.
    pub fn new() -> Self {
        // Validating transport: the public feed fetch resolves through the
        // egress policy like every other HTTP client (defense in depth; no
        // credential is ever attached here).
        let client = crate::model::provider::endpoint::policy_validating_client_builder()
            .timeout(Duration::from_secs(
                crate::config::canonical::DEFAULT_METADATA_HTTP_TIMEOUT_SECS,
            ))
            .connect_timeout(Duration::from_secs(
                crate::config::canonical::DEFAULT_METADATA_CONNECT_TIMEOUT_SECS,
            ))
            .build()
            .unwrap_or_else(|_| Client::new());

        let mut resolver = Self {
            client,
            featured_models_url: Self::DEFAULT_FEATURED_MODELS_URL.to_string(),
            reference_registry: HashMap::new(),
            cached_remote_featured: Arc::new(RwLock::new(None)),
            concurrency_limit: crate::config::canonical::DEFAULT_METADATA_CONCURRENCY,
            remote_enrichment_enabled: true,
        };

        resolver.load_official_nvidia_references();
        resolver
    }

    /// Construct an offline/isolated resolver for tests.
    pub fn new_offline() -> Self {
        let mut resolver = Self::new();
        resolver.remote_enrichment_enabled = false;
        resolver
    }

    /// Override the remote featured models URL (useful for test mocks).
    pub fn with_featured_models_url(mut self, url: impl Into<String>) -> Self {
        self.featured_models_url = url.into();
        self
    }

    /// Disable remote HTTP enrichment (pure offline reference matching).
    pub fn with_remote_enrichment(mut self, enabled: bool) -> Self {
        self.remote_enrichment_enabled = enabled;
        self
    }

    /// Register a custom or test model reference record.
    pub fn register_reference(&mut self, record: NvidiaModelReferenceRecord) {
        self.reference_registry
            .insert(record.model_id.clone(), record);
    }

    /// Populate canonical reference records derived from official NVIDIA model cards and docs.
    fn load_official_nvidia_references(&mut self) {
        let official_records = vec![
            // Google Gemma Series
            NvidiaModelReferenceRecord::new(
                "google/gemma-4-31b-it",
                256_000,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Gemma 4 31B Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "google/gemma-3-12b-it",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Gemma 3 12B Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "google/gemma-3-4b-it",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Gemma 3 4B Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "google/codegemma-7b",
                8192,
                ModelKind::CodeGeneration,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("CodeGemma 7B")
            .with_max_output(2048),
            NvidiaModelReferenceRecord::new(
                "google/codegemma-1.1-7b",
                8192,
                ModelKind::CodeGeneration,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("CodeGemma 1.1 7B")
            .with_max_output(2048),
            NvidiaModelReferenceRecord::new(
                "google/gemma-2b",
                8192,
                ModelKind::TextGeneration,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Gemma 2B")
            .with_max_output(2048),
            NvidiaModelReferenceRecord::new(
                "google/recurrentgemma-2b",
                8192,
                ModelKind::TextGeneration,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("RecurrentGemma 2B")
            .with_max_output(2048),
            NvidiaModelReferenceRecord::new(
                "google/deplot",
                4096,
                ModelKind::VisualLanguage,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("DePlot")
            .with_modalities(vec!["text".to_string(), "image".to_string()]),
            NvidiaModelReferenceRecord::new(
                "google/diffusiongemma-26b-a4b-it",
                4096,
                ModelKind::ImageGeneration,
                CapabilitySupport::Unsupported,
                ModelTier::Standard,
            )
            .with_display_name("DiffusionGemma 26B")
            .with_modalities(vec!["text".to_string(), "image".to_string()]),
            // NVIDIA Nemotron & Foundation Models
            NvidiaModelReferenceRecord::new(
                "nvidia/nemotron-3-super-120b-a12b",
                1_000_000,
                ModelKind::Reasoning,
                CapabilitySupport::Supported,
                ModelTier::Reasoning,
            )
            .with_display_name("Nemotron 3 Super 120B")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "nvidia/nemotron-3-ultra-550b-a55b",
                1_048_576,
                ModelKind::Reasoning,
                CapabilitySupport::Supported,
                ModelTier::Reasoning,
            )
            .with_display_name("Nemotron 3 Ultra 550B")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "nvidia/llama-3.1-nemotron-70b-instruct",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Llama 3.1 Nemotron 70B Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "nvidia/llama-3.1-nemotron-51b-instruct",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Llama 3.1 Nemotron 51B Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "nvidia/llama-3.1-nemotron-ultra-253b-v1",
                131_072,
                ModelKind::Reasoning,
                CapabilitySupport::Supported,
                ModelTier::Reasoning,
            )
            .with_display_name("Nemotron Ultra 253B")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "nvidia/mistral-nemo-minitron-8b-8k-instruct",
                8192,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Mistral-NeMo Minitron 8B")
            .with_max_output(4096),
            NvidiaModelReferenceRecord::new(
                "nvidia/nemotron-4-340b-instruct",
                4096,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Nemotron 4 340B Instruct")
            .with_max_output(4096),
            NvidiaModelReferenceRecord::new(
                "nvidia/nemotron-4-340b-reward",
                4096,
                ModelKind::Reward,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Nemotron 4 340B Reward"),
            NvidiaModelReferenceRecord::new(
                "nvidia/llama-3.1-nemoguard-8b-content-safety",
                8192,
                ModelKind::GuardSafety,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("NeMoGuard 8B Content Safety"),
            NvidiaModelReferenceRecord::new(
                "nvidia/llama-3.1-nemoguard-8b-topic-control",
                8192,
                ModelKind::GuardSafety,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("NeMoGuard 8B Topic Control"),
            NvidiaModelReferenceRecord::new(
                "nvidia/llama-3.1-nemotron-safety-guard-8b-v3",
                8192,
                ModelKind::GuardSafety,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Nemotron Safety Guard 8B"),
            NvidiaModelReferenceRecord::new(
                "nvidia/nemotron-3.5-content-safety",
                8192,
                ModelKind::GuardSafety,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Nemotron 3.5 Content Safety"),
            NvidiaModelReferenceRecord::new(
                "nvidia/cosmos-reason2-8b",
                32_768,
                ModelKind::Reasoning,
                CapabilitySupport::Supported,
                ModelTier::Reasoning,
            )
            .with_display_name("Cosmos Reason2 8B"),
            NvidiaModelReferenceRecord::new(
                "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning",
                65_536,
                ModelKind::Reasoning,
                CapabilitySupport::Supported,
                ModelTier::Reasoning,
            )
            .with_display_name("Nemotron 3 Nano Omni 30B"),
            NvidiaModelReferenceRecord::new(
                "nvidia/nemotron-3.5-lightning-30b-a3b",
                65_536,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Nemotron 3.5 Lightning 30B"),
            NvidiaModelReferenceRecord::new(
                "nvidia/nemotron-nano-3-30b-a3b",
                65_536,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Nemotron Nano 3 30B"),
            NvidiaModelReferenceRecord::new(
                "nvidia/ising-calibration-1.5-31b",
                4096,
                ModelKind::TextGeneration,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Ising Calibration 1.5"),
            NvidiaModelReferenceRecord::new(
                "nvidia/llama3-chatqa-1.5-70b",
                8192,
                ModelKind::Chat,
                CapabilitySupport::Unsupported,
                ModelTier::Standard,
            )
            .with_display_name("Llama 3 ChatQA 1.5 70B"),
            // NVIDIA Embeddings & Multimodal Embeddings
            NvidiaModelReferenceRecord::new(
                "nvidia/embed-qa-4",
                512,
                ModelKind::Embedding,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Embed QA 4"),
            NvidiaModelReferenceRecord::new(
                "nvidia/nv-embedqa-mistral-7b-v2",
                32_768,
                ModelKind::Embedding,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("NV EmbedQA Mistral 7B"),
            NvidiaModelReferenceRecord::new(
                "nvidia/llama-3.2-nv-embedqa-1b-v1",
                8192,
                ModelKind::Embedding,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Llama 3.2 NV EmbedQA 1B"),
            NvidiaModelReferenceRecord::new(
                "nvidia/llama-3.2-nemoretriever-1b-vlm-embed-v1",
                8192,
                ModelKind::Embedding,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("NeMoRetriever 1B VLM Embed")
            .with_modalities(vec!["text".to_string(), "image".to_string()]),
            NvidiaModelReferenceRecord::new(
                "nvidia/nemotron-3-embed-1b",
                8192,
                ModelKind::Embedding,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Nemotron 3 Embed 1B"),
            NvidiaModelReferenceRecord::new(
                "nvidia/llama-nemotron-embed-vl-1b-v2",
                8192,
                ModelKind::Embedding,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Nemotron Embed VL 1B")
            .with_modalities(vec!["text".to_string(), "image".to_string()]),
            NvidiaModelReferenceRecord::new(
                "snowflake/arctic-embed-l",
                512,
                ModelKind::Embedding,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Snowflake Arctic Embed L"),
            NvidiaModelReferenceRecord::new(
                "nvidia/nvclip",
                77,
                ModelKind::Embedding,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("NV-CLIP")
            .with_modalities(vec!["image".to_string()]),
            // NVIDIA Vision & Video
            NvidiaModelReferenceRecord::new(
                "nvidia/ai-synthetic-video-detector",
                2048,
                ModelKind::VisualLanguage,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Synthetic Video Detector")
            .with_modalities(vec!["video".to_string()]),
            NvidiaModelReferenceRecord::new(
                "nvidia/neva-22b",
                4096,
                ModelKind::VisualLanguage,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("NEVA 22B")
            .with_modalities(vec!["text".to_string(), "image".to_string()]),
            NvidiaModelReferenceRecord::new(
                "nvidia/vila",
                4096,
                ModelKind::VisualLanguage,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("VILA")
            .with_modalities(vec!["text".to_string(), "image".to_string()]),
            NvidiaModelReferenceRecord::new(
                "nvidia/nemotron-parse",
                8192,
                ModelKind::VisualLanguage,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Nemotron Parse")
            .with_modalities(vec!["text".to_string(), "image".to_string()]),
            NvidiaModelReferenceRecord::new(
                "nvidia/nemotron-parse-2.0",
                8192,
                ModelKind::VisualLanguage,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Nemotron Parse 2.0")
            .with_modalities(vec!["text".to_string(), "image".to_string()]),
            // NVIDIA Riva Speech Translation
            NvidiaModelReferenceRecord::new(
                "nvidia/riva-translate-4b-instruct",
                4096,
                ModelKind::SpeechSynthesis,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Riva Translate 4B"),
            NvidiaModelReferenceRecord::new(
                "nvidia/riva-translate-4b-instruct-v1.1",
                4096,
                ModelKind::SpeechSynthesis,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Riva Translate 4B v1.1"),
            NvidiaModelReferenceRecord::new(
                "nvidia/riva-translate-4b-instruct-v2",
                4096,
                ModelKind::SpeechSynthesis,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Riva Translate 4B v2"),
            // Meta Models
            NvidiaModelReferenceRecord::new(
                "meta/llama-3.1-70b-instruct",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Llama 3.1 70B Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "meta/llama-3.1-8b-instruct",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Llama 3.1 8B Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "meta/llama-3.3-70b-instruct",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Llama 3.3 70B Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "meta/llama-3.1-405b-instruct",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Reasoning,
            )
            .with_display_name("Llama 3.1 405B Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "meta/llama-3-70b-instruct",
                8192,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Llama 3 70B Instruct")
            .with_max_output(2048),
            NvidiaModelReferenceRecord::new(
                "meta/llama-3-8b-instruct",
                8192,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Llama 3 8B Instruct")
            .with_max_output(2048),
            NvidiaModelReferenceRecord::new(
                "meta/llama-3.2-11b-vision-instruct",
                131_072,
                ModelKind::VisualLanguage,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Llama 3.2 11B Vision Instruct")
            .with_max_output(8192)
            .with_modalities(vec!["text".to_string(), "image".to_string()]),
            NvidiaModelReferenceRecord::new(
                "meta/llama-3.2-90b-vision-instruct",
                131_072,
                ModelKind::VisualLanguage,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Llama 3.2 90B Vision Instruct")
            .with_max_output(8192)
            .with_modalities(vec!["text".to_string(), "image".to_string()]),
            NvidiaModelReferenceRecord::new(
                "meta/llama2-70b",
                4096,
                ModelKind::TextGeneration,
                CapabilitySupport::Unsupported,
                ModelTier::Standard,
            )
            .with_display_name("Llama 2 70B")
            .with_max_output(2048),
            NvidiaModelReferenceRecord::new(
                "meta/codellama-70b",
                16_384,
                ModelKind::CodeGeneration,
                CapabilitySupport::Unsupported,
                ModelTier::Standard,
            )
            .with_display_name("CodeLlama 70B")
            .with_max_output(4096),
            NvidiaModelReferenceRecord::new(
                "meta/llama-guard-4-12b",
                8192,
                ModelKind::GuardSafety,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Llama Guard 4 12B"),
            NvidiaModelReferenceRecord::new(
                "meta/muse-glimmer-30b",
                4096,
                ModelKind::ImageGeneration,
                CapabilitySupport::Unsupported,
                ModelTier::Standard,
            )
            .with_display_name("Muse Glimmer 30B")
            .with_modalities(vec!["text".to_string(), "image".to_string()]),
            // DeepSeek
            NvidiaModelReferenceRecord::new(
                "deepseek-ai/deepseek-r1",
                65_536,
                ModelKind::Reasoning,
                CapabilitySupport::Unsupported,
                ModelTier::Reasoning,
            )
            .with_display_name("DeepSeek R1")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "deepseek-ai/deepseek-coder-6.7b-instruct",
                16_384,
                ModelKind::CodeGeneration,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("DeepSeek Coder 6.7B")
            .with_max_output(4096),
            NvidiaModelReferenceRecord::new(
                "deepseek-ai/deepseek-v4.1-flash",
                65_536,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("DeepSeek V4.1 Flash")
            .with_max_output(8192),
            // Mistral AI
            NvidiaModelReferenceRecord::new(
                "mistralai/mistral-large-2-instruct",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Mistral Large 2 Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "mistralai/mistral-large",
                32_768,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Mistral Large")
            .with_max_output(4096),
            NvidiaModelReferenceRecord::new(
                "mistralai/mistral-7b-instruct-v0.3",
                32_768,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Mistral 7B Instruct v0.3")
            .with_max_output(4096),
            NvidiaModelReferenceRecord::new(
                "mistralai/mixtral-8x22b-v0.1",
                65_536,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Mixtral 8x22B v0.1")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "mistralai/codestral-22b-instruct-v0.1",
                32_768,
                ModelKind::CodeGeneration,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Codestral 22B Instruct")
            .with_max_output(4096),
            NvidiaModelReferenceRecord::new(
                "nv-mistralai/mistral-nemo-12b-instruct",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Mistral-NeMo 12B Instruct")
            .with_max_output(8192),
            // Microsoft
            NvidiaModelReferenceRecord::new(
                "microsoft/phi-3-vision-128k-instruct",
                131_072,
                ModelKind::VisualLanguage,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Phi 3 Vision 128K")
            .with_max_output(8192)
            .with_modalities(vec!["text".to_string(), "image".to_string()]),
            NvidiaModelReferenceRecord::new(
                "microsoft/phi-3.5-moe-instruct",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Phi 3.5 MoE Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "microsoft/kosmos-2",
                4096,
                ModelKind::VisualLanguage,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Kosmos 2")
            .with_modalities(vec!["text".to_string(), "image".to_string()]),
            // Moonshot AI
            NvidiaModelReferenceRecord::new(
                "moonshotai/kimi-k2.6",
                262_144,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Kimi k2.6")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "moonshotai/kimi-k3",
                1_000_000,
                ModelKind::Reasoning,
                CapabilitySupport::Supported,
                ModelTier::Reasoning,
            )
            .with_display_name("Kimi k3")
            .with_max_output(8192),
            // OpenAI & Other Foundation Models
            NvidiaModelReferenceRecord::new(
                "openai/gpt-oss-20b",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("GPT OSS 20B")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "writer/palmyra-fin-70b-32k",
                32_768,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Palmyra Fin 70B 32K")
            .with_max_output(4096),
            NvidiaModelReferenceRecord::new(
                "writer/palmyra-med-70b-32k",
                32_768,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Palmyra Med 70B 32K")
            .with_max_output(4096),
            NvidiaModelReferenceRecord::new(
                "writer/palmyra-med-70b",
                8192,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Palmyra Med 70B")
            .with_max_output(2048),
            NvidiaModelReferenceRecord::new(
                "writer/palmyra-creative-122b",
                8192,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Palmyra Creative 122B")
            .with_max_output(4096),
            NvidiaModelReferenceRecord::new(
                "01-ai/yi-large",
                32_768,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Yi Large")
            .with_max_output(4096),
            NvidiaModelReferenceRecord::new(
                "ai21labs/jamba-1.5-large-instruct",
                262_144,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Jamba 1.5 Large Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "databricks/dbrx-instruct",
                32_768,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("DBRX Instruct")
            .with_max_output(4096),
            NvidiaModelReferenceRecord::new(
                "ibm/granite-3.0-8b-instruct",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Granite 3.0 8B Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "ibm/granite-3.0-3b-a800m-instruct",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Granite 3.0 3B Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "ibm/granite-8b-code-instruct",
                131_072,
                ModelKind::CodeGeneration,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Granite 8B Code Instruct")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "ibm/granite-34b-code-instruct",
                8192,
                ModelKind::CodeGeneration,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("Granite 34B Code Instruct")
            .with_max_output(4096),
            NvidiaModelReferenceRecord::new(
                "bigcode/starcoder2-15b",
                16_384,
                ModelKind::CodeGeneration,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("StarCoder2 15B")
            .with_max_output(4096),
            NvidiaModelReferenceRecord::new(
                "z-ai/glm-5.3",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Standard,
            )
            .with_display_name("GLM 5.3")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "z-ai/glm-5.3-flash",
                131_072,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("GLM 5.3 Flash")
            .with_max_output(8192),
            NvidiaModelReferenceRecord::new(
                "zyphra/zamba2-7b-instruct",
                4096,
                ModelKind::Chat,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Zamba2 7B Instruct")
            .with_max_output(2048),
            NvidiaModelReferenceRecord::new(
                "adept/fuyu-8b",
                4096,
                ModelKind::VisualLanguage,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("Fuyu 8B")
            .with_modalities(vec!["text".to_string(), "image".to_string()]),
            NvidiaModelReferenceRecord::new(
                "aisingapore/sea-lion-7b-instruct",
                4096,
                ModelKind::Chat,
                CapabilitySupport::Unsupported,
                ModelTier::Fast,
            )
            .with_display_name("SEA-LION 7B Instruct")
            .with_max_output(2048),
            NvidiaModelReferenceRecord::new(
                "poolside/laguna-xs-2.1",
                32_768,
                ModelKind::Chat,
                CapabilitySupport::Supported,
                ModelTier::Fast,
            )
            .with_display_name("Laguna XS 2.1")
            .with_max_output(4096),
        ];

        for record in official_records {
            self.reference_registry
                .insert(record.model_id.clone(), record);
        }
    }

    /// Fetch remote featured models feed with local caching.
    async fn get_remote_featured_models(&self) -> HashMap<String, FeaturedModelEntry> {
        if !self.remote_enrichment_enabled {
            return HashMap::new();
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Check in-memory cache
        {
            let guard = self.cached_remote_featured.read().await;
            if let Some((ts, ref map)) = *guard {
                if now.saturating_sub(ts) < Self::REMOTE_CACHE_TTL_SECS {
                    return map.clone();
                }
            }
        }

        // Fetch fresh feed (no credential attached; still authorize the
        // destination so a test-injected URL cannot become an SSRF vector).
        if crate::policy::destination::NetworkDestinationPolicy::new()
            .validate_url(&self.featured_models_url)
            .await
            .is_err()
        {
            return HashMap::new();
        }
        match self.client.get(&self.featured_models_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(body) = resp.json::<FeaturedModelsResponse>().await {
                    let mut map = HashMap::new();
                    for entry in body.featured_models {
                        map.insert(entry.model.clone(), entry);
                    }
                    let mut guard = self.cached_remote_featured.write().await;
                    *guard = Some((now, map.clone()));
                    return map;
                }
            }
            _ => {}
        }

        // Return stale cache if available
        let guard = self.cached_remote_featured.read().await;
        guard
            .as_ref()
            .map(|(_, map)| map.clone())
            .unwrap_or_default()
    }
}

impl Default for NvidiaModelMetadataResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProviderModelMetadataSource for NvidiaModelMetadataResolver {
    async fn enrich_candidate(&self, candidate: &mut ModelCandidate) -> Result<(), ModelError> {
        let remote_featured = self.get_remote_featured_models().await;

        // Step 1: Check if candidate already has authoritative context from provider API
        let provider_has_context = candidate.is_context_known();

        // Step 2: Check remote official featured models
        let featured_entry = remote_featured.get(&candidate.model_id).or_else(|| {
            // Also check without vendor prefix if applicable
            candidate
                .model_id
                .split_once('/')
                .and_then(|(_, rest)| remote_featured.get(rest))
        });

        if let Some(featured) = featured_entry {
            if !provider_has_context && featured.context > 0 {
                candidate.context_capacity = featured.context;
                candidate.context_limits = Some(ContextLimits {
                    advertised_tokens: Some(featured.context),
                    deployed_tokens: None,
                    effective_tokens: featured.context,
                    max_output_tokens: if featured.max_output > 0 {
                        Some(featured.max_output)
                    } else {
                        None
                    },
                });
                candidate.set_context_provenance("nvidia:api_catalog");
            }
            if candidate.display_name.is_none() {
                if let Some(ref name) = featured.model_name {
                    candidate.display_name = Some(name.clone());
                }
            }
        }

        // Step 3: Check official NVIDIA model reference registry
        if let Some(ref_record) = self.reference_registry.get(&candidate.model_id) {
            // Only overwrite context if provider did not explicitly declare it
            if !provider_has_context && candidate.context_provenance() != Some("nvidia:api_catalog")
            {
                candidate.context_capacity = ref_record.context_length;
                candidate.context_limits = Some(ContextLimits {
                    advertised_tokens: Some(ref_record.context_length),
                    deployed_tokens: None,
                    effective_tokens: ref_record.context_length,
                    max_output_tokens: ref_record.max_output_tokens,
                });
                candidate.set_context_provenance("nvidia:model_reference");
            }

            if candidate.display_name.is_none() {
                if let Some(ref name) = ref_record.display_name {
                    candidate.display_name = Some(name.clone());
                }
            }

            candidate.model_kind = ref_record.model_kind;
            candidate.modalities = ref_record.modalities.clone();
            candidate.tool_support = ref_record.tool_calling;
            candidate.supports_tools = ref_record.tool_calling.is_supported();
            candidate.structured_output_support = ref_record.structured_output;
            candidate.supports_structured_output = ref_record.structured_output.is_supported();
            candidate.tier = ref_record.tier;
        } else if !provider_has_context
            && candidate.context_provenance() != Some("nvidia:api_catalog")
        {
            // Step 4: Unknown model - fail closed
            candidate.context_capacity = 0;
            candidate.context_limits = None;
            candidate.set_context_provenance("unknown");
            candidate.tool_support = CapabilitySupport::Unknown;
            candidate.supports_tools = false;
            candidate.model_kind = ModelKind::Unknown;
        }

        Ok(())
    }

    async fn enrich_candidates(&self, candidates: &mut [ModelCandidate]) -> Result<(), ModelError> {
        let semaphore = Arc::new(Semaphore::new(self.concurrency_limit));
        let mut handles = Vec::new();

        // Warm up remote featured models once before parallel enrichment
        let _ = self.get_remote_featured_models().await;

        for candidate in candidates.iter_mut() {
            let _permit = semaphore.clone().acquire_owned().await.map_err(|e| {
                ModelError::ProviderInternalFailure(format!("concurrency semaphore error: {e}"))
            })?;
            // Enrich synchronously per candidate in the current task
            let _ = self.enrich_candidate(candidate).await;
            handles.push(());
        }

        Ok(())
    }
}
