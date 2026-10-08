//! Canonical domain types for model communication, proposals, streaming, and error classification.
//!
//! Core principle: The model proposes. The runtime decides. (MDL-01, MDL-03)

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Selectable option presented to the operator in an AskUser interaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserOption {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Structured proposal emitted by a model turn (The model proposes. The runtime decides.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelProposal {
    /// Non-terminating conversational assistant text (does NOT complete task).
    AssistantText { content: String },
    /// Model proposes one or more native tool actions.
    ToolCalls { calls: Vec<ModelToolCall> },
    /// Model requests operator decision or clarification before proceeding.
    AskUser {
        question: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        options: Vec<UserOption>,
    },
    /// Model proposes handing off work to another agent role.
    Handoff { target_role: String, reason: String },
    /// Model proposes task completion with summary and generated artifact references.
    Complete {
        summary: String,
        artifacts: Vec<crate::ids::ArtifactId>,
    },
}

impl ModelProposal {
    /// Check whether this proposal explicitly requests task completion.
    pub fn is_completion(&self) -> bool {
        matches!(self, ModelProposal::Complete { .. })
    }

    /// Extract all tool calls from `ToolCalls` variant.
    pub fn tool_calls(&self) -> Vec<ModelToolCall> {
        match self {
            ModelProposal::ToolCalls { calls } => calls.clone(),
            _ => Vec::new(),
        }
    }

    /// Extract text summary or narrative content if present.
    pub fn text_content(&self) -> Option<&str> {
        match self {
            ModelProposal::AssistantText { content } => Some(content.as_str()),
            ModelProposal::Complete { summary, .. } => Some(summary.as_str()),
            _ => None,
        }
    }

    /// Structural discriminant name for safe diagnostic logging without content leakage (Finding A).
    pub fn kind_name(&self) -> &'static str {
        match self {
            ModelProposal::AssistantText { .. } => "assistant_text",
            ModelProposal::ToolCalls { .. } => "tool_calls",
            ModelProposal::AskUser { .. } => "ask_user",
            ModelProposal::Handoff { .. } => "handoff",
            ModelProposal::Complete { .. } => "complete",
        }
    }

    /// Extract safe tool identifiers (names only, no arguments) for structural diagnostic logging.
    pub fn tool_names(&self) -> Vec<String> {
        match self {
            ModelProposal::ToolCalls { calls } => calls.iter().map(|c| c.name.clone()).collect(),
            _ => Vec::new(),
        }
    }

    /// Number of proposed tool calls.
    pub fn tool_calls_count(&self) -> usize {
        match self {
            ModelProposal::ToolCalls { calls } => calls.len(),
            _ => 0,
        }
    }
}

/// Structured tool call requested by an assistant message (MDL-01, MDL-03).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

impl ModelToolCall {
    pub fn new(name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self {
            id: uuid::Uuid::now_v7().to_string(),
            name: name.into(),
            arguments,
        }
    }

    pub fn with_id(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }
}

/// Normalized conversational message turn for multi-turn model interaction (D-02, D-05).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum ChatMessage {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        tool_calls: Vec<ModelToolCall>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

/// Normalized streaming chunk emitted incrementally by a model provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamChunk {
    /// canonical model invocation began.
    InvocationStarted { invocation_id: uuid::Uuid },
    /// Incremental generated text/reasoning token delta.
    TextDelta(String),
    /// Incremental tool call fragment.
    ToolCallDelta {
        index: usize,
        name: Option<String>,
        arguments_delta: String,
    },
    /// Periodic or final token usage update.
    UsageUpdate(TokenUsage),
    /// Stream completion reason (e.g. "stop", "tool_calls", "length").
    FinishReason(String),
}

/// Authoritative or estimated token usage for an invocation attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
    pub reasoning_tokens: usize,
    pub source: UsageSource,
}

impl TokenUsage {
    /// Construct a new TokenUsage record.
    pub fn new(
        prompt_tokens: usize,
        completion_tokens: usize,
        total_tokens: usize,
        reasoning_tokens: usize,
        source: UsageSource,
    ) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            reasoning_tokens,
            source,
        }
    }

    /// Accumulate token usage from a subsequent turn into this record.
    pub fn accumulate(&mut self, other: &Self) {
        self.prompt_tokens = self.prompt_tokens.saturating_add(other.prompt_tokens);
        self.completion_tokens = self
            .completion_tokens
            .saturating_add(other.completion_tokens);
        self.total_tokens = self.total_tokens.saturating_add(other.total_tokens);
        self.reasoning_tokens = self.reasoning_tokens.saturating_add(other.reasoning_tokens);
        if other.source == UsageSource::AuthoritativeProvider {
            self.source = UsageSource::AuthoritativeProvider;
        }
    }
}

/// Provenance of token accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UsageSource {
    /// Final token usage reported authoritatively by the provider.
    #[default]
    AuthoritativeProvider,
    /// Estimated token usage (e.g. tokenizer heuristic or prematurely halted stream).
    Estimated,
}

/// Canonical operational capability status of a model or provider (WS-I).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCapabilityStatus {
    /// Provider/model can execute a real production request.
    Available,
    /// Provider/model is known to the architecture but not currently supported/enabled in this runtime.
    Unavailable,
    /// Provider/model is supported by the architecture but required configuration/credentials are invalid or missing.
    Misconfigured,
    /// Implementation exists only for deterministic testing and offline fixtures.
    MockOnly,
    /// Capability cannot be established.
    #[default]
    Unknown,
}

impl std::fmt::Display for ProviderCapabilityStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Available => write!(f, "AVAILABLE"),
            Self::Unavailable => write!(f, "UNAVAILABLE"),
            Self::Misconfigured => write!(f, "MISCONFIGURED"),
            Self::MockOnly => write!(f, "MOCK / TEST-ONLY"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// Strongly typed domain errors occurring during model invocation or streaming transport (D-04, WS-I §11).
#[derive(Debug, Error, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum ModelError {
    #[error("missing configuration: {0}")]
    MissingConfiguration(String),
    #[error("missing credentials: {0}")]
    MissingCredentials(String),
    #[error("authentication failed: invalid or expired API key")]
    AuthenticationFailed,
    #[error("authentication failure: {0}")]
    AuthenticationFailure(String),
    #[error("endpoint unavailable: {0}")]
    EndpointUnavailable(String),
    #[error("model unavailable: {0}")]
    ModelUnavailable(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("unsupported capability: {0}")]
    UnsupportedCapability(String),
    #[error("provider internal failure: {0}")]
    ProviderInternalFailure(String),
    #[error("network connection or transport error: {0}")]
    Network(String),
    #[error("provider returned HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("rate limit exceeded, retry after {cooldown_secs}s")]
    RateLimited { cooldown_secs: u64 },
    #[error("context window exhausted: requested {requested}, capacity {capacity}")]
    ContextWindowExhausted { requested: usize, capacity: usize },
    #[error("operation cancelled by runtime")]
    Cancelled,
    #[error("SSE stream interrupted: {0}")]
    StreamInterrupted(String),
    #[error("invalid response payload: {0}")]
    InvalidResponse(String),
    #[error("protocol violation: {0}")]
    ProtocolViolation(String),
}

impl ModelError {
    /// Returns true if the error is considered transient (rate limits, timeouts, transport drops, 5xx server errors).
    pub fn is_transient(&self) -> bool {
        match self {
            Self::RateLimited { .. }
            | Self::Timeout(_)
            | Self::Network(_)
            | Self::StreamInterrupted(_)
            | Self::EndpointUnavailable(_)
            | Self::ModelUnavailable(_)
            | Self::ProviderInternalFailure(_) => true,
            Self::Http { status, .. } => *status == 429 || (*status >= 500 && *status <= 504),
            Self::MissingConfiguration(_)
            | Self::MissingCredentials(_)
            | Self::AuthenticationFailed
            | Self::AuthenticationFailure(_)
            | Self::InvalidRequest(_)
            | Self::UnsupportedCapability(_)
            | Self::ContextWindowExhausted { .. }
            | Self::Cancelled
            | Self::InvalidResponse(_)
            | Self::ProtocolViolation(_) => false,
        }
    }

    /// Returns recommended retry delay / cooldown if declared by the provider.
    pub fn retry_delay(&self) -> Option<std::time::Duration> {
        match self {
            Self::RateLimited { cooldown_secs } => {
                Some(std::time::Duration::from_secs(*cooldown_secs))
            }
            Self::Timeout(_) | Self::Network(_) | Self::StreamInterrupted(_) => {
                Some(std::time::Duration::from_millis(500))
            }
            Self::EndpointUnavailable(_)
            | Self::ModelUnavailable(_)
            | Self::ProviderInternalFailure(_) => Some(std::time::Duration::from_millis(1000)),
            Self::Http { status, .. } if *status == 429 => Some(std::time::Duration::from_secs(2)),
            Self::Http { status, .. } if *status >= 500 && *status <= 504 => {
                Some(std::time::Duration::from_millis(1000))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_model_proposal_serialization_tool_calls_single() {
        let proposal = ModelProposal::ToolCalls {
            calls: vec![ModelToolCall {
                id: "call_1".to_string(),
                name: "file_read".to_string(),
                arguments: json!({ "path": "src/main.rs" }),
            }],
        };
        let serialized = serde_json::to_string(&proposal).unwrap();
        assert!(serialized.contains("\"type\":\"tool_calls\""));
        assert!(serialized.contains("\"name\":\"file_read\""));

        let deserialized: ModelProposal = serde_json::from_str(&serialized).unwrap();
        assert_eq!(proposal, deserialized);
        assert_eq!(deserialized.tool_calls().len(), 1);
        assert!(!deserialized.is_completion());
    }

    #[test]
    fn test_model_proposal_serialization_tool_calls_multiple() {
        let proposal = ModelProposal::ToolCalls {
            calls: vec![
                ModelToolCall {
                    id: "call_1".to_string(),
                    name: "glob".to_string(),
                    arguments: json!({ "pattern": "src/*.rs" }),
                },
                ModelToolCall {
                    id: "call_2".to_string(),
                    name: "file_read".to_string(),
                    arguments: json!({ "path": "src/lib.rs" }),
                },
            ],
        };
        let serialized = serde_json::to_string(&proposal).unwrap();
        assert!(serialized.contains("\"type\":\"tool_calls\""));

        let deserialized: ModelProposal = serde_json::from_str(&serialized).unwrap();
        assert_eq!(proposal, deserialized);
        assert_eq!(deserialized.tool_calls().len(), 2);
    }

    #[test]
    fn test_model_proposal_serialization_assistant_text() {
        let proposal = ModelProposal::AssistantText {
            content: "I will inspect the codebase.".to_string(),
        };
        let serialized = serde_json::to_string(&proposal).unwrap();
        assert!(serialized.contains("\"type\":\"assistant_text\""));
        assert!(serialized.contains("\"content\":\"I will inspect the codebase.\""));

        let deserialized: ModelProposal = serde_json::from_str(&serialized).unwrap();
        assert_eq!(proposal, deserialized);
        assert!(!deserialized.is_completion());
        assert_eq!(deserialized.tool_calls().len(), 0);
    }

    #[test]
    fn test_model_proposal_serialization_ask_user() {
        let proposal = ModelProposal::AskUser {
            question: "Choose database:".to_string(),
            options: vec![UserOption {
                id: "sqlite".to_string(),
                label: "SQLite".to_string(),
                description: None,
            }],
        };
        let serialized = serde_json::to_string(&proposal).unwrap();
        assert!(serialized.contains("\"type\":\"ask_user\""));

        let deserialized: ModelProposal = serde_json::from_str(&serialized).unwrap();
        assert_eq!(proposal, deserialized);
        assert!(!deserialized.is_completion());
        assert_eq!(deserialized.tool_calls().len(), 0);
    }

    #[test]
    fn test_model_proposal_serialization_handoff() {
        let proposal = ModelProposal::Handoff {
            target_role: "reviewer".to_string(),
            reason: "needs code review".to_string(),
        };
        let serialized = serde_json::to_string(&proposal).unwrap();
        assert!(serialized.contains("\"type\":\"handoff\""));

        let deserialized: ModelProposal = serde_json::from_str(&serialized).unwrap();
        assert_eq!(proposal, deserialized);
    }

    #[test]
    fn test_model_proposal_serialization_complete() {
        let proposal = ModelProposal::Complete {
            summary: "all done".to_string(),
            artifacts: vec![],
        };
        let serialized = serde_json::to_string(&proposal).unwrap();
        assert!(serialized.contains("\"type\":\"complete\""));

        let deserialized: ModelProposal = serde_json::from_str(&serialized).unwrap();
        assert_eq!(proposal, deserialized);
    }

    #[test]
    fn test_stream_chunk_serialization() {
        let chunk = StreamChunk::ToolCallDelta {
            index: 0,
            name: Some("test_tool".to_string()),
            arguments_delta: "{\"arg\":".to_string(),
        };
        let serialized = serde_json::to_string(&chunk).unwrap();
        assert!(serialized.contains("\"arguments_delta\":\"{\\\"arg\\\":"));

        let deserialized: StreamChunk = serde_json::from_str(&serialized).unwrap();
        assert_eq!(chunk, deserialized);
    }

    #[test]
    fn test_token_usage_defaults_and_construction() {
        let usage = TokenUsage::new(100, 50, 150, 20, UsageSource::AuthoritativeProvider);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
        assert_eq!(usage.reasoning_tokens, 20);
        assert_eq!(usage.source, UsageSource::AuthoritativeProvider);

        let default_usage = TokenUsage::default();
        assert_eq!(default_usage.source, UsageSource::AuthoritativeProvider);
        assert_eq!(default_usage.total_tokens, 0);
    }

    #[test]
    fn test_model_error_formatting() {
        let err = ModelError::RateLimited { cooldown_secs: 30 };
        assert_eq!(err.to_string(), "rate limit exceeded, retry after 30s");

        let err = ModelError::ContextWindowExhausted {
            requested: 9000,
            capacity: 8192,
        };
        assert_eq!(
            err.to_string(),
            "context window exhausted: requested 9000, capacity 8192"
        );

        let err = ModelError::AuthenticationFailed;
        assert_eq!(
            err.to_string(),
            "authentication failed: invalid or expired API key"
        );
    }
}
