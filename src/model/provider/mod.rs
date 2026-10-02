//! Provider-neutral abstractions for LLM reasoning and streaming.
//!
//! Core principle: The model proposes. The runtime decides. (MDL-01, Law 1)

pub mod mock;
pub mod nvidia;
pub mod nvidia_metadata;
pub mod sse;

pub use mock::MockProvider;
pub use nvidia::NvidiaProvider;
pub use nvidia_metadata::*;
pub use sse::{StreamAccumulator, normalize_http_error};

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use tokio_util::sync::CancellationToken;

use crate::model::types::{ModelError, ModelProposal, StreamChunk, TokenUsage};

/// Pinned boxed stream of normalized stream chunks.
pub type BoxStreamChunk = Pin<Box<dyn Stream<Item = Result<StreamChunk, ModelError>> + Send>>;

/// Provider-neutral seam trait for invoking model reasoning (MDL-01).
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Invoke the model with structured prompt and tools, receiving an authoritative proposal and token usage.
    async fn call_model(
        &self,
        model_name: &str,
        system_prompt: &str,
        tools: Vec<serde_json::Value>,
        cancellation: &CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), ModelError>;

    /// Invoke the model with structured multi-turn conversation messages and tools (D-02, D-05).
    async fn call_model_with_messages(
        &self,
        model_name: &str,
        messages: &[crate::model::types::ChatMessage],
        tools: Vec<serde_json::Value>,
        cancellation: &CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), ModelError> {
        let system_prompt = messages
            .iter()
            .find_map(|m| match m {
                crate::model::types::ChatMessage::System { content } => Some(content.as_str()),
                _ => None,
            })
            .unwrap_or_default();
        self.call_model(model_name, system_prompt, tools, cancellation)
            .await
    }

    /// Stream the model reasoning chunks asynchronously.
    async fn stream_model(
        &self,
        model_name: &str,
        system_prompt: &str,
        tools: Vec<serde_json::Value>,
        cancellation: &CancellationToken,
    ) -> Result<BoxStreamChunk, ModelError>;

    /// Discover available models from the provider endpoint (Dynamic Discovery).
    async fn discover_models(
        &self,
    ) -> Result<Vec<crate::model::router::resolver::ModelCandidate>, ModelError> {
        Ok(vec![])
    }

    /// Identify whether this provider is a deterministic test double.
    ///
    /// Production constructors (`AppRuntime::from_pool_workspace_and_config`,
    /// `ProductionWorkerDispatcher::new_with_roots_and_config`) only ever
    /// install the real `NvidiaProvider` (or `None`, which fails closed).
    /// `MockProvider` overrides this to `true` so the runtime, the TUI, and
    /// executable audits can distinguish structural test wiring from the
    /// live provider path. Defaults to `false` so real providers need no
    /// changes and can never silently report themselves as test-only.
    fn is_test_double(&self) -> bool {
        false
    }
}
