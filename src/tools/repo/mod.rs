//! Repository intelligence model-facing tools consuming RepositoryService (TL-02, D-07).

use crate::capability::family::CapabilityFamily;
use crate::capability::traits::repo::{CodeSearchResult, RepositoryService};
use crate::repo::types::RepositorySymbol;
use crate::tools::definition::{ResourceLimits, ToolExecutionContext, TypedTool};
use crate::tools::error::ToolError;
use crate::tools::risk::RiskClass;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

fn get_repo(ctx: &ToolExecutionContext) -> Result<Arc<dyn RepositoryService>, ToolError> {
    ctx.capability_registry
        .repository()
        .ok_or_else(|| ToolError::capability_unavailable("repository", None))
}

// ---------------------------------------------------------------------------
// 8. repo_search
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepoSearchInput {
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepoSearchOutput {
    pub query: String,
    pub results: Vec<CodeSearchResult>,
}

pub struct RepoSearchTool;

#[async_trait]
impl TypedTool for RepoSearchTool {
    type Input = RepoSearchInput;
    type Output = RepoSearchOutput;

    fn id(&self) -> &str {
        "repo_search"
    }

    fn description(&self) -> &str {
        "Search codebase text and comments for query terms across repository files."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Repository]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 2 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let repo = get_repo(ctx)?;
        let results = repo.search_code(&input.query).await?;
        Ok(RepoSearchOutput {
            query: input.query,
            results,
        })
    }
}

// ---------------------------------------------------------------------------
// 9. repo_symbols
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepoSymbolsInput {
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepoSymbolsOutput {
    pub query: String,
    pub symbols: Vec<RepositorySymbol>,
}

pub struct RepoSymbolsTool;

#[async_trait]
impl TypedTool for RepoSymbolsTool {
    type Input = RepoSymbolsInput;
    type Output = RepoSymbolsOutput;

    fn id(&self) -> &str {
        "repo_symbols"
    }

    fn description(&self) -> &str {
        "Query code symbols (functions, structs, traits, classes) by name or pattern."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Repository]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 2 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let repo = get_repo(ctx)?;
        let symbols = repo.query_symbols(&input.query).await?;
        Ok(RepoSymbolsOutput {
            query: input.query,
            symbols,
        })
    }
}

// ---------------------------------------------------------------------------
// 10. repo_dependencies
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepoDependenciesInput {
    pub symbol_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepoDependenciesOutput {
    pub symbol_id: String,
    pub dependencies: Vec<RepositorySymbol>,
}

pub struct RepoDependenciesTool;

#[async_trait]
impl TypedTool for RepoDependenciesTool {
    type Input = RepoDependenciesInput;
    type Output = RepoDependenciesOutput;

    fn id(&self) -> &str {
        "repo_dependencies"
    }

    fn description(&self) -> &str {
        "Find callers, callees, or dependencies associated with a repository symbol ID."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Repository]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 2 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let repo = get_repo(ctx)?;
        let dependencies = repo.query_dependencies(&input.symbol_id).await?;
        Ok(RepoDependenciesOutput {
            symbol_id: input.symbol_id,
            dependencies,
        })
    }
}

// ---------------------------------------------------------------------------
// 11. repo_impact
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepoImpactInput {
    pub changed_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepoImpactOutput {
    pub report: crate::repo::query::ChangeImpactReport,
}

pub struct RepoImpactTool;

#[async_trait]
impl TypedTool for RepoImpactTool {
    type Input = RepoImpactInput;
    type Output = RepoImpactOutput;

    fn id(&self) -> &str {
        "repo_impact"
    }

    fn description(&self) -> &str {
        "Analyze change impact, downstream callers, affected files, recommended tests, and architectural subsystems for proposed changes."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Repository]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 2 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let repo = get_repo(ctx)?;
        let report = repo.analyze_impact(&input.changed_files).await?;
        Ok(RepoImpactOutput { report })
    }
}

// ---------------------------------------------------------------------------
// 12. repo_overview
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct RepoOverviewInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepoOverviewOutput {
    pub status: crate::repo::types::IndexStatus,
}

pub struct RepoOverviewTool;

#[async_trait]
impl TypedTool for RepoOverviewTool {
    type Input = RepoOverviewInput;
    type Output = RepoOverviewOutput;

    fn id(&self) -> &str {
        "repo_overview"
    }

    fn description(&self) -> &str {
        "Retrieve repository intelligence index status, staleness, and overview."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Repository]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 2 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let repo = get_repo(ctx)?;
        let status = repo.get_overview().await?;
        Ok(RepoOverviewOutput { status })
    }
}

// ---------------------------------------------------------------------------
// LSP Tools (Issue 12)
// ---------------------------------------------------------------------------

use crate::repo::lsp::{HoverInfo, LspBackend, LspService, SourceLocation, SymbolInfo};

fn get_lsp(ctx: &ToolExecutionContext) -> Arc<LspService> {
    ctx.capability_registry
        .lsp()
        .unwrap_or_else(|| Arc::new(LspService::new(&ctx.workspace_root)))
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct LspGotoDefinitionInput {
    pub file_path: Option<String>,
    #[serde(default)]
    pub line: usize,
    #[serde(default)]
    pub column: usize,
    pub symbol_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LspGotoDefinitionOutput {
    pub locations: Vec<SourceLocation>,
    pub backend_used: LspBackend,
    pub fallback_used: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
}

pub struct LspGotoDefinitionTool;

#[async_trait]
impl TypedTool for LspGotoDefinitionTool {
    type Input = LspGotoDefinitionInput;
    type Output = LspGotoDefinitionOutput;

    fn id(&self) -> &str {
        "lsp_goto_definition"
    }

    fn description(&self) -> &str {
        "Find the definition of a symbol at a given file location or by symbol name."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Repository]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 2 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let service = get_lsp(ctx);
        let file_path = input.file_path.as_deref().unwrap_or("");
        let res = service
            .goto_definition(
                file_path,
                input.line,
                input.column,
                input.symbol_name.as_deref(),
            )
            .await
            .map_err(|e| ToolError::execution_failed(e, None, None))?;
        Ok(LspGotoDefinitionOutput {
            locations: res.data,
            backend_used: res.backend_used,
            fallback_used: res.fallback_used,
            degraded_reason: res.degraded_reason,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct LspFindReferencesInput {
    pub file_path: Option<String>,
    #[serde(default)]
    pub line: usize,
    #[serde(default)]
    pub column: usize,
    pub symbol_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LspFindReferencesOutput {
    pub references: Vec<SourceLocation>,
    pub backend_used: LspBackend,
    pub fallback_used: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
}

pub struct LspFindReferencesTool;

#[async_trait]
impl TypedTool for LspFindReferencesTool {
    type Input = LspFindReferencesInput;
    type Output = LspFindReferencesOutput;

    fn id(&self) -> &str {
        "lsp_find_references"
    }

    fn description(&self) -> &str {
        "Find references to a symbol across the workspace using LSP or code analysis."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Repository]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 2 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let service = get_lsp(ctx);
        let file_path = input.file_path.as_deref().unwrap_or("");
        let res = service
            .find_references(
                file_path,
                input.line,
                input.column,
                input.symbol_name.as_deref(),
            )
            .await
            .map_err(|e| ToolError::execution_failed(e, None, None))?;
        Ok(LspFindReferencesOutput {
            references: res.data,
            backend_used: res.backend_used,
            fallback_used: res.fallback_used,
            degraded_reason: res.degraded_reason,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
pub struct LspHoverInput {
    pub file_path: Option<String>,
    #[serde(default)]
    pub line: usize,
    #[serde(default)]
    pub column: usize,
    pub symbol_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LspHoverOutput {
    pub hover: Option<HoverInfo>,
    pub backend_used: LspBackend,
    pub fallback_used: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
}

pub struct LspHoverTool;

#[async_trait]
impl TypedTool for LspHoverTool {
    type Input = LspHoverInput;
    type Output = LspHoverOutput;

    fn id(&self) -> &str {
        "lsp_hover"
    }

    fn description(&self) -> &str {
        "Hover over a symbol at a given location or by name to inspect type signature and documentation."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Repository]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 2 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let service = get_lsp(ctx);
        let file_path = input.file_path.as_deref().unwrap_or("");
        let res = service
            .hover(
                file_path,
                input.line,
                input.column,
                input.symbol_name.as_deref(),
            )
            .await
            .map_err(|e| ToolError::execution_failed(e, None, None))?;
        Ok(LspHoverOutput {
            hover: res.data,
            backend_used: res.backend_used,
            fallback_used: res.fallback_used,
            degraded_reason: res.degraded_reason,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LspSymbolsInput {
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LspSymbolsOutput {
    pub symbols: Vec<SymbolInfo>,
    pub backend_used: LspBackend,
    pub fallback_used: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_reason: Option<String>,
}

pub struct LspSymbolsTool;

#[async_trait]
impl TypedTool for LspSymbolsTool {
    type Input = LspSymbolsInput;
    type Output = LspSymbolsOutput;

    fn id(&self) -> &str {
        "lsp_symbols"
    }

    fn description(&self) -> &str {
        "Query symbols across the workspace with fuzzy matching."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Repository]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 2 * 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let service = get_lsp(ctx);
        let res = service
            .workspace_symbols(&input.query)
            .await
            .map_err(|e| ToolError::execution_failed(e, None, None))?;
        Ok(LspSymbolsOutput {
            symbols: res.data,
            backend_used: res.backend_used,
            fallback_used: res.fallback_used,
            degraded_reason: res.degraded_reason,
        })
    }
}
