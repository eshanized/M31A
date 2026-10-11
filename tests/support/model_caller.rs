//! Canned and deterministic model callers for offline testing (test-only).
//!
//! Relocated out of production runtime modules (`src/agent/model_policy.rs`).

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use m31a::agent::model_policy::{ModelCaller, ModelProposal};
use m31a::model::types::TokenUsage;

/// Scripted test model caller returning queued proposals.
#[derive(Debug, Clone)]
pub struct TestModelCaller {
    pub proposals: Arc<Mutex<Vec<Result<ModelProposal, String>>>>,
    pub default_summary: Option<String>,
    pub token_usage: Option<TokenUsage>,
    pub provider: String,
    pub model: String,
}

impl TestModelCaller {
    /// Create a test model caller with an explicit default completion summary.
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            proposals: Arc::new(Mutex::new(Vec::new())),
            default_summary: Some(summary.into()),
            token_usage: None,
            provider: "test".to_string(),
            model: "test-model".to_string(),
        }
    }

    /// Create a caller from a scripted sequence of proposals.
    /// When the sequence is exhausted and no fallback is set, calls fail explicitly.
    pub fn from_proposals(proposals: Vec<Result<ModelProposal, String>>) -> Self {
        Self {
            proposals: Arc::new(Mutex::new(proposals)),
            default_summary: None,
            token_usage: None,
            provider: "test".to_string(),
            model: "test-model".to_string(),
        }
    }

    /// Create a caller with exactly one canned proposal.
    pub fn with_proposal(proposal: ModelProposal) -> Self {
        Self {
            proposals: Arc::new(Mutex::new(vec![Ok(proposal)])),
            default_summary: None,
            token_usage: None,
            provider: "test".to_string(),
            model: "test-model".to_string(),
        }
    }

    pub fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.token_usage = Some(usage);
        self
    }

    pub fn with_provider_and_model(
        mut self,
        provider: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        self.provider = provider.into();
        self.model = model.into();
        self
    }
}

#[async_trait]
impl ModelCaller for TestModelCaller {
    fn provider_name(&self) -> String {
        self.provider.clone()
    }

    fn model_name(&self) -> String {
        self.model.clone()
    }

    async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
        let mut queue = self.proposals.lock().await;
        if !queue.is_empty() {
            queue.remove(0)
        } else if let Some(ref fallback) = self.default_summary {
            Ok(ModelProposal::Complete {
                summary: fallback.clone(),
                artifacts: Vec::new(),
            })
        } else {
            Err("test model caller proposals exhausted: no scripted response available".to_string())
        }
    }

    async fn call_model_cancellable_with_usage(
        &self,
        context: &str,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        let proposal = self.call_model_cancellable(context, cancellation).await?;
        let usage = self.token_usage.clone().unwrap_or_else(|| {
            TokenUsage::new(0, 0, 0, 0, m31a::model::types::UsageSource::Estimated)
        });
        Ok((proposal, usage))
    }

    async fn call_model_with_context_and_usage(
        &self,
        compiled: &m31a::kernel::seams::context::CompiledContext,
        cancellation: &tokio_util::sync::CancellationToken,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        let proposal = self.call_model_with_context(compiled, cancellation).await?;
        let usage = self.token_usage.clone().unwrap_or_else(|| {
            TokenUsage::new(0, 0, 0, 0, m31a::model::types::UsageSource::Estimated)
        });
        Ok((proposal, usage))
    }

    async fn call_model_with_context_and_usage_streaming(
        &self,
        compiled: &m31a::kernel::seams::context::CompiledContext,
        cancellation: &tokio_util::sync::CancellationToken,
        _chunk_tx: Option<tokio::sync::mpsc::UnboundedSender<m31a::model::types::StreamChunk>>,
    ) -> Result<(ModelProposal, TokenUsage), String> {
        self.call_model_with_context_and_usage(compiled, cancellation)
            .await
    }
}

/// Deterministic model caller implementing prompt contracts for autonomous lifecycle verification.
#[derive(Debug, Default, Clone)]
pub struct DeterministicLifecycleModelCaller {
    pub fail_next: Arc<std::sync::atomic::AtomicBool>,
}

impl DeterministicLifecycleModelCaller {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn failing() -> Self {
        let caller = Self::default();
        caller
            .fail_next
            .store(true, std::sync::atomic::Ordering::SeqCst);
        caller
    }
}

#[async_trait]
impl ModelCaller for DeterministicLifecycleModelCaller {
    async fn call_model(&self, context: &str) -> Result<ModelProposal, String> {
        if self
            .fail_next
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err("Injected deterministic model failure".to_string());
        }

        if context.contains("Discovery Analyst")
            || context.contains("Dynamic Socratic Questioning")
            || context.contains("genesis.dynamic_questions")
            || context.contains("Dynamic Questions")
            || context.contains("Unresolved Unknowns")
            || context.contains("UNRESOLVED UNKNOWNS")
        {
            if context.contains("unk_arch_choice") {
                let questions = r#"[
                    {
                        "question_id": "q_unk_arch_choice",
                        "text": "What is the intended preference or architecture?",
                        "target_unknown": "unk_arch_choice",
                        "reason": "Application architecture and deployment boundaries are unspecified",
                        "options": ["Lightweight", "Full-stack"]
                    }
                ]"#;
                return Ok(ModelProposal::Complete {
                    summary: questions.to_string(),
                    artifacts: Vec::new(),
                });
            } else {
                return Ok(ModelProposal::Complete {
                    summary: "[]".to_string(),
                    artifacts: Vec::new(),
                });
            }
        }

        if context.contains("planning.revision") || context.contains("Lead Planner (Plan Revision)")
        {
            let revised = r#"{
                "tasks": [
                    {
                        "id": "TASK-01",
                        "title": "Implement requested functionality with revisions",
                        "description": "Deterministic revised task with user feedback incorporated",
                        "depends_on": [],
                        "role": "implementer",
                        "required_capabilities": ["fs.write"],
                        "verification": "compilation"
                    }
                ]
            }"#;
            return Ok(ModelProposal::Complete {
                summary: revised.to_string(),
                artifacts: Vec::new(),
            });
        }

        if context.contains("planning.task_revision")
            || context.contains("Task Decomposition Planner")
        {
            let tasks = r#"[
                {
                    "id": "TASK-01",
                    "objective": "Implement regenerated candidate task",
                    "description": "Deterministic regenerated candidate task",
                    "role": "implementer",
                    "capabilities": ["fs.write"],
                    "verification": "compilation",
                    "resource_estimate": { "estimated_tokens": 100, "estimated_duration_seconds": 10 }
                }
            ]"#;
            return Ok(ModelProposal::Complete {
                summary: tasks.to_string(),
                artifacts: Vec::new(),
            });
        }

        // Default: planning.decompose
        let decomp = r#"{
            "tasks": [
                {
                    "id": "TASK-01",
                    "title": "Execute mission objective",
                    "description": "Standard deterministic implementation task",
                    "depends_on": [],
                    "role": "implementer",
                    "required_capabilities": ["fs.write"],
                    "verification": "compilation"
                }
            ]
        }"#;
        Ok(ModelProposal::Complete {
            summary: decomp.to_string(),
            artifacts: Vec::new(),
        })
    }
}
