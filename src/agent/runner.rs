//! Seam-driven bounded step execution loop (AGT-05, D-03, D-06).
//!
//! Executes bounded agent steps where 1 step equals 1 model turn cycle (model request + validation + action execution).
//! Tool calls are metered independently, and cancellation is evaluated at step boundaries.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use crate::agent::model_policy::{ModelCaller, ModelProposal, StepBudget};
use crate::agent::profile::AgentProfile;
use crate::agent::supervisor::{AgentOutcome, FailureClass, FailureEvidence};
use crate::ids::{AgentId, MissionId, TaskId};
use crate::kernel::seams::context::{ContextCompilationRequest, ContextCompiler};

/// Strongly typed request to execute a runtime tool action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionRequest {
    pub id: String,
    pub tool_name: String,
    pub parameters: serde_json::Value,
}

/// Structured outcome of an executed tool action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionResult {
    pub action_id: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

/// In-memory working record of an executed agent step (D-03, P3-G).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentStepRecord {
    pub step_number: u32,
    pub model_proposal_summary: String,
    pub actions_executed: Vec<(ActionRequest, ActionResult)>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    #[serde(default)]
    pub inference_duration_ms: u64,
    #[serde(default)]
    pub action_fingerprint: Option<String>,
    #[serde(default)]
    pub tool_validation_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_provenance: Option<crate::prompt::provenance::PromptInvocationProvenance>,
}

impl Default for AgentStepRecord {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            step_number: 0,
            model_proposal_summary: String::new(),
            actions_executed: Vec::new(),
            started_at: now,
            completed_at: now,
            inference_duration_ms: 0,
            action_fingerprint: None,
            tool_validation_status: None,
            prompt_provenance: None,
        }
    }
}

impl AgentStepRecord {
    /// Builder method to attach prompt invocation provenance.
    pub fn with_prompt_provenance(
        mut self,
        prov: crate::prompt::provenance::PromptInvocationProvenance,
    ) -> Self {
        self.prompt_provenance = Some(prov);
        self
    }
}

/// Computes a canonical, deterministic hash fingerprint for an action proposal (P3-D).
pub fn compute_action_fingerprint(tool_name: &str, params: &serde_json::Value) -> String {
    let mut payload = tool_name.trim().to_lowercase().into_bytes();
    payload.push(b':');
    payload.extend_from_slice(canonicalize_json_for_fingerprint(params).as_bytes());
    crate::kernel::hash::sha256_hex(&payload)
}

pub fn canonicalize_json_for_fingerprint(val: &serde_json::Value) -> String {
    crate::pipeline::dedup::canonical_json(val)
}

/// Seam trait for dispatching side-effecting actions passing through runtime policies (D-06).
#[async_trait]
pub trait ActionDispatcher: Send + Sync {
    /// Dispatch an action request to the runtime tool engine.
    async fn dispatch(&self, action: &ActionRequest) -> Result<ActionResult, String>;

    /// dispatch a canonical agent action to produce an authoritative agent observation.
    async fn dispatch_action(
        &self,
        action: &crate::agent::action::AgentAction,
    ) -> Result<crate::agent::action::AgentObservation, String> {
        let (tool_name, params) = match action {
            crate::agent::action::AgentAction::CallTool {
                tool_name,
                parameters,
            } => (tool_name.clone(), parameters.clone()),
            crate::agent::action::AgentAction::WriteChange { path, content, .. } => (
                "write_file".to_string(),
                serde_json::json!({ "path": path.display().to_string(), "content": content }),
            ),
            crate::agent::action::AgentAction::RunVerification { tier, target } => (
                "run_tests".to_string(),
                serde_json::json!({ "tier": tier, "target": target }),
            ),
            crate::agent::action::AgentAction::Complete { summary } => {
                return Ok(crate::agent::action::AgentObservation::Completed {
                    summary: summary.clone(),
                });
            }
            crate::agent::action::AgentAction::Fail { error } => {
                return Ok(crate::agent::action::AgentObservation::Failed {
                    error: error.clone(),
                    classification: None,
                });
            }
            crate::agent::action::AgentAction::Cancel { reason } => {
                return Ok(crate::agent::action::AgentObservation::Cancelled {
                    reason: reason.clone(),
                });
            }
            crate::agent::action::AgentAction::Pause { .. } => {
                return Ok(crate::agent::action::AgentObservation::Paused);
            }
            crate::agent::action::AgentAction::Resume { .. } => {
                return Ok(crate::agent::action::AgentObservation::Resumed);
            }
            crate::agent::action::AgentAction::AskUser { question, options } => (
                "ask_user".to_string(),
                serde_json::json!({ "question": question, "options": options }),
            ),
            crate::agent::action::AgentAction::RequestApproval {
                tool_name,
                parameters,
                risk,
                reason,
            } => (
                "request_approval".to_string(),
                serde_json::json!({
                    "tool_name": tool_name,
                    "parameters": parameters,
                    "risk": risk,
                    "reason": reason,
                }),
            ),
            crate::agent::action::AgentAction::CreateTask {
                title,
                description,
                role,
                dependencies,
            } => (
                "create_task".to_string(),
                serde_json::json!({
                    "title": title,
                    "description": description,
                    "role": role.as_ref().map(|r| r.as_str()),
                    "dependencies": dependencies,
                }),
            ),
            crate::agent::action::AgentAction::SupersedeTask { task_id, reason } => (
                "supersede_task".to_string(),
                serde_json::json!({
                    "task_id": task_id.to_string(),
                    "reason": reason,
                }),
            ),
            crate::agent::action::AgentAction::Delegate {
                target_role,
                task_id,
                reason,
                inputs,
            } => (
                "delegate_task".to_string(),
                serde_json::json!({
                    "target_role": target_role.to_string(),
                    "task_id": task_id.map(|t| t.to_string()),
                    "subtask": reason,
                    "inputs": inputs,
                }),
            ),
            crate::agent::action::AgentAction::Replan {
                reason,
                tasks_to_supersede,
                new_tasks,
            } => (
                "adapt_strategy".to_string(),
                serde_json::json!({
                    "reason": reason,
                    "tasks_to_supersede": tasks_to_supersede,
                    "new_tasks": new_tasks,
                }),
            ),
        };
        let req = ActionRequest {
            id: format!("act-{}", uuid::Uuid::now_v7()),
            tool_name: tool_name.clone(),
            parameters: params,
        };
        let res = self.dispatch(&req).await?;
        Ok(crate::agent::action::AgentObservation::ToolOutput {
            tool_name,
            call_id: res.action_id,
            output: res.output,
            success: res.success,
            duration_ms: 0,
        })
    }
}

/// Validate that a path stays contained within the canonical workspace root (BUG-02).
pub fn validate_workspace_path(
    workspace_root: &std::path::Path,
    raw_path: &str,
) -> Result<std::path::PathBuf, String> {
    crate::policy::matcher::canonicalize_and_validate_path(
        std::path::Path::new(raw_path),
        workspace_root,
    )
    .map_err(|e| format!("Workspace containment violation: {e}"))
}

/// Extract all candidate paths from a tool call's name and arguments (BUG-02).
pub fn extract_all_action_paths(tool_name: &str, params: &serde_json::Value) -> Vec<String> {
    let mut paths =
        crate::verification::gate::ToolCapabilityClass::extract_mutation_paths(tool_name, params);

    let mut additional = Vec::new();
    crate::verification::gate::extract_json_path_candidates(params, &mut additional);
    for p in additional {
        let c = p
            .trim()
            .trim_start_matches('@')
            .trim_start_matches("./")
            .to_string();
        if !c.is_empty() && !paths.contains(&c) {
            paths.push(c);
        }
    }
    paths
}

/// Bounded worker execution runner maintaining step budget and in-task working history (D-03, D-06).
///
/// Governance contract: policy evaluation and approval are owned SOLELY by
/// the execution pipeline behind the dispatched `ActionDispatcher`. This
/// runner carries NO policy gate field — a second gate here would fork
/// governance decisions for the same action (wiring remediation v0.1.1).
pub struct WorkerRunner {
    pub mission_id: MissionId,
    pub agent_id: AgentId,
    pub task_id: TaskId,
    pub profile: AgentProfile,
    pub step_budget: StepBudget,
    pub step_history: Vec<AgentStepRecord>,
    pub context_compiler: Arc<dyn ContextCompiler>,
    pub delegation_depth: usize,
    pub max_delegation_depth: usize,
    pub mission_objective: Option<String>,
    pub task_objective: Option<String>,
    pub workspace_root: Option<std::path::PathBuf>,
    /// Target-specific work description carried into task context (Gap 1).
    pub task_description: Option<String>,
    /// Task-specific completion criteria carried into task context (Gap 1).
    pub task_criteria: Vec<String>,
    /// Requirement keys this work satisfies (Gap 1, traceability).
    pub requirement_keys: Vec<String>,
    /// Assumptions the work may rely on (Gap 1).
    pub task_assumptions: Vec<String>,
    /// Upstream project charter markdown (Gap 2).
    pub upstream_charter: Option<String>,
    /// Upstream target architecture markdown (Gap 2).
    pub upstream_architecture: Option<String>,
    /// Upstream requirement statements (Gap 2).
    pub upstream_requirements: Vec<String>,
    /// Upstream assumption statements (Gap 2).
    pub upstream_assumptions: Vec<String>,
    /// Upstream decision records (Gap 2).
    pub upstream_decisions: Vec<String>,
    /// Upstream research summary markdown (Gap 2).
    pub upstream_research_summary: Option<String>,
    /// Declared verification strategy for this task. `None` preserves the
    /// strict test-evidence gate.
    pub verification: Option<crate::kernel::plan::VerificationStrategy>,
    /// Typed prompt execution binding selected for this work.
    ///
    /// Explicit task/workflow binding from the work request. When `Some`,
    /// it takes precedence over the profile (role default) prompt at
    /// context compilation time. The profile prompt is NEVER silently
    /// substituted while this binding is present.
    pub prompt_ref: Option<crate::prompt::PromptReference>,
    /// Declared target change surface for boundary enforcement.
    pub target_surface: Option<crate::kernel::change::ChangeSurface>,
    /// Pre-mutation file baselines captured before the first write to each path.
    pub baseline_snapshots: std::collections::HashMap<String, Option<Vec<u8>>>,
    /// Cooperative pause signal observed at step boundaries (F-03/F-12).
    /// When cancelled, the worker stops starting new side effects and yields
    /// so the controller can halt at a safe boundary. `None` (tests, lanes
    /// without a controller) disables pause observation.
    pub pause_token: Option<tokio_util::sync::CancellationToken>,
}

impl WorkerRunner {
    /// Create a new worker runner bound to the canonical runtime context
    /// authority.
    ///
    /// `context_compiler` MUST be the runtime-shared
    /// [`ContextCompiler`](crate::kernel::seams::context::ContextCompiler)
    /// (owned by
    /// [`RuntimeAuthorities`](crate::runtime_authorities::RuntimeAuthorities)
    /// in production, explicitly constructed isolated infrastructure in
    /// tests). This runner NEVER constructs its own prompt catalog or
    /// prompt compiler: a worker-owned authority would silently diverge
    /// from the runtime's canonical prompt chain (wiring remediation
    /// v0.1.1).
    pub fn new(
        mission_id: MissionId,
        agent_id: AgentId,
        task_id: TaskId,
        profile: AgentProfile,
        context_compiler: Arc<dyn ContextCompiler>,
    ) -> Self {
        assert_ne!(
            mission_id,
            MissionId::from_bytes([0u8; 16]),
            "WorkerRunner requires a valid non-zero MissionId"
        );
        let max_steps = profile.max_steps;
        Self {
            mission_id,
            agent_id,
            task_id,
            profile,
            step_budget: StepBudget::new(max_steps),
            step_history: Vec::new(),
            context_compiler,
            delegation_depth: 0,
            max_delegation_depth: crate::agent::delegation::MAX_DELEGATION_DEPTH_DEFAULT,
            mission_objective: None,
            task_objective: None,
            workspace_root: None,
            task_description: None,
            task_criteria: Vec::new(),
            requirement_keys: Vec::new(),
            task_assumptions: Vec::new(),
            upstream_charter: None,
            upstream_architecture: None,
            upstream_requirements: Vec::new(),
            upstream_assumptions: Vec::new(),
            upstream_decisions: Vec::new(),
            upstream_research_summary: None,
            verification: None,
            prompt_ref: None,
            target_surface: None,
            baseline_snapshots: std::collections::HashMap::new(),
            pause_token: None,
        }
    }

    /// Create a worker runner with an explicitly isolated test compiler.
    ///
    /// TEST INFRASTRUCTURE ONLY: builds a standalone
    /// [`ProductionContextCompiler`](crate::context::compiler::ProductionContextCompiler)
    /// with built-in prompts instead of consuming the runtime-shared
    /// authority. Production code MUST use [`WorkerRunner::new`] with the
    /// canonical compiler. The isolated instance is never a substitute
    /// for the runtime chain — it exists so unit and integration tests can
    /// construct runners without a full runtime composition root.
    pub fn new_isolated_test(
        mission_id: MissionId,
        agent_id: AgentId,
        task_id: TaskId,
        profile: AgentProfile,
    ) -> Self {
        Self::new(
            mission_id,
            agent_id,
            task_id,
            profile,
            Arc::new(
                crate::context::compiler::ProductionContextCompiler::new()
                    .with_role_stage_fallback(std::sync::Arc::new(|role| {
                        crate::agent::registry::RoleRegistry::global()
                            .read()
                            .ok()
                            .and_then(|guard| guard.stage_for(role))
                    })),
            ),
        )
    }

    /// Configure workspace root directory for filesystem and diff validation.
    pub fn with_workspace_root(mut self, root: impl Into<std::path::PathBuf>) -> Self {
        self.workspace_root = Some(root.into());
        self
    }

    /// Attach the cooperative pause signal observed at step boundaries
    /// (F-03/F-12). The worker stops starting new side effects once it is
    /// cancelled and yields at the next safe boundary.
    pub fn with_pause_token(mut self, token: tokio_util::sync::CancellationToken) -> Self {
        self.pause_token = Some(token);
        self
    }

    /// Configure declared target change surface for boundary enforcement.
    pub fn with_target_surface(mut self, surface: crate::kernel::change::ChangeSurface) -> Self {
        self.target_surface = Some(surface);
        self
    }

    /// Configure a custom context compiler.
    pub fn with_context_compiler(mut self, compiler: Arc<dyn ContextCompiler>) -> Self {
        self.context_compiler = compiler;
        self
    }

    /// Attach the typed prompt execution binding for this work.
    ///
    /// Explicit task/workflow binding from the work request. Takes
    /// precedence over the profile (role default) prompt at context
    /// compilation time.
    pub fn with_prompt_ref(mut self, prompt_ref: crate::prompt::PromptReference) -> Self {
        self.prompt_ref = Some(prompt_ref);
        self
    }

    /// Attach the optional typed prompt execution binding for this work.
    pub fn with_prompt_ref_opt(
        mut self,
        prompt_ref: Option<crate::prompt::PromptReference>,
    ) -> Self {
        self.prompt_ref = prompt_ref;
        self
    }

    /// Configure mission objective.
    pub fn with_mission_objective(mut self, objective: impl Into<String>) -> Self {
        self.mission_objective = Some(objective.into());
        self
    }

    /// Configure task objective.
    pub fn with_task_objective(mut self, objective: impl Into<String>) -> Self {
        self.task_objective = Some(objective.into());
        self
    }

    /// Configure target-specific work description (Gap 1).
    pub fn with_task_description(mut self, desc: impl Into<String>) -> Self {
        self.task_description = Some(desc.into());
        self
    }

    /// Configure task-specific completion criteria (Gap 1).
    pub fn with_task_criteria(mut self, criteria: Vec<String>) -> Self {
        self.task_criteria = criteria;
        self
    }

    /// Configure requirement keys this work satisfies (Gap 1).
    pub fn with_requirement_keys(mut self, keys: Vec<String>) -> Self {
        self.requirement_keys = keys;
        self
    }

    /// Configure assumptions the work may rely on (Gap 1).
    pub fn with_task_assumptions(mut self, assumptions: Vec<String>) -> Self {
        self.task_assumptions = assumptions;
        self
    }

    /// Configure upstream project charter markdown (Gap 2).
    pub fn with_upstream_charter(mut self, charter: impl Into<String>) -> Self {
        self.upstream_charter = Some(charter.into());
        self
    }

    /// Configure upstream target architecture markdown (Gap 2).
    pub fn with_upstream_architecture(mut self, architecture: impl Into<String>) -> Self {
        self.upstream_architecture = Some(architecture.into());
        self
    }

    /// Configure upstream requirement statements (Gap 2).
    pub fn with_upstream_requirements(mut self, requirements: Vec<String>) -> Self {
        self.upstream_requirements = requirements;
        self
    }

    /// Configure upstream assumption statements (Gap 2).
    pub fn with_upstream_assumptions(mut self, assumptions: Vec<String>) -> Self {
        self.upstream_assumptions = assumptions;
        self
    }

    /// Configure upstream decision records (Gap 2).
    pub fn with_upstream_decisions(mut self, decisions: Vec<String>) -> Self {
        self.upstream_decisions = decisions;
        self
    }

    /// Configure upstream research summary markdown (Gap 2).
    pub fn with_upstream_research_summary(mut self, summary: impl Into<String>) -> Self {
        self.upstream_research_summary = Some(summary.into());
        self
    }

    /// Configure the declared verification strategy.
    pub fn with_verification(
        mut self,
        verification: crate::kernel::plan::VerificationStrategy,
    ) -> Self {
        self.verification = Some(verification);
        self
    }

    /// Access the step budget tracker.
    pub fn step_budget(&self) -> &StepBudget {
        &self.step_budget
    }

    /// Access recorded step history.
    pub fn step_history(&self) -> &[AgentStepRecord] {
        &self.step_history
    }

    /// Access recorded step history mutably.
    pub fn step_history_mut(&mut self) -> &mut Vec<AgentStepRecord> {
        &mut self.step_history
    }

    /// Push a step record into step history.
    pub fn push_step_record(&mut self, record: AgentStepRecord) {
        self.step_history.push(record);
    }

    /// Whether task completion requires passing test-tool output in history.
    ///
    /// The requirement applies to strategies whose evidence IS test output
    /// (Compilation, AutomatedTest, StaticAnalysis, composites containing
    /// them, and legacy unknown strategies). Artifact inspection carries
    /// non-empty expected paths that the controller verifies for real via
    /// the hierarchy, so demanding test output here would make such tasks
    /// uncompletable on toolchain-free workspaces. An ArtifactInspection
    /// with NO paths is vacuous and stays under the strict gate
    /// (fail-safe); ReviewGate likewise stays strict.
    fn completion_needs_test_evidence(&self) -> bool {
        match &self.verification {
            None => true,
            Some(strategy) => {
                use crate::kernel::plan::VerificationStrategy as VS;
                fn contains_testable(s: &VS) -> bool {
                    match s {
                        VS::Compilation | VS::AutomatedTest { .. } | VS::StaticAnalysis { .. } => {
                            true
                        }
                        VS::Composite { strategies } => strategies.iter().any(contains_testable),
                        VS::ArtifactInspection { .. } | VS::ReviewGate { .. } => false,
                    }
                }
                match strategy {
                    VS::ArtifactInspection { paths } => paths.is_empty(),
                    VS::ReviewGate { .. } => true,
                    other => contains_testable(other),
                }
            }
        }
    }

    /// Authoritative completion evidence evaluation (BUG-01 repair).
    ///
    /// Evaluates step history + current batch actions in linear execution order.
    /// Requires that any mutations have passing verification strictly AFTER
    /// the latest mutation, that diff review passes, and that roles requiring
    /// modifications have made mutations.
    pub fn evaluate_completion_evidence(
        &self,
        current_batch: &[(ActionRequest, ActionResult)],
    ) -> Result<Vec<String>, String> {
        let mut all_actions: Vec<(&ActionRequest, &ActionResult)> = Vec::new();
        for s in &self.step_history {
            for (req, res) in &s.actions_executed {
                all_actions.push((req, res));
            }
        }
        for (req, res) in current_batch {
            all_actions.push((req, res));
        }

        if current_batch
            .iter()
            .any(|(_, res)| !res.success || res.error.is_some())
        {
            return Err("Premature completion rejected: current execution batch contains failed tool actions.".to_string());
        }

        let last_mutation_idx = all_actions.iter().rposition(|(req, res)| {
            res.success
                && crate::verification::gate::ToolCapabilityClass::classify(&req.tool_name)
                    .is_mutation()
        });

        if let Some(mut_idx) = last_mutation_idx {
            let has_verified = all_actions[(mut_idx + 1)..].iter().any(|(req, res)| {
                res.success
                    && crate::verification::gate::ToolCapabilityClass::classify(&req.tool_name)
                        .is_verification()
                    && !res.output.contains("Failed tests:")
                    && !res.output.contains("Compilation failed")
                    && !res.output.contains("Compiler errors detected")
                    && !res.output.contains("test result: FAILED")
                    && res.error.is_none()
            });

            if !has_verified && self.completion_needs_test_evidence() {
                return Err("Premature completion rejected (AGENTS.md Rule 6: Completion requires evidence): Code changes have been made, but verification has not passed. You must run 'run_tests' and fix all compiler and test failures until tests pass before completing.".to_string());
            }

            let ws = self
                .workspace_root
                .as_deref()
                .unwrap_or(std::path::Path::new("."));
            let mut modified_files = Vec::new();
            for (req, res) in &all_actions {
                if res.success
                    && crate::verification::gate::ToolCapabilityClass::classify(&req.tool_name)
                        .is_mutation()
                {
                    for path in
                        crate::verification::gate::ToolCapabilityClass::extract_mutation_paths(
                            &req.tool_name,
                            &req.parameters,
                        )
                    {
                        if !modified_files.contains(&path) {
                            modified_files.push(path);
                        }
                    }
                }
            }

            if !modified_files.is_empty() {
                let surface = self.target_surface.clone().unwrap_or_else(|| {
                    crate::kernel::change::ChangeSurface::new(modified_files.clone())
                });
                let mut synthesized_diff = String::new();
                for file in &modified_files {
                    let old_bytes = self
                        .baseline_snapshots
                        .get(file)
                        .and_then(|opt| opt.as_deref());
                    let new_bytes = std::fs::read(ws.join(file)).ok();
                    let file_diff =
                        crate::change::observer::PostMutationObserver::generate_file_unified_diff(
                            file,
                            old_bytes,
                            new_bytes.as_deref(),
                        );
                    synthesized_diff.push_str(&file_diff);
                }
                let review = crate::change::review::DiffReviewer::review(
                    &synthesized_diff,
                    &surface,
                    &modified_files,
                );
                if !review.passed {
                    return Err(format!(
                        "Premature completion rejected (AGENTS.md Rule 5: Never fake success): Diff review detected quality violations in changes: {:?}",
                        review.violations
                    ));
                }
            }
            Ok(modified_files)
        } else if crate::agent::registry::RoleRegistry::requires_modification_for(
            &self.profile.role,
        ) {
            Err("Premature completion rejected (AGENTS.md Rule 5 & 6): Implementer tasks require making the code changes specified by the objective before completing. You have not called edit_file or write_file to modify the workspace. Please inspect the code and execute edit_file/write_file to implement the required changes.".to_string())
        } else {
            Ok(Vec::new())
        }
    }

    /// Run the bounded step iteration loop until completion, handoff, exhaustion, or cancellation.
    pub async fn run_step_loop<M, A>(
        &mut self,
        model_caller: &M,
        action_dispatcher: &A,
        cancellation_token: &CancellationToken,
        activity_tracker: Arc<crate::agent::supervisor::ExecutionActivityTracker>,
    ) -> AgentOutcome
    where
        M: ModelCaller + ?Sized,
        A: ActionDispatcher,
    {
        loop {
            // 1. Cooperative cancellation check at step boundary
            if cancellation_token.is_cancelled() {
                return AgentOutcome::Cancelled {
                    reason: "cancelled at step boundary".to_string(),
                    steps_consumed: self.step_budget.steps_consumed(),
                };
            }

            // 1b. Cooperative pause check at step boundary (F-03/F-12).
            // Pause never abandons in-flight work: reaching this boundary
            // proves the previous turn fully completed, so yielding here
            // starts no new side effects and drops nothing partially done.
            if self.pause_token.as_ref().is_some_and(|t| t.is_cancelled()) {
                return AgentOutcome::Paused {
                    reason: "pause requested; yielding at safe step boundary".to_string(),
                    steps_consumed: self.step_budget.steps_consumed(),
                };
            }

            // 2. Pre-step admission check
            if let Err(limit_err) = self.step_budget.check_admission() {
                return AgentOutcome::StepLimitExceeded {
                    limit: limit_err.limit,
                    consumed: limit_err.consumed,
                };
            }

            // 3. Mark step start progress
            let step_start = Utc::now();
            activity_tracker.touch().await;

            // 4. Compile fresh context from durable task state
            let mut step_dtos = Vec::new();
            for s in &self.step_history {
                for (req, res) in &s.actions_executed {
                    step_dtos.push(crate::kernel::seams::context::StepRecordDto {
                        step_number: s.step_number,
                        tool_name: req.tool_name.clone(),
                        parameters: req.parameters.clone(),
                        success: res.success,
                        output: res.output.clone(),
                        error: res.error.clone(),
                    });
                }
            }

            let mut comp_req = ContextCompilationRequest::new(
                self.mission_id,
                self.task_id,
                self.profile.model_policy.min_context_tokens.max(4096),
            )
            .with_step_history(step_dtos)
            .with_role(self.profile.role.clone())
            // Explicit prompt selection precedence: task/workflow binding >
            // profile (role/session) binding. The profile prompt is the
            // session-level default; a task-level PromptReference from the
            // work request MUST NOT be silently replaced by it.
            .with_prompt_ref(
                self.prompt_ref
                    .clone()
                    .unwrap_or_else(|| self.profile.prompt_ref.clone()),
            )
            .with_prompt_source(if self.prompt_ref.is_some() {
                crate::kernel::seams::context::PromptSelectionSource::ExplicitTask
            } else {
                crate::kernel::seams::context::PromptSelectionSource::ExplicitSession
            })
            .with_agent_id(self.agent_id);

            if let Some(ref obj) = self.mission_objective {
                comp_req = comp_req.with_mission_objective(obj.clone());
            }
            if let Some(ref obj) = self.task_objective {
                comp_req = comp_req.with_task_objective(obj.clone());
            }
            if let Some(ref desc) = self.task_description {
                comp_req = comp_req.with_task_description(desc.clone());
            }
            if !self.task_criteria.is_empty() {
                comp_req = comp_req.with_task_criteria(self.task_criteria.clone());
            }
            if !self.requirement_keys.is_empty() {
                comp_req = comp_req.with_task_requirement_keys(self.requirement_keys.clone());
            }
            if !self.task_assumptions.is_empty() {
                comp_req = comp_req.with_task_assumptions(self.task_assumptions.clone());
            }
            if let Some(ref charter) = self.upstream_charter {
                comp_req = comp_req.with_upstream_charter(charter.clone());
            }
            if let Some(ref arch) = self.upstream_architecture {
                comp_req = comp_req.with_upstream_architecture(arch.clone());
            }
            if !self.upstream_requirements.is_empty() {
                comp_req = comp_req.with_upstream_requirements(self.upstream_requirements.clone());
            }
            if !self.upstream_assumptions.is_empty() {
                comp_req = comp_req.with_upstream_assumptions(self.upstream_assumptions.clone());
            }
            if !self.upstream_decisions.is_empty() {
                comp_req = comp_req.with_upstream_decisions(self.upstream_decisions.clone());
            }
            if let Some(ref summary) = self.upstream_research_summary {
                comp_req = comp_req.with_upstream_research_summary(summary.clone());
            }

            let compiled_context = match self.context_compiler.compile_context(comp_req).await {
                Ok(ctx) => ctx,
                Err(err) => {
                    return AgentOutcome::Failed(FailureEvidence {
                        failure_class: FailureClass::ModelError,
                        message: format!("context compilation failure: {}", err),
                        step_number: self.step_budget.steps_consumed() + 1,
                        occurred_at: Utc::now(),
                        is_panic: false,
                        diagnostics: std::collections::HashMap::new(),
                    });
                }
            };

            // 5. Model decision cycle with cooperative cancellation
            activity_tracker.mark_inference_start().await;
            let inf_start = std::time::Instant::now();
            let model_result = model_caller
                .call_model_with_context_and_usage(&compiled_context, cancellation_token)
                .await;
            let inference_duration_ms = inf_start.elapsed().as_millis() as u64;
            activity_tracker.mark_idle().await;

            let proposal = match model_result {
                Ok((p, usage)) => {
                    activity_tracker.record_token_usage(&usage).await;
                    p
                }
                Err(err) => {
                    if cancellation_token.is_cancelled() {
                        return AgentOutcome::Cancelled {
                            reason: "cancelled during model invocation".to_string(),
                            steps_consumed: self.step_budget.steps_consumed(),
                        };
                    }
                    return AgentOutcome::Failed(FailureEvidence {
                        failure_class: FailureClass::ModelError,
                        message: format!("model call failure: {}", err),
                        step_number: self.step_budget.steps_consumed() + 1,
                        occurred_at: Utc::now(),
                        is_panic: false,
                        diagnostics: std::collections::HashMap::new(),
                    });
                }
            };

            let tool_names = proposal.tool_names();
            tracing::debug!(
                step = self.step_budget.steps_consumed() + 1,
                max_steps = self.profile.max_steps,
                role = ?self.profile.role,
                proposal_kind = proposal.kind_name(),
                tool_calls_count = proposal.tool_calls_count(),
                tool_names = ?tool_names,
                "[Runner Step] executing proposal"
            );

            // 6. Proposal handling
            match proposal {
                ModelProposal::ToolCalls { calls } => {
                    // BUG-09: All-or-nothing proposal conversion and parameter validation.
                    // If any tool call in the batch is invalid, fail the entire proposal
                    // immediately without executing any calls.
                    let mut parse_error = None;
                    for (idx, call) in calls.iter().enumerate() {
                        if let Err(err) = crate::agent::action::AgentAction::from_tool_call(call) {
                            parse_error = Some((idx, call.clone(), err));
                            break;
                        }
                    }
                    if let Some((err_idx, err_call, parse_err)) = parse_error {
                        self.step_budget.record_step();
                        let step_end = Utc::now();
                        activity_tracker.touch().await;
                        let action_req = ActionRequest {
                            id: if err_call.id.is_empty() {
                                format!(
                                    "act-step-{}-{}",
                                    self.step_budget.steps_consumed(),
                                    err_idx + 1
                                )
                            } else {
                                err_call.id
                            },
                            tool_name: err_call.name.clone(),
                            parameters: err_call.arguments,
                        };
                        let action_res = ActionResult {
                            action_id: action_req.id.clone(),
                            success: false,
                            output: String::new(),
                            error: Some(format!(
                                "Proposal rejected (all-or-nothing validation failed on call #{}: {parse_err}). No actions were executed.",
                                err_idx + 1
                            )),
                        };
                        self.step_history.push(AgentStepRecord {
                            step_number: self.step_budget.steps_consumed(),
                            model_proposal_summary: format!(
                                "proposal_validation_failure:call_{}:{}",
                                err_idx + 1,
                                parse_err
                            ),
                            actions_executed: vec![(action_req, action_res)],
                            started_at: step_start,
                            completed_at: step_end,
                            inference_duration_ms,
                            action_fingerprint: None,
                            tool_validation_status: Some("rejected".to_string()),
                            prompt_provenance: compiled_context.prompt_provenance.clone(),
                        });
                        continue;
                    }

                    let mut executed = Vec::new();
                    let mut any_complete = false;
                    let mut final_out = String::new();
                    let mut last_fingerprint = None;

                    for (c_idx, call) in calls.into_iter().enumerate() {
                        // No post-cancel tool execution may begin. A cancellation
                        // arriving mid-turn stops any remaining tool calls in this turn.
                        if cancellation_token.is_cancelled() {
                            return AgentOutcome::Cancelled {
                                reason: "cancelled before tool execution".to_string(),
                                steps_consumed: self.step_budget.steps_consumed(),
                            };
                        }
                        let agent_action_res =
                            crate::agent::action::AgentAction::from_tool_call(&call);
                        let action_req = ActionRequest {
                            id: if call.id.is_empty() {
                                format!(
                                    "act-step-{}-{}",
                                    self.step_budget.steps_consumed() + 1,
                                    c_idx + 1
                                )
                            } else {
                                call.id
                            },
                            tool_name: call.name.clone(),
                            parameters: call.arguments,
                        };

                        let action_fingerprint =
                            compute_action_fingerprint(&call.name, &action_req.parameters);
                        last_fingerprint = Some(action_fingerprint.clone());

                        // Check if previous action was identical and failed (P3-D non-progress control)
                        let is_repeated_failure = self.step_history.last().and_then(|last_step| {
                            last_step
                                .actions_executed
                                .last()
                                .and_then(|(last_req, last_res)| {
                                    if !last_res.success {
                                        let last_fp = compute_action_fingerprint(
                                            &last_req.tool_name,
                                            &last_req.parameters,
                                        );
                                        if last_fp == action_fingerprint {
                                            Some(last_step.step_number)
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                })
                        });

                        // Policy governance is owned SOLELY by the execution
                        // pipeline (stages 7-8) behind `action_dispatcher`.
                        // This runner performs NO separate policy precheck:
                        // Ask/Deny/approval all resolve inside the pipeline
                        // against the same bound identities, so duplicate
                        // governance decisions for the same action are
                        // impossible. Local rejections below are
                        // non-governance static guards (role write scope,
                        // workspace containment, generated-file protection, loop suppression).
                        activity_tracker.mark_tool_start(&call.name).await;
                        let mut action_result = if let Some(failed_step_num) = is_repeated_failure {
                            tracing::warn!(
                                tool = %call.name,
                                fingerprint = %action_fingerprint,
                                failed_step = failed_step_num,
                                "Rejected repeated non-progress action"
                            );
                            ActionResult {
                                action_id: action_req.id.clone(),
                                success: false,
                                output: String::new(),
                                error: Some(format!(
                                    "Repeated non-progress action rejected: This identical '{}' tool call failed on step {}. You MUST either inspect the file with 'read_file', use a different line range, or change your arguments before retrying.",
                                    call.name, failed_step_num
                                )),
                            }
                        } else if let Err(parse_err) = agent_action_res {
                            ActionResult {
                                action_id: action_req.id.clone(),
                                success: false,
                                output: String::new(),
                                error: Some(format!("Tool call parameter validation failed: {parse_err}")),
                            }
                        } else if {
                            let ws = self.workspace_root.as_deref().unwrap_or(std::path::Path::new("."));
                            let paths = extract_all_action_paths(&call.name, &action_req.parameters);
                            paths.iter().find_map(|p| validate_workspace_path(ws, p).err())
                        }.is_some() {
                            let ws = self.workspace_root.as_deref().unwrap_or(std::path::Path::new("."));
                            let paths = extract_all_action_paths(&call.name, &action_req.parameters);
                            let path_err = paths.iter().find_map(|p| validate_workspace_path(ws, p).err()).unwrap();
                            ActionResult {
                                action_id: action_req.id.clone(),
                                success: false,
                                output: String::new(),
                                error: Some(path_err),
                            }
                        // Write-tool gate (registry-driven): only roles whose
                        // definition permits write tools may invoke mutating
                        // tools. Unregistered roles fail closed here — dispatch
                        // only materializes registered profiles, so reaching this
                        // branch unregistered indicates a broken invariant.
                        } else if !crate::agent::registry::RoleRegistry::write_tools_permitted_for(
                            &self.profile.role,
                        ) && crate::verification::gate::ToolCapabilityClass::classify(&call.name)
                            .is_mutation()
                        {
                            ActionResult {
                                action_id: action_req.id.clone(),
                                success: false,
                                output: String::new(),
                                error: Some(format!(
                                    "Role authorization failure: Tool '{}' is not permitted for role '{:?}'. The '{:?}' role is strictly read-only.",
                                    call.name, self.profile.role, self.profile.role
                                )),
                            }
                        } else if crate::verification::gate::ToolCapabilityClass::classify(&call.name)
                            .is_mutation()
                            && {
                                let target_paths =
                                    crate::verification::gate::ToolCapabilityClass::extract_mutation_paths(
                                        &call.name,
                                        &action_req.parameters,
                                    );
                                let ws = self
                                    .workspace_root
                                    .as_deref()
                                    .unwrap_or(std::path::Path::new("."));
                                target_paths.into_iter().find_map(|tp| {
                                    let validated = validate_workspace_path(ws, &tp).ok()?;
                                    if validated.exists() && let Ok(content) = std::fs::read_to_string(&validated) {
                                        crate::change::detect_generated_file(&content, &tp)
                                            .map(|sig| (tp, sig))
                                    } else {
                                        None
                                    }
                                })
                            }
                            .is_some()
                        {
                            let ws = self
                                .workspace_root
                                .as_deref()
                                .unwrap_or(std::path::Path::new("."));
                            let (gen_path, generator_sig) =
                                crate::verification::gate::ToolCapabilityClass::extract_mutation_paths(
                                    &call.name,
                                    &action_req.parameters,
                                )
                                .into_iter()
                                .find_map(|tp| {
                                    let validated = validate_workspace_path(ws, &tp).ok()?;
                                    if validated.exists() && let Ok(content) = std::fs::read_to_string(&validated) {
                                        crate::change::detect_generated_file(&content, &tp)
                                            .map(|sig| (tp, sig))
                                    } else {
                                        None
                                    }
                                })
                                .unwrap();
                            ActionResult {
                                action_id: action_req.id.clone(),
                                success: false,
                                output: String::new(),
                                error: Some(format!(
                                    "Direct edit of generated file '{}' rejected (signature: {}). Modify the source generator/template instead of the generated artifact.",
                                    gen_path, generator_sig
                                )),
                            }
                        } else {
                            // Capture pre-mutation baselines before executing mutating tools (BUG-02)
                            if crate::verification::gate::ToolCapabilityClass::classify(&call.name)
                                .is_mutation()
                            {
                                let ws = self
                                    .workspace_root
                                    .as_deref()
                                    .unwrap_or(std::path::Path::new("."));
                                for target_path in
                                    crate::verification::gate::ToolCapabilityClass::extract_mutation_paths(
                                        &call.name,
                                        &action_req.parameters,
                                    )
                                {
                                    if let Ok(validated) = validate_workspace_path(ws, &target_path) {
                                        if let std::collections::hash_map::Entry::Vacant(e) =
                                            self.baseline_snapshots.entry(target_path)
                                        {
                                            e.insert(std::fs::read(&validated).ok());
                                        }
                                    }
                                }
                            }

                            // canonical action dispatch through agent_action protocol:
                            // no production path directly interprets raw tool calls and bypasses agent_action.
                            let agent_action = agent_action_res.unwrap();
                            let obs_res = action_dispatcher.dispatch_action(&agent_action).await;
                            match obs_res {
                                Ok(obs) => ActionResult {
                                    action_id: action_req.id.clone(),
                                    success: obs.is_success(),
                                    output: obs.output_text(),
                                    error: obs.error_text(),
                                },
                                Err(err) => ActionResult {
                                    action_id: action_req.id.clone(),
                                    success: false,
                                    output: String::new(),
                                    error: Some(err),
                                },
                            }
                        };
                        activity_tracker.mark_idle().await;

                        self.step_budget.record_tool_call();

                        // Track consecutive edit failures on the same target file (P3-D)
                        if !action_result.success
                            && let Some(target_path) =
                                action_req.parameters.get("path").and_then(|p| p.as_str())
                        {
                            let consecutive_path_failures = self
                                .step_history
                                .iter()
                                .rev()
                                .take_while(|s| {
                                    s.actions_executed.iter().any(|(req, res)| {
                                        !res.success
                                            && req.parameters.get("path").and_then(|p| p.as_str())
                                                == Some(target_path)
                                    })
                                })
                                .count()
                                + 1;

                            if consecutive_path_failures >= 3 {
                                let warning = format!(
                                    "\nWarning: {} consecutive edits to '{}' have failed. Recommend calling 'read_file' to verify current file content before attempting further edits.",
                                    consecutive_path_failures, target_path
                                );
                                if let Some(ref mut err) = action_result.error {
                                    err.push_str(&warning);
                                } else {
                                    action_result.error = Some(warning);
                                }
                            }
                        }

                        if call.name == "complete" || call.name == "complete_task" {
                            match self.evaluate_completion_evidence(&executed) {
                                Ok(_) => {}
                                Err(err_msg) => {
                                    action_result.success = false;
                                    action_result.error = Some(err_msg);
                                }
                            }
                        }

                        let action_success = action_result.success;
                        let action_out = action_result.output.clone();

                        tracing::debug!(
                            step = self.step_budget.steps_consumed(),
                            tool = %call.name,
                            success = action_success,
                            output_bytes = action_out.len(),
                            has_error = action_result.error.is_some(),
                            "[Runner Step Result]"
                        );

                        if (call.name == "complete" || call.name == "complete_task")
                            && action_success
                        {
                            any_complete = true;
                            final_out = action_out;
                        }

                        executed.push((action_req, action_result));
                    }

                    self.step_budget.record_step();
                    let step_end = Utc::now();
                    activity_tracker.touch().await;

                    let summary = if executed.len() == 1 {
                        format!("action:{}", executed[0].0.tool_name)
                    } else {
                        "action:multi_tool".to_string()
                    };

                    self.step_history.push(AgentStepRecord {
                        step_number: self.step_budget.steps_consumed(),
                        model_proposal_summary: summary,
                        actions_executed: executed.clone(),
                        started_at: step_start,
                        completed_at: step_end,
                        inference_duration_ms,
                        action_fingerprint: last_fingerprint,
                        tool_validation_status: Some("ok".to_string()),
                        prompt_provenance: compiled_context.prompt_provenance.clone(),
                    });

                    if any_complete {
                        match self.evaluate_completion_evidence(&executed) {
                            Ok(_) => {
                                return AgentOutcome::Succeeded {
                                    output: if !final_out.is_empty() {
                                        final_out
                                    } else {
                                        "task completed".to_string()
                                    },
                                    steps_consumed: self.step_budget.steps_consumed(),
                                };
                            }
                            Err(e) => {
                                tracing::warn!("Step completion rejected after batch: {e}");
                            }
                        }
                    }
                }

                ModelProposal::AssistantText { content } => {
                    self.step_budget.record_step();
                    let step_end = Utc::now();
                    activity_tracker.touch().await;
                    self.step_history.push(AgentStepRecord {
                        step_number: self.step_budget.steps_consumed(),
                        model_proposal_summary: format!(
                            "text:{}",
                            content.chars().take(80).collect::<String>()
                        ),
                        actions_executed: Vec::new(),
                        started_at: step_start,
                        completed_at: step_end,
                        inference_duration_ms,
                        action_fingerprint: None,
                        tool_validation_status: None,
                        prompt_provenance: compiled_context.prompt_provenance.clone(),
                    });
                    continue;
                }

                ModelProposal::AskUser { question, options } => {
                    self.step_budget.record_step();
                    let step_end = Utc::now();
                    activity_tracker.touch().await;

                    let user_action = crate::agent::action::AgentAction::AskUser {
                        question: question.clone(),
                        options: options
                            .iter()
                            .map(|o| crate::agent::action::ActionUserOption {
                                id: o.id.clone(),
                                label: o.label.clone(),
                                description: o.description.clone(),
                            })
                            .collect(),
                    };

                    let obs = match action_dispatcher.dispatch_action(&user_action).await {
                        Ok(obs) => obs,
                        Err(err) => {
                            return AgentOutcome::Failed(FailureEvidence {
                                failure_class: FailureClass::ToolFailure,
                                message: format!("ask_user dispatch failure: {err}"),
                                step_number: self.step_budget.steps_consumed(),
                                occurred_at: Utc::now(),
                                is_panic: false,
                                diagnostics: std::collections::HashMap::new(),
                            });
                        }
                    };

                    let answer = match obs {
                        crate::agent::action::AgentObservation::UserResponse { answer } => answer,
                        crate::agent::action::AgentObservation::ToolOutput {
                            output,
                            success,
                            ..
                        } => {
                            if success {
                                output
                            } else {
                                return AgentOutcome::Failed(FailureEvidence {
                                    failure_class: FailureClass::ToolFailure,
                                    message: format!("ask_user failed: {output}"),
                                    step_number: self.step_budget.steps_consumed(),
                                    occurred_at: Utc::now(),
                                    is_panic: false,
                                    diagnostics: std::collections::HashMap::new(),
                                });
                            }
                        }
                        crate::agent::action::AgentObservation::Cancelled { reason } => {
                            return AgentOutcome::Cancelled {
                                reason,
                                steps_consumed: self.step_budget.steps_consumed(),
                            };
                        }
                        other => {
                            return AgentOutcome::Failed(FailureEvidence {
                                failure_class: FailureClass::ToolFailure,
                                message: format!(
                                    "unexpected observation for ask_user: {:?}",
                                    other
                                ),
                                step_number: self.step_budget.steps_consumed(),
                                occurred_at: Utc::now(),
                                is_panic: false,
                                diagnostics: std::collections::HashMap::new(),
                            });
                        }
                    };

                    self.step_history.push(AgentStepRecord {
                        step_number: self.step_budget.steps_consumed(),
                        model_proposal_summary: format!("ask_user:{}:answer={}", question, answer),
                        actions_executed: vec![(
                            ActionRequest {
                                id: format!("ask-user-{}", self.step_budget.steps_consumed()),
                                tool_name: "ask_user".to_string(),
                                parameters: serde_json::json!({
                                    "question": question,
                                    "options": options,
                                }),
                            },
                            ActionResult {
                                action_id: format!(
                                    "ask-user-{}",
                                    self.step_budget.steps_consumed()
                                ),
                                success: true,
                                output: answer,
                                error: None,
                            },
                        )],
                        started_at: step_start,
                        completed_at: step_end,
                        inference_duration_ms,
                        action_fingerprint: None,
                        tool_validation_status: Some("answered".to_string()),
                        prompt_provenance: compiled_context.prompt_provenance.clone(),
                    });
                    continue;
                }

                ModelProposal::Handoff {
                    target_role,
                    reason,
                } => {
                    // BUG-07: Enforce delegation depth ceiling
                    if self.delegation_depth >= self.max_delegation_depth {
                        return AgentOutcome::Failed(FailureEvidence {
                            failure_class: FailureClass::PolicyDenial,
                            message: format!(
                                "Delegation rejected: maximum delegation depth of {} exceeded (current depth {})",
                                self.max_delegation_depth, self.delegation_depth
                            ),
                            step_number: self.step_budget.steps_consumed() + 1,
                            occurred_at: Utc::now(),
                            is_panic: false,
                            diagnostics: std::collections::HashMap::new(),
                        });
                    }

                    let target_agent_role =
                        crate::state_machine::agent::AgentRole::new(&target_role);
                    let target_known = crate::agent::registry::RoleRegistry::global()
                        .read()
                        .map(|guard| guard.contains(&target_agent_role))
                        .unwrap_or(false);
                    if !target_known {
                        return AgentOutcome::Failed(FailureEvidence {
                            failure_class: FailureClass::PolicyDenial,
                            message: format!(
                                "Handoff rejected: target role '{target_role}' is not registered in RoleRegistry"
                            ),
                            step_number: self.step_budget.steps_consumed() + 1,
                            occurred_at: Utc::now(),
                            is_panic: false,
                            diagnostics: std::collections::HashMap::new(),
                        });
                    }

                    if target_role
                        .trim()
                        .eq_ignore_ascii_case(self.profile.role.as_str())
                    {
                        return AgentOutcome::Failed(FailureEvidence {
                            failure_class: FailureClass::PolicyDenial,
                            message: format!(
                                "Handoff rejected: self-delegation to current role '{target_role}' is prohibited"
                            ),
                            step_number: self.step_budget.steps_consumed() + 1,
                            occurred_at: Utc::now(),
                            is_panic: false,
                            diagnostics: std::collections::HashMap::new(),
                        });
                    }

                    self.step_budget.record_step();
                    let step_end = Utc::now();
                    activity_tracker.touch().await;

                    let child_task_id = TaskId::new();

                    self.step_history.push(AgentStepRecord {
                        step_number: self.step_budget.steps_consumed(),
                        model_proposal_summary: format!("handoff:{}:{}", target_role, reason),
                        actions_executed: vec![(
                            ActionRequest {
                                id: format!("handoff-{}", self.step_budget.steps_consumed()),
                                tool_name: "delegate".to_string(),
                                parameters: serde_json::json!({
                                    "target_role": target_role,
                                    "reason": reason,
                                    "child_task_id": child_task_id.to_string(),
                                }),
                            },
                            ActionResult {
                                action_id: format!("handoff-{}", self.step_budget.steps_consumed()),
                                success: true,
                                output: format!("Delegated to role '{target_role}' (child task {child_task_id}): {reason}"),
                                error: None,
                            },
                        )],
                        started_at: step_start,
                        completed_at: step_end,
                        inference_duration_ms,
                        action_fingerprint: None,
                        tool_validation_status: Some("delegated".to_string()),
                        prompt_provenance: compiled_context.prompt_provenance.clone(),
                    });

                    let target_profile =
                        match crate::agent::registry::RoleRegistry::profile_for_global(
                            &target_agent_role,
                        ) {
                            Ok(p) => p,
                            Err(e) => {
                                return AgentOutcome::Failed(FailureEvidence {
                                    failure_class: FailureClass::ToolFailure,
                                    message: format!(
                                        "Failed to materialize profile for role '{target_role}': {e}"
                                    ),
                                    step_number: self.step_budget.steps_consumed(),
                                    occurred_at: Utc::now(),
                                    is_panic: false,
                                    diagnostics: std::collections::HashMap::new(),
                                });
                            }
                        };

                    let remaining_steps = self
                        .profile
                        .max_steps
                        .saturating_sub(self.step_budget.steps_consumed())
                        .max(1);
                    let mut child_profile = target_profile;
                    child_profile.max_steps = child_profile.max_steps.min(remaining_steps);

                    let mut child_runner = WorkerRunner::new(
                        self.mission_id,
                        crate::ids::AgentId::new(),
                        child_task_id,
                        child_profile,
                        self.context_compiler.clone(),
                    );

                    child_runner.delegation_depth = self.delegation_depth + 1;
                    child_runner.max_delegation_depth = self.max_delegation_depth;
                    child_runner.workspace_root = self.workspace_root.clone();
                    child_runner.target_surface = self.target_surface.clone();
                    child_runner.mission_objective = self.mission_objective.clone();
                    child_runner.task_objective = Some(format!("Delegated subtask: {reason}"));
                    child_runner.task_description = Some(format!(
                        "Delegated from parent task {} ({:?}): {}",
                        self.task_id, self.profile.role, reason
                    ));
                    child_runner.task_criteria = self.task_criteria.clone();
                    child_runner.requirement_keys = self.requirement_keys.clone();
                    child_runner.task_assumptions = self.task_assumptions.clone();
                    child_runner.upstream_charter = self.upstream_charter.clone();
                    child_runner.upstream_architecture = self.upstream_architecture.clone();
                    child_runner.upstream_requirements = self.upstream_requirements.clone();
                    child_runner.upstream_assumptions = self.upstream_assumptions.clone();
                    child_runner.upstream_decisions = self.upstream_decisions.clone();
                    child_runner.upstream_research_summary = self.upstream_research_summary.clone();
                    child_runner.verification = self.verification.clone();
                    child_runner.baseline_snapshots = self.baseline_snapshots.clone();
                    child_runner.step_history = Vec::new();
                    // Delegated work obeys the same pause request.
                    child_runner.pause_token = self.pause_token.clone();

                    let child_outcome = Box::pin(child_runner.run_step_loop(
                        model_caller,
                        action_dispatcher,
                        cancellation_token,
                        activity_tracker.clone(),
                    ))
                    .await;

                    let reason_lower = reason.to_lowercase();
                    let is_subtask = reason_lower.contains("subtask")
                        || reason_lower.starts_with("subtask")
                        || reason_lower.contains("delegate");

                    if is_subtask {
                        match child_outcome {
                            AgentOutcome::Succeeded {
                                output,
                                steps_consumed,
                            } => {
                                for _ in 0..steps_consumed {
                                    self.step_budget.record_step();
                                }
                                for mut step in child_runner.step_history {
                                    step.step_number = self.step_budget.steps_consumed();
                                    self.step_history.push(step);
                                }
                                self.step_history.push(AgentStepRecord {
                                    step_number: self.step_budget.steps_consumed(),
                                    model_proposal_summary: format!("subtask_completed:{target_role}"),
                                    actions_executed: vec![(
                                        ActionRequest {
                                            id: format!("subtask-res-{}", self.step_budget.steps_consumed()),
                                            tool_name: "delegate_task".to_string(),
                                            parameters: serde_json::json!({
                                                "target_role": target_role,
                                                "reason": reason,
                                                "child_task_id": child_task_id.to_string(),
                                            }),
                                        },
                                        ActionResult {
                                            action_id: format!("subtask-res-{}", self.step_budget.steps_consumed()),
                                            success: true,
                                            output: format!("Delegated subtask to '{target_role}' succeeded: {output}"),
                                            error: None,
                                        },
                                    )],
                                    started_at: step_start,
                                    completed_at: Utc::now(),
                                    inference_duration_ms: 0,
                                    action_fingerprint: None,
                                    tool_validation_status: Some("subtask_completed".to_string()),
                                    prompt_provenance: compiled_context.prompt_provenance.clone(),
                                });
                                continue;
                            }
                            AgentOutcome::Cancelled {
                                reason: cancel_reason,
                                steps_consumed,
                            } => {
                                return AgentOutcome::Cancelled {
                                    reason: cancel_reason,
                                    steps_consumed: self.step_budget.steps_consumed()
                                        + steps_consumed,
                                };
                            }
                            AgentOutcome::Failed(evidence) => {
                                return AgentOutcome::Failed(evidence);
                            }
                            other => {
                                return other;
                            }
                        }
                    } else {
                        match child_outcome {
                            AgentOutcome::Succeeded {
                                output,
                                steps_consumed,
                            } => {
                                self.step_history = child_runner.step_history;
                                return AgentOutcome::Succeeded {
                                    output,
                                    steps_consumed: self.step_budget.steps_consumed()
                                        + steps_consumed,
                                };
                            }
                            AgentOutcome::Cancelled {
                                reason: cancel_reason,
                                steps_consumed,
                            } => {
                                return AgentOutcome::Cancelled {
                                    reason: cancel_reason,
                                    steps_consumed: self.step_budget.steps_consumed()
                                        + steps_consumed,
                                };
                            }
                            other => {
                                self.step_history = child_runner.step_history;
                                return other;
                            }
                        }
                    }
                }

                ModelProposal::Complete { summary, artifacts } => {
                    self.step_budget.record_step();
                    let step_end = Utc::now();
                    activity_tracker.touch().await;

                    match self.evaluate_completion_evidence(&[]) {
                        Ok(_) => {
                            self.step_history.push(AgentStepRecord {
                                step_number: self.step_budget.steps_consumed(),
                                model_proposal_summary: format!(
                                    "complete:{} artifacts:{}",
                                    summary,
                                    artifacts.len()
                                ),
                                actions_executed: Vec::new(),
                                started_at: step_start,
                                completed_at: step_end,
                                inference_duration_ms,
                                action_fingerprint: None,
                                tool_validation_status: None,
                                prompt_provenance: compiled_context.prompt_provenance.clone(),
                            });

                            return AgentOutcome::Succeeded {
                                output: summary,
                                steps_consumed: self.step_budget.steps_consumed(),
                            };
                        }
                        Err(err_msg) => {
                            tracing::warn!(
                                step = self.step_budget.steps_consumed(),
                                summary_len = summary.len(),
                                artifacts_count = artifacts.len(),
                                "[Runner] Premature completion rejected: {err_msg}"
                            );
                            self.step_history.push(AgentStepRecord {
                                step_number: self.step_budget.steps_consumed(),
                                model_proposal_summary: format!(
                                    "complete_rejected:{} ({})",
                                    summary, err_msg
                                ),
                                actions_executed: vec![(
                                    ActionRequest {
                                        id: format!(
                                            "reject-complete-{}",
                                            self.step_budget.steps_consumed()
                                        ),
                                        tool_name: "complete".to_string(),
                                        parameters: serde_json::json!({"summary": summary}),
                                    },
                                    ActionResult {
                                        action_id: format!(
                                            "reject-complete-{}",
                                            self.step_budget.steps_consumed()
                                        ),
                                        success: false,
                                        output: String::new(),
                                        error: Some(err_msg),
                                    },
                                )],
                                started_at: step_start,
                                completed_at: step_end,
                                inference_duration_ms,
                                action_fingerprint: None,
                                tool_validation_status: Some("rejected".to_string()),
                                prompt_provenance: compiled_context.prompt_provenance.clone(),
                            });
                            continue;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::model_policy::ModelProposal;
    use crate::model::types::ModelToolCall;
    use crate::state_machine::agent::AgentRole;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct MockModelCaller {
        proposals: Vec<ModelProposal>,
        call_idx: AtomicU32,
    }

    #[async_trait]
    impl ModelCaller for MockModelCaller {
        async fn call_model(&self, _context: &str) -> Result<ModelProposal, String> {
            let idx = self.call_idx.fetch_add(1, Ordering::SeqCst) as usize;
            if idx < self.proposals.len() {
                Ok(self.proposals[idx].clone())
            } else {
                Err("mock proposals exhausted".to_string())
            }
        }
    }

    struct MockActionDispatcher;

    #[async_trait]
    impl ActionDispatcher for MockActionDispatcher {
        async fn dispatch(&self, action: &ActionRequest) -> Result<ActionResult, String> {
            Ok(ActionResult {
                action_id: action.id.clone(),
                success: true,
                output: format!("executed {}", action.tool_name),
                error: None,
            })
        }
    }

    #[tokio::test]
    async fn test_runner_executes_action_then_completion() {
        let mission_id = MissionId::new();
        let profile = AgentProfile::built_in(AgentRole::implementer());
        let mut runner =
            WorkerRunner::new_isolated_test(mission_id, AgentId::new(), TaskId::new(), profile);
        assert_eq!(runner.mission_id, mission_id);

        let model = MockModelCaller {
            proposals: vec![
                ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new(
                        "edit_file",
                        serde_json::json!({"path": "src/lib.rs"}),
                    )],
                },
                ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new("run_tests", serde_json::json!({}))],
                },
                ModelProposal::Complete {
                    summary: "task finished successfully".to_string(),
                    artifacts: Vec::new(),
                },
            ],
            call_idx: AtomicU32::new(0),
        };

        let dispatcher = MockActionDispatcher;
        let token = CancellationToken::new();
        let tracker = Arc::new(crate::agent::supervisor::ExecutionActivityTracker::default());

        let outcome = runner
            .run_step_loop(&model, &dispatcher, &token, tracker)
            .await;

        assert_eq!(
            outcome,
            AgentOutcome::Succeeded {
                output: "task finished successfully".to_string(),
                steps_consumed: 3,
            }
        );
        assert_eq!(runner.step_budget().steps_consumed(), 3);
        assert_eq!(runner.step_budget().tool_calls_consumed(), 2);
        assert_eq!(runner.step_history().len(), 3);
    }

    #[tokio::test]
    async fn test_runner_rejects_premature_completion_without_mutations() {
        let mission_id = MissionId::new();
        let mut profile = AgentProfile::built_in(AgentRole::implementer());
        profile.max_steps = 2;
        let mut runner =
            WorkerRunner::new_isolated_test(mission_id, AgentId::new(), TaskId::new(), profile);

        let model = MockModelCaller {
            proposals: vec![
                ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new(
                        "fs.read",
                        serde_json::json!({"path": "src/lib.rs"}),
                    )],
                },
                ModelProposal::Complete {
                    summary: "task finished successfully".to_string(),
                    artifacts: Vec::new(),
                },
            ],
            call_idx: AtomicU32::new(0),
        };

        let dispatcher = MockActionDispatcher;
        let token = CancellationToken::new();
        let tracker = Arc::new(crate::agent::supervisor::ExecutionActivityTracker::default());

        let outcome = runner
            .run_step_loop(&model, &dispatcher, &token, tracker)
            .await;

        assert!(matches!(outcome, AgentOutcome::StepLimitExceeded { .. }));
        assert!(
            runner
                .step_history()
                .iter()
                .any(|s| s.model_proposal_summary.contains("complete_rejected"))
        );
    }

    #[tokio::test]
    async fn test_runner_step_budget_exhaustion() {
        let mut profile = AgentProfile::built_in(AgentRole::reviewer());
        profile.max_steps = 2; // small ceiling for test

        let mut runner = WorkerRunner::new_isolated_test(
            MissionId::new(),
            AgentId::new(),
            TaskId::new(),
            profile,
        );

        let model = MockModelCaller {
            proposals: vec![
                ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new("fs.read", serde_json::json!({}))],
                },
                ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new("fs.read", serde_json::json!({}))],
                },
                ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new("fs.read", serde_json::json!({}))],
                },
            ],
            call_idx: AtomicU32::new(0),
        };

        let dispatcher = MockActionDispatcher;
        let token = CancellationToken::new();
        let tracker = Arc::new(crate::agent::supervisor::ExecutionActivityTracker::default());

        let outcome = runner
            .run_step_loop(&model, &dispatcher, &token, tracker)
            .await;

        assert_eq!(
            outcome,
            AgentOutcome::StepLimitExceeded {
                limit: 2,
                consumed: 2
            }
        );
        assert_eq!(runner.step_budget().steps_consumed(), 2);
        assert_eq!(runner.step_budget().tool_calls_consumed(), 2);
    }

    #[tokio::test]
    async fn test_runner_step_boundary_cancellation() {
        let profile = AgentProfile::built_in(AgentRole::implementer());
        let mut runner = WorkerRunner::new_isolated_test(
            MissionId::new(),
            AgentId::new(),
            TaskId::new(),
            profile,
        );

        let model = MockModelCaller {
            proposals: vec![ModelProposal::ToolCalls {
                calls: vec![ModelToolCall::new("fs.read", serde_json::json!({}))],
            }],
            call_idx: AtomicU32::new(0),
        };

        let dispatcher = MockActionDispatcher;
        let token = CancellationToken::new();
        token.cancel(); // cancel before step 1
        let tracker = Arc::new(crate::agent::supervisor::ExecutionActivityTracker::default());

        let outcome = runner
            .run_step_loop(&model, &dispatcher, &token, tracker)
            .await;

        assert_eq!(
            outcome,
            AgentOutcome::Cancelled {
                reason: "cancelled at step boundary".to_string(),
                steps_consumed: 0,
            }
        );
    }

    #[tokio::test]
    async fn test_runner_rejects_repeated_failing_action() {
        struct FailingDispatcher;
        #[async_trait]
        impl ActionDispatcher for FailingDispatcher {
            async fn dispatch(&self, action: &ActionRequest) -> Result<ActionResult, String> {
                Ok(ActionResult {
                    action_id: action.id.clone(),
                    success: false,
                    output: String::new(),
                    error: Some("Target content not found in file".to_string()),
                })
            }
        }

        let mut profile = AgentProfile::built_in(AgentRole::implementer());
        profile.max_steps = 3;
        let mut runner = WorkerRunner::new_isolated_test(
            MissionId::new(),
            AgentId::new(),
            TaskId::new(),
            profile,
        );

        // Propose identical failing action twice consecutively
        let model = MockModelCaller {
            proposals: vec![
                ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new(
                        "edit_file",
                        serde_json::json!({"path": "src/lib.rs", "old_content": "abc"}),
                    )],
                },
                ModelProposal::ToolCalls {
                    calls: vec![ModelToolCall::new(
                        "edit_file",
                        serde_json::json!({"path": "src/lib.rs", "old_content": "abc"}),
                    )],
                },
                ModelProposal::Complete {
                    summary: "done".to_string(),
                    artifacts: Vec::new(),
                },
            ],
            call_idx: AtomicU32::new(0),
        };

        let dispatcher = FailingDispatcher;
        let token = CancellationToken::new();
        let tracker = Arc::new(crate::agent::supervisor::ExecutionActivityTracker::default());

        let _ = runner
            .run_step_loop(&model, &dispatcher, &token, tracker)
            .await;

        let history = runner.step_history();
        assert_eq!(history.len(), 3);
        // Step 2 should have been rejected as repeated non-progress action
        let step2_error = history[1].actions_executed[0]
            .1
            .error
            .as_deref()
            .unwrap_or("");
        assert!(step2_error.contains("Repeated non-progress action rejected"));
        assert!(step2_error.contains("failed on step 1"));
    }

    #[test]
    #[should_panic(expected = "WorkerRunner requires a valid non-zero MissionId")]
    fn test_runner_rejects_zero_mission_id() {
        let profile = AgentProfile::built_in(AgentRole::implementer());
        let zero_mission = MissionId::from_bytes([0u8; 16]);
        let _ =
            WorkerRunner::new_isolated_test(zero_mission, AgentId::new(), TaskId::new(), profile);
    }
}
