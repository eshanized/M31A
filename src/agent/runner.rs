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
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(tool_name.trim().to_lowercase().as_bytes());
    hasher.update(b":");
    hasher.update(canonicalize_json_for_fingerprint(params).as_bytes());
    format!("{:x}", hasher.finalize())
}

fn canonicalize_json_for_fingerprint(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Object(map) => {
            let mut sorted: std::collections::BTreeMap<&String, String> =
                std::collections::BTreeMap::new();
            for (k, v) in map {
                sorted.insert(k, canonicalize_json_for_fingerprint(v));
            }
            let entries: Vec<String> = sorted
                .into_iter()
                .map(|(k, v)| format!("\"{}\":{}", k, v))
                .collect();
            format!("{{{}}}", entries.join(","))
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(canonicalize_json_for_fingerprint).collect();
            format!("[{}]", items.join(","))
        }
        serde_json::Value::String(s) => format!("\"{}\"", s),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
    }
}

/// Seam trait for dispatching side-effecting actions passing through runtime policies (D-06).
#[async_trait]
pub trait ActionDispatcher: Send + Sync {
    /// Dispatch an action request to the runtime tool engine.
    async fn dispatch(&self, action: &ActionRequest) -> Result<ActionResult, String>;
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
                        // generated-file protection, loop suppression).
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
                        // Write-tool gate (registry-driven): only roles whose
                        // definition permits write tools may invoke mutating
                        // tools. Unregistered roles fail closed here — dispatch
                        // only materializes registered profiles, so reaching this
                        // branch unregistered indicates a broken invariant.
                        } else if !crate::agent::registry::RoleRegistry::write_tools_permitted_for(
                            &self.profile.role,
                        ) && matches!(
                            call.name.as_str(),
                            "edit_file"
                                | "write_file"
                                | "apply_patch"
                                | "fs.write"
                                | "workspace_fs_write"
                        ) {
                            ActionResult {
                                action_id: action_req.id.clone(),
                                success: false,
                                output: String::new(),
                                error: Some(format!(
                                    "Role authorization failure: Tool '{}' is not permitted for role '{:?}'. The '{:?}' role is strictly read-only.",
                                    call.name, self.profile.role, self.profile.role
                                )),
                            }
                        } else if matches!(
                            call.name.as_str(),
                            "edit_file" | "write_file" | "apply_patch"
                        ) && let Some(target_path) =
                            action_req.parameters.get("path").and_then(|p| p.as_str())
                            && let ws = self
                                .workspace_root
                                .as_deref()
                                .unwrap_or(std::path::Path::new("."))
                            && let full = ws.join(target_path)
                            && full.exists()
                            && let Ok(content) = std::fs::read_to_string(&full)
                            && let Some(generator_sig) =
                                crate::change::detect_generated_file(&content, target_path)
                        {
                            ActionResult {
                                action_id: action_req.id.clone(),
                                success: false,
                                output: String::new(),
                                error: Some(format!(
                                    "Direct edit of generated file '{}' rejected (signature: {}). Modify the source generator/template instead of the generated artifact.",
                                    target_path, generator_sig
                                )),
                            }
                        } else {
                            // Sole governance path: the pipeline evaluates
                            // policy and resolves approval authoritatively.
                            let dispatch_res = action_dispatcher.dispatch(&action_req).await;
                            match dispatch_res {
                                Ok(res) => res,
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
                            let has_mutated = self.step_history.iter().any(|s| {
                                s.actions_executed.iter().any(|(req, res)| {
                                    res.success
                                        && matches!(
                                            req.tool_name.as_str(),
                                            "edit_file"
                                                | "write_file"
                                                | "apply_patch"
                                                | "fs.write"
                                                | "workspace_fs_write"
                                        )
                                })
                            });
                            if has_mutated {
                                let last_mutation_idx = self.step_history.iter().rposition(|s| {
                                    s.actions_executed.iter().any(|(req, res)| {
                                        res.success
                                            && matches!(
                                                req.tool_name.as_str(),
                                                "edit_file"
                                                    | "write_file"
                                                    | "apply_patch"
                                                    | "fs.write"
                                                    | "workspace_fs_write"
                                            )
                                    })
                                });
                                let has_verified = if let Some(mut_idx) = last_mutation_idx {
                                    self.step_history[mut_idx..].iter().any(|s| {
                                        s.actions_executed.iter().any(|(req, res)| {
                                            res.success
                                                && matches!(
                                                    req.tool_name.as_str(),
                                                    "run_tests"
                                                        | "run_linter"
                                                        | "cargo.test"
                                                        | "cargo.check"
                                                        | "test_runner"
                                                )
                                                && !res.output.contains("Failed tests:")
                                                && !res.output.contains("Compilation failed")
                                                && !res.output.contains("Compiler errors detected")
                                                && !res.output.contains("test result: FAILED")
                                                && res.error.is_none()
                                        })
                                    })
                                } else {
                                    false
                                };
                                if !has_verified && self.completion_needs_test_evidence() {
                                    action_result.success = false;
                                    action_result.error = Some("Premature completion rejected (AGENTS.md Rule 6: Completion requires evidence): Code changes have been made, but verification has not passed. You must run 'run_tests' and fix all compiler and test failures until tests pass before completing.".to_string());
                                } else {
                                    // Anti-fake diff review: inspect modified files for placeholders or fake implementations
                                    let ws = self
                                        .workspace_root
                                        .as_deref()
                                        .unwrap_or(std::path::Path::new("."));
                                    let mut modified_files = Vec::new();
                                    for s in &self.step_history {
                                        for (req, res) in &s.actions_executed {
                                            if res.success
                                                && matches!(
                                                    req.tool_name.as_str(),
                                                    "edit_file" | "write_file" | "apply_patch"
                                                )
                                                && let Some(p) =
                                                    req.parameters.get("path").and_then(|p| p.as_str())
                                            {
                                                let clean = p
                                                    .trim()
                                                    .trim_start_matches('@')
                                                    .trim_start_matches("./")
                                                    .to_string();
                                                if !modified_files.contains(&clean) {
                                                    modified_files.push(clean);
                                                }
                                            }
                                        }
                                    }
                                    if !modified_files.is_empty() {
                                        let surface = crate::kernel::change::ChangeSurface::new(
                                            modified_files.clone(),
                                        );
                                        let mut synthesized_diff = String::new();
                                        for file in &modified_files {
                                            if let Ok(bytes) = std::fs::read(ws.join(file)) {
                                                let file_diff =
                                                    crate::change::observer::PostMutationObserver::generate_file_unified_diff(
                                                        file,
                                                        None,
                                                        Some(&bytes),
                                                    );
                                                synthesized_diff.push_str(&file_diff);
                                            }
                                        }
                                        let review = crate::change::review::DiffReviewer::review(
                                            &synthesized_diff,
                                            &surface,
                                            &modified_files,
                                        );
                                        if !review.passed {
                                            action_result.success = false;
                                            action_result.error = Some(format!(
                                                "Premature completion rejected (AGENTS.md Rule 5: Never fake success): Diff review detected quality violations in changes: {:?}",
                                                review.violations
                                            ));
                                        }
                                    }
                                }
                            // Modification-required gate (registry-driven): roles whose
                            // definition requires observed workspace modification
                            // (implementer-class) cannot complete without it.
                            // Unregistered roles fail open here only in the sense
                            // of skipping this role-specific check — the write
                            // gate above already denied them any mutation.
                            } else if crate::agent::registry::RoleRegistry::requires_modification_for(
                                &self.profile.role,
                            ) {
                                action_result.success = false;
                                action_result.error = Some("Premature completion rejected (AGENTS.md Rule 5 & 6): Implementer tasks require making the code changes specified by the objective before completing. You have not called edit_file or write_file to modify the workspace. Please inspect the code and execute edit_file/write_file to implement the required changes.".to_string());
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
                        actions_executed: executed,
                        started_at: step_start,
                        completed_at: step_end,
                        inference_duration_ms,
                        action_fingerprint: last_fingerprint,
                        tool_validation_status: Some("ok".to_string()),
                        prompt_provenance: compiled_context.prompt_provenance.clone(),
                    });

                    if any_complete {
                        return AgentOutcome::Succeeded {
                            output: if !final_out.is_empty() {
                                final_out
                            } else {
                                "task completed".to_string()
                            },
                            steps_consumed: self.step_budget.steps_consumed(),
                        };
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

                ModelProposal::AskUser { question, .. } => {
                    self.step_budget.record_step();
                    let step_end = Utc::now();
                    activity_tracker.touch().await;
                    self.step_history.push(AgentStepRecord {
                        step_number: self.step_budget.steps_consumed(),
                        model_proposal_summary: format!("ask_user:{}", question),
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

                ModelProposal::Handoff {
                    target_role,
                    reason,
                } => {
                    // Handoff proposals never complete a task (false-success prevention).
                    // Delegation execution lives behind the handoff arbiter, which is not
                    // wired into the step loop; until it is, the runtime decides that the
                    // current assignment continues. The proposal is preserved in step
                    // history (visible to future steps) and the loop continues under the
                    // existing step budget. An unknown target role is recorded as a
                    // rejection, never success.
                    let target_known = crate::agent::registry::RoleRegistry::global()
                        .read()
                        .map(|guard| {
                            guard.contains(&crate::state_machine::agent::AgentRole::new(
                                &target_role,
                            ))
                        })
                        .unwrap_or(false);
                    self.step_budget.record_step();
                    let step_end = Utc::now();
                    activity_tracker.touch().await;

                    let summary = if target_known {
                        format!("handoff:{}:{}", target_role, reason)
                    } else {
                        format!(
                            "handoff-rejected:unknown-target-role '{}' (task continues): {}",
                            target_role, reason
                        )
                    };
                    self.step_history.push(AgentStepRecord {
                        step_number: self.step_budget.steps_consumed(),
                        model_proposal_summary: summary,
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

                ModelProposal::Complete { summary, artifacts } => {
                    self.step_budget.record_step();
                    let step_end = Utc::now();
                    activity_tracker.touch().await;

                    let has_mutated = self.step_history.iter().any(|s| {
                        s.actions_executed.iter().any(|(req, res)| {
                            res.success
                                && matches!(
                                    req.tool_name.as_str(),
                                    "edit_file"
                                        | "write_file"
                                        | "apply_patch"
                                        | "fs.write"
                                        | "workspace_fs_write"
                                )
                        })
                    });

                    if has_mutated {
                        let last_mutation_idx = self.step_history.iter().rposition(|s| {
                            s.actions_executed.iter().any(|(req, res)| {
                                res.success
                                    && matches!(
                                        req.tool_name.as_str(),
                                        "edit_file"
                                            | "write_file"
                                            | "apply_patch"
                                            | "fs.write"
                                            | "workspace_fs_write"
                                    )
                            })
                        });
                        let has_verified = if let Some(mut_idx) = last_mutation_idx {
                            self.step_history[mut_idx..].iter().any(|s| {
                                s.actions_executed.iter().any(|(req, res)| {
                                    res.success
                                        && matches!(
                                            req.tool_name.as_str(),
                                            "run_tests"
                                                | "run_linter"
                                                | "cargo.test"
                                                | "cargo.check"
                                                | "test_runner"
                                        )
                                        && !res.output.contains("Failed tests:")
                                        && !res.output.contains("Compilation failed")
                                        && !res.output.contains("Compiler errors detected")
                                        && !res.output.contains("test result: FAILED")
                                        && res.error.is_none()
                                })
                            })
                        } else {
                            false
                        };
                        if !has_verified && self.completion_needs_test_evidence() {
                            let err_msg = "Premature completion rejected (AGENTS.md Rule 6: Completion requires evidence): Code changes have been made, but verification has not passed. You must run 'run_tests' and fix all compiler and test failures until tests pass before completing.".to_string();
                            tracing::warn!(
                                step = self.step_budget.steps_consumed(),
                                summary_len = summary.len(),
                                artifacts_count = artifacts.len(),
                                "[Runner] Premature completion rejected (verification required)"
                            );
                            self.step_history.push(AgentStepRecord {
                                step_number: self.step_budget.steps_consumed(),
                                model_proposal_summary: format!(
                                    "complete_rejected:{} (verification required)",
                                    summary
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
                    // Same modification-required gate for the no-modification
                    // path (registry-driven; see above).
                    } else if crate::agent::registry::RoleRegistry::requires_modification_for(
                        &self.profile.role,
                    ) {
                        let is_commentary = summary.starts_with("The function")
                            || summary.starts_with("The problem")
                            || summary.starts_with("Based on")
                            || summary.starts_with("It seems")
                            || summary.starts_with("To fix")
                            || summary.starts_with("The code still")
                            || summary.contains("should be called")
                            || summary.contains("run_tests");

                        let err_msg = if is_commentary {
                            "No tool call executed: You provided plain text commentary instead of executing a tool action. To complete work on this task, you MUST invoke a tool (such as 'read_file', 'edit_file', or 'run_tests') directly using structured tool calls or JSON. Do not reply with conversational text or advice.".to_string()
                        } else {
                            "Premature completion rejected (AGENTS.md Rule 5 & 6): Implementer tasks require making the code changes specified by the objective before completing. You have not called edit_file or write_file to modify the workspace. Please inspect the code and execute edit_file/write_file to implement the required changes.".to_string()
                        };
                        tracing::warn!(
                            step = self.step_budget.steps_consumed(),
                            summary_len = summary.len(),
                            artifacts_count = artifacts.len(),
                            "[Runner] Premature completion rejected (no modifications made)"
                        );
                        self.step_history.push(AgentStepRecord {
                            step_number: self.step_budget.steps_consumed(),
                            model_proposal_summary: format!(
                                "complete_rejected:{} (no modifications made)",
                                summary
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
        let mut runner = WorkerRunner::new_isolated_test(mission_id, AgentId::new(), TaskId::new(), profile);
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
        let mut runner = WorkerRunner::new_isolated_test(mission_id, AgentId::new(), TaskId::new(), profile);

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

        let mut runner =
            WorkerRunner::new_isolated_test(MissionId::new(), AgentId::new(), TaskId::new(), profile);

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
        let mut runner =
            WorkerRunner::new_isolated_test(MissionId::new(), AgentId::new(), TaskId::new(), profile);

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
        let mut runner =
            WorkerRunner::new_isolated_test(MissionId::new(), AgentId::new(), TaskId::new(), profile);

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
        let _ = WorkerRunner::new_isolated_test(zero_mission, AgentId::new(), TaskId::new(), profile);
    }
}
