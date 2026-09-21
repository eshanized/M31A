//! Model capability provider wrapping ModelCaller (CTL-02, D-01, D-03).

use crate::agent::model_policy::{ModelCaller, ModelProposal};
use crate::capability::error::CapabilityError;
use crate::capability::traits::model::{
    ModelInvocationRequest, ModelInvocationResponse, ModelService,
};
use async_trait::async_trait;
use std::sync::Arc;

/// Native provider for model invocations wrapping a ModelCaller implementation.
pub struct ModelCallerProvider {
    caller: Arc<dyn ModelCaller>,
    default_model: String,
}

impl ModelCallerProvider {
    /// Create a new provider wrapping the given ModelCaller seam.
    pub fn new(caller: Arc<dyn ModelCaller>, default_model: impl Into<String>) -> Self {
        Self {
            caller,
            default_model: default_model.into(),
        }
    }
}

#[async_trait]
impl ModelService for ModelCallerProvider {
    async fn invoke_model(
        &self,
        request: &ModelInvocationRequest,
    ) -> Result<ModelInvocationResponse, CapabilityError> {
        let proposal = self.caller.call_model(&request.prompt).await.map_err(|e| {
            CapabilityError::ExecutionFailed {
                exit_code: None,
                message: e,
            }
        })?;

        let content = match proposal {
            ModelProposal::ToolCalls { calls } => {
                let list = calls
                    .iter()
                    .map(|c| format!("{}:{}", c.name, c.arguments))
                    .collect::<Vec<_>>()
                    .join("; ");
                format!("tool_calls:{list}")
            }
            ModelProposal::AssistantText { content } => content,
            ModelProposal::AskUser { question, .. } => format!("ask_user:{question}"),
            ModelProposal::Handoff {
                target_role,
                reason,
            } => {
                format!("handoff:{target_role}:{reason}")
            }
            ModelProposal::Complete { summary, .. } => summary,
        };

        let model = request
            .model
            .clone()
            .unwrap_or_else(|| self.default_model.clone());

        Ok(ModelInvocationResponse {
            content,
            model,
            prompt_tokens: 0,
            completion_tokens: 0,
        })
    }
}
