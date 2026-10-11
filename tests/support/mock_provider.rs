//! Deterministic MockProvider for offline integration and contract tests (test-only).
//!
//! Isolated under `tests/support/` — never exported from production `src/` modules.

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

use async_trait::async_trait;
use futures::stream;
use tokio_util::sync::CancellationToken;

use m31a::model::provider::{BoxStreamChunk, ModelProvider};
use m31a::model::types::{ModelError, ModelProposal, StreamChunk, TokenUsage, UsageSource};

/// Type alias for queued mock model responses.
pub type MockResponse = Result<(ModelProposal, TokenUsage), ModelError>;
type ResponseQueue = Arc<Mutex<VecDeque<MockResponse>>>;

/// Recorded invocation parameters received by MockProvider.
#[derive(Debug, Clone)]
pub struct MockModelCall {
    pub model_name: String,
    pub system_prompt: String,
    pub messages: Vec<m31a::model::types::ChatMessage>,
    pub tools: Vec<serde_json::Value>,
}

/// Deterministic offline model provider for unit and contract testing.
#[derive(Debug, Clone)]
pub struct MockProvider {
    responses: ResponseQueue,
    default_response: Option<(ModelProposal, TokenUsage)>,
    recorded_calls: Arc<Mutex<Vec<MockModelCall>>>,
    discovered_models: Arc<std::sync::Mutex<Vec<m31a::model::router::resolver::ModelCandidate>>>,
}

impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl MockProvider {
    /// Create a new MockProvider with an empty response queue and no fabricated fallback.
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::new())),
            default_response: None,
            recorded_calls: Arc::new(Mutex::new(Vec::new())),
            discovered_models: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Explicitly configure a fallback response when the response queue is empty.
    /// Note: Default mock completions must use `UsageSource::Estimated` to prevent
    /// fabricating authoritative provider accounting.
    pub fn with_default_response(mut self, proposal: ModelProposal, mut usage: TokenUsage) -> Self {
        if usage.source == UsageSource::AuthoritativeProvider {
            usage.source = UsageSource::Estimated;
        }
        self.default_response = Some((proposal, usage));
        self
    }

    /// Add a canned proposal to the response queue (test helper).
    pub fn with_response(self, proposal: ModelProposal) -> Self {
        self.responses.try_lock().unwrap().push_back(Ok((
            proposal,
            TokenUsage::new(0, 0, 0, 0, UsageSource::Estimated),
        )));
        self
    }

    /// Configure discovered models for test mocking.
    pub fn with_discovered_models(
        self,
        models: Vec<m31a::model::router::resolver::ModelCandidate>,
    ) -> Self {
        *self.discovered_models.lock().unwrap() = models;
        self
    }

    /// Dynamically update discovered models (used by Critical Test §18).
    pub fn set_discovered_models(
        &self,
        models: Vec<m31a::model::router::resolver::ModelCandidate>,
    ) {
        *self.discovered_models.lock().unwrap() = models;
    }

    /// Push a canned response onto the FIFO queue.
    pub async fn push_response(&self, response: Result<(ModelProposal, TokenUsage), ModelError>) {
        let mut q = self.responses.lock().await;
        q.push_back(response);
    }

    /// Access all recorded model calls.
    pub async fn recorded_calls(&self) -> Vec<MockModelCall> {
        self.recorded_calls.lock().await.clone()
    }
}

#[async_trait]
impl ModelProvider for MockProvider {
    async fn call_model_with_messages(
        &self,
        model_name: &str,
        messages: &[m31a::model::types::ChatMessage],
        tools: Vec<serde_json::Value>,
        cancellation: &CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), ModelError> {
        if cancellation.is_cancelled() {
            return Err(ModelError::Cancelled);
        }

        let system_prompt = messages
            .iter()
            .find_map(|m| match m {
                m31a::model::types::ChatMessage::System { content } => Some(content.as_str()),
                _ => None,
            })
            .unwrap_or_default()
            .to_string();

        self.recorded_calls.lock().await.push(MockModelCall {
            model_name: model_name.to_string(),
            system_prompt,
            messages: messages.to_vec(),
            tools,
        });

        let mut q = self.responses.lock().await;
        if let Some(res) = q.pop_front() {
            res
        } else if let Some(ref fallback) = self.default_response {
            Ok(fallback.clone())
        } else {
            Err(ModelError::InvalidRequest(
                "mock provider response queue exhausted: no queued responses available".to_string(),
            ))
        }
    }

    async fn call_model_with_messages_streaming(
        &self,
        model_name: &str,
        messages: &[m31a::model::types::ChatMessage],
        tools: Vec<serde_json::Value>,
        cancellation: &CancellationToken,
        chunk_tx: Option<tokio::sync::mpsc::UnboundedSender<StreamChunk>>,
    ) -> Result<(ModelProposal, TokenUsage), ModelError> {
        let (proposal, usage) = self
            .call_model_with_messages(model_name, messages, tools, cancellation)
            .await?;

        if let Some(ref tx) = chunk_tx {
            match &proposal {
                ModelProposal::AssistantText { content } => {
                    let _ = tx.send(StreamChunk::TextDelta(content.clone()));
                }
                ModelProposal::ToolCalls { calls } => {
                    for (i, c) in calls.iter().enumerate() {
                        let _ = tx.send(StreamChunk::ToolCallDelta {
                            index: i,
                            name: Some(c.name.clone()),
                            arguments_delta: c.arguments.to_string(),
                        });
                    }
                }
                _ => {}
            }
            let _ = tx.send(StreamChunk::UsageUpdate(usage.clone()));
            let _ = tx.send(StreamChunk::FinishReason("stop".to_string()));
        }

        Ok((proposal, usage))
    }

    async fn call_model(
        &self,
        model_name: &str,
        system_prompt: &str,
        tools: Vec<serde_json::Value>,
        cancellation: &CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), ModelError> {
        if cancellation.is_cancelled() {
            return Err(ModelError::Cancelled);
        }

        self.recorded_calls.lock().await.push(MockModelCall {
            model_name: model_name.to_string(),
            system_prompt: system_prompt.to_string(),
            messages: Vec::new(),
            tools,
        });

        let mut q = self.responses.lock().await;
        if let Some(res) = q.pop_front() {
            res
        } else if let Some(ref fallback) = self.default_response {
            Ok(fallback.clone())
        } else {
            Err(ModelError::InvalidRequest(
                "mock provider response queue exhausted: no queued responses available".to_string(),
            ))
        }
    }

    /// Test-double marker: this provider is always a test double.
    fn is_test_double(&self) -> bool {
        true
    }

    async fn stream_model(
        &self,
        _model_name: &str,
        _system_prompt: &str,
        _tools: Vec<serde_json::Value>,
        cancellation: &CancellationToken,
    ) -> Result<BoxStreamChunk, ModelError> {
        if cancellation.is_cancelled() {
            return Err(ModelError::Cancelled);
        }

        let mut q = self.responses.lock().await;
        let (proposal, usage) = match q.pop_front() {
            Some(Ok(res)) => res,
            Some(Err(e)) => return Err(e),
            None => {
                if let Some(ref fallback) = self.default_response {
                    fallback.clone()
                } else {
                    return Err(ModelError::InvalidRequest(
                        "mock provider response queue exhausted: no queued responses available"
                            .to_string(),
                    ));
                }
            }
        };

        let mut chunks = Vec::new();
        match &proposal {
            ModelProposal::ToolCalls { calls } => {
                for (idx, call) in calls.iter().enumerate() {
                    chunks.push(Ok(StreamChunk::ToolCallDelta {
                        index: idx,
                        name: Some(call.name.clone()),
                        arguments_delta: call.arguments.to_string(),
                    }));
                }
                chunks.push(Ok(StreamChunk::FinishReason("tool_calls".to_string())));
            }
            ModelProposal::AssistantText { content } => {
                chunks.push(Ok(StreamChunk::TextDelta(content.clone())));
                chunks.push(Ok(StreamChunk::FinishReason("stop".to_string())));
            }
            ModelProposal::AskUser { question, options } => {
                let json = serde_json::json!({
                    "question": question,
                    "options": options,
                });
                chunks.push(Ok(StreamChunk::ToolCallDelta {
                    index: 0,
                    name: Some("ask_user".to_string()),
                    arguments_delta: json.to_string(),
                }));
                chunks.push(Ok(StreamChunk::FinishReason("tool_calls".to_string())));
            }
            ModelProposal::Handoff {
                target_role,
                reason,
            } => {
                chunks.push(Ok(StreamChunk::TextDelta(format!(
                    "Handoff to {}: {}",
                    target_role, reason
                ))));
                chunks.push(Ok(StreamChunk::FinishReason("stop".to_string())));
            }
            ModelProposal::Complete { summary, .. } => {
                chunks.push(Ok(StreamChunk::TextDelta(summary.clone())));
                chunks.push(Ok(StreamChunk::FinishReason("stop".to_string())));
            }
        }
        chunks.push(Ok(StreamChunk::UsageUpdate(usage)));

        Ok(Box::pin(stream::iter(chunks)))
    }

    async fn discover_models(
        &self,
    ) -> Result<Vec<m31a::model::router::resolver::ModelCandidate>, ModelError> {
        let models = self.discovered_models.lock().unwrap().clone();
        Ok(models)
    }

    fn provider_name(&self) -> &'static str {
        "mock"
    }
}
