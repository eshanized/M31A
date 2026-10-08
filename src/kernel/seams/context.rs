use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::{AgentId, MissionId, TaskId};
use crate::model::types::ChatMessage;
use crate::prompt::PromptReference;
use crate::state_machine::agent::AgentRole;

/// Minimal contract entry for an individual context section within compilation manifest (D-16, CTX-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSectionContract {
    pub section_id: String,
    pub original_tokens: usize,
    pub final_tokens: usize,
    /// Compaction action: "retained_full", "compacted_excerpt", "compacted_summary", "dropped", "externalized"
    pub compaction_action: String,
    pub provenance: String,
}

impl ContextSectionContract {
    pub fn new(
        section_id: impl Into<String>,
        original_tokens: usize,
        final_tokens: usize,
        compaction_action: impl Into<String>,
        provenance: impl Into<String>,
    ) -> Self {
        Self {
            section_id: section_id.into(),
            original_tokens,
            final_tokens,
            compaction_action: compaction_action.into(),
            provenance: provenance.into(),
        }
    }
}

/// Contract record for selected evidence within the compiled context (D-16, CTX-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedEvidenceContract {
    pub id: String,
    pub source_path: String,
    pub origin: String,
    pub reason: String,
    pub slice_kind: String,
    pub token_count: usize,
    pub relevance_score: u32,
    pub provenance: String,
}

/// Contract record for candidate evidence that was omitted due to budget or priority limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmittedEvidenceContract {
    pub id: String,
    pub source_path: String,
    pub reason: String,
    pub relevance_score: u32,
}

/// Where the prompt reference used for a context compilation came from.
///
/// Explicit selection precedence (highest first):
/// `ExplicitTask` (workflow/task binding) > `ExplicitSession` (agent/session
/// binding) > `RoleDefault` (role registry default). There is no silent
/// fourth option: when no binding resolves, compilation fails closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptSelectionSource {
    /// Workflow step / task record carried an explicit `PromptReference`.
    ExplicitTask,
    /// Agent profile / session carried an explicit `PromptReference`.
    ExplicitSession,
    /// Fell back to the role registry default for the active role.
    RoleDefault,
}

impl PromptSelectionSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExplicitTask => "explicit_task",
            Self::ExplicitSession => "explicit_session",
            Self::RoleDefault => "role_default",
        }
    }
}

impl std::fmt::Display for PromptSelectionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Minimal auditable contract recording decisions made during context assembly and compaction (D-16, CTX-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextCompilationContract {
    pub revision: u32,
    pub total_budget: usize,
    pub reserved_headroom: usize,
    pub compiled_tokens: usize,
    pub sections: Vec<ContextSectionContract>,
    #[serde(default)]
    pub selected_evidence: Vec<SelectedEvidenceContract>,
    #[serde(default)]
    pub omitted_evidence: Vec<OmittedEvidenceContract>,
    #[serde(default)]
    pub repository_revision: Option<String>,
    #[serde(default)]
    pub selection_status: String,
    /// Prompt contract id actually compiled (never the requested alias when
    /// canonicalization upgraded it — this records the resolved contract).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_id: Option<String>,
    /// Prompt contract version actually compiled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_version: Option<u32>,
    /// Which binding won prompt selection precedence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_source: Option<PromptSelectionSource>,
    /// Content hash of the prompt contract actually compiled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_content_hash: Option<String>,
}

impl ContextCompilationContract {
    pub fn new(
        revision: u32,
        total_budget: usize,
        reserved_headroom: usize,
        compiled_tokens: usize,
        sections: Vec<ContextSectionContract>,
    ) -> Self {
        Self {
            revision,
            total_budget,
            reserved_headroom,
            compiled_tokens,
            sections,
            selected_evidence: Vec::new(),
            omitted_evidence: Vec::new(),
            repository_revision: None,
            selection_status: "complete".to_string(),
            prompt_id: None,
            prompt_version: None,
            prompt_source: None,
            prompt_content_hash: None,
        }
    }

    pub fn with_evidence(
        mut self,
        selected: Vec<SelectedEvidenceContract>,
        omitted: Vec<OmittedEvidenceContract>,
        repo_rev: Option<String>,
        status: impl Into<String>,
    ) -> Self {
        self.selected_evidence = selected;
        self.omitted_evidence = omitted;
        self.repository_revision = repo_rev;
        self.selection_status = status.into();
        self
    }
}

/// Record of an executed step action and its outcome for context compilation (D-02, D-05).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepRecordDto {
    pub step_number: u32,
    pub tool_name: String,
    pub parameters: serde_json::Value,
    pub success: bool,
    pub output: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextCompilationRequest {
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub max_tokens: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mission_objective: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_objective: Option<String>,
    /// Target-specific work description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_description: Option<String>,
    /// Task-specific completion criteria, rendered as the acceptance
    /// contract instead of generic filler.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_criteria: Vec<String>,
    /// Requirement keys this work satisfies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_requirement_keys: Vec<String>,
    /// Assumptions the work may rely on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_assumptions: Vec<String>,
    /// Upstream project charter markdown, when mission planning produced
    /// one. Rendered as compactable P2 evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_charter: Option<String>,
    /// Upstream target architecture markdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_architecture: Option<String>,
    /// Upstream requirement statements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upstream_requirements: Vec<String>,
    /// Upstream assumption statements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upstream_assumptions: Vec<String>,
    /// Upstream decision records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upstream_decisions: Vec<String>,
    /// Upstream research summary markdown, when research was conducted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_research_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub step_history: Vec<StepRecordDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<AgentRole>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_ref: Option<PromptReference>,
    /// Where the caller believes `prompt_ref` came from (task/workflow
    /// binding vs. session/profile binding). Recorded in the compilation
    /// manifest; the compiler trusts the caller-supplied classification
    /// and never re-derives it from prompt text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_source: Option<PromptSelectionSource>,
    /// Agent identity requesting compilation, carried into prompt provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_root: Option<std::path::PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_context: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub explicit_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub explicit_symbols: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_snapshot: Option<crate::kernel::memory::EngineeringMemorySnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interactive_messages: Vec<ChatMessage>,
}

impl ContextCompilationRequest {
    pub fn new(mission_id: MissionId, task_id: TaskId, max_tokens: usize) -> Self {
        Self {
            mission_id,
            task_id,
            max_tokens,
            mission_objective: None,
            task_objective: None,
            task_description: None,
            task_criteria: Vec::new(),
            task_requirement_keys: Vec::new(),
            task_assumptions: Vec::new(),
            upstream_charter: None,
            upstream_architecture: None,
            upstream_requirements: Vec::new(),
            upstream_assumptions: Vec::new(),
            upstream_decisions: Vec::new(),
            upstream_research_summary: None,
            step_history: Vec::new(),
            role: None,
            prompt_ref: None,
            prompt_source: None,
            agent_id: None,
            workspace_root: None,
            error_context: None,
            explicit_files: Vec::new(),
            explicit_symbols: Vec::new(),
            context_mode: None,
            memory_snapshot: None,
            interactive_messages: Vec::new(),
        }
    }

    pub fn with_interactive_messages(mut self, messages: Vec<ChatMessage>) -> Self {
        self.interactive_messages = messages;
        self
    }

    pub fn with_objectives(
        mut self,
        mission_obj: Option<String>,
        task_obj: Option<String>,
    ) -> Self {
        self.mission_objective = mission_obj;
        self.task_objective = task_obj;
        self
    }

    pub fn with_mission_objective(mut self, obj: impl Into<String>) -> Self {
        self.mission_objective = Some(obj.into());
        self
    }

    pub fn with_task_objective(mut self, obj: impl Into<String>) -> Self {
        self.task_objective = Some(obj.into());
        self
    }

    pub fn with_task_description(mut self, desc: impl Into<String>) -> Self {
        self.task_description = Some(desc.into());
        self
    }

    pub fn with_task_criteria(mut self, criteria: Vec<String>) -> Self {
        self.task_criteria = criteria;
        self
    }

    pub fn with_task_requirement_keys(mut self, keys: Vec<String>) -> Self {
        self.task_requirement_keys = keys;
        self
    }

    pub fn with_task_assumptions(mut self, assumptions: Vec<String>) -> Self {
        self.task_assumptions = assumptions;
        self
    }

    pub fn with_upstream_charter(mut self, charter: impl Into<String>) -> Self {
        self.upstream_charter = Some(charter.into());
        self
    }

    pub fn with_upstream_architecture(mut self, architecture: impl Into<String>) -> Self {
        self.upstream_architecture = Some(architecture.into());
        self
    }

    pub fn with_upstream_requirements(mut self, requirements: Vec<String>) -> Self {
        self.upstream_requirements = requirements;
        self
    }

    pub fn with_upstream_assumptions(mut self, assumptions: Vec<String>) -> Self {
        self.upstream_assumptions = assumptions;
        self
    }

    pub fn with_upstream_decisions(mut self, decisions: Vec<String>) -> Self {
        self.upstream_decisions = decisions;
        self
    }

    pub fn with_upstream_research_summary(mut self, summary: impl Into<String>) -> Self {
        self.upstream_research_summary = Some(summary.into());
        self
    }

    pub fn with_step_history(mut self, history: Vec<StepRecordDto>) -> Self {
        self.step_history = history;
        self
    }

    pub fn with_role(mut self, role: AgentRole) -> Self {
        self.role = Some(role);
        self
    }

    pub fn with_prompt_ref(mut self, prompt_ref: PromptReference) -> Self {
        self.prompt_ref = Some(prompt_ref);
        self
    }

    pub fn with_prompt_source(mut self, source: PromptSelectionSource) -> Self {
        self.prompt_source = Some(source);
        self
    }

    pub fn with_agent_id(mut self, agent_id: AgentId) -> Self {
        self.agent_id = Some(agent_id);
        self
    }

    pub fn with_workspace_root(mut self, root: impl Into<std::path::PathBuf>) -> Self {
        self.workspace_root = Some(root.into());
        self
    }

    pub fn with_error_context(mut self, error: impl Into<String>) -> Self {
        self.error_context = Some(error.into());
        self
    }

    pub fn with_explicit_files(mut self, files: Vec<String>) -> Self {
        self.explicit_files = files;
        self
    }

    pub fn with_explicit_symbols(mut self, symbols: Vec<String>) -> Self {
        self.explicit_symbols = symbols;
        self
    }

    pub fn with_context_mode(mut self, mode: impl Into<String>) -> Self {
        self.context_mode = Some(mode.into());
        self
    }

    pub fn with_memory_snapshot(
        mut self,
        snapshot: crate::kernel::memory::EngineeringMemorySnapshot,
    ) -> Self {
        self.memory_snapshot = Some(snapshot);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompiledContext {
    pub context_id: String,
    pub token_count: usize,
    pub system_prompt: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<ChatMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<ContextCompilationContract>,
    /// Invocation provenance of the EffectivePrompt actually compiled for
    /// this context. `Some` whenever context compilation resolved a prompt
    /// contract; the worker records it on the step so every model
    /// invocation carries auditable prompt provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_provenance: Option<crate::prompt::provenance::PromptInvocationProvenance>,
}

impl CompiledContext {
    pub fn new(
        context_id: impl Into<String>,
        token_count: usize,
        system_prompt: impl Into<String>,
    ) -> Self {
        Self {
            context_id: context_id.into(),
            token_count,
            system_prompt: system_prompt.into(),
            messages: Vec::new(),
            manifest: None,
            prompt_provenance: None,
        }
    }

    pub fn with_prompt_provenance(
        mut self,
        provenance: crate::prompt::provenance::PromptInvocationProvenance,
    ) -> Self {
        self.prompt_provenance = Some(provenance);
        self
    }

    pub fn with_messages(mut self, messages: Vec<ChatMessage>) -> Self {
        self.messages = messages;
        self
    }

    pub fn with_manifest(mut self, manifest: impl Into<ContextCompilationContract>) -> Self {
        self.manifest = Some(manifest.into());
        self
    }

    pub fn with_contract(mut self, contract: ContextCompilationContract) -> Self {
        self.manifest = Some(contract);
        self
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ContextError {
    #[error("context compilation error: {0}")]
    CompilationFailed(String),
}

#[async_trait]
pub trait ContextCompiler: Send + Sync {
    async fn compile_context(
        &self,
        req: ContextCompilationRequest,
    ) -> Result<CompiledContext, ContextError>;

    /// Shared prompt catalog backing this compiler, when the
    /// implementation is catalog-backed.
    ///
    /// Default: no catalog. Production compilers override this so
    /// downstream consumers (recovery diagnosticians, planners) can bind
    /// the SAME catalog instance instead of constructing divergent ones.
    fn prompt_catalog(&self) -> Option<&std::sync::Arc<dyn crate::prompt::PromptCatalog>> {
        None
    }

    /// Shared prompt compiler backing this compiler, when the
    /// implementation is compiler-backed.
    ///
    /// Default: no compiler. Production compilers override this so all
    /// production context generation shares one compilation authority.
    fn prompt_compiler(&self) -> Option<&std::sync::Arc<dyn crate::prompt::PromptCompiler>> {
        None
    }

    /// Bound engineering memory authority, when the implementation is
    /// memory-backed. Default: none. Production compilers override this so
    /// identity tests can prove worktree compilers share the canonical
    /// memory `Arc` (same SQLite pool, same long-horizon semantics).
    fn memory_store(&self) -> Option<std::sync::Arc<dyn crate::memory::EngineeringMemoryStore>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_dto_serde() {
        let req = ContextCompilationRequest::new(MissionId::new(), TaskId::new(), 8192);
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: ContextCompilationRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(req, deserialized);
    }
}
