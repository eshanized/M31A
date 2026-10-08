//! Central tool catalog registry managing model-facing tools (TL-02, D-07).

use crate::capability::registry::CapabilityRegistry;
use crate::tools::artifact::{CreateArtifactTool, ReadArtifactTool};
use crate::tools::definition::{AnyTool, ToolAdapter, TypedTool};
use crate::tools::fs::{
    ApplyPatchTool, EditFileTool, GlobTool, GrepTool, ListFilesTool, ReadFileTool, WriteFileTool,
};
use crate::tools::git::{
    GitAddTool, GitBranchTool, GitCheckoutTool, GitCommitTool, GitDiffTool, GitLogTool,
    GitShowTool, GitStatusTool,
};
use crate::tools::process::{
    JobOutputTool, JobStatusTool, JobStopTool, RunCommandTool, StartJobTool,
};
use crate::tools::qa::{RunFormatterTool, RunLinterTool, RunTestsTool};
use crate::tools::repo::{
    LspFindReferencesTool, LspGotoDefinitionTool, LspHoverTool, LspSymbolsTool,
    RepoDependenciesTool, RepoImpactTool, RepoOverviewTool, RepoSearchTool, RepoSymbolsTool,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Central catalog maintaining registered model-facing tools (TL-02).
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn AnyTool>>,
    capabilities: Option<Arc<CapabilityRegistry>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create a new empty ToolRegistry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            capabilities: None,
        }
    }

    /// Register a typed tool, wrapping it in `ToolAdapter` for object safety.
    pub fn register<T: TypedTool + 'static>(&mut self, tool: T) {
        let adapter = ToolAdapter::new(tool);
        let id = adapter.id().to_string();
        self.tools.insert(id, Arc::new(adapter));
    }

    /// Register an object-safe `AnyTool` directly.
    pub fn register_any(&mut self, tool: Arc<dyn AnyTool>) {
        let id = tool.id().to_string();
        self.tools.insert(id, tool);
    }

    /// Retrieve a tool by its stable identifier.
    pub fn get(&self, id: &str) -> Option<Arc<dyn AnyTool>> {
        self.tools.get(id).cloned()
    }

    /// List all registered tools.
    pub fn list_tools(&self) -> Vec<Arc<dyn AnyTool>> {
        self.tools.values().cloned().collect()
    }

    /// Number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Access attached capability registry, if any.
    pub fn capabilities(&self) -> Option<&Arc<CapabilityRegistry>> {
        self.capabilities.as_ref()
    }

    /// Construct a default registry pre-loaded with all 28 core tools (TL-02, D-07).
    pub fn new_default(capabilities: Arc<CapabilityRegistry>) -> Self {
        let mut registry = Self::new();
        registry.capabilities = Some(capabilities);

        // 1. Filesystem tools (7)
        registry.register(ReadFileTool);
        registry.register(WriteFileTool);
        registry.register(EditFileTool);
        registry.register(ApplyPatchTool);
        registry.register(ListFilesTool);
        registry.register(GlobTool);
        registry.register(GrepTool);

        // 2. Repository tools (3)
        registry.register(RepoSearchTool);
        registry.register(RepoSymbolsTool);
        registry.register(RepoDependenciesTool);

        // 3. Process & Job tools (5)
        registry.register(RunCommandTool);
        registry.register(StartJobTool);
        registry.register(JobStatusTool);
        registry.register(JobOutputTool);
        registry.register(JobStopTool);

        // 4. Git tools (8)
        registry.register(GitStatusTool);
        registry.register(GitDiffTool);
        registry.register(GitLogTool);
        registry.register(GitShowTool);
        registry.register(GitBranchTool);
        registry.register(GitCheckoutTool);
        registry.register(GitAddTool);
        registry.register(GitCommitTool);

        // 5. Quality Assurance tools (3)
        registry.register(RunTestsTool);
        registry.register(RunFormatterTool);
        registry.register(RunLinterTool);

        // 6. Artifact tools (2)
        registry.register(CreateArtifactTool);
        registry.register(ReadArtifactTool);

        registry
    }

    /// Register skill discovery and inspection tools (`skills_list`, `skills_inspect`)
    /// and adaptive replanning tool (`adapt_strategy`).
    pub fn register_agentic_tools(&mut self) {
        self.register(crate::tools::skill::SkillsListTool);
        self.register(crate::tools::skill::SkillsInspectTool);
        self.register(crate::tools::definition::AdaptStrategyTool);
    }

    /// Register repository intelligence tools (`repo_impact`, `repo_overview`).
    pub fn register_repository_intelligence(&mut self) {
        self.register(RepoImpactTool);
        self.register(RepoOverviewTool);
    }

    /// Register web tools (`web_search`, `web_fetch`).
    pub fn register_web_tools(&mut self) {
        self.register(crate::tools::web::WebSearchTool);
        self.register(crate::tools::web::WebFetchTool);
    }

    /// Register first-class filesystem mutation tools (`create_directory`, `delete_file`, `move_file`, `rename_file`, `copy_file`).
    pub fn register_extended_filesystem_tools(&mut self) {
        self.register(crate::tools::fs::CreateDirectoryTool);
        self.register(crate::tools::fs::DeleteFileTool);
        self.register(crate::tools::fs::MoveFileTool);
        self.register(crate::tools::fs::RenameFileTool);
        self.register(crate::tools::fs::CopyFileTool);
        self.register(crate::tools::fs::ApplyWorkspacePatchTool);
    }

    /// Register LSP-aware code intelligence tools (`lsp_goto_definition`, `lsp_find_references`, `lsp_hover`, `lsp_symbols`).
    pub fn register_lsp_tools(&mut self) {
        self.register(LspGotoDefinitionTool);
        self.register(LspFindReferencesTool);
        self.register(LspHoverTool);
        self.register(LspSymbolsTool);
    }

    /// Register remote repository hosting / PR tools (`pr_create`, `pr_status`, `pr_comment`, `pr_list_checks`).
    pub fn register_pr_tools(&mut self) {
        self.register(crate::tools::git::PrCreateTool);
        self.register(crate::tools::git::PrStatusTool);
        self.register(crate::tools::git::PrCommentTool);
        self.register(crate::tools::git::PrListChecksTool);
    }

    /// Register multi-agent task delegation tool (`delegate_task`).
    pub fn register_delegation_tools(&mut self) {
        self.register(crate::tools::agent::DelegateTaskTool);
    }

    /// Register interactive terminal session tools.
    pub fn register_terminal_tools(&mut self) {
        self.register(crate::tools::terminal::TerminalStartSessionTool);
        self.register(crate::tools::terminal::TerminalWriteInputTool);
        self.register(crate::tools::terminal::TerminalReadStreamTool);
        self.register(crate::tools::terminal::TerminalResizeTool);
        self.register(crate::tools::terminal::TerminalSessionStatusTool);
        self.register(crate::tools::terminal::TerminalTerminateSessionTool);
    }

    /// Register all extended tools (filesystem, web, LSP, PR, delegation, terminal, etc.).
    pub fn register_extended_tools(&mut self) {
        self.register_extended_filesystem_tools();
        self.register_web_tools();
        self.register_lsp_tools();
        self.register_pr_tools();
        self.register_delegation_tools();
        self.register_terminal_tools();
    }
}
