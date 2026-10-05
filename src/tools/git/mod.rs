//! Git version control model-facing tools consuming GitService (TL-02, D-07).

use crate::capability::family::CapabilityFamily;
use crate::capability::traits::git::{GitBranchInfo, GitCommitInfo, GitService, GitStatusResult};
use crate::tools::definition::{ResourceLimits, ToolExecutionContext, TypedTool};
use crate::tools::error::ToolError;
use crate::tools::risk::RiskClass;
use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

fn get_git(ctx: &ToolExecutionContext) -> Result<Arc<dyn GitService>, ToolError> {
    ctx.capability_registry
        .git()
        .ok_or_else(|| ToolError::capability_unavailable("git", None))
}

/// Mint the scoped Git gate for a model-invoked git tool.
///
/// The tool pipeline already evaluated this exact tool call through the
/// canonical policy gate before `execute` runs; this binds that same
/// execution (mission / task / agent / live policy hash / workspace) to the
/// concrete Git operation about to run. Missing mission, task, or policy
/// hash fails closed — an unbound tool context can never mint mutation
/// authority.
fn pipeline_git_gate(
    ctx: &ToolExecutionContext,
    tool_id: &str,
    op: crate::git::GitOperation,
    additional: Vec<crate::git::GitOperation>,
) -> Result<crate::git::GitGate, ToolError> {
    use crate::git::{GIT_AUTH_DEFAULT_TTL, GitGate};
    let mission_id = ctx.mission_id.ok_or_else(|| {
        ToolError::permission_denied(
            "GIT_AUTHORIZATION_UNBOUND: git mutation requires a bound mission identity",
            Some("Tool context carries no mission; refusing unattributed mutation".to_string()),
        )
    })?;
    let task_id = ctx.task_id.ok_or_else(|| {
        ToolError::permission_denied(
            "GIT_AUTHORIZATION_UNBOUND: git mutation requires a bound task identity",
            Some("Tool context carries no task; refusing unattributed mutation".to_string()),
        )
    })?;
    let policy_hash = ctx.policy_hash.clone().ok_or_else(|| {
        ToolError::permission_denied(
            "GIT_AUTHORIZATION_UNBOUND: git mutation requires the live policy generation hash",
            Some(
                "Tool context carries no policy hash; refusing mutation under unknown policy"
                    .to_string(),
            ),
        )
    })?;
    let authority = ctx.capability_registry.authorization_authority().clone();
    let auth = authority.mint_git_authorization(
        mission_id,
        Some(task_id),
        ctx.agent_id,
        policy_hash,
        ctx.workspace_root.clone(),
        op,
        additional,
        format!("tool:{tool_id}"),
        format!("tool-pipeline-policy-decision:{tool_id}"),
        GIT_AUTH_DEFAULT_TTL,
    );
    GitGate::authorized_verified(auth, &authority).map_err(|e| {
        ToolError::permission_denied(
            format!("GIT_AUTHORIZATION_REJECTED: git authorization rejected: {e}"),
            None,
        )
    })
}

// ---------------------------------------------------------------------------
// 16. git_status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GitStatusInput {}

pub struct GitStatusTool;

#[async_trait]
impl TypedTool for GitStatusTool {
    type Input = GitStatusInput;
    type Output = GitStatusResult;

    fn id(&self) -> &str {
        "git_status"
    }

    fn description(&self) -> &str {
        "Retrieve repository working copy status (branch, staged, unstaged, untracked files)."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Git]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(15, 512 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let git = get_git(ctx)?;
        let status = git.status().await?;
        Ok(status)
    }
}

// ---------------------------------------------------------------------------
// 17. git_diff
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GitDiffInput {
    pub staged: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitDiffOutput {
    pub staged: bool,
    pub diff: String,
}

pub struct GitDiffTool;

#[async_trait]
impl TypedTool for GitDiffTool {
    type Input = GitDiffInput;
    type Output = GitDiffOutput;

    fn id(&self) -> &str {
        "git_diff"
    }

    fn description(&self) -> &str {
        "Compute repository diff for working tree or staged changes."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Git]
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
        let git = get_git(ctx)?;
        let staged = input.staged.unwrap_or(false);
        let diff = git.diff(staged).await?;
        Ok(GitDiffOutput { staged, diff })
    }
}

// ---------------------------------------------------------------------------
// 18. git_log
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GitLogInput {
    pub max_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitLogOutput {
    pub commits: Vec<GitCommitInfo>,
}

pub struct GitLogTool;

#[async_trait]
impl TypedTool for GitLogTool {
    type Input = GitLogInput;
    type Output = GitLogOutput;

    fn id(&self) -> &str {
        "git_log"
    }

    fn description(&self) -> &str {
        "Retrieve recent commit history up to max_count."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Git]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(15, 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let git = get_git(ctx)?;
        let count = input.max_count.unwrap_or(10);
        let commits = git.log(count).await?;
        Ok(GitLogOutput { commits })
    }
}

// ---------------------------------------------------------------------------
// 19. git_show
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitShowInput {
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitShowOutput {
    pub revision: String,
    pub content: String,
}

pub struct GitShowTool;

#[async_trait]
impl TypedTool for GitShowTool {
    type Input = GitShowInput;
    type Output = GitShowOutput;

    fn id(&self) -> &str {
        "git_show"
    }

    fn description(&self) -> &str {
        "Show commit details or object content at a specific git revision."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Git]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(15, 1024 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let git = get_git(ctx)?;
        let content = git.show(&input.revision).await?;
        Ok(GitShowOutput {
            revision: input.revision,
            content,
        })
    }
}

// ---------------------------------------------------------------------------
// 20. git_branch
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GitBranchInput {}

pub struct GitBranchTool;

#[async_trait]
impl TypedTool for GitBranchTool {
    type Input = GitBranchInput;
    type Output = GitBranchInfo;

    fn id(&self) -> &str {
        "git_branch"
    }

    fn description(&self) -> &str {
        "List all available git branches and indicate current branch."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Git]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(15, 512 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        _input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let git = get_git(ctx)?;
        let info = git.branch().await?;
        Ok(info)
    }
}

// ---------------------------------------------------------------------------
// 21. git_checkout
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCheckoutInput {
    pub branch_or_commit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCheckoutOutput {
    pub target: String,
    pub success: bool,
}

pub struct GitCheckoutTool;

#[async_trait]
impl TypedTool for GitCheckoutTool {
    type Input = GitCheckoutInput;
    type Output = GitCheckoutOutput;

    fn id(&self) -> &str {
        "git_checkout"
    }

    fn description(&self) -> &str {
        "Checkout a git branch or commit revision."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Git]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::HighRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 512 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let git = get_git(ctx)?;
        let gate = pipeline_git_gate(
            ctx,
            self.id(),
            crate::git::GitOperation::Checkout {
                target: input.branch_or_commit.clone(),
                force: false,
            },
            Vec::new(),
        )?;
        git.checkout(&input.branch_or_commit, &gate).await?;
        Ok(GitCheckoutOutput {
            target: input.branch_or_commit,
            success: true,
        })
    }
}

// ---------------------------------------------------------------------------
// 22. git_add
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitAddInput {
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitAddOutput {
    pub staged_count: usize,
    pub success: bool,
}

pub struct GitAddTool;

#[async_trait]
impl TypedTool for GitAddTool {
    type Input = GitAddInput;
    type Output = GitAddOutput;

    fn id(&self) -> &str {
        "git_add"
    }

    fn description(&self) -> &str {
        "Stage workspace files or directories for the next git commit."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Git]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::LowRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 512 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let git = get_git(ctx)?;
        let paths: Vec<PathBuf> = input.paths.into_iter().map(PathBuf::from).collect();
        let count = paths.len();
        let gate = pipeline_git_gate(
            ctx,
            self.id(),
            crate::git::GitOperation::Add {
                paths: paths.iter().map(|p| p.display().to_string()).collect(),
            },
            Vec::new(),
        )?;
        git.add(&paths, &gate).await?;

        Ok(GitAddOutput {
            staged_count: count,
            success: true,
        })
    }
}

// ---------------------------------------------------------------------------
// 23. git_commit
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCommitInput {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GitCommitOutput {
    pub commit_hash: String,
    pub message: String,
}

pub struct GitCommitTool;

#[async_trait]
impl TypedTool for GitCommitTool {
    type Input = GitCommitInput;
    type Output = GitCommitOutput;

    fn id(&self) -> &str {
        "git_commit"
    }

    fn description(&self) -> &str {
        "Create a new git commit with staged changes and commit message."
    }

    fn required_capabilities(&self) -> &[CapabilityFamily] {
        &[CapabilityFamily::Git]
    }

    fn base_risk(&self) -> RiskClass {
        RiskClass::HighRiskMutation
    }

    fn resource_limits(&self) -> ResourceLimits {
        ResourceLimits::new(30, 512 * 1024)
    }

    async fn execute(
        &self,
        ctx: &ToolExecutionContext,
        input: Self::Input,
    ) -> Result<Self::Output, ToolError> {
        let git = get_git(ctx)?;
        let gate = pipeline_git_gate(
            ctx,
            self.id(),
            crate::git::GitOperation::Commit {
                message: input.message.clone(),
            },
            Vec::new(),
        )?;
        let hash = git.commit(&input.message, &gate).await?;
        Ok(GitCommitOutput {
            commit_hash: hash,
            message: input.message,
        })
    }
}
