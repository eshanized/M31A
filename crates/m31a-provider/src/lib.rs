//! `m31a-provider`: NVIDIA NIM LLM abstraction layer.
//!
//! Provides dedicated, high-performance integration with NVIDIA NIM frontier intelligence,
//! handling token streaming, DeepSeek R1 thinking traces, tool call extraction,
//! dynamic model discovery, canonical 85-model registry, rate limiting, and exponential retries.

pub mod cache_policy;
pub mod capabilities;
pub mod client;
pub mod debug_context;
pub mod gateway_oauth;
pub mod health_monitor;
#[cfg(feature = "live-nvidia-tests")]
pub mod live_test_support;
pub mod manager;
pub mod models;
pub mod models_identity;
pub mod provider_trait;
pub mod retry;
pub mod router;
pub mod token_refresh;
pub mod wire_server;
pub mod workspace_routing;

pub use cache_policy::{
    BREAKPOINT_CAP, CachePolicy, ModelRef, NimCacheAlignmentResult, NimPrefixCacheOptimizer,
};
pub use capabilities::{
    ModelConfigOverrides, ModelProviderCapabilities, PERSONALITY_SECTION_HEADER,
    RemoteCompactionSupport, apply_model_config_overrides, estimate_interaction_cost,
    strip_personality_section, validate_capability_requirement,
};
pub use client::{NimRequestOptions, NvidiaNimClient, SseStreamAccumulator, StreamCallback};
pub use debug_context::ResponseDebugContext;
pub use gateway_oauth::{
    GatewayOAuthConfig, GatewayOAuthDelivery, RESERVED_GATEWAY_OAUTH_HEADER_PREFIXES,
    RESERVED_GATEWAY_OAUTH_HEADERS,
};
pub use health_monitor::{HealthStatus, LocalModelHealthMonitor};
pub use manager::{ContextBudgetStatus, ModelPreset, ModelsManager};
pub use models::{
    CanonicalCapabilities, CanonicalCatalog, CanonicalEligibility, CanonicalModel,
    CanonicalPricing, CanonicalProvenance, InputModality, ModelInfo, ServiceTier, ToolMode,
    TruncationPolicy, fetch_remote_models, find_canonical_model, find_model, get_canonical_catalog,
    get_nvidia_model_catalog, list_coding_models, list_embedding_models, list_guardrail_models,
    list_models_by_publisher, list_publishers, list_reasoning_models, list_vision_models,
    load_cached_models, save_cached_models, search_models,
};
pub use models_identity::{
    ModelFamily, ModelIdentityInfo, ModelVendor, calculate_token_cost, get_model_pricing,
    is_code_specialized, is_embedding_model, is_guardrail_model, is_reasoning_model,
    is_vision_model, resolve_model_identity,
};
#[cfg(any(test, feature = "test-support"))]
pub use provider_trait::MockProvider;
pub use provider_trait::{
    ChatCompletionResponse, ChatRequest, EmbeddingsProvider, GenericOpenAiProvider,
    ModelCapabilities, ModelMetadata, ModelPricing, ModelProvider, NimProvider, ProviderBuilder,
    ProviderClient, ProviderConfig, ProviderError, ProviderType, StreamEvent, StreamHandler,
    validate_provider_config,
};
pub use retry::RetryPolicy;
pub use router::{HybridModelRouter, ModelTier, RouteModelConfig, classify_task_tier};
pub use token_refresh::{CachedToken, DynamicTokenProvider, StaticTokenProvider, TokenProvider};
pub use wire_server::{LocalWireServer, WireServerResponse};
pub use workspace_routing::{
    ComplexityTier, WorkspaceComplexityAnalyzer, WorkspaceProfile, WorkspaceRoutingPolicy,
};
