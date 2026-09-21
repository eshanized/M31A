//! Model inference service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Request payload for model invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInvocationRequest {
    pub prompt: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
}

/// Normalized response payload from model invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInvocationResponse {
    pub content: String,
    pub model: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

/// Asynchronous service seam for model reasoning and completion.
#[async_trait]
pub trait ModelService: Send + Sync + 'static {
    /// Send prompt to model and return normalized response.
    async fn invoke_model(
        &self,
        request: &ModelInvocationRequest,
    ) -> Result<ModelInvocationResponse, CapabilityError>;
}
