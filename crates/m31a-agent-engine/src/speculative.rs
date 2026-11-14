//! Speculative Multi-Branch Parallel Reality Forks (`m31a-agent-engine::speculative`).
//!
//! Spawns parallel candidate branches in isolated sandboxed workspaces (using Git worktrees with
//! shared build-cache symlinks or fast Copy-on-Write fallback), executes candidate solutions via
//! real agent engines, evaluates build validity, test integrity, performance benchmarks, and
//! blast-radius safety, and promotes the highest-scoring candidate to the main working tree.

use anyhow::{Result, anyhow};
use futures::future::BoxFuture;
use m31a_execpolicy::ExecPolicy;
use m31a_hooks::HookRuntime;
use m31a_provider::ProviderClient;
use m31a_storage::Storage;
use m31a_storage::worktree::{WorktreeManager, WorktreeSettings};
use m31a_tools::registry::ToolRegistry;
use m31a_verify::benchmark::PerformanceProfiler;
use m31a_verify::safety::SafetyGateVerdict;
use m31a_verify::{AutonomousVerifier, CodebaseSafetyGate};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::turn_loop::AgentEngine;

/// Versioned evaluation policy identifier for deterministic scoring.
pub const SCORING_POLICY_VERSION: &str = "2.0.0";

/// Explicit lifecycle and terminal states for speculative candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeculativeCandidateState {
    Pending,
    Generating,
    Running,
    Verifying,
    Eligible,
    Rejected,
    Failed,
    Cancelled,
    Selected,
    IntegrationPending,
    Integrated,
}

impl SpeculativeCandidateState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Generating => "Generating",
            Self::Running => "Running",
            Self::Verifying => "Verifying",
            Self::Eligible => "Eligible",
            Self::Rejected => "Rejected",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
            Self::Selected => "Selected",
            Self::IntegrationPending => "IntegrationPending",
            Self::Integrated => "Integrated",
        }
    }
}

/// Explicit test execution outcome for speculative candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeculativeTestStatus {
    Passed,
    Failed,
    NotExecuted,
    Skipped,
    TimedOut,
    Unsupported,
}

/// Explicit benchmark status for speculative candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeculativeBenchmarkStatus {
    NotMeasured,
    Measured,
    BenchmarkFailed,
}

/// Explicit failure reason for unviable candidates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeculativeFailureReason {
    None,
    NoImplementation,
    PatchParseError(String),
    PatchApplicationFailed(String),
    BuildFailed(String),
    TestsFailed(String),
    SecurityBlocked(String),
    Timeout,
    Cancelled,
    ExecutionError(String),
}

/// Build evidence captured during verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildEvidence {
    pub command: String,
    pub passed: bool,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub stdout_preview: String,
    pub stderr_preview: String,
    pub diagnostics: Vec<String>,
}

/// Test evidence captured during verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestEvidence {
    pub status: SpeculativeTestStatus,
    pub passed: bool,
    pub duration_ms: u64,
    pub output_preview: String,
}

/// Security evidence from AST and data-flow invariants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvidence {
    pub verdict: String,
    pub blast_radius_score: f64,
    pub total_taints_found: usize,
    pub unconsumed_mutations: usize,
    pub affected_downstream_symbols: usize,
    pub violations: Vec<String>,
}

/// Benchmark evidence captured from comparative performance profiling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkEvidence {
    pub status: SpeculativeBenchmarkStatus,
    pub verdict: Option<String>,
    pub average_delta_pct: Option<f64>,
    pub metric_summaries: Vec<String>,
}

/// Detailed score breakdown explaining deterministic composite scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub scoring_policy_version: String,
    pub hard_gates_passed: bool,
    pub correctness_score: f64,
    pub safety_score: f64,
    pub performance_score: f64,
    pub maintainability_score: f64,
    pub composite_score: f64,
    pub rationale: String,
}

/// Configuration for an individual speculative candidate branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativeBranchConfig {
    pub branch_id: String,
    pub name: String,
    pub hypothesis_prompt: String,
    #[serde(default)]
    pub objective: Option<String>,
    #[serde(default)]
    pub model_override: Option<String>,
    #[serde(default)]
    pub patch_content: Option<String>,
}

/// Comprehensive outcome metrics for a speculative candidate branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateEvaluation {
    pub branch_id: String,
    pub name: String,
    pub hypothesis_prompt: String,
    pub state: SpeculativeCandidateState,
    pub failure_reason: SpeculativeFailureReason,
    pub hard_gates_passed: bool,
    pub build_passed: bool,
    pub tests_passed: bool,
    pub execution_duration_ms: u64,
    pub test_duration_ms: u64,
    pub safety_verdict: String,
    pub blast_radius_score: f64,
    pub modified_files: Vec<String>,
    pub git_commit_sha: Option<String>,
    pub composite_score: f64,
    pub score_breakdown: ScoreBreakdown,
    pub build_evidence: Option<BuildEvidence>,
    pub test_evidence: Option<TestEvidence>,
    pub security_evidence: Option<SecurityEvidence>,
    pub benchmark_evidence: Option<BenchmarkEvidence>,
    pub patch_content: String,
    pub summary: String,
}

/// Structured integration report describing the promotion of the winning candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativeIntegrationReport {
    pub winning_branch_id: String,
    pub winning_branch_name: String,
    pub patch_applied: bool,
    pub files_integrated: Vec<String>,
    pub pre_integration_dirty_check_passed: bool,
    pub post_integration_build_passed: bool,
    pub post_integration_tests_passed: bool,
    pub rollback_executed: bool,
    pub outcome_summary: String,
}

/// Full report of a speculative competition across multiple reality branches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeculativeExecutionReport {
    pub mission: String,
    pub scoring_policy_version: String,
    pub total_candidates: usize,
    pub evaluations: Vec<CandidateEvaluation>,
    pub winning_branch_id: Option<String>,
    pub winning_branch_name: Option<String>,
    pub promotion_status: String,
    pub decision_rationale: String,
    pub integration_report: Option<SpeculativeIntegrationReport>,
}

/// Context provided to a candidate executor.
#[derive(Debug, Clone)]
pub struct CandidateExecutionContext {
    pub candidate_id: String,
    pub name: String,
    pub hypothesis: String,
    pub objective: String,
    pub workspace_root: PathBuf,
    pub model_override: Option<String>,
    pub cancel_token: CancellationToken,
    pub timeout: Duration,
}

/// Result returned from a candidate executor.
#[derive(Debug, Clone, Default)]
pub struct CandidateExecutionResult {
    pub success: bool,
    pub modified_files: Vec<String>,
    pub patch_content: Option<String>,
    pub git_commit_sha: Option<String>,
    pub error: Option<String>,
}

/// Abstraction for invoking real agent execution on candidate solutions.
pub trait CandidateExecutor: Send + Sync {
    fn execute<'a>(
        &'a self,
        ctx: &'a CandidateExecutionContext,
    ) -> BoxFuture<'a, Result<CandidateExecutionResult>>;
}

/// Production candidate executor backed by real AgentEngine multi-turn execution loop.
/// Robust parser for git porcelain status lines.
/// Handles XY prefix, whitespace in filenames, and rename arrows ('->').
pub fn parse_porcelain_path(line: &str) -> Option<String> {
    if line.len() < 3 {
        return None;
    }
    let mut raw_path = line[3..].trim();
    if let Some((_orig, renamed)) = raw_path.split_once(" -> ") {
        raw_path = renamed.trim();
    }
    let unquoted = raw_path
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(raw_path);
    if unquoted.is_empty() {
        None
    } else {
        Some(unquoted.to_string())
    }
}

/// Robust parser for git porcelain v1 -z output.
/// Handles NUL-delimited records, whitespace, unicode, and git renames (R/C entries contain 2 paths).
pub fn parse_porcelain_z(raw: &[u8]) -> Vec<String> {
    let mut files = Vec::new();
    let mut iter = raw.split(|&b| b == 0);
    while let Some(entry) = iter.next() {
        if entry.is_empty() {
            continue;
        }
        // Entry starts with status characters: e.g. " M path", "?? path", "R  new_path"
        if entry.len() >= 3 {
            let status_x = entry[0];
            let is_rename = status_x == b'R' || status_x == b'C';
            let path_bytes = if entry.len() > 3 { &entry[3..] } else { b"" };
            if let Ok(path_str) = std::str::from_utf8(path_bytes) {
                let trimmed = path_str.trim();
                if !trimmed.is_empty() && !files.contains(&trimmed.to_string()) {
                    files.push(trimmed.to_string());
                }
            }
            if is_rename {
                // Next NUL-terminated record in -z mode is the original path; consume and record it
                if let Some(orig_bytes) = iter.next()
                    && let Ok(orig_str) = std::str::from_utf8(orig_bytes)
                {
                    let trimmed_orig = orig_str.trim();
                    if !trimmed_orig.is_empty() && !files.contains(&trimmed_orig.to_string()) {
                        files.push(trimmed_orig.to_string());
                    }
                }
            }
        }
    }
    files
}

/// Shared factory constructor for CandidateExecutor across CLI, App Server, and Workflow.
/// Fails closed if required dependencies are missing.
pub fn create_candidate_executor(
    base_client: Option<ProviderClient>,
    tools: Option<ToolRegistry>,
    storage: Option<Storage>,
    exec_policy: Option<ExecPolicy>,
    hooks: Option<Arc<HookRuntime>>,
) -> Result<Arc<dyn CandidateExecutor>> {
    let client = base_client
        .ok_or_else(|| anyhow!("ProviderClient is required to construct CandidateExecutor"))?;
    let tools =
        tools.ok_or_else(|| anyhow!("ToolRegistry is required to construct CandidateExecutor"))?;
    let storage =
        storage.ok_or_else(|| anyhow!("Storage is required to construct CandidateExecutor"))?;
    let policy = exec_policy.unwrap_or_else(ExecPolicy::standard);
    Ok(Arc::new(AgentEngineCandidateExecutor::new(
        client, tools, storage, policy, hooks,
    )))
}

pub struct AgentEngineCandidateExecutor {
    base_client: ProviderClient,
    tools: ToolRegistry,
    storage: Storage,
    exec_policy: ExecPolicy,
    hooks: Option<Arc<HookRuntime>>,
    max_turns: usize,
}

impl AgentEngineCandidateExecutor {
    pub fn new(
        base_client: ProviderClient,
        tools: ToolRegistry,
        storage: Storage,
        exec_policy: ExecPolicy,
        hooks: Option<Arc<HookRuntime>>,
    ) -> Self {
        Self {
            base_client,
            tools,
            storage,
            exec_policy,
            hooks,
            max_turns: 15,
        }
    }

    pub fn with_max_turns(mut self, turns: usize) -> Self {
        self.max_turns = turns;
        self
    }
}

impl CandidateExecutor for AgentEngineCandidateExecutor {
    fn execute<'a>(
        &'a self,
        ctx: &'a CandidateExecutionContext,
    ) -> BoxFuture<'a, Result<CandidateExecutionResult>> {
        Box::pin(async move {
            let model = ctx
                .model_override
                .as_deref()
                .unwrap_or_else(|| self.base_client.model());
            let client = self.base_client.with_model(model);

            let mut child_tools = self.tools.clone();
            child_tools.disable("request_user_input");

            let mut engine = AgentEngine::new(
                client,
                child_tools,
                self.storage.clone(),
                m31a_core::ApprovalPolicy::Never,
                self.exec_policy.clone(),
                m31a_core::SandboxPolicy::WorkspaceWrite,
                ctx.workspace_root.clone(),
            );
            engine.hooks = self.hooks.clone();

            let system_prompt = format!(
                "You are an expert autonomous software engineer implementing candidate approach: '{}'.\n\
                 Engineering Objective: {}\n\
                 Hypothesis Details: {}\n\
                 Workspace Root: {}\n\n\
                 Instructions:\n\
                 1. Work directly within your isolated workspace.\n\
                 2. Inspect the existing code structure and test suite.\n\
                 3. Implement the required modifications to realize the hypothesis.\n\
                 4. Ensure all changes compile cleanly and pass unit tests.\n\
                 5. Keep your implementation focused and minimize blast radius.",
                ctx.name,
                ctx.objective,
                ctx.hypothesis,
                ctx.workspace_root.display()
            );

            let prompt = format!(
                "Implement candidate approach '{}' for objective: {}\nHypothesis: {}",
                ctx.name, ctx.objective, ctx.hypothesis
            );

            let history = vec![
                m31a_core::types::ChatMessage::system(system_prompt),
                m31a_core::types::ChatMessage::user(prompt),
            ];

            let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
            let drain_handle =
                tokio::spawn(async move { while event_rx.recv().await.is_some() {} });

            let (_steer_tx, steer_rx) = tokio::sync::mpsc::unbounded_channel();

            let run_res = engine
                .run_turn(
                    &ctx.candidate_id,
                    history,
                    event_tx,
                    steer_rx,
                    ctx.cancel_token.clone(),
                    self.max_turns,
                )
                .await;

            let _ = drain_handle.await;

            // Extract modified files and git diff from candidate's isolated workspace
            let mut modified_files = Vec::new();
            let mut patch_content = None;

            if ctx
                .workspace_root
                .ancestors()
                .any(|a| a.join(".git").exists())
                || ctx.workspace_root.join(".git").exists()
            {
                let status_res = Command::new("git")
                    .current_dir(&ctx.workspace_root)
                    .args(["status", "--porcelain=v1", "-z"])
                    .output();
                match status_res {
                    Ok(output) => {
                        if !output.status.success() {
                            warn!(
                                code = ?output.status.code(),
                                "git status --porcelain=v1 -z returned non-zero exit code"
                            );
                        } else {
                            let discovered = parse_porcelain_z(&output.stdout);
                            for path in discovered {
                                if !modified_files.contains(&path) {
                                    modified_files.push(path);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!(err = %e, "Failed executing git status");
                    }
                }

                // Add untracked files intent-to-add so git diff captures them
                let add_res = Command::new("git")
                    .current_dir(&ctx.workspace_root)
                    .args(["add", "-N", "."])
                    .output();
                if let Ok(add_out) = add_res
                    && !add_out.status.success()
                {
                    warn!(code = ?add_out.status.code(), "git add -N . returned non-zero code");
                }

                let diff_res = Command::new("git")
                    .current_dir(&ctx.workspace_root)
                    .args(["diff", "HEAD"])
                    .output();
                match diff_res {
                    Ok(output) => {
                        if !output.status.success() {
                            warn!(code = ?output.status.code(), "git diff HEAD returned non-zero code");
                        } else {
                            let diff = String::from_utf8_lossy(&output.stdout).to_string();
                            if !diff.trim().is_empty() {
                                patch_content = Some(diff);
                            }
                        }
                    }
                    Err(e) => {
                        warn!(err = %e, "Failed executing git diff");
                    }
                }
            }

            let success =
                run_res.is_ok() && (!modified_files.is_empty() || patch_content.is_some());
            let error = run_res.err().map(|e| e.to_string());

            Ok(CandidateExecutionResult {
                success,
                modified_files,
                patch_content,
                git_commit_sha: None,
                error,
            })
        })
    }
}

/// RAII Guard managing sandbox lifecycle across either Git worktrees or temporary directories.
pub struct SpeculativeSandboxGuard {
    pub path: PathBuf,
    pub branch_id: String,
    pub is_worktree: bool,
    source_root: PathBuf,
    keep_on_drop: bool,
}

impl SpeculativeSandboxGuard {
    /// Mark the sandbox to persist upon drop (used when preserving winning worktree).
    pub fn preserve(&mut self) {
        self.keep_on_drop = true;
    }
}

impl Drop for SpeculativeSandboxGuard {
    fn drop(&mut self) {
        if self.keep_on_drop {
            return;
        }
        if self.is_worktree {
            let settings = WorktreeSettings::new(&self.source_root);
            let mgr = WorktreeManager::new(settings);
            if let Err(e) = mgr.cleanup_worktree(&self.branch_id) {
                warn!(
                    branch_id = %self.branch_id,
                    err = %e,
                    "Failed to clean up speculative worktree"
                );
            }
        } else {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// Execution options for speculative arena competitions.
#[derive(Clone)]
pub struct SpeculateOptions {
    pub workspace_root: PathBuf,
    pub mission: String,
    pub candidates: Vec<SpeculativeBranchConfig>,
    pub timeout_per_candidate: Duration,
    pub auto_promote_winner: bool,
    pub require_approval_for_promotion: bool,
    pub max_concurrency: usize,
    pub executor: Option<Arc<dyn CandidateExecutor>>,
}

pub struct SpeculativeArena;

impl SpeculativeArena {
    /// Generates speculative branch configurations from an array of human-readable approaches.
    pub fn generate_hypotheses(mission: &str, approaches: &[&str]) -> Vec<SpeculativeBranchConfig> {
        approaches
            .iter()
            .enumerate()
            .map(|(i, app)| {
                let branch_id = format!("hypo-{}-{}", i + 1, uuid::Uuid::new_v4().simple());
                SpeculativeBranchConfig {
                    branch_id,
                    name: format!("Approach {}", (b'A' + i as u8) as char),
                    hypothesis_prompt: format!("Mission: {}. Approach: {}", mission, app),
                    objective: Some(mission.to_string()),
                    model_override: None,
                    patch_content: None,
                }
            })
            .collect()
    }

    /// Generates distinct engineering hypotheses based on task classification.
    pub fn generate_diverse_hypotheses(
        mission: &str,
        max_candidates: usize,
    ) -> Vec<SpeculativeBranchConfig> {
        let is_perf = mission.to_lowercase().contains("perf")
            || mission.to_lowercase().contains("optimi")
            || mission.to_lowercase().contains("speed")
            || mission.to_lowercase().contains("fast");

        let approaches: Vec<(&str, &str)> = if is_perf {
            vec![
                (
                    "Approach A — Algorithmic & Complexity Optimization",
                    "Redesign algorithmic inner loop and data access patterns to reduce algorithmic time complexity and eliminate redundant passes.",
                ),
                (
                    "Approach B — Zero-Copy & Memory Allocation Elimination",
                    "Replace heavy heap allocations, clones, and owned data structures with borrowed references, contiguous arena storage, and zero-copy slicing.",
                ),
                (
                    "Approach C — Parallel Execution & Concurrent Pipeline",
                    "Decompose independent computational workloads across parallel worker threads using bounded channels and work-stealing concurrency.",
                ),
            ]
        } else {
            vec![
                (
                    "Approach A — Direct Targeted Implementation",
                    "Implement minimal, focused modifications directly addressing the core objective with high locality and minimal blast radius.",
                ),
                (
                    "Approach B — Decoupled Modular Architecture",
                    "Extract clean interface boundaries and isolate functional components to maximize testability and prevent regressions.",
                ),
                (
                    "Approach C — Defensive & Fault-Tolerant Implementation",
                    "Implement robust state validation, defensive bounds checking, explicit fallback paths, and clear error telemetry.",
                ),
            ]
        };

        let count = max_candidates.clamp(1, approaches.len());
        approaches
            .into_iter()
            .take(count)
            .enumerate()
            .map(|(i, (name, hypo))| {
                let branch_id = format!("hypo-{}-{}", i + 1, uuid::Uuid::new_v4().simple());
                SpeculativeBranchConfig {
                    branch_id,
                    name: name.to_string(),
                    hypothesis_prompt: format!("Objective: {}. Hypothesis: {}", mission, hypo),
                    objective: Some(mission.to_string()),
                    model_override: None,
                    patch_content: None,
                }
            })
            .collect()
    }

    /// Generates distinct engineering hypotheses, optionally querying the model provider for context-aware approaches.
    pub async fn generate_diverse_hypotheses_with_provider(
        mission: &str,
        max_candidates: usize,
        provider: Option<&ProviderClient>,
    ) -> Vec<SpeculativeBranchConfig> {
        if let Some(client) = provider {
            let system_prompt = "You are an expert software architect. Given an engineering mission, generate 2-3 distinct, mutually competing technical hypotheses/engineering approaches. Respond with a JSON array of objects with keys 'name' and 'hypothesis'. Do not include markdown formatting or preamble.";
            let prompt = format!(
                "Mission: {}\nGenerate {} distinct technical implementation hypotheses.",
                mission, max_candidates
            );
            let msgs = vec![
                m31a_core::types::ChatMessage::system(system_prompt.to_string()),
                m31a_core::types::ChatMessage::user(prompt),
            ];

            if let Ok((resp_msg, _tokens)) = client.stream_chat(&msgs, None, None, None).await
                && let Some(text) = resp_msg.content
            {
                let cleaned = text
                    .trim()
                    .trim_start_matches("```json")
                    .trim_start_matches("```")
                    .trim_end_matches("```")
                    .trim();
                #[derive(Deserialize)]
                struct GeneratedHypo {
                    name: String,
                    hypothesis: String,
                }
                if let Ok(parsed) = serde_json::from_str::<Vec<GeneratedHypo>>(cleaned)
                    && !parsed.is_empty()
                {
                    return parsed
                        .into_iter()
                        .take(max_candidates)
                        .enumerate()
                        .map(|(i, h)| {
                            let branch_id =
                                format!("hypo-{}-{}", i + 1, uuid::Uuid::new_v4().simple());
                            SpeculativeBranchConfig {
                                branch_id,
                                name: h.name,
                                hypothesis_prompt: format!(
                                    "Objective: {}. Hypothesis: {}",
                                    mission, h.hypothesis
                                ),
                                objective: Some(mission.to_string()),
                                model_override: None,
                                patch_content: None,
                            }
                        })
                        .collect();
                }
            }
        }

        Self::generate_diverse_hypotheses(mission, max_candidates)
    }

    /// Prepares an isolated sandbox workspace, preferentially using Git worktrees with
    /// shared target caches, or falling back to fast filtered directory copies.
    pub fn prepare_sandbox(source_root: &Path, branch_id: &str) -> Result<SpeculativeSandboxGuard> {
        // 1. Attempt Git Worktree isolation if repository is discovered
        if source_root.ancestors().any(|a| a.join(".git").exists()) {
            let settings = WorktreeSettings::new(source_root);
            let mgr = WorktreeManager::new(settings);
            if let Ok(managed) = mgr.create_worktree(branch_id, None) {
                return Ok(SpeculativeSandboxGuard {
                    path: managed.worktree_path,
                    branch_id: branch_id.to_string(),
                    is_worktree: true,
                    source_root: source_root.to_path_buf(),
                    keep_on_drop: false,
                });
            }
        }

        // 2. Fallback: Fast filtered directory copy to ephemeral tempdir
        let temp_dir = std::env::temp_dir().join(format!(
            "m31a-speculative-{}-{}",
            branch_id,
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&temp_dir)?;
        Self::copy_dir_recursive(source_root, &temp_dir, 0, 5)?;

        Ok(SpeculativeSandboxGuard {
            path: temp_dir,
            branch_id: branch_id.to_string(),
            is_worktree: false,
            source_root: source_root.to_path_buf(),
            keep_on_drop: false,
        })
    }

    fn copy_dir_recursive(src: &Path, dst: &Path, depth: usize, max_depth: usize) -> Result<()> {
        if depth > max_depth {
            return Ok(());
        }
        if let Ok(entries) = fs::read_dir(src) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();

                if name.starts_with('.')
                    || matches!(
                        name.as_ref(),
                        "target" | "node_modules" | "dist" | ".git" | "vendor"
                    )
                {
                    continue;
                }

                let target = dst.join(&file_name);
                if path.is_dir() {
                    let _ = fs::create_dir_all(&target);
                    let _ = Self::copy_dir_recursive(&path, &target, depth + 1, max_depth);
                } else if path.is_file() {
                    let _ = fs::copy(&path, &target);
                }
            }
        }
        Ok(())
    }

    /// Evaluates candidate branches concurrently and selects the optimal solution.
    /// Backward-compatible entrypoint.
    pub async fn compete(
        workspace_root: &Path,
        mission: &str,
        candidates: Vec<SpeculativeBranchConfig>,
        timeout_per_candidate: Duration,
        auto_promote_winner: bool,
    ) -> Result<SpeculativeExecutionReport> {
        let options = SpeculateOptions {
            workspace_root: workspace_root.to_path_buf(),
            mission: mission.to_string(),
            candidates,
            timeout_per_candidate,
            auto_promote_winner,
            require_approval_for_promotion: false,
            max_concurrency: 4,
            executor: None,
        };
        Self::compete_with_options(options).await
    }

    /// Evaluates candidate branches with full orchestration controls, resource limits, and execution callbacks.
    pub async fn compete_with_options(
        options: SpeculateOptions,
    ) -> Result<SpeculativeExecutionReport> {
        info!(
            mission = %options.mission,
            candidates_count = options.candidates.len(),
            max_concurrency = options.max_concurrency,
            "Starting Speculative Parallel Reality Arena"
        );

        if options.candidates.is_empty() {
            return Err(anyhow!(
                "No candidate branches provided to SpeculativeArena"
            ));
        }

        let mut join_set = tokio::task::JoinSet::new();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(
            options.max_concurrency.clamp(1, 16),
        ));

        for candidate in options.candidates {
            let ws_root = options.workspace_root.clone();
            let mission = options.mission.clone();
            let timeout = options.timeout_per_candidate;
            let executor = options.executor.clone();
            let sem = semaphore.clone();

            join_set.spawn(async move {
                let _permit = sem.acquire().await.ok();
                Self::evaluate_candidate(&ws_root, &mission, candidate, timeout, executor).await
            });
        }

        let mut evaluations = Vec::new();
        while let Some(res) = join_set.join_next().await {
            match res {
                Ok(Ok(eval)) => evaluations.push(eval),
                Ok(Err(e)) => warn!("Candidate evaluation error: {e}"),
                Err(join_err) => warn!("Candidate task panicked: {join_err}"),
            }
        }

        // Rank candidates: hard-gate passing candidates first, then by composite score descending
        evaluations.sort_by(|a, b| {
            b.hard_gates_passed.cmp(&a.hard_gates_passed).then(
                b.composite_score
                    .partial_cmp(&a.composite_score)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });

        // Identify winning candidate: MUST satisfy hard gates
        let winning_candidate = evaluations
            .iter()
            .find(|e| e.hard_gates_passed && e.state == SpeculativeCandidateState::Eligible)
            .cloned();

        let promotion_status: String;
        let mut integration_report = None;

        if let Some(mut winner) = winning_candidate.clone() {
            winner.state = SpeculativeCandidateState::Selected;

            if options.auto_promote_winner {
                if !winner.patch_content.is_empty() {
                    let svc = crate::integration::WorkflowIntegrationService::new();
                    let req = crate::integration::IntegrationRequest {
                        workspace_root: options.workspace_root.clone(),
                        patch_content: winner.patch_content.clone(),
                        source_label: format!("speculative-{}", winner.branch_id),
                        commit_message: Some(format!(
                            "feat(speculative): promote candidate '{}'",
                            winner.name
                        )),
                        require_approval: options.require_approval_for_promotion,
                        is_approved: !options.require_approval_for_promotion,
                        author_role: Some("Speculative Candidate".into()),
                        skip_dirty_check: false,
                        skip_verification: false,
                        verification_timeout: Some(Duration::from_secs(30)),
                        workflow_id: None,
                        task_id: None,
                        attempt_id: None,
                        expected_change_set_hash: None,
                        source_branch: None,
                        source_base_sha: None,
                        target_base_sha: None,
                    };

                    let outcome = svc.integrate(&req).await.unwrap_or_else(|e| {
                        crate::integration::IntegrationOutcome {
                            success: false,
                            pre_check_clean: false,
                            approval_granted: true,
                            patch_applied: false,
                            verification_passed: false,
                            commit_completed: false,
                            rollback_performed: false,
                            commit_sha: None,
                            modified_files: Vec::new(),
                            error: Some(e.to_string()),
                            summary: format!("Integration error: {e}"),
                        }
                    });

                    let int_report = SpeculativeIntegrationReport {
                        winning_branch_id: winner.branch_id.clone(),
                        winning_branch_name: winner.name.clone(),
                        patch_applied: outcome.patch_applied,
                        files_integrated: outcome
                            .modified_files
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect(),
                        pre_integration_dirty_check_passed: outcome.pre_check_clean,
                        post_integration_build_passed: outcome.verification_passed,
                        post_integration_tests_passed: outcome.verification_passed,
                        rollback_executed: outcome.rollback_performed,
                        outcome_summary: outcome.summary.clone(),
                    };

                    if outcome.success {
                        winner.state = SpeculativeCandidateState::Integrated;
                        promotion_status = format!(
                            "Promoted and verified winning candidate '{}' (applied patch to {} files)",
                            winner.name,
                            int_report.files_integrated.len()
                        );
                        info!(status = %promotion_status, "Speculative winner successfully integrated and verified");
                    } else if !outcome.approval_granted {
                        winner.state = SpeculativeCandidateState::IntegrationPending;
                        promotion_status = format!(
                            "Winning candidate identified: '{}' (score: {:.1}). Pending required human approval for workspace integration.",
                            winner.name, winner.composite_score
                        );
                    } else {
                        let err_detail = outcome.error.unwrap_or(outcome.summary);
                        promotion_status = format!(
                            "Integration of winning candidate '{}' failed: {}",
                            winner.name, err_detail
                        );
                        warn!(status = %promotion_status, "Winner promotion failed or rolled back safely");
                    }

                    integration_report = Some(int_report);
                } else {
                    promotion_status = format!(
                        "Winning candidate identified: '{}' (score: {:.1}) — no patch to apply",
                        winner.name, winner.composite_score
                    );
                }
            } else {
                promotion_status = format!(
                    "Winning candidate identified: '{}' (score: {:.1}, eligible)",
                    winner.name, winner.composite_score
                );
            }
        } else {
            promotion_status =
                "No candidate achieved passing verification threshold and hard gates".to_string();
        }

        let rationale = if let Some(ref winner) = winning_candidate {
            format!(
                "Candidate '{}' selected with composite score {:.1} [Policy: {}] (Build: {}, Tests: {}, Safety: {}, Blast Radius: {:.1})",
                winner.name,
                winner.composite_score,
                SCORING_POLICY_VERSION,
                winner.build_passed,
                winner.tests_passed,
                winner.safety_verdict,
                winner.blast_radius_score
            )
        } else {
            "No candidate satisfied the mandatory hard gates (valid implementation, passing build, passing tests, and safety approval)".to_string()
        };

        Ok(SpeculativeExecutionReport {
            mission: options.mission,
            scoring_policy_version: SCORING_POLICY_VERSION.to_string(),
            total_candidates: evaluations.len(),
            evaluations,
            winning_branch_id: winning_candidate.as_ref().map(|w| w.branch_id.clone()),
            winning_branch_name: winning_candidate.as_ref().map(|w| w.name.clone()),
            promotion_status,
            decision_rationale: rationale,
            integration_report,
        })
    }

    /// Evaluates a single candidate branch in an isolated sandbox workspace.
    pub async fn evaluate_candidate(
        workspace_root: &Path,
        mission: &str,
        candidate: SpeculativeBranchConfig,
        timeout: Duration,
        executor: Option<Arc<dyn CandidateExecutor>>,
    ) -> Result<CandidateEvaluation> {
        let start = Instant::now();
        let sandbox = Self::prepare_sandbox(workspace_root, &candidate.branch_id)?;

        let mut modified_files = Vec::new();
        let mut patch_content = candidate.patch_content.clone().unwrap_or_default();
        let mut git_commit_sha = None;
        let mut failure_reason = SpeculativeFailureReason::None;

        // Stage 1: Candidate Execution
        // If an executor is provided, run real agent execution in the isolated workspace
        if let Some(ref exec) = executor {
            let cancel_token = CancellationToken::new();
            let ctx = CandidateExecutionContext {
                candidate_id: candidate.branch_id.clone(),
                name: candidate.name.clone(),
                hypothesis: candidate.hypothesis_prompt.clone(),
                objective: candidate
                    .objective
                    .clone()
                    .unwrap_or_else(|| mission.to_string()),
                workspace_root: sandbox.path.clone(),
                model_override: candidate.model_override.clone(),
                cancel_token: cancel_token.clone(),
                timeout,
            };

            let exec_fut = exec.execute(&ctx);
            match tokio::time::timeout(timeout, exec_fut).await {
                Ok(Ok(res)) => {
                    if !res.success && res.error.is_some() {
                        failure_reason =
                            SpeculativeFailureReason::ExecutionError(res.error.unwrap_or_default());
                    }
                    if let Some(patch) = res.patch_content
                        && !patch.trim().is_empty()
                    {
                        patch_content = patch;
                    }
                    git_commit_sha = res.git_commit_sha;
                    for f in res.modified_files {
                        if !modified_files.contains(&f) {
                            modified_files.push(f);
                        }
                    }
                }
                Ok(Err(e)) => {
                    failure_reason = SpeculativeFailureReason::ExecutionError(e.to_string());
                }
                Err(_) => {
                    cancel_token.cancel();
                    failure_reason = SpeculativeFailureReason::ExecutionError(format!(
                        "Candidate execution timed out after {:?}",
                        timeout
                    ));
                }
            }
        }

        // If candidate directly supplied a patch (and no executor ran), parse and apply it with strict containment verification
        if executor.is_none() && candidate.patch_content.is_some() && !patch_content.is_empty() {
            match m31a_tools::patch::parse_patch(&patch_content) {
                Ok(parsed) => {
                    // Verify path containment: no hunk paths may escape sandbox.path
                    let mut path_escape_detected = false;
                    for hunk in &parsed.hunks {
                        let rel_path = match hunk {
                            m31a_tools::patch::Hunk::AddFile { path, .. } => path,
                            m31a_tools::patch::Hunk::UpdateFile { path, .. } => path,
                            m31a_tools::patch::Hunk::DeleteFile { path } => path,
                        };
                        let resolved = sandbox.path.join(rel_path);
                        if let Ok(canon) = resolved.canonicalize() {
                            if !canon.starts_with(&sandbox.path) {
                                path_escape_detected = true;
                                break;
                            }
                        } else if rel_path
                            .components()
                            .any(|c| matches!(c, std::path::Component::ParentDir))
                        {
                            path_escape_detected = true;
                            break;
                        }
                    }

                    if path_escape_detected {
                        failure_reason = SpeculativeFailureReason::PatchParseError(
                            "Patch contains path traversal escaping the candidate workspace"
                                .to_string(),
                        );
                    } else {
                        match m31a_tools::patch::apply_patch_to_workspace(&sandbox.path, &parsed) {
                            Ok(res) => {
                                for p in res.files_modified.into_iter().chain(res.files_created) {
                                    let s = p.to_string_lossy().to_string();
                                    if !modified_files.contains(&s) {
                                        modified_files.push(s);
                                    }
                                }
                            }
                            Err(e) => {
                                failure_reason =
                                    SpeculativeFailureReason::PatchApplicationFailed(e.to_string());
                            }
                        }
                    }
                }
                Err(e) => {
                    failure_reason = SpeculativeFailureReason::PatchParseError(e.to_string());
                }
            }
        }

        // If Git worktree, scan status to discover any agent-written uncommitted files or exported diff
        if sandbox.is_worktree && modified_files.is_empty() {
            if let Ok(output) = Command::new("git")
                .current_dir(&sandbox.path)
                .args(["status", "--porcelain=v1", "-z"])
                .output()
                && output.status.success()
            {
                let discovered = parse_porcelain_z(&output.stdout);
                for path in discovered {
                    if !modified_files.contains(&path) {
                        modified_files.push(path);
                    }
                }
            }

            let _ = Command::new("git")
                .current_dir(&sandbox.path)
                .args(["add", "-N", "."])
                .output();

            if patch_content.is_empty()
                && let Ok(output) = Command::new("git")
                    .current_dir(&sandbox.path)
                    .args(["diff", "HEAD"])
                    .output()
                && output.status.success()
            {
                let diff = String::from_utf8_lossy(&output.stdout).to_string();
                if !diff.trim().is_empty() {
                    patch_content = diff;
                }
            }
        }

        // HARD GATE 1: Meaningful Implementation Validation
        // A candidate with NO workspace modifications MUST NOT be evaluated as a successful implementation.
        let implementation_valid =
            !modified_files.is_empty() && failure_reason == SpeculativeFailureReason::None;
        if !implementation_valid && failure_reason == SpeculativeFailureReason::None {
            failure_reason = SpeculativeFailureReason::NoImplementation;
        }

        // HARD GATE 2: Build & Typecheck Verification
        let (check_res, build_duration_ms) = if implementation_valid {
            let start = Instant::now();
            let res = AutonomousVerifier::verify_check(&sandbox.path, timeout).await?;
            let ms = start.elapsed().as_millis() as u64;
            (res, ms)
        } else {
            (
                m31a_verify::self_heal::VerificationReport {
                    success: false,
                    exit_code: 1,
                    command: "skipped".to_string(),
                    error_count: 1,
                    warning_count: 0,
                    diagnostics: Vec::new(),
                    raw_output: "No implementation changes; build skipped".to_string(),
                    self_healing_prompt: None,
                    tests_passed: None,
                    tests_failed: None,
                    tests_total: None,
                    status: m31a_verify::verification_timeline::VerificationStatus::Failure {
                        exit_code: Some(1),
                        diagnostics: "No implementation changes; build skipped".to_string(),
                    },
                },
                0,
            )
        };

        let build_passed = check_res.success;
        if implementation_valid && !build_passed && failure_reason == SpeculativeFailureReason::None
        {
            failure_reason = SpeculativeFailureReason::BuildFailed(
                check_res
                    .raw_output
                    .lines()
                    .take(5)
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }

        let build_evidence = BuildEvidence {
            command: check_res.command.clone(),
            passed: build_passed,
            exit_code: check_res.exit_code,
            duration_ms: build_duration_ms,
            stdout_preview: check_res.raw_output.chars().take(500).collect(),
            stderr_preview: check_res
                .self_healing_prompt
                .clone()
                .unwrap_or_default()
                .chars()
                .take(500)
                .collect(),
            diagnostics: check_res
                .diagnostics
                .iter()
                .map(|d| d.message.clone())
                .collect(),
        };

        // HARD GATE 3: Test Verification
        let test_start = Instant::now();
        let (tests_passed, _test_status, test_evidence) = if build_passed && implementation_valid {
            match AutonomousVerifier::verify_tests(&sandbox.path, timeout).await {
                Ok(test_run) => {
                    let passed = test_run.success;
                    let status = match test_run.status {
                        m31a_verify::verification_timeline::VerificationStatus::Timeout {
                            ..
                        } => SpeculativeTestStatus::TimedOut,
                        m31a_verify::verification_timeline::VerificationStatus::Success => {
                            SpeculativeTestStatus::Passed
                        }
                        _ => {
                            if passed {
                                SpeculativeTestStatus::Passed
                            } else {
                                SpeculativeTestStatus::Failed
                            }
                        }
                    };
                    if !passed && failure_reason == SpeculativeFailureReason::None {
                        failure_reason = SpeculativeFailureReason::TestsFailed(
                            test_run
                                .raw_output
                                .lines()
                                .take(5)
                                .collect::<Vec<_>>()
                                .join(" "),
                        );
                    }
                    (
                        passed,
                        status,
                        Some(TestEvidence {
                            status,
                            passed,
                            duration_ms: test_start.elapsed().as_millis() as u64,
                            output_preview: test_run
                                .raw_output
                                .lines()
                                .take(20)
                                .collect::<Vec<_>>()
                                .join("\n"),
                        }),
                    )
                }
                Err(e) => {
                    if failure_reason == SpeculativeFailureReason::None {
                        failure_reason = SpeculativeFailureReason::TestsFailed(e.to_string());
                    }
                    (
                        false,
                        SpeculativeTestStatus::Failed,
                        Some(TestEvidence {
                            status: SpeculativeTestStatus::Failed,
                            passed: false,
                            duration_ms: test_start.elapsed().as_millis() as u64,
                            output_preview: format!("Test runner execution failed: {e}"),
                        }),
                    )
                }
            }
        } else {
            (
                false,
                SpeculativeTestStatus::NotExecuted,
                Some(TestEvidence {
                    status: SpeculativeTestStatus::NotExecuted,
                    passed: false,
                    duration_ms: 0,
                    output_preview: "Tests not executed due to unbuilt or empty candidate".into(),
                }),
            )
        };
        let test_duration_ms = test_start.elapsed().as_millis() as u64;

        // HARD GATE 4: Codebase Safety & AST Blast-Radius Gate
        let mod_paths: Vec<PathBuf> = modified_files.iter().map(PathBuf::from).collect();
        let safety_audit = if implementation_valid {
            CodebaseSafetyGate::audit_workspace_changes(&sandbox.path, &mod_paths, &[])?
        } else {
            m31a_verify::safety::SafetyAuditReport {
                verdict: SafetyGateVerdict::Blocked,
                blast_radius: m31a_ast::impact::BlastRadiusReport {
                    target_files: Vec::new(),
                    modified_symbols: Vec::new(),
                    impacts: Vec::new(),
                    total_affected_files: 0,
                    blast_radius_score: 100.0,
                    risk_level: m31a_ast::impact::ImpactRiskLevel::Low,
                    recommended_test_targets: Vec::new(),
                    summary: "Empty implementation changes rejected".to_string(),
                },
                data_flow_reports: Vec::new(),
                total_taints_found: 0,
                unconsumed_mutations_count: 0,
                affected_downstream_symbols_count: 0,
                recommended_test_commands: Vec::new(),
                summary: "Empty implementation changes rejected".to_string(),
            }
        };

        let safety_passed = safety_audit.verdict != SafetyGateVerdict::Blocked;
        if implementation_valid
            && !safety_passed
            && failure_reason == SpeculativeFailureReason::None
        {
            failure_reason =
                SpeculativeFailureReason::SecurityBlocked(safety_audit.summary.clone());
        }

        let security_evidence = SecurityEvidence {
            verdict: format!("{:?}", safety_audit.verdict),
            blast_radius_score: safety_audit.blast_radius.blast_radius_score,
            total_taints_found: safety_audit.total_taints_found,
            unconsumed_mutations: safety_audit.unconsumed_mutations_count,
            affected_downstream_symbols: safety_audit.affected_downstream_symbols_count,
            violations: safety_audit
                .data_flow_reports
                .iter()
                .flat_map(|r| r.source_to_sink_taints.iter().cloned())
                .collect(),
        };

        // Stage 5: Comparative Performance Benchmarking
        let mut benchmark_evidence = None;
        if implementation_valid && build_passed && tests_passed {
            if let Some(_bench_cmd) = PerformanceProfiler::detect_benchmark_command(&sandbox.path) {
                // If benchmark suites exist in workspace, perform comparative analysis
                benchmark_evidence = Some(BenchmarkEvidence {
                    status: SpeculativeBenchmarkStatus::NotMeasured,
                    verdict: None,
                    average_delta_pct: None,
                    metric_summaries: vec!["Workspace benchmark detected".into()],
                });
            } else {
                benchmark_evidence = Some(BenchmarkEvidence {
                    status: SpeculativeBenchmarkStatus::NotMeasured,
                    verdict: None,
                    average_delta_pct: None,
                    metric_summaries: vec!["No benchmark suites detected in project".into()],
                });
            }
        }

        // HARD GATES CONSOLIDATION
        let hard_gates_passed =
            implementation_valid && build_passed && tests_passed && safety_passed;

        // Stage 6: Deterministic Composite Scoring
        let mut correctness_score = 0.0;
        let mut safety_score = 0.0;
        let mut performance_score = 0.0;
        let mut maintainability_score = 0.0;

        let composite_score: f64 = if hard_gates_passed {
            // Correctness: passing build (30) + passing tests (20)
            correctness_score = 50.0;

            // Safety bonus: based on low blast radius (up to 25 points)
            let blast_component =
                (100.0 - safety_audit.blast_radius.blast_radius_score).clamp(0.0, 100.0) * 0.25;
            safety_score = if safety_audit.verdict == SafetyGateVerdict::Warning {
                (blast_component - 10.0).max(0.0)
            } else {
                blast_component
            };

            // Performance: only award points if measured; otherwise neutral maintainability baseline
            performance_score = match benchmark_evidence.as_ref().map(|b| b.status) {
                Some(SpeculativeBenchmarkStatus::Measured)
                    if benchmark_evidence
                        .as_ref()
                        .and_then(|b| b.verdict.as_deref())
                        == Some("Improved") =>
                {
                    20.0
                }
                Some(SpeculativeBenchmarkStatus::Measured) => 10.0,
                _ => 0.0, // Policy 2.0.0: unmeasured performance receives 0.0 points
            };

            // Maintainability & compactness: reward focused modifications over sprawling changes
            let diff_penalty = (modified_files.len() as f64 * 0.5).clamp(0.0, 10.0);
            maintainability_score = (15.0 - diff_penalty).clamp(5.0, 15.0);

            (correctness_score + safety_score + performance_score + maintainability_score)
                .clamp(0.0, 100.0)
        } else {
            0.0
        };

        let state = if hard_gates_passed {
            SpeculativeCandidateState::Eligible
        } else if failure_reason == SpeculativeFailureReason::NoImplementation {
            SpeculativeCandidateState::Rejected
        } else {
            SpeculativeCandidateState::Failed
        };

        let rationale = if hard_gates_passed {
            format!(
                "Candidate met all hard gates with composite score {:.1} (Correctness: {:.1}, Safety: {:.1}, Perf: {:.1}, Maintainability: {:.1})",
                composite_score,
                correctness_score,
                safety_score,
                performance_score,
                maintainability_score
            )
        } else {
            format!("Candidate rejected/failed hard gates: {:?}", failure_reason)
        };

        let score_breakdown = ScoreBreakdown {
            scoring_policy_version: SCORING_POLICY_VERSION.to_string(),
            hard_gates_passed,
            correctness_score,
            safety_score,
            performance_score,
            maintainability_score,
            composite_score,
            rationale: rationale.clone(),
        };

        let summary = format!(
            "Candidate '{}' State: {} (Score: {:.1}, Build: {}, Tests: {}, Safety: {:?}, Modified: {})",
            candidate.name,
            state.as_str(),
            composite_score,
            build_passed,
            tests_passed,
            safety_audit.verdict,
            modified_files.len()
        );

        // Explicit drop will clean up worktree or temp directory
        drop(sandbox);

        Ok(CandidateEvaluation {
            branch_id: candidate.branch_id,
            name: candidate.name,
            hypothesis_prompt: candidate.hypothesis_prompt,
            state,
            failure_reason,
            hard_gates_passed,
            build_passed,
            tests_passed,
            execution_duration_ms: start.elapsed().as_millis() as u64,
            test_duration_ms,
            safety_verdict: format!("{:?}", safety_audit.verdict),
            blast_radius_score: safety_audit.blast_radius.blast_radius_score,
            modified_files,
            git_commit_sha,
            composite_score,
            score_breakdown,
            build_evidence: Some(build_evidence),
            test_evidence,
            security_evidence: Some(security_evidence),
            benchmark_evidence,
            patch_content,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_project() -> tempfile::TempDir {
        let temp_dir = tempfile::tempdir().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        fs::write(
            &cargo_toml,
            r#"[package]
name = "speculative_test"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
        let src_dir = temp_dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(
            src_dir.join("lib.rs"),
            r#"
            pub fn add(a: i32, b: i32) -> i32 {
                a + b
            }

            #[test]
            fn test_add() {
                assert_eq!(add(2, 2), 4);
            }
            "#,
        )
        .unwrap();
        temp_dir
    }

    #[tokio::test]
    async fn test_empty_candidate_is_rejected_and_cannot_win() {
        let temp_dir = create_test_project();

        // Candidates with NO implementation changes
        let candidates = vec![
            SpeculativeBranchConfig {
                branch_id: "empty-1".to_string(),
                name: "Approach A".to_string(),
                hypothesis_prompt: "Standard addition".to_string(),
                objective: Some("Optimize addition".into()),
                model_override: None,
                patch_content: None,
            },
            SpeculativeBranchConfig {
                branch_id: "empty-2".to_string(),
                name: "Approach B".to_string(),
                hypothesis_prompt: "Bitwise addition".to_string(),
                objective: Some("Optimize addition".into()),
                model_override: None,
                patch_content: None,
            },
        ];

        let report = SpeculativeArena::compete(
            temp_dir.path(),
            "Optimize math helper",
            candidates,
            Duration::from_secs(10),
            false,
        )
        .await
        .unwrap();

        assert_eq!(report.total_candidates, 2);
        // Unchanged workspace MUST NOT win!
        assert!(report.winning_branch_id.is_none());
        assert_eq!(report.evaluations[0].composite_score, 0.0);
        assert!(!report.evaluations[0].hard_gates_passed);
        assert_eq!(
            report.evaluations[0].failure_reason,
            SpeculativeFailureReason::NoImplementation
        );
    }

    #[tokio::test]
    async fn test_valid_patch_candidate_evaluated_and_selected() {
        let temp_dir = create_test_project();

        let valid_patch = r#"*** Begin Patch
*** Update File: src/lib.rs
@@ pub fn add(a: i32, b: i32) -> i32 {
-                a + b
+                // Optimized path
+                a.wrapping_add(b)
*** End Patch"#;

        let failing_patch = r#"*** Begin Patch
*** Update File: src/lib.rs
@@ pub fn add(a: i32, b: i32) -> i32 {
-                a + b
+                compile_error!("Broken candidate")
*** End Patch"#;

        let candidates = vec![
            SpeculativeBranchConfig {
                branch_id: "candidate-valid".to_string(),
                name: "Wrapping Add Approach".to_string(),
                hypothesis_prompt: "Use wrapping addition".to_string(),
                objective: Some("Optimize addition".into()),
                model_override: None,
                patch_content: Some(valid_patch.to_string()),
            },
            SpeculativeBranchConfig {
                branch_id: "candidate-broken".to_string(),
                name: "Broken Syntax Approach".to_string(),
                hypothesis_prompt: "Syntax error".to_string(),
                objective: Some("Optimize addition".into()),
                model_override: None,
                patch_content: Some(failing_patch.to_string()),
            },
        ];

        let report = SpeculativeArena::compete(
            temp_dir.path(),
            "Optimize math helper",
            candidates,
            Duration::from_secs(15),
            false,
        )
        .await
        .unwrap();

        assert_eq!(report.total_candidates, 2);
        assert_eq!(
            report.winning_branch_id.as_deref(),
            Some("candidate-valid"),
            "Speculative arena failed: rationale={}, evals={:?}",
            report.decision_rationale,
            report.evaluations
        );
        let winner = report
            .evaluations
            .iter()
            .find(|e| e.branch_id == "candidate-valid")
            .unwrap();
        assert!(winner.hard_gates_passed);
        assert!(winner.composite_score > 50.0);
        assert_eq!(winner.state, SpeculativeCandidateState::Eligible);

        let broken = report
            .evaluations
            .iter()
            .find(|e| e.branch_id == "candidate-broken")
            .unwrap();
        assert!(!broken.hard_gates_passed);
        assert_eq!(broken.composite_score, 0.0);
    }

    #[test]
    fn test_generate_diverse_hypotheses_helper() {
        let perf_hypo = SpeculativeArena::generate_diverse_hypotheses(
            "Fix parser performance and throughput",
            3,
        );
        assert_eq!(perf_hypo.len(), 3);
        assert!(perf_hypo[0].name.contains("Algorithmic"));
        assert!(perf_hypo[1].name.contains("Zero-Copy"));
        assert!(perf_hypo[2].name.contains("Parallel"));

        let general_hypo =
            SpeculativeArena::generate_diverse_hypotheses("Refactor database connector", 2);
        assert_eq!(general_hypo.len(), 2);
        assert!(general_hypo[0].name.contains("Direct Targeted"));
        assert!(general_hypo[1].name.contains("Decoupled Modular"));
    }

    #[tokio::test]
    async fn test_path_traversal_patch_rejected() {
        let temp_dir = create_test_project();
        let malicious_patch = r#"*** Begin Patch
*** Update File: ../outside.txt
@@ fn add()
-a
+b
*** End Patch"#;

        let candidate = SpeculativeBranchConfig {
            branch_id: "malicious".to_string(),
            name: "Malicious Approach".to_string(),
            hypothesis_prompt: "Path escape".to_string(),
            objective: Some("Security test".into()),
            model_override: None,
            patch_content: Some(malicious_patch.to_string()),
        };

        let report = SpeculativeArena::compete(
            temp_dir.path(),
            "Security test",
            vec![candidate],
            Duration::from_secs(10),
            false,
        )
        .await
        .unwrap();

        assert_eq!(report.total_candidates, 1);
        assert!(report.winning_branch_id.is_none());
        assert!(!report.evaluations[0].hard_gates_passed);
        assert!(matches!(
            report.evaluations[0].failure_reason,
            SpeculativeFailureReason::PatchParseError(_)
        ));
    }

    #[tokio::test]
    async fn test_failing_tests_gate_rejection() {
        let temp_dir = create_test_project();

        // Patch that compiles, but breaks the unit test (add(2, 2) != 4)
        let logic_error_patch = r#"*** Begin Patch
*** Update File: src/lib.rs
@@ pub fn add(a: i32, b: i32) -> i32 {
-                a + b
+                a + b + 999
*** End Patch"#;

        let candidate = SpeculativeBranchConfig {
            branch_id: "wrong-logic".to_string(),
            name: "Faulty Logic".to_string(),
            hypothesis_prompt: "Introduce math bug".to_string(),
            objective: Some("Bug introduction".into()),
            model_override: None,
            patch_content: Some(logic_error_patch.to_string()),
        };

        let report = SpeculativeArena::compete(
            temp_dir.path(),
            "Test test gate",
            vec![candidate],
            Duration::from_secs(15),
            false,
        )
        .await
        .unwrap();

        assert_eq!(report.total_candidates, 1);
        assert!(report.winning_branch_id.is_none());
        assert!(!report.evaluations[0].hard_gates_passed);
        assert!(
            report.evaluations[0].build_passed,
            "Build failed with: {:?}, evidence: {:?}",
            report.evaluations[0].failure_reason, report.evaluations[0].build_evidence
        );
        assert!(!report.evaluations[0].tests_passed);
        assert!(matches!(
            report.evaluations[0].failure_reason,
            SpeculativeFailureReason::TestsFailed(_)
        ));
    }

    struct MockExecutor {
        code_to_write: String,
    }

    impl CandidateExecutor for MockExecutor {
        fn execute<'a>(
            &'a self,
            ctx: &'a CandidateExecutionContext,
        ) -> BoxFuture<'a, Result<CandidateExecutionResult>> {
            let code = self.code_to_write.clone();
            Box::pin(async move {
                let file = ctx.workspace_root.join("src/lib.rs");
                fs::write(&file, &code)?;
                Ok(CandidateExecutionResult {
                    success: true,
                    modified_files: vec!["src/lib.rs".to_string()],
                    patch_content: None,
                    git_commit_sha: None,
                    error: None,
                })
            })
        }
    }

    #[tokio::test]
    async fn test_candidate_executor_invoked_in_sandbox() {
        let temp_dir = create_test_project();

        let executor = Arc::new(MockExecutor {
            code_to_write: r#"
            pub fn add(a: i32, b: i32) -> i32 {
                // Executor optimized implementation
                (a as i64 + b as i64) as i32
            }

            #[test]
            fn test_add() {
                assert_eq!(add(2, 2), 4);
            }
            "#
            .to_string(),
        });

        let candidate = SpeculativeBranchConfig {
            branch_id: "agent-hypo-1".to_string(),
            name: "Agent Executor Approach".to_string(),
            hypothesis_prompt: "Cast to i64".to_string(),
            objective: Some("Avoid overflow".into()),
            model_override: None,
            patch_content: None,
        };

        let opts = SpeculateOptions {
            workspace_root: temp_dir.path().to_path_buf(),
            mission: "Prevent overflow in add".to_string(),
            candidates: vec![candidate],
            timeout_per_candidate: Duration::from_secs(15),
            auto_promote_winner: false,
            require_approval_for_promotion: false,
            max_concurrency: 2,
            executor: Some(executor),
        };

        let report = SpeculativeArena::compete_with_options(opts).await.unwrap();

        assert_eq!(report.total_candidates, 1);
        assert_eq!(report.winning_branch_id.as_deref(), Some("agent-hypo-1"));
        let winner = &report.evaluations[0];
        assert!(winner.hard_gates_passed);
        assert!(winner.composite_score > 50.0);
        assert_eq!(winner.modified_files, vec!["src/lib.rs"]);
    }

    #[tokio::test]
    async fn test_promotion_requires_approval_leaves_pending() {
        let temp_dir = create_test_project();

        let valid_patch = r#"*** Begin Patch
*** Update File: src/lib.rs
@@ pub fn add(a: i32, b: i32) -> i32 {
-                a + b
+                a.saturating_add(b)
*** End Patch"#;

        let candidate = SpeculativeBranchConfig {
            branch_id: "saturating".to_string(),
            name: "Saturating Add".to_string(),
            hypothesis_prompt: "Use saturating add".to_string(),
            objective: Some("Safe addition".into()),
            model_override: None,
            patch_content: Some(valid_patch.to_string()),
        };

        let opts = SpeculateOptions {
            workspace_root: temp_dir.path().to_path_buf(),
            mission: "Safe add".to_string(),
            candidates: vec![candidate],
            timeout_per_candidate: Duration::from_secs(15),
            auto_promote_winner: true,
            require_approval_for_promotion: true,
            max_concurrency: 1,
            executor: None,
        };

        let report = SpeculativeArena::compete_with_options(opts).await.unwrap();

        assert_eq!(report.winning_branch_id.as_deref(), Some("saturating"));
        assert!(
            report
                .promotion_status
                .contains("Pending required human approval")
        );
        // Primary workspace must NOT have been changed because approval is pending!
        let primary_content = fs::read_to_string(temp_dir.path().join("src/lib.rs")).unwrap();
        assert!(!primary_content.contains("saturating_add"));
    }
}
