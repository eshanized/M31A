pub mod budget_tracker;
pub mod dependencies;
pub mod error;
pub mod halting;
pub mod loop_detector;
pub mod progress;
pub mod stage;

#[cfg(test)]
pub mod harness;

pub use budget_tracker::{BudgetKind, BudgetTracker};
pub use dependencies::ControllerDependencies;
pub use error::ControllerError;
pub use halting::{ControllerHaltReason, HaltEvaluation};
pub use loop_detector::{LoopDetector, LoopSignature};
pub use progress::{ControllerProgress, LoopStage};
pub use stage::StageOutcome;

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use crate::checkpoint::manager::CheckpointManager;
use crate::checkpoint::manifest::CheckpointManifest;
use crate::events::bus::EventBus;
use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::ids::{AgentId, CheckpointId, MissionId, TaskId};
use crate::kernel::seams;
use crate::kernel::seams::{
    CompletionGateOutcome, ContextCompilationRequest, FailureClassification,
    FailureClassificationRequest, PlanRequest, PolicyEvaluationRequest, RecoveryAction,
    RecoveryStrategyRequest, ReplanRequest, TaskVerificationRequest, VerificationOutcome,
    WorkExecutionHandle, WorkExecutionRequest, WorkExecutionResult, WorkItem,
};
use crate::state::budget::ResourceBudget;
use crate::state::intake::AutonomyMode;

/// The Autonomy Controller coordinates the closed-loop execution lifecycle (D-01, AUT-01, AUT-04, AUT-05).
pub struct AutonomyController {
    pub mission_id: MissionId,
    pub mode: AutonomyMode,
    pub dependencies: ControllerDependencies,
    pub progress: ControllerProgress,
    pub budget_tracker: BudgetTracker,
    pub budget_limits: ResourceBudget,
    pub loop_detector: LoopDetector,
    pub max_cycles: Option<u64>,
    pub event_bus: Arc<dyn EventBus>,
    pub cancellation_token: CancellationToken,

    // Ephemeral per-cycle task tracking state
    pub active_task: Option<WorkItem>,
    pub active_agent: Option<AgentId>,
    pub active_context_id: Option<String>,
    pub active_handle: Option<WorkExecutionHandle>,
    pub active_receipt: Option<crate::budget::ReservationReceipt>,
    pub last_execution_result: Option<WorkExecutionResult>,
    pub last_failure_class: Option<FailureClassification>,
    pub mission_objective: Option<String>,
    pub workspace_root: Option<std::path::PathBuf>,
    pub step_history: Vec<crate::kernel::seams::context::StepRecordDto>,
    pub upstream_context: Option<crate::kernel::seams::planner::UpstreamPlanContext>,
    /// Explicit worker role bound to policy evaluation for this controller.
    /// `None` fails closed at the policy gate (no silent role default).
    pub policy_role: Option<crate::state_machine::agent::AgentRole>,
}

impl AutonomyController {
    pub fn new(
        mission_id: MissionId,
        mode: AutonomyMode,
        dependencies: ControllerDependencies,
        event_bus: Arc<dyn EventBus>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self::with_budget(
            mission_id,
            mode,
            dependencies,
            ResourceBudget::default(),
            None,
            event_bus,
            cancellation_token,
        )
    }

    pub fn with_budget(
        mission_id: MissionId,
        mode: AutonomyMode,
        dependencies: ControllerDependencies,
        budget_limits: ResourceBudget,
        max_cycles: Option<u64>,
        event_bus: Arc<dyn EventBus>,
        cancellation_token: CancellationToken,
    ) -> Self {
        Self {
            mission_id,
            mode,
            dependencies,
            progress: ControllerProgress::new(),
            budget_tracker: BudgetTracker::new(),
            budget_limits,
            loop_detector: LoopDetector::default(),
            max_cycles,
            event_bus,
            cancellation_token,
            active_task: None,
            active_agent: None,
            active_context_id: None,
            active_handle: None,
            active_receipt: None,
            last_execution_result: None,
            last_failure_class: None,
            mission_objective: None,
            workspace_root: None,
            step_history: Vec::new(),
            upstream_context: None,
            policy_role: None,
        }
    }

    pub fn with_mission_objective(mut self, objective: impl Into<String>) -> Self {
        self.mission_objective = Some(objective.into());
        self
    }

    pub fn with_workspace_root(mut self, root: impl Into<std::path::PathBuf>) -> Self {
        self.workspace_root = Some(root.into());
        self
    }

    /// Bind the explicit worker role used for policy evaluation.
    /// Production callers MUST set this; `None` fails closed at the gate.
    pub fn with_policy_role(mut self, role: crate::state_machine::agent::AgentRole) -> Self {
        self.policy_role = Some(role);
        self
    }

    pub fn with_step_history(
        mut self,
        history: Vec<crate::kernel::seams::context::StepRecordDto>,
    ) -> Self {
        self.step_history = history;
        self
    }

    pub fn with_upstream_context(
        mut self,
        context: crate::kernel::seams::planner::UpstreamPlanContext,
    ) -> Self {
        self.upstream_context = Some(context);
        self
    }

    pub fn effective_workspace_root(&self) -> Option<&std::path::Path> {
        self.workspace_root
            .as_deref()
            .or(self.dependencies.workspace_root())
    }

    /// Build the canonical typed policy request for task execution gating.
    ///
    /// Fails closed (`Err`) when role, mode binding, or workspace identity is
    /// missing — never silently defaults.
    fn task_policy_request(
        &self,
        task_id: TaskId,
    ) -> Result<PolicyEvaluationRequest, ControllerError> {
        let role = self.policy_role.clone().ok_or(ControllerError::SeamError {
            seam: "policy".into(),
            message: "missing security-critical policy attribute: agent_role is required"
                .to_string(),
        })?;
        let workspace = self
            .effective_workspace_root()
            .ok_or(ControllerError::SeamError {
                seam: "policy".into(),
                message: "missing security-critical policy attribute: workspace_root is required"
                    .to_string(),
            })?;
        let mut req = PolicyEvaluationRequest::new(self.mission_id, task_id, "execute_task")
            .with_role(role)
            .with_autonomy_mode(self.mode)
            .with_workspace(workspace.to_path_buf());
        if let Some(hash) = self.dependencies.policy().policy_hash() {
            req = req.with_policy_hash(hash);
        }
        if let Some(agent_id) = self.active_agent {
            req = req.with_agent_id(agent_id);
        }
        Ok(req)
    }

    /// Authorize a recovery mutation through the canonical policy lifecycle.
    ///
    /// A recovery proposal is still a proposal: repair and rollback obey the
    /// same authority boundary as model-generated mutations. Only an explicit
    /// `Allow` (including `Ask` resolved to `Allow` by a verified durable
    /// session grant inside policy evaluation) authorizes the mutation. Any
    /// other decision, or any missing security-critical attribute, fails
    /// closed. There is no "trusted recovery" bypass.
    async fn authorize_recovery_mutation(
        &self,
        task_id: TaskId,
        tool_action: &str,
        target_paths: Vec<std::path::PathBuf>,
        args: serde_json::Value,
    ) -> Result<(), ControllerError> {
        use crate::kernel::seams::policy::PolicyDecision;
        let role = self.policy_role.clone().ok_or(ControllerError::SeamError {
            seam: "policy".into(),
            message: "missing security-critical policy attribute: agent_role is required for recovery authorization".to_string(),
        })?;
        let workspace = self.effective_workspace_root().ok_or(ControllerError::SeamError {
            seam: "policy".into(),
            message: "missing security-critical policy attribute: workspace_root is required for recovery authorization".to_string(),
        })?;
        let mut req = PolicyEvaluationRequest::new(self.mission_id, task_id, tool_action)
            .with_role(role)
            .with_autonomy_mode(self.mode)
            .with_workspace(workspace.to_path_buf())
            .with_target_paths(target_paths)
            .with_arguments(args);
        if let Some(hash) = self.dependencies.policy().policy_hash() {
            req = req.with_policy_hash(hash);
        }
        if let Some(agent_id) = self.active_agent {
            req = req.with_agent_id(agent_id);
        }
        let decision = self
            .dependencies
            .policy()
            .evaluate(req)
            .await
            .map_err(|e| ControllerError::SeamError {
                seam: "policy".into(),
                message: e.to_string(),
            })?;
        if decision != PolicyDecision::Allow {
            return Err(ControllerError::SeamError {
                seam: "policy".into(),
                message: format!(
                    "recovery mutation '{tool_action}' denied by policy: decision is {decision:?}, not Allow"
                ),
            });
        }
        Ok(())
    }

    /// Mint a bound Git gate for a recovery mutation (rollback / repair
    /// staging) after policy authorization. Fails closed without a wired
    /// authorization authority or live policy generation hash.
    ///
    /// Like runtime orchestration, the policy check carries workspace
    /// identity but no target paths (the gate binds the exact operation and
    /// workspace; file-target vetoes apply to repair proposals, not to
    /// bounded restores).
    async fn authorize_recovery_git(
        &self,
        task_id: TaskId,
        tool_action: &str,
        workspace: std::path::PathBuf,
        operation: crate::git::GitOperation,
        additional_operations: Vec<crate::git::GitOperation>,
        resource_scope: &str,
    ) -> Result<crate::git::GitGate, ControllerError> {
        self.authorize_recovery_mutation(
            task_id,
            tool_action,
            Vec::new(),
            serde_json::json!({"path": workspace.display().to_string()}),
        )
        .await?;
        let authority =
            self.dependencies
                .auth_authority()
                .cloned()
                .ok_or(ControllerError::SeamError {
                    seam: "policy".into(),
                    message: "missing runtime authorization authority for recovery git mutation"
                        .to_string(),
                })?;
        let policy_hash =
            self.dependencies
                .policy()
                .policy_hash()
                .ok_or(ControllerError::SeamError {
                    seam: "policy".into(),
                    message: "missing policy generation hash for recovery git mutation".to_string(),
                })?;
        let auth = authority.mint_git_authorization(
            self.mission_id,
            Some(task_id),
            self.active_agent,
            policy_hash,
            workspace,
            operation,
            additional_operations,
            resource_scope,
            format!("recovery:policy-allow:{tool_action}"),
            crate::git::GIT_AUTH_DEFAULT_TTL,
        );
        crate::git::GitGate::authorized_verified(auth, &authority).map_err(|e| {
            ControllerError::SeamError {
                seam: "policy".into(),
                message: format!("recovery git authorization rejected: {e}"),
            }
        })
    }

    /// Emits a domain event to the broadcast bus.
    async fn emit_event(&self, event_type: EventType) {
        let envelope = EventEnvelope::new(
            0,
            Some(self.mission_id),
            None,
            "autonomy_controller".to_string(),
            event_type,
        );
        let _ = self.event_bus.publish(envelope).await;
    }

    /// Transitions the autonomy mode strictly at cycle boundaries before side-effecting operations (D-16, Edge 3).
    pub async fn set_autonomy_mode(
        &mut self,
        new_mode: AutonomyMode,
    ) -> Result<(), ControllerError> {
        if self.progress.current_stage != LoopStage::Observe {
            return Err(ControllerError::InvalidStageTransition {
                from: self.progress.current_stage,
                attempted: "SetAutonomyMode mid-cycle".to_string(),
            });
        }
        let old_mode = self.mode;
        self.mode = new_mode;
        self.emit_event(EventType::ControllerDecisionModeChanged {
            mission_id: self.mission_id,
            from: old_mode,
            to: new_mode,
        })
        .await;
        Ok(())
    }

    /// Layer 1: Execute a single stage and return its outcome (D-01).
    pub async fn step(&mut self) -> Result<StageOutcome, ControllerError> {
        if self.cancellation_token.is_cancelled() {
            let reason = ControllerHaltReason::Cancelled;
            self.emit_event(EventType::ControllerHalted {
                mission_id: self.mission_id,
                reason: format!("{reason:?}"),
            })
            .await;
            return Ok(StageOutcome::Halt(reason));
        }

        let current = self.progress.current_stage;

        // Emit cycle started event when at the beginning of a cycle
        if current == LoopStage::Observe {
            self.emit_event(EventType::ControllerCycleStarted {
                mission_id: self.mission_id,
                cycle: self.progress.cycle,
            })
            .await;
        }

        // Max cycles check at Observe boundary
        if current == LoopStage::Observe
            && let Some(limit) = self.max_cycles
            && self.progress.cycle >= limit
        {
            let outcome = StageOutcome::Halt(ControllerHaltReason::MaxCyclesExceeded { limit });
            StageOutcome::validate_transition(current, &outcome)?;
            self.emit_event(EventType::ControllerHalted {
                mission_id: self.mission_id,
                reason: format!("{outcome:?}"),
            })
            .await;
            return Ok(outcome);
        }

        let outcome = match current {
            LoopStage::Observe => {
                // Layered pre-flight budget check
                if let Err(kind) = self
                    .budget_tracker
                    .check_preflight_wall_clock(&self.budget_limits)
                {
                    StageOutcome::Halt(ControllerHaltReason::BudgetExhausted { kind })
                } else {
                    let has_plan = self
                        .dependencies
                        .planner()
                        .has_valid_plan(self.mission_id)
                        .await
                        .map_err(|e| ControllerError::SeamError {
                            seam: "planner".into(),
                            message: e.to_string(),
                        })?;

                    if !has_plan {
                        // Declarative authority boundary: plan regeneration
                        // must not discard, duplicate, or re-invent work owned
                        // by another runtime authority. The planner seam runs
                        // ONLY when the mission has no plan, no scheduler work
                        // (ready or blocked), and no completed work. Otherwise
                        // the controller proceeds with the materialized graph
                        // (workflow-engine lowering) or the terminal state (all
                        // work done). A missing model then fails explicitly
                        // instead of fabricating tasks.
                        let ready_resp = self
                            .dependencies
                            .scheduler()
                            .find_ready_work(self.mission_id)
                            .await
                            .map_err(|e| ControllerError::SeamError {
                                seam: "scheduler".into(),
                                message: e.to_string(),
                            })?;
                        let work_complete = self
                            .dependencies
                            .scheduler()
                            .is_work_complete(self.mission_id)
                            .await
                            .map_err(|e| ControllerError::SeamError {
                                seam: "scheduler".into(),
                                message: e.to_string(),
                            })?;
                        let has_scheduler_work = !ready_resp.ready_tasks.is_empty()
                            || ready_resp.blocked_tasks_count > 0
                            || work_complete;

                        if !has_scheduler_work {
                            let objective = if let Some(repo) = self.dependencies.mission_repo() {
                                repo.get(self.mission_id)
                                    .await
                                    .ok()
                                    .flatten()
                                    .map(|m| m.objective)
                                    .unwrap_or_else(|| "Execute Mission".into())
                            } else {
                                "Execute Mission".into()
                            };

                            let mut plan_req = PlanRequest::new(self.mission_id, objective);
                            if let Some(ref upstream) = self.upstream_context {
                                plan_req = plan_req.with_upstream_context(upstream.clone());
                            }

                            let plan_resp = self
                                .dependencies
                                .planner()
                                .generate_initial_plan(plan_req)
                                .await
                                .map_err(|e| ControllerError::SeamError {
                                    seam: "planner".into(),
                                    message: e.to_string(),
                                })?;

                            // Materialize candidate plan into authoritative task graph (AUT-01, PLN-05, DAG-06, FINDING-01)
                            self.dependencies
                                .scheduler()
                                .materialize_plan(self.mission_id, &plan_resp.candidate_plan)
                                .await
                                .map_err(|e| ControllerError::SeamError {
                                    seam: "scheduler".into(),
                                    message: e.to_string(),
                                })?;
                        }
                    }
                    StageOutcome::Advance(LoopStage::IdentifyReadyWork)
                }
            }

            LoopStage::IdentifyReadyWork => {
                let ready_resp = self
                    .dependencies
                    .scheduler()
                    .find_ready_work(self.mission_id)
                    .await
                    .map_err(|e| ControllerError::SeamError {
                        seam: "scheduler".into(),
                        message: e.to_string(),
                    })?;

                if let Some(task) = ready_resp.ready_tasks.into_iter().next() {
                    self.active_task = Some(task);
                    StageOutcome::Advance(LoopStage::ValidatePolicyAndResources)
                } else {
                    let is_complete = self
                        .dependencies
                        .scheduler()
                        .is_work_complete(self.mission_id)
                        .await
                        .map_err(|e| ControllerError::SeamError {
                            seam: "scheduler".into(),
                            message: e.to_string(),
                        })?;

                    if is_complete {
                        self.active_task = None;
                        StageOutcome::SkipTo(LoopStage::Verify)
                    } else if self.active_handle.is_none() && ready_resp.blocked_tasks_count > 0 {
                        StageOutcome::Halt(ControllerHaltReason::Blocked)
                    } else {
                        StageOutcome::Yield(None)
                    }
                }
            }

            LoopStage::ValidatePolicyAndResources => {
                if let Some(ref task) = self.active_task {
                    // Single authoritative admission: the BudgetEnforcer owns
                    // token/cost/worker/artifact admission. No independent
                    // tracker pre-check (that would be a second accounting
                    // universe that can disagree with the enforcer).
                    if let Some(enforcer) = self.dependencies.budget_enforcer() {
                        let estimates = crate::budget::enforcer::TaskEstimates {
                            estimated_tokens: task.estimated_tokens,
                            estimated_cost_usd: 0.01,
                            requires_worker: true,
                            estimated_artifact_bytes: 4096,
                        };
                        match enforcer.reserve(&estimates, false) {
                            Ok(receipt) => {
                                self.active_receipt = Some(receipt);
                                let req = match self.task_policy_request(task.task_id) {
                                    Ok(r) => r,
                                    Err(e) => {
                                        if let (Some(enforcer), Some(receipt)) = (
                                            self.dependencies.budget_enforcer(),
                                            self.active_receipt.take(),
                                        ) {
                                            enforcer.release_reservation(&receipt);
                                        }
                                        return Err(e);
                                    }
                                };

                                let decision =
                                    self.dependencies.policy().evaluate(req).await.map_err(
                                        |e| ControllerError::SeamError {
                                            seam: "policy".into(),
                                            message: e.to_string(),
                                        },
                                    )?;

                                let resolved = seams::resolve_decision(
                                    self.mode,
                                    decision,
                                    "policy evaluation",
                                    true,
                                );
                                match resolved {
                                    seams::ResolvedAction::Proceed => {
                                        StageOutcome::Advance(LoopStage::AllocateWorkers)
                                    }
                                    seams::ResolvedAction::Deny { reason: _ } => {
                                        if let (Some(enforcer), Some(receipt)) = (
                                            self.dependencies.budget_enforcer(),
                                            self.active_receipt.take(),
                                        ) {
                                            enforcer.release_reservation(&receipt);
                                        }
                                        StageOutcome::SkipTo(LoopStage::ClassifyFailure)
                                    }
                                    seams::ResolvedAction::PauseForApproval { reason: _ } => {
                                        // A reservation held across a Pause
                                        // halt would leak (workers, tokens, cost,
                                        // and bytes held forever). Release
                                        // before halting; resume re-reserves on
                                        // next cycle.
                                        if let (Some(enforcer), Some(receipt)) = (
                                            self.dependencies.budget_enforcer(),
                                            self.active_receipt.take(),
                                        ) {
                                            enforcer.release_reservation(&receipt);
                                        }
                                        StageOutcome::Halt(ControllerHaltReason::Paused)
                                    }
                                    seams::ResolvedAction::Escalate { reason: _ } => {
                                        if let (Some(enforcer), Some(receipt)) = (
                                            self.dependencies.budget_enforcer(),
                                            self.active_receipt.take(),
                                        ) {
                                            enforcer.release_reservation(&receipt);
                                        }
                                        StageOutcome::Halt(ControllerHaltReason::Blocked)
                                    }
                                }
                            }
                            Err(action) => match action {
                                crate::budget::BudgetExhaustionAction::PauseForApproval => {
                                    StageOutcome::Halt(ControllerHaltReason::Paused)
                                }
                                crate::budget::BudgetExhaustionAction::Block => {
                                    StageOutcome::Halt(ControllerHaltReason::Blocked)
                                }
                                crate::budget::BudgetExhaustionAction::FailClosed => {
                                    StageOutcome::Halt(ControllerHaltReason::BudgetExhausted {
                                        kind: BudgetKind::Tokens,
                                    })
                                }
                            },
                        }
                    } else {
                        let req = match self.task_policy_request(task.task_id) {
                            Ok(r) => r,
                            Err(e) => return Err(e),
                        };

                        let decision =
                            self.dependencies
                                .policy()
                                .evaluate(req)
                                .await
                                .map_err(|e| ControllerError::SeamError {
                                    seam: "policy".into(),
                                    message: e.to_string(),
                                })?;

                        let resolved =
                            seams::resolve_decision(self.mode, decision, "policy evaluation", true);
                        match resolved {
                            seams::ResolvedAction::Proceed => {
                                StageOutcome::Advance(LoopStage::AllocateWorkers)
                            }
                            seams::ResolvedAction::Deny { reason: _ } => {
                                StageOutcome::SkipTo(LoopStage::ClassifyFailure)
                            }
                            seams::ResolvedAction::PauseForApproval { reason: _ } => {
                                StageOutcome::Halt(ControllerHaltReason::Paused)
                            }
                            seams::ResolvedAction::Escalate { reason: _ } => {
                                StageOutcome::Halt(ControllerHaltReason::Blocked)
                            }
                        }
                    }
                } else {
                    StageOutcome::SkipTo(LoopStage::ClassifyFailure)
                }
            }

            LoopStage::AllocateWorkers => {
                if let Some(ref task) = self.active_task {
                    // The worker slot is already held by the enforcer receipt
                    // taken in ValidatePolicyAndResources: allocating here
                    // consumes that reservation, never a second independent
                    // slot. Only verify the receipt is still live.
                    if self.active_receipt.is_none()
                        && self.dependencies.budget_enforcer().is_some()
                    {
                        return Ok(StageOutcome::SkipTo(LoopStage::ClassifyFailure));
                    }
                    let alloc_res = self
                        .dependencies
                        .dispatcher()
                        .allocate_worker(task.task_id, self.mission_id, &task.required_capabilities)
                        .await;

                    let agent_id = match alloc_res {
                        Ok(id) => id,
                        Err(e) => {
                            if let (Some(enforcer), Some(receipt)) = (
                                self.dependencies.budget_enforcer(),
                                self.active_receipt.take(),
                            ) {
                                enforcer.release_reservation(&receipt);
                            }
                            // GAP-02: Recoverable allocation failures must NOT crash the mission controller with fatal SeamError
                            let err_msg = format!("worker allocation failed: {e}");
                            self.last_execution_result = Some(WorkExecutionResult {
                                task_id: task.task_id,
                                success: false,
                                output: String::new(),
                                error_detail: Some(err_msg.clone()),
                                token_usage: None,
                            });
                            let _ = self
                                .dependencies
                                .scheduler()
                                .mark_task_failed(task.task_id, err_msg, true)
                                .await;
                            return Ok(StageOutcome::Advance(LoopStage::ClassifyFailure));
                        }
                    };

                    self.dependencies
                        .scheduler()
                        .mark_task_started(task.task_id, agent_id)
                        .await
                        .map_err(|e| ControllerError::SeamError {
                            seam: "scheduler".into(),
                            message: e.to_string(),
                        })?;

                    self.active_agent = Some(agent_id);
                    StageOutcome::Advance(LoopStage::CompileContext)
                } else {
                    StageOutcome::Yield(None)
                }
            }

            LoopStage::CompileContext => {
                if let Some(ref task) = self.active_task {
                    let mut comp_req =
                        ContextCompilationRequest::new(self.mission_id, task.task_id, 16384)
                            .with_task_objective(task.title.clone())
                            .with_step_history(self.step_history.clone())
                            .with_task_criteria(task.completion_criteria.clone())
                            .with_task_requirement_keys(task.requirement_keys.clone())
                            .with_task_assumptions(task.assumptions.clone());
                    // Typed prompt execution binding: the work item's
                    // workflow/task reference flows into context
                    // compilation with task-over-role precedence. It is
                    // NEVER downgraded into description text.
                    if let Some(ref prompt_ref) = task.prompt_ref {
                        comp_req = comp_req
                            .with_prompt_ref(prompt_ref.clone())
                            .with_prompt_source(
                                crate::kernel::seams::context::PromptSelectionSource::ExplicitTask,
                            );
                    }
                    if let Some(agent_id) = self.active_agent {
                        comp_req = comp_req.with_agent_id(agent_id);
                    }
                    if let Some(ref desc) = task.description {
                        comp_req = comp_req.with_task_description(desc.clone());
                    }

                    if let Some(ref obj) = self.mission_objective {
                        comp_req = comp_req.with_mission_objective(obj.clone());
                    }
                    // Upstream planning artifacts reach task context here as
                    // well, not only via the worker path.
                    if let Some(ref upstream) = self.upstream_context {
                        comp_req = comp_req
                            .with_upstream_charter(upstream.charter.clone())
                            .with_upstream_architecture(upstream.architecture.clone())
                            .with_upstream_requirements(upstream.requirements.clone())
                            .with_upstream_assumptions(upstream.assumptions.clone())
                            .with_upstream_decisions(upstream.decisions.clone());
                        if let Some(ref summary) = upstream.research_summary {
                            comp_req = comp_req.with_upstream_research_summary(summary.clone());
                        }
                    }

                    let compiled = self
                        .dependencies
                        .context()
                        .compile_context(comp_req)
                        .await
                        .map_err(|e| ControllerError::SeamError {
                            seam: "context".into(),
                            message: e.to_string(),
                        })?;

                    self.active_context_id = Some(compiled.context_id);
                    StageOutcome::Advance(LoopStage::ExecuteBoundedWork)
                } else {
                    StageOutcome::SkipTo(LoopStage::ClassifyFailure)
                }
            }

            LoopStage::ExecuteBoundedWork => {
                if let (Some(task), Some(agent_id), Some(ctx_id)) = (
                    &self.active_task,
                    self.active_agent,
                    &self.active_context_id,
                ) {
                    let mut work_req = WorkExecutionRequest::new(
                        self.mission_id,
                        task.task_id,
                        agent_id,
                        ctx_id.clone(),
                    )
                    .with_task_objective(task.title.clone())
                    .with_task_description_opt(task.description.clone())
                    .with_task_criteria(task.completion_criteria.clone())
                    .with_requirement_keys(task.requirement_keys.clone())
                    .with_task_assumptions(task.assumptions.clone())
                    .with_verification_opt(task.verification.clone())
                    // Typed prompt execution binding: the durable work
                    // item's workflow/task reference reaches the worker.
                    .with_prompt_ref_opt(task.prompt_ref.clone());

                    if let Some(ref obj) = self.mission_objective {
                        work_req = work_req.with_mission_objective(obj.clone());
                    }

                    // Upstream planning artifacts travel with the work item so
                    // the worker sees target requirements, architecture, and
                    // research — not just the raw mission sentence (Gap 2).
                    if let Some(ref upstream) = self.upstream_context {
                        work_req = work_req
                            .with_upstream_charter_opt(Some(upstream.charter.clone()))
                            .with_upstream_architecture_opt(Some(upstream.architecture.clone()))
                            .with_upstream_requirements(upstream.requirements.clone())
                            .with_upstream_assumptions(upstream.assumptions.clone())
                            .with_upstream_decisions(upstream.decisions.clone())
                            .with_upstream_research_summary_opt(upstream.research_summary.clone());
                    }

                    let handle = self
                        .dependencies
                        .dispatcher()
                        .dispatch_work(work_req)
                        .await
                        .map_err(|e| ControllerError::SeamError {
                            seam: "dispatcher".into(),
                            message: e.to_string(),
                        })?;

                    self.active_handle = Some(handle);
                }
                StageOutcome::Advance(LoopStage::CollectResult)
            }

            LoopStage::CollectResult => {
                if let Some(ref handle) = self.active_handle {
                    let result = self
                        .dependencies
                        .dispatcher()
                        .collect_result(handle)
                        .await
                        .map_err(|e| ControllerError::SeamError {
                            seam: "dispatcher".into(),
                            message: e.to_string(),
                        })?;

                    self.last_execution_result = Some(result);
                }
                StageOutcome::Advance(LoopStage::UpdateState)
            }

            LoopStage::UpdateState => {
                // Single authoritative settlement: the enforcer releases the
                // held receipt and records actuals. The tracker then mirrors
                // the enforcer snapshot (projection, never a second ledger).
                // Usage provenance is truthful: provider-reported usage
                // settles authoritative; missing or estimated usage settles
                // estimated and stays explicitly non-authoritative.
                if let Some(enforcer) = self.dependencies.budget_enforcer() {
                    let receipt = self.active_receipt.take().unwrap_or_else(|| {
                        let tokens = self
                            .active_task
                            .as_ref()
                            .map(|t| t.estimated_tokens)
                            .unwrap_or(100);
                        crate::budget::ReservationReceipt::new(tokens, 0.01, true, 4096)
                    });
                    if let Some(ref res) = self.last_execution_result
                        && let Some(ref usage) = res.token_usage
                    {
                        enforcer.settle_model_usage(&receipt, usage);
                    } else {
                        let tokens = self
                            .active_task
                            .as_ref()
                            .map(|t| t.estimated_tokens)
                            .unwrap_or(100);
                        enforcer.settle_estimated(
                            &receipt,
                            &crate::budget::enforcer::ActualUsage {
                                tokens,
                                cost_usd: 0.0,
                                artifact_bytes: 0,
                                steps: 1,
                                calls: 0,
                                retries: 0,
                            },
                        );
                    }
                    // Mirror authoritative state into the projection and
                    // detect post-settlement overruns truthfully: an overrun
                    // halts further admissions instead of being swallowed.
                    self.budget_tracker
                        .sync_shared_from_snapshot(&enforcer.snapshot());
                } else {
                    // No enforcer wired (standalone/test): record the cycle
                    // informationally in the tracker only. This path never
                    // gates production work.
                    let actual_tokens: u64 = self
                        .last_execution_result
                        .as_ref()
                        .and_then(|r| r.token_usage.as_ref())
                        .map(|u| u.total_tokens as u64)
                        .unwrap_or_else(|| {
                            self.active_task
                                .as_ref()
                                .map(|t| t.estimated_tokens)
                                .unwrap_or(100)
                        });
                    let _ = self.budget_tracker.reconcile_consumption(
                        1,
                        1,
                        actual_tokens,
                        0.0,
                        0,
                        &self.budget_limits,
                    );
                }
                if let Err(kind) = self.budget_tracker.verify_bounds(&self.budget_limits) {
                    return Ok(StageOutcome::Halt(ControllerHaltReason::BudgetExhausted {
                        kind,
                    }));
                }

                // Persist state transition to SQLite (PST-01, AUT-01, FINDING-05).
                // The task result, agent linkage, invocation telemetry, and
                // budget-ledger snapshot form ONE logical state transition
                // committed atomically by the transaction manager. A failed
                // transaction surfaces as SeamError so the runtime knows
                // persistence failed — errors here are never swallowed.
                if let (Some(tx_manager), Some(task)) = (
                    self.dependencies.transaction_manager(),
                    self.active_task.as_ref(),
                ) {
                    // The tasks.result column is typed as TaskResult (summary +
                    // artifacts + metadata). The interim write must use that
                    // canonical shape — serializing the internal WorkExecutionResult
                    // here created rows that fail closed on read-back.
                    let canonical_result = self.last_execution_result.as_ref().map(|res| {
                        let mut summary = if res.success {
                            format!("executing: {}", res.output)
                        } else {
                            format!(
                                "failed: {}",
                                res.error_detail.as_deref().unwrap_or(&res.output)
                            )
                        };
                        if summary.len() > 2000 {
                            summary.truncate(2000);
                            summary.push_str("…[truncated]");
                        }
                        let mut task_result = crate::state::task::TaskResult::new(summary);
                        task_result
                            .metadata
                            .insert("success".to_string(), res.success.to_string());
                        task_result
                            .metadata
                            .insert("output_bytes".to_string(), res.output.len().to_string());
                        task_result
                    });
                    let (status_str, result_json) =
                        if let Some(ref res) = self.last_execution_result {
                            if res.success {
                                (
                                    "running",
                                    canonical_result
                                        .as_ref()
                                        .and_then(|r| serde_json::to_string(r).ok()),
                                )
                            } else {
                                (
                                    "failed",
                                    canonical_result
                                        .as_ref()
                                        .and_then(|r| serde_json::to_string(r).ok()),
                                )
                            }
                        } else {
                            ("running", None)
                        };
                    let agent_id = self.active_agent.unwrap_or_default();
                    let telemetry = self
                        .last_execution_result
                        .as_ref()
                        .and_then(|res| res.token_usage.as_ref())
                        .map(|usage| {
                            crate::persistence::sqlite::transaction::TaskExecutionTelemetry {
                                id: uuid::Uuid::now_v7(),
                                step_number: self.progress.cycle as u32,
                                provider: "production_provider".to_string(),
                                model_name: "autonomous_worker".to_string(),
                                attempt_number: 1,
                                outcome: if self
                                    .last_execution_result
                                    .as_ref()
                                    .is_some_and(|r| r.success)
                                {
                                    "success".to_string()
                                } else {
                                    "failure".to_string()
                                },
                                prompt_tokens: usage.prompt_tokens,
                                completion_tokens: usage.completion_tokens,
                                total_tokens: usage.total_tokens,
                                usage_source: match usage.source {
                                    crate::model::types::UsageSource::AuthoritativeProvider => {
                                        "authoritative_provider".to_string()
                                    }
                                    crate::model::types::UsageSource::Estimated => {
                                        "estimated".to_string()
                                    }
                                },
                                routing_reason: "worker_execution".to_string(),
                                prompt_provenance: None,
                            }
                        });
                    let budget = self.dependencies.budget_enforcer().map(|enforcer| {
                        let snap = enforcer.snapshot();
                        crate::persistence::sqlite::transaction::BudgetConsumption {
                            tokens: snap.tokens_consumed,
                            cost_microcents: (snap.cost_consumed_usd * 100_000_000.0) as u64,
                            artifact_bytes: snap.artifact_bytes_consumed,
                            steps: snap.agent_steps_consumed,
                            calls: snap.model_calls_consumed,
                            retries: snap.retries_consumed,
                            estimated_tokens: snap.tokens_consumed_estimated,
                            estimated_cost_microcents: (snap.cost_consumed_estimated_usd
                                * 100_000_000.0)
                                as u64,
                        }
                    });
                    tx_manager
                        .commit_task_execution_side_effects(
                            self.mission_id,
                            task.task_id,
                            agent_id,
                            status_str,
                            result_json.as_deref(),
                            telemetry,
                            budget,
                        )
                        .await
                        .map_err(|e| ControllerError::SeamError {
                            seam: "update_state_persistence".into(),
                            message: e.to_string(),
                        })?;
                }

                if self
                    .last_execution_result
                    .as_ref()
                    .is_some_and(|res| !res.success)
                {
                    StageOutcome::SkipTo(LoopStage::ClassifyFailure)
                } else {
                    StageOutcome::Advance(LoopStage::Verify)
                }
            }

            LoopStage::Verify => {
                if let Some(ref task) = self.active_task {
                    let output = self
                        .last_execution_result
                        .as_ref()
                        .map(|r| r.output.clone())
                        .unwrap_or_default();

                    let outcome = self
                        .dependencies
                        .verifier()
                        .verify_task(TaskVerificationRequest {
                            mission_id: self.mission_id,
                            task_id: task.task_id,
                            execution_output: output.clone(),
                        })
                        .await
                        .map_err(|e| ControllerError::SeamError {
                            seam: "verifier".into(),
                            message: e.to_string(),
                        })?;

                    match outcome {
                        VerificationOutcome::Passed => {
                            let mut task_res =
                                crate::state::task::TaskResult::new("Task executed successfully");
                            task_res
                                .metadata
                                .insert("output".to_string(), output.clone());
                            self.dependencies
                                .scheduler()
                                .mark_task_completed(task.task_id, Some(task_res))
                                .await
                                .map_err(|e| ControllerError::SeamError {
                                    seam: "scheduler".into(),
                                    message: e.to_string(),
                                })?;
                            // Task-scoped approval grants die with the task: a
                            // completed task's grants must never authorize
                            // later work. Invalidation failure is recorded,
                            // never swallowed (fail-safe: scheduler state
                            // already advanced, so this cannot roll back).
                            if let Some(tx_manager) = self.dependencies.transaction_manager() {
                                if let Err(e) = crate::policy::approval::PolicyGrantStore::new()
                                    .invalidate_task_grants(tx_manager.pool(), task.task_id)
                                    .await
                                {
                                    tracing::warn!(
                                        task_id = ?task.task_id,
                                        "task grant invalidation failed after completion: {e}"
                                    );
                                }
                            }
                            StageOutcome::SkipTo(LoopStage::Checkpoint)
                        }
                        VerificationOutcome::Failed { ref reason } => {
                            self.last_execution_result = Some(WorkExecutionResult {
                                task_id: task.task_id,
                                success: false,
                                output: output.clone(),
                                error_detail: Some(reason.clone()),
                                token_usage: None,
                            });
                            self.dependencies
                                .scheduler()
                                .mark_task_failed(task.task_id, reason.clone(), true)
                                .await
                                .map_err(|e| ControllerError::SeamError {
                                    seam: "scheduler".into(),
                                    message: e.to_string(),
                                })?;
                            self.emit_event(EventType::TaskFailed {
                                task_id: task.task_id,
                                mission_id: self.mission_id,
                                error: reason.clone(),
                            })
                            .await;
                            StageOutcome::Advance(LoopStage::ClassifyFailure)
                        }
                    }
                } else {
                    let gate = self
                        .dependencies
                        .verifier()
                        .verify_completion_gate(self.mission_id)
                        .await
                        .map_err(|e| ControllerError::SeamError {
                            seam: "verifier".into(),
                            message: e.to_string(),
                        })?;

                    match gate {
                        CompletionGateOutcome::Satisfied => {
                            if let Some(report_gen) = self.dependencies.report_generator()
                                && let Ok(candidate) =
                                    report_gen.build_candidate(&self.mission_id).await
                            {
                                let _ = report_gen.seal_report(candidate, &Ok(())).await;
                            }
                            StageOutcome::Halt(ControllerHaltReason::MissionCompleted)
                        }
                        CompletionGateOutcome::Deficient { violations } => {
                            self.last_execution_result = Some(WorkExecutionResult {
                                task_id: TaskId::default(),
                                success: false,
                                output: "Mission completion gate deficient".to_string(),
                                error_detail: Some(violations.join("; ")),
                                token_usage: None,
                            });
                            StageOutcome::Advance(LoopStage::ClassifyFailure)
                        }
                    }
                }
            }

            LoopStage::ClassifyFailure => {
                self.progress.record_failure();
                let err_msg = self
                    .last_execution_result
                    .as_ref()
                    .and_then(|r| r.error_detail.clone())
                    .unwrap_or_else(|| "Stage execution failed".into());

                let task_id = self.active_task.as_ref().map(|t| t.task_id);

                let class = self
                    .dependencies
                    .recovery()
                    .classify_failure(FailureClassificationRequest {
                        mission_id: self.mission_id,
                        task_id: task_id.unwrap_or_default(),
                        error_message: err_msg,
                    })
                    .await
                    .map_err(|e| ControllerError::SeamError {
                        seam: "recovery".into(),
                        message: e.to_string(),
                    })?;

                self.last_failure_class = Some(class);

                // Preserve failure diagnostics and error output in step_history for context compilation (GAP-03)
                if let Some(ref res) = self.last_execution_result {
                    self.step_history
                        .push(crate::kernel::seams::context::StepRecordDto {
                            step_number: self.step_history.len() as u32 + 1,
                            tool_name: "task_execution".to_string(),
                            parameters: serde_json::json!({
                                "task_id": res.task_id.to_string(),
                                "failure_class": format!("{:?}", class),
                            }),
                            success: res.success,
                            output: res.output.clone(),
                            error: res.error_detail.clone(),
                        });
                }

                // Check sliding-window loop detector using canonical execution evidence (Section 10)
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                std::hash::Hash::hash(&task_id, &mut hasher);
                std::hash::Hash::hash(&format!("{class:?}"), &mut hasher);
                if let Some(ref res) = self.last_execution_result {
                    std::hash::Hash::hash(&res.error_detail, &mut hasher);
                    std::hash::Hash::hash(&res.success, &mut hasher);
                }
                let progress_fingerprint = std::hash::Hasher::finish(&hasher);
                let sig = LoopSignature {
                    task_id,
                    failure_class: format!("{class:?}"),
                    recovery_strategy: "determine_recovery".into(),
                    stage: current,
                    progress_fingerprint,
                };

                if self.loop_detector.record(sig.clone()) {
                    StageOutcome::Halt(ControllerHaltReason::LoopDetected { signature: sig })
                } else if matches!(class, FailureClassification::FatalViolation) {
                    StageOutcome::Halt(ControllerHaltReason::FatalError {
                        failure_class: format!("{class:?}"),
                    })
                } else {
                    StageOutcome::Advance(LoopStage::RecoverOrReplan)
                }
            }

            LoopStage::RecoverOrReplan => {
                let task_id = self
                    .active_task
                    .as_ref()
                    .map(|t| t.task_id)
                    .unwrap_or_default();

                let failure_class = self
                    .last_failure_class
                    .unwrap_or(FailureClassification::Transient);

                let action = self
                    .dependencies
                    .recovery()
                    .determine_recovery(RecoveryStrategyRequest {
                        mission_id: self.mission_id,
                        task_id,
                        failure_class,
                        retry_count: self.progress.failure_count as usize,
                        error_message: self
                            .last_execution_result
                            .as_ref()
                            .and_then(|r| r.error_detail.clone()),
                    })
                    .await
                    .map_err(|e| ControllerError::SeamError {
                        seam: "recovery".into(),
                        message: e.to_string(),
                    })?;

                let err_detail = self
                    .last_execution_result
                    .as_ref()
                    .and_then(|r| r.error_detail.clone())
                    .unwrap_or_else(|| "task execution failed".into());

                // Persist the failure diagnosis to engineering memory so
                // future retrieval (recent_failures, error-signature matching)
                // learns from this failure across retries and missions.
                // Fail-safe: memory errors are logged and never break the
                // recovery loop.
                if let Some(store) = self.dependencies.memory_store() {
                    let mut err_hasher = Sha256::new();
                    err_hasher.update(err_detail.as_bytes());
                    let err_hash = format!("{:x}", err_hasher.finalize());
                    let signature = format!(
                        "{task_id}:{failure_class:?}:{}",
                        &err_hash[..16.min(err_hash.len())]
                    );
                    let (hypothesis, recommended_action, repair_json): (
                        String,
                        String,
                        Option<String>,
                    ) = match &action {
                        RecoveryAction::Retry { delay_ms } => (
                            format!(
                                "Failure appears transient; retrying after {delay_ms}ms backoff"
                            ),
                            format!("Retry task (failure #{})", self.progress.failure_count + 1),
                            None,
                        ),
                        RecoveryAction::Replan { reason } => (
                            format!("Task plan inadequate: {reason}"),
                            "Replan with revised decomposition".to_string(),
                            None,
                        ),
                        RecoveryAction::Repair { proposal, reason } => (
                            format!("Targeted repair proposed: {reason}"),
                            "Apply structured repair proposal".to_string(),
                            serde_json::to_string(proposal).ok(),
                        ),
                        RecoveryAction::Rollback { reason } => (
                            format!("Workspace state suspect: {reason}"),
                            "Roll back to last clean baseline".to_string(),
                            None,
                        ),
                        RecoveryAction::SkipTask => (
                            "Task deemed non-essential after failure".to_string(),
                            "Skip task and continue mission".to_string(),
                            None,
                        ),
                        RecoveryAction::AbortMission { reason } => (
                            format!("Unrecoverable failure: {reason}"),
                            "Abort mission".to_string(),
                            None,
                        ),
                        RecoveryAction::Escalate { reason } => (
                            format!("Operator decision required: {reason}"),
                            "Escalate to operator".to_string(),
                            None,
                        ),
                    };
                    let mut record = crate::kernel::memory::FailureDiagnosisRecord::new(
                        self.mission_id,
                        task_id,
                        signature.clone(),
                        format!("{failure_class:?}"),
                        err_detail.clone(),
                        String::new(),
                        hypothesis,
                        err_detail.clone(),
                        recommended_action,
                    );
                    if let Some(json) = repair_json {
                        record = record.with_repair_proposal(json);
                    }
                    match store
                        .find_diagnosis_by_signature(self.mission_id, &signature)
                        .await
                    {
                        Ok(Some(existing)) => {
                            if let Err(e) = store.increment_recurrence(existing.id).await {
                                tracing::warn!(
                                    "engineering memory increment_recurrence failed: {e}"
                                );
                            }
                        }
                        Ok(None) => {
                            if let Err(e) = store.save_failure_diagnosis(&record).await {
                                tracing::warn!(
                                    "engineering memory save_failure_diagnosis failed: {e}"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!("engineering memory signature lookup failed: {e}");
                        }
                    }
                }

                // GAP-03: Worktree reconciliation before retrying or replanning.
                // If dirty with uncompilable changes, restore modified tracked
                // files to clean git commit baseline. This is a governed
                // recovery mutation: it requires policy authorization and a
                // bound git gate like any other mutation, and its real result
                // is recorded (never `let _ =` + `success = true`).
                //
                // Reconciliation runs ONLY inside the workspace's OWN git
                // repository (`.git` directly under the workspace root): git
                // upward discovery must never let a non-repository workspace
                // reconcile (or mutate) a parent repository's tree.
                if matches!(
                    action,
                    RecoveryAction::Retry { .. } | RecoveryAction::Replan { .. }
                ) && self
                    .effective_workspace_root()
                    .is_some_and(|ws| ws.join(".git").exists())
                {
                    let ws_root = match self.effective_workspace_root() {
                        Some(ws) => ws.to_path_buf(),
                        None => {
                            self.last_execution_result = Some(WorkExecutionResult {
                                task_id,
                                success: false,
                                output:
                                    "Recovery reconciliation refused: missing workspace identity"
                                        .to_string(),
                                error_detail: Some(
                                    "missing security-critical policy attribute: workspace_root"
                                        .to_string(),
                                ),
                                token_usage: None,
                            });
                            self.progress.record_recovery();
                            return Ok(StageOutcome::SkipTo(LoopStage::ClassifyFailure));
                        }
                    };
                    // Recovery respects cancellation: a cancelled mission must
                    // not start mutating the workspace to reconcile it.
                    if self.cancellation_token.is_cancelled() {
                        return Ok(StageOutcome::Halt(ControllerHaltReason::Cancelled));
                    }
                    let git: Arc<dyn crate::capability::traits::git::GitService> =
                        self.dependencies.git_service().cloned().unwrap_or_else(|| {
                            Arc::new(crate::capability::providers::CliGitProvider::new(
                                ws_root.clone(),
                            ))
                        });

                    if let Ok(status_str) = git.status_porcelain().await {
                        let has_modified_tracked = status_str.lines().any(|line| {
                            let trimmed = line.trim_start();
                            trimmed.starts_with('M')
                                || line.starts_with(" M")
                                || trimmed.starts_with('D')
                                || line.starts_with(" D")
                        });

                        if has_modified_tracked {
                            tracing::warn!(
                                workspace = ?ws_root,
                                "Restoring dirty uncompilable workspace tracked files to clean git commit baseline"
                            );
                            let gate = match self
                                .authorize_recovery_git(
                                    task_id,
                                    "recovery_reconcile_restore",
                                    ws_root.clone(),
                                    crate::git::GitOperation::SyncCheckout {
                                        target: ".".to_string(),
                                    },
                                    Vec::new(),
                                    "recovery-reconcile:.",
                                )
                                .await
                            {
                                Ok(g) => g,
                                Err(e) => {
                                    self.last_execution_result = Some(WorkExecutionResult {
                                        task_id,
                                        success: false,
                                        output: format!(
                                            "Recovery reconciliation denied by policy: {e}"
                                        ),
                                        error_detail: Some(e.to_string()),
                                        token_usage: None,
                                    });
                                    self.progress.record_recovery();
                                    return Ok(StageOutcome::SkipTo(LoopStage::ClassifyFailure));
                                }
                            };
                            match git.restore_head(".", &gate).await {
                                Ok(()) => {
                                    self.step_history.push(crate::kernel::seams::context::StepRecordDto {
                                        step_number: self.step_history.len() as u32 + 1,
                                        tool_name: "workspace_reconciliation".to_string(),
                                        parameters: serde_json::json!({
                                            "action": "git_checkout_head",
                                            "reason": "Restored modified tracked files to clean git baseline after uncompilable failure",
                                        }),
                                        success: true,
                                        output: "Restored modified tracked files to clean git commit baseline".to_string(),
                                        error: None,
                                    });
                                }
                                Err(e) => {
                                    self.last_execution_result = Some(WorkExecutionResult {
                                        task_id,
                                        success: false,
                                        output: format!("Recovery reconciliation failed: {e}"),
                                        error_detail: Some(e.to_string()),
                                        token_usage: None,
                                    });
                                    self.step_history.push(crate::kernel::seams::context::StepRecordDto {
                                        step_number: self.step_history.len() as u32 + 1,
                                        tool_name: "workspace_reconciliation".to_string(),
                                        parameters: serde_json::json!({
                                            "action": "git_checkout_head",
                                            "reason": "Restored modified tracked files to clean git baseline after uncompilable failure",
                                        }),
                                        success: false,
                                        output: format!("Reconciliation failed: {e}"),
                                        error: Some(e.to_string()),
                                    });
                                    self.progress.record_recovery();
                                    return Ok(StageOutcome::SkipTo(LoopStage::ClassifyFailure));
                                }
                            }
                        }
                    }
                }

                match action {
                    RecoveryAction::Retry { delay_ms } => {
                        if let Some(ref task) = self.active_task {
                            let _ = self
                                .dependencies
                                .scheduler()
                                .mark_task_failed(task.task_id, err_detail, true)
                                .await;
                        }
                        self.active_task = None;
                        self.active_agent = None;
                        self.active_context_id = None;
                        self.active_handle = None;
                        self.last_execution_result = None;
                        self.progress.record_recovery();
                        if delay_ms > 0 {
                            tokio::time::sleep(std::time::Duration::from_millis(delay_ms.min(500)))
                                .await;
                        }
                        StageOutcome::SkipTo(LoopStage::Observe)
                    }
                    RecoveryAction::Replan { reason } => {
                        let replan_reason =
                            if err_detail.is_empty() || err_detail == "task execution failed" {
                                reason
                            } else {
                                format!("{}: {}", reason, err_detail)
                            };
                        if let Some(ref task) = self.active_task {
                            let _ = self
                                .dependencies
                                .scheduler()
                                .mark_task_failed(task.task_id, err_detail, false)
                                .await;
                        }
                        let replan_res = self
                            .dependencies
                            .planner()
                            .replan(ReplanRequest {
                                mission_id: self.mission_id,
                                failed_task_id: task_id,
                                reason: replan_reason,
                            })
                            .await
                            .map_err(|e| ControllerError::SeamError {
                                seam: "planner".into(),
                                message: e.to_string(),
                            })?;

                        // GAP-06: Materialize replanned candidate tasks into the scheduler
                        if let Some(ref plan) = replan_res.candidate_plan {
                            self.dependencies
                                .scheduler()
                                .materialize_plan(self.mission_id, plan)
                                .await
                                .map_err(|e| ControllerError::SeamError {
                                    seam: "scheduler".into(),
                                    message: e.to_string(),
                                })?;
                        }

                        self.progress.record_recovery();
                        StageOutcome::Advance(LoopStage::Checkpoint)
                    }
                    RecoveryAction::SkipTask => {
                        if let Some(ref task) = self.active_task {
                            let _ = self
                                .dependencies
                                .scheduler()
                                .mark_task_failed(task.task_id, err_detail, false)
                                .await;
                        }
                        self.progress.record_recovery();
                        StageOutcome::Advance(LoopStage::Checkpoint)
                    }
                    RecoveryAction::AbortMission { reason } => {
                        if let Some(ref task) = self.active_task {
                            let _ = self
                                .dependencies
                                .scheduler()
                                .mark_task_failed(task.task_id, err_detail, false)
                                .await;
                        }
                        self.active_task = None;
                        self.active_agent = None;
                        self.active_context_id = None;
                        self.active_handle = None;
                        self.last_execution_result = None;
                        StageOutcome::Halt(ControllerHaltReason::FatalError {
                            failure_class: reason,
                        })
                    }
                    RecoveryAction::Repair { proposal, reason } => {
                        tracing::info!(
                            task_id = ?task_id,
                            reason = %reason,
                            "Executing closed-loop repair change proposal through governed recovery pipeline"
                        );
                        // Recovery proposals obey the same authority boundary
                        // as normal mutations: policy evaluation first, and
                        // only an explicit Allow executes. There is no
                        // "trusted recovery" bypass.
                        let repair_targets: Vec<std::path::PathBuf> = proposal
                            .change_surface
                            .all_target_files()
                            .into_iter()
                            .map(std::path::PathBuf::from)
                            .collect();
                        if let Err(e) = self
                            .authorize_recovery_mutation(
                                task_id,
                                "recovery_repair_apply",
                                repair_targets,
                                serde_json::json!({
                                    "proposal_id": proposal.id.to_string(),
                                }),
                            )
                            .await
                        {
                            tracing::warn!(
                                error = %e,
                                "Repair proposal denied by policy; falling back to failure classification"
                            );
                            self.last_execution_result = Some(WorkExecutionResult {
                                task_id,
                                success: false,
                                output: format!("Repair denied by policy: {e}"),
                                error_detail: Some(e.to_string()),
                                token_usage: None,
                            });
                            self.progress.record_recovery();
                            return Ok(StageOutcome::SkipTo(LoopStage::ClassifyFailure));
                        }
                        let ws_root = self
                            .effective_workspace_root()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| std::env::temp_dir().join("m31a"));

                        let change_auth = match self.dependencies.change_authority().cloned() {
                            Some(auth) => auth,
                            None => {
                                tracing::warn!(
                                    "No shared ChangeAuthority wired; refusing recovery repair without mutation authority"
                                );
                                self.last_execution_result = Some(WorkExecutionResult {
                                    task_id,
                                    success: false,
                                    output: "Repair refused: no shared ChangeAuthority wired"
                                        .to_string(),
                                    error_detail: Some(
                                        "missing ChangeAuthority for recovery repair".to_string(),
                                    ),
                                    token_usage: None,
                                });
                                self.progress.record_recovery();
                                return Ok(StageOutcome::SkipTo(LoopStage::ClassifyFailure));
                            }
                        };

                        let fs: Arc<dyn crate::capability::traits::fs::FileSystemService> =
                            match crate::capability::providers::LocalFileSystemProvider::new(
                                &ws_root,
                            ) {
                                Ok(p) => Arc::new(p),
                                Err(e) => {
                                    tracing::warn!(
                                        error = ?e,
                                        "Failed to initialize LocalFileSystemProvider for repair"
                                    );
                                    self.last_execution_result = Some(WorkExecutionResult {
                                        task_id,
                                        success: false,
                                        output: format!("Filesystem initialization failed: {e}"),
                                        error_detail: Some(e.to_string()),
                                        token_usage: None,
                                    });
                                    self.progress.record_recovery();
                                    return Ok(StageOutcome::SkipTo(LoopStage::ClassifyFailure));
                                }
                            };

                        match change_auth
                            .execute_change_proposal(&ws_root, &proposal, &fs, None, None)
                            .await
                        {
                            Ok(outcome) => {
                                tracing::info!(
                                    proposal_id = %outcome.proposal_id,
                                    files = ?outcome.files_modified,
                                    "Repair proposal atomically applied, initiating immediate re-verification"
                                );
                                self.step_history.push(
                                    crate::kernel::seams::context::StepRecordDto {
                                        step_number: self.step_history.len() as u32 + 1,
                                        tool_name: "closed_loop_repair".to_string(),
                                        parameters: serde_json::json!({
                                            "proposal_id": outcome.proposal_id.to_string(),
                                            "files_modified": outcome.files_modified,
                                            "reason": reason,
                                        }),
                                        success: true,
                                        output: format!(
                                            "Applied repair: {} files modified",
                                            outcome.files_modified.len()
                                        ),
                                        error: None,
                                    },
                                );

                                self.progress.record_recovery();
                                // Re-verify immediately!
                                StageOutcome::SkipTo(LoopStage::Verify)
                            }
                            Err(e) => {
                                tracing::warn!(
                                    error = ?e,
                                    "Repair proposal execution failed, falling back to failure classification"
                                );
                                self.last_execution_result = Some(WorkExecutionResult {
                                    task_id,
                                    success: false,
                                    output: format!("Repair application failed: {e}"),
                                    error_detail: Some(e.to_string()),
                                    token_usage: None,
                                });
                                self.progress.record_recovery();
                                StageOutcome::SkipTo(LoopStage::ClassifyFailure)
                            }
                        }
                    }
                    RecoveryAction::Rollback { reason } => {
                        tracing::warn!(
                            task_id = ?task_id,
                            reason = %reason,
                            "Rolling back unviable task modifications to clean baseline"
                        );
                        // Rollback is a governed recovery mutation: it requires
                        // real authorization, validates workspace identity and
                        // repository state, returns the real git result, and
                        // records truthful state. A failed rollback is never
                        // recorded as success.
                        if self.cancellation_token.is_cancelled() {
                            return Ok(StageOutcome::Halt(ControllerHaltReason::Cancelled));
                        }
                        match self.effective_workspace_root() {
                            None => {
                                self.last_execution_result = Some(WorkExecutionResult {
                                    task_id,
                                    success: false,
                                    output: "Rollback refused: missing workspace identity"
                                        .to_string(),
                                    error_detail: Some(
                                        "missing security-critical policy attribute: workspace_root"
                                            .to_string(),
                                    ),
                                    token_usage: None,
                                });
                                self.step_history.push(
                                    crate::kernel::seams::context::StepRecordDto {
                                        step_number: self.step_history.len() as u32 + 1,
                                        tool_name: "rollback_modification".to_string(),
                                        parameters: serde_json::json!({
                                            "action": "git_restore_head",
                                            "reason": reason,
                                        }),
                                        success: false,
                                        output: "Rollback refused: missing workspace identity"
                                            .to_string(),
                                        error: Some("missing workspace_root".to_string()),
                                    },
                                );
                            }
                            Some(ws_root) => {
                                // Validate expected git state before mutating:
                                // rollback requires an actual repository.
                                if !ws_root.join(".git").exists() {
                                    self.last_execution_result = Some(WorkExecutionResult {
                                        task_id,
                                        success: false,
                                        output:
                                            "Rollback failed: workspace is not a git repository"
                                                .to_string(),
                                        error_detail: Some(format!(
                                            "missing repository at {}",
                                            ws_root.display()
                                        )),
                                        token_usage: None,
                                    });
                                    self.step_history.push(
                                        crate::kernel::seams::context::StepRecordDto {
                                            step_number: self.step_history.len() as u32 + 1,
                                            tool_name: "rollback_modification".to_string(),
                                            parameters: serde_json::json!({
                                                "action": "git_restore_head",
                                                "reason": reason,
                                            }),
                                            success: false,
                                            output:
                                                "Rollback failed: workspace is not a git repository"
                                                    .to_string(),
                                            error: Some("missing repository".to_string()),
                                        },
                                    );
                                } else {
                                    let git: Arc<dyn crate::capability::traits::git::GitService> =
                                        self.dependencies.git_service().cloned().unwrap_or_else(|| {
                                            Arc::new(crate::capability::providers::CliGitProvider::new(
                                                ws_root,
                                            ))
                                        });
                                    let gate = match self
                                        .authorize_recovery_git(
                                            task_id,
                                            "recovery_rollback_restore",
                                            ws_root.to_path_buf(),
                                            crate::git::GitOperation::SyncCheckout {
                                                target: ".".to_string(),
                                            },
                                            Vec::new(),
                                            "recovery-rollback:.",
                                        )
                                        .await
                                    {
                                        Ok(g) => g,
                                        Err(e) => {
                                            self.last_execution_result =
                                                Some(WorkExecutionResult {
                                                    task_id,
                                                    success: false,
                                                    output: format!(
                                                        "Rollback denied by policy: {e}"
                                                    ),
                                                    error_detail: Some(e.to_string()),
                                                    token_usage: None,
                                                });
                                            self.step_history.push(
                                                crate::kernel::seams::context::StepRecordDto {
                                                    step_number: self.step_history.len() as u32 + 1,
                                                    tool_name: "rollback_modification".to_string(),
                                                    parameters: serde_json::json!({
                                                        "action": "git_restore_head",
                                                        "reason": reason,
                                                    }),
                                                    success: false,
                                                    output: format!(
                                                        "Rollback denied by policy: {e}"
                                                    ),
                                                    error: Some(e.to_string()),
                                                },
                                            );
                                            self.progress.record_recovery();
                                            return Ok(StageOutcome::Advance(
                                                LoopStage::Checkpoint,
                                            ));
                                        }
                                    };
                                    match git.restore_head(".", &gate).await {
                                        Ok(()) => {
                                            self.step_history.push(crate::kernel::seams::context::StepRecordDto {
                                                step_number: self.step_history.len() as u32 + 1,
                                                tool_name: "rollback_modification".to_string(),
                                                parameters: serde_json::json!({
                                                    "action": "git_restore_head",
                                                    "reason": reason,
                                                }),
                                                success: true,
                                                output: "Restored tracked files to clean git commit baseline".to_string(),
                                                error: None,
                                            });
                                        }
                                        Err(e) => {
                                            self.last_execution_result =
                                                Some(WorkExecutionResult {
                                                    task_id,
                                                    success: false,
                                                    output: format!("Rollback failed: {e}"),
                                                    error_detail: Some(e.to_string()),
                                                    token_usage: None,
                                                });
                                            self.step_history.push(
                                                crate::kernel::seams::context::StepRecordDto {
                                                    step_number: self.step_history.len() as u32 + 1,
                                                    tool_name: "rollback_modification".to_string(),
                                                    parameters: serde_json::json!({
                                                        "action": "git_restore_head",
                                                        "reason": reason,
                                                    }),
                                                    success: false,
                                                    output: format!("Rollback failed: {e}"),
                                                    error: Some(e.to_string()),
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        if let Some(ref task) = self.active_task {
                            let _ = self
                                .dependencies
                                .scheduler()
                                .mark_task_failed(task.task_id, reason.clone(), false)
                                .await;
                        }
                        self.progress.record_recovery();
                        StageOutcome::Advance(LoopStage::Checkpoint)
                    }
                    RecoveryAction::Escalate { reason } => {
                        tracing::error!(
                            task_id = ?task_id,
                            reason = %reason,
                            "Escalating unresolvable failure"
                        );
                        if let Some(ref task) = self.active_task {
                            let _ = self
                                .dependencies
                                .scheduler()
                                .mark_task_failed(task.task_id, reason.clone(), false)
                                .await;
                        }
                        self.active_task = None;
                        self.active_agent = None;
                        self.active_context_id = None;
                        self.active_handle = None;
                        self.last_execution_result = None;
                        StageOutcome::Halt(ControllerHaltReason::FatalError {
                            failure_class: format!("Escalated: {reason}"),
                        })
                    }
                }
            }

            LoopStage::Checkpoint => {
                // Persist durable checkpoint via CheckpointManager (CHK-01, PST-01, AUT-01)
                let completed_cycle = self.progress.cycle + 1;

                let cp_manager_opt =
                    self.dependencies.checkpoint_manager().cloned().or_else(|| {
                        self.dependencies.transaction_manager().map(|tx| {
                            let pool = tx.pool().clone();
                            let ws = self
                                .effective_workspace_root()
                                .map(|p| p.to_path_buf())
                                .unwrap_or_else(|| std::env::temp_dir().join("m31a"));
                            // Channel-aware fallback storage (unreachable in
                            // production: dependencies always wire a manager).
                            let channel = crate::deployment::DeploymentChannel::current();
                            let artifacts =
                                Arc::new(crate::persistence::artifacts::FsArtifactStore::new(
                                    crate::deployment::DeploymentPaths::project_artifacts_dir(
                                        &ws, channel,
                                    ),
                                ));
                            Arc::new(CheckpointManager::new(
                                pool,
                                artifacts,
                                crate::deployment::DeploymentPaths::project_staging_dir(
                                    &ws, channel,
                                ),
                            ))
                        })
                    });

                if let Some(cp_manager) = cp_manager_opt {
                    let checkpoint_id = CheckpointId::new();
                    let pool = self
                        .dependencies
                        .transaction_manager()
                        .as_ref()
                        .map(|tx| tx.pool());

                    // 1. Snapshot identity from workspace baseline if available
                    let (snapshot_identity, baseline_opt) = match self.effective_workspace_root() {
                        Some(ws) if ws.exists() => {
                            match crate::repo::drift::RepositoryBaseline::capture(
                                ws,
                                self.mission_id,
                                None,
                                None,
                            ) {
                                Ok(baseline) => {
                                    let mut hasher = Sha256::new();
                                    let fp_json = serde_json::to_string(&baseline.file_hashes)
                                        .unwrap_or_default();
                                    hasher.update(fp_json.as_bytes());
                                    (format!("{:x}", hasher.finalize()), Some(baseline))
                                }
                                Err(_) => {
                                    (format!("{}-cp-{}", self.mission_id, completed_cycle), None)
                                }
                            }
                        }
                        _ => (format!("{}-cp-{}", self.mission_id, completed_cycle), None),
                    };

                    if let (Some(pool), Some(baseline)) = (pool, baseline_opt) {
                        let _ = baseline.save_to_db(pool).await;
                    }

                    // 2. Query task states for this mission from DB
                    let mut task_states = if let Some(pool) = pool {
                        let task_repo =
                            crate::persistence::sqlite::repositories::SqliteTaskRepository::new(
                                pool.clone(),
                            );
                        task_repo
                            .get_task_states_by_mission(self.mission_id)
                            .await
                            .unwrap_or_default()
                    } else {
                        BTreeMap::new()
                    };
                    if let Some(ref active) = self.active_task {
                        task_states
                            .entry(active.task_id)
                            .or_insert(crate::state_machine::TaskState::Running);
                    }

                    // 3. Compute policy context hash
                    let policy_context_hash = if let Some(mission_repo) =
                        self.dependencies.mission_repo()
                        && let Ok(Some(mission)) = mission_repo.get(self.mission_id).await
                    {
                        let mut hasher = Sha256::new();
                        let json =
                            serde_json::to_string(&mission.policy_context).unwrap_or_default();
                        hasher.update(json.as_bytes());
                        format!("{:x}", hasher.finalize())
                    } else {
                        let mut hasher = Sha256::new();
                        let json = serde_json::to_string(
                            &crate::state::policy_context::PolicyContext::standard(),
                        )
                        .unwrap_or_default();
                        hasher.update(json.as_bytes());
                        format!("{:x}", hasher.finalize())
                    };

                    // 4. Query verification check IDs
                    let verification_check_ids = if let Some(pool) = pool {
                        crate::verification::gate::EvidenceCompletionGate::list_check_ids_for_mission(pool, self.mission_id)
                            .await
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };

                    let state_summary = format!(
                        "cycle={}, stage={:?}, active_task={:?}, failures={}",
                        completed_cycle,
                        self.progress.current_stage,
                        self.active_task.as_ref().map(|t| t.task_id),
                        self.progress.failure_count
                    );

                    let latest_seq = if let Some(pool) = pool {
                        use crate::persistence::sqlite::repositories::EventRepository;
                        let event_repo =
                            crate::persistence::sqlite::repositories::SqliteEventRepository::new(
                                pool.clone(),
                            );
                        event_repo
                            .latest_sequence(Some(self.mission_id))
                            .await
                            .unwrap_or(0)
                    } else {
                        0
                    };

                    let manifest = CheckpointManifest::new(
                        checkpoint_id,
                        self.mission_id,
                        latest_seq,
                        self.progress.current_stage.name(),
                        completed_cycle,
                        snapshot_identity,
                        task_states,
                        BTreeMap::new(),
                        policy_context_hash,
                        verification_check_ids,
                        vec![],
                        state_summary,
                    );

                    match cp_manager.create_checkpoint(&manifest, vec![]).await {
                        Ok(cid) => {
                            self.emit_event(EventType::CheckpointCreated {
                                checkpoint_id: cid,
                                mission_id: self.mission_id,
                                description: format!(
                                    "Cycle {} checkpoint: {}",
                                    completed_cycle,
                                    self.progress.current_stage.name()
                                ),
                            })
                            .await;
                        }
                        Err(e) => {
                            tracing::warn!(
                                mission_id = %self.mission_id,
                                error = %e,
                                "Failed to create checkpoint via CheckpointManager"
                            );
                        }
                    }
                }

                if let Some(pool) = self
                    .dependencies
                    .transaction_manager()
                    .as_ref()
                    .map(|tx| tx.pool())
                {
                    use crate::persistence::sqlite::repositories::EventRepository;
                    let event_repo =
                        crate::persistence::sqlite::repositories::SqliteEventRepository::new(
                            pool.clone(),
                        );
                    if let Ok(seq) = event_repo.latest_sequence(Some(self.mission_id)).await {
                        if seq > 0 {
                            if let Some(mission_repo) = self.dependencies.mission_repo() {
                                let _ = mission_repo.update_watermark(self.mission_id, seq).await;
                            }
                        }
                    }
                }

                // Clear ephemeral per-cycle task state
                self.active_task = None;
                self.active_agent = None;
                self.active_context_id = None;
                self.active_handle = None;
                self.last_execution_result = None;

                StageOutcome::Advance(LoopStage::Observe)
            }
        };

        StageOutcome::validate_transition(current, &outcome)?;

        match &outcome {
            StageOutcome::Advance(next) | StageOutcome::SkipTo(next) => {
                self.emit_event(EventType::ControllerStageTransitioned {
                    mission_id: self.mission_id,
                    cycle: self.progress.cycle,
                    from_stage: current.name().to_string(),
                    to_stage: next.name().to_string(),
                })
                .await;
                self.progress.advance_stage(*next);
            }
            StageOutcome::Yield(_) => {
                self.progress.record_idle();
            }
            StageOutcome::Halt(reason) => {
                self.emit_event(EventType::ControllerHalted {
                    mission_id: self.mission_id,
                    reason: format!("{reason:?}"),
                })
                .await;
            }
        }

        Ok(outcome)
    }

    /// Restores controller execution progress from the latest durable SQLite checkpoint (PST-01, PST-06, FINDING-05).
    pub async fn restore_from_checkpoint(&mut self) -> Result<bool, ControllerError> {
        if let Some(tx_manager) = self.dependencies.transaction_manager() {
            let row =
                CheckpointManager::get_latest_checkpoint_info(tx_manager.pool(), self.mission_id)
                    .await
                    .map_err(|e| ControllerError::SeamError {
                        seam: "persistence".into(),
                        message: e.to_string(),
                    })?;

            if let Some((_seq, _stage, cycle)) = row {
                self.progress.cycle = cycle as u64;
                self.progress.current_stage = LoopStage::Observe;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Coordinate startup crash recovery scan, process reconciliation, and safe resume (D-14, D-16).
    pub async fn startup_crash_recovery_and_resume(
        &mut self,
        workspace_root: impl AsRef<std::path::Path>,
        artifact_store: Arc<dyn crate::persistence::artifacts::fs_store::ArtifactStore>,
    ) -> Result<StageOutcome, ControllerError> {
        let pool = self
            .dependencies
            .transaction_manager()
            .as_ref()
            .map(|tm| tm.pool().clone())
            .ok_or_else(|| ControllerError::SeamError {
                seam: "persistence".into(),
                message: "No transaction manager / pool available for crash recovery".into(),
            })?;

        let scanner = crate::checkpoint::crash_recovery::StartupCrashRecoveryScanner::new(
            pool.clone(),
            artifact_store.clone(),
            workspace_root.as_ref(),
        );

        let scan_result = scanner
            .scan_and_reconcile(self.mission_id)
            .await
            .map_err(|e| ControllerError::SeamError {
                seam: "crash_recovery".into(),
                message: e.to_string(),
            })?;

        match scan_result.classification {
            crate::checkpoint::crash_recovery::CrashRecoveryClassification::SafeToResume => {
                // Revalidate the execution authorization before replaying
                // execution. Stale/corrupt/unauthorized bindings halt here
                // instead of resuming under a dead grant.
                let lifecycle_repo =
                    crate::persistence::sqlite::repositories::SqliteLifecycleRepository::new(
                        pool.clone(),
                    );
                if let Err(e) = lifecycle_repo
                    .revalidate_authorization_for_resume(self.mission_id)
                    .await
                {
                    let reason = ControllerHaltReason::UnrecoverableState {
                        reason: format!("resume authorization revalidation failed: {e}"),
                    };
                    self.emit_event(EventType::ControllerHalted {
                        mission_id: self.mission_id,
                        reason: format!("{reason:?}"),
                    })
                    .await;
                    return Ok(StageOutcome::Halt(reason));
                }
                // Hydrate admission-critical budget counters from the durable
                // ledger before resuming. Unknown ledger state halts (fail
                // closed) rather than re-admitting blind.
                if let Some(enforcer) = self.dependencies.budget_enforcer() {
                    if let Err(e) = crate::budget::BudgetLedger::new(pool.clone())
                        .hydrate(self.mission_id, enforcer)
                        .await
                    {
                        let reason = ControllerHaltReason::UnrecoverableState {
                            reason: format!("budget ledger hydration failed: {e}"),
                        };
                        self.emit_event(EventType::ControllerHalted {
                            mission_id: self.mission_id,
                            reason: format!("{reason:?}"),
                        })
                        .await;
                        return Ok(StageOutcome::Halt(reason));
                    }
                }
                if let Some(cp_id) = scan_result.checkpoint_id {
                    let cp_data = CheckpointManager::get_checkpoint_manifest(&pool, cp_id)
                        .await
                        .map_err(|e| ControllerError::SeamError {
                            seam: "persistence".into(),
                            message: e.to_string(),
                        })?;

                    if let Some((cycle, manifest)) = cp_data {
                        let resume_engine = crate::checkpoint::resume::SafeResumeEngine::new(
                            pool.clone(),
                            artifact_store.clone(),
                            workspace_root.as_ref(),
                        );
                        // Resume reconciliation failures halt instead of
                        // advancing to Observe over unreconciled state.
                        if let Err(e) = resume_engine
                            .resume_mission(self.mission_id, &manifest)
                            .await
                        {
                            let reason = ControllerHaltReason::UnrecoverableState {
                                reason: format!("resume reconciliation failed: {e}"),
                            };
                            self.emit_event(EventType::ControllerHalted {
                                mission_id: self.mission_id,
                                reason: format!("{reason:?}"),
                            })
                            .await;
                            return Ok(StageOutcome::Halt(reason));
                        }
                        self.progress.cycle = cycle as u64;
                    }
                }
                self.progress.current_stage = LoopStage::Observe;
                Ok(StageOutcome::Advance(LoopStage::Observe))
            }
            crate::checkpoint::crash_recovery::CrashRecoveryClassification::NeedsRepair => {
                let ws_path = workspace_root.as_ref();
                let mut repaired = false;
                if !ws_path.exists() {
                    if tokio::fs::create_dir_all(ws_path).await.is_ok() {
                        repaired = true;
                    }
                } else {
                    repaired = true;
                }
                if repaired && let Some(cp_id) = scan_result.checkpoint_id {
                    // Channel-aware staging (crash recovery must restore from
                    // the same channel's staging area).
                    let staging = crate::deployment::DeploymentPaths::project_staging_dir(
                        ws_path,
                        crate::deployment::DeploymentChannel::current(),
                    );
                    let checkpoint_mgr = crate::checkpoint::manager::CheckpointManager::new(
                        pool.clone(),
                        artifact_store.clone(),
                        staging,
                    );
                    if checkpoint_mgr
                        .restore_checkpoint(cp_id, ws_path)
                        .await
                        .is_ok()
                    {
                        repaired = true;
                    }
                }

                if repaired {
                    tracing::info!(
                        mission_id = %self.mission_id,
                        "Crash recovery successfully performed deterministic repair"
                    );
                    self.progress.current_stage = LoopStage::Observe;
                    Ok(StageOutcome::Advance(LoopStage::Observe))
                } else {
                    let reason = ControllerHaltReason::UnrecoverableState {
                        reason: format!(
                            "Crash recovery repair failed: {}",
                            scan_result.explanation
                        ),
                    };
                    self.emit_event(EventType::ControllerHalted {
                        mission_id: self.mission_id,
                        reason: format!("{reason:?}"),
                    })
                    .await;
                    Ok(StageOutcome::Halt(reason))
                }
            }
            crate::checkpoint::crash_recovery::CrashRecoveryClassification::Ambiguous => {
                let reason = ControllerHaltReason::UnrecoverableState {
                    reason: scan_result.explanation,
                };
                self.emit_event(EventType::ControllerHalted {
                    mission_id: self.mission_id,
                    reason: format!("{reason:?}"),
                })
                .await;
                Ok(StageOutcome::Halt(reason))
            }
            crate::checkpoint::crash_recovery::CrashRecoveryClassification::Corrupt => {
                let reason = ControllerHaltReason::FatalError {
                    failure_class: format!("CorruptCheckpoint: {}", scan_result.explanation),
                };
                self.emit_event(EventType::ControllerHalted {
                    mission_id: self.mission_id,
                    reason: format!("{reason:?}"),
                })
                .await;
                Ok(StageOutcome::Halt(reason))
            }
        }
    }

    /// Layer 2: Execute a bounded single cycle until Checkpoint, Yield, or Halt (D-01).
    pub async fn tick(&mut self) -> Result<StageOutcome, ControllerError> {
        loop {
            if self.cancellation_token.is_cancelled() {
                let reason = ControllerHaltReason::Cancelled;
                self.emit_event(EventType::ControllerHalted {
                    mission_id: self.mission_id,
                    reason: format!("{reason:?}"),
                })
                .await;
                return Ok(StageOutcome::Halt(reason));
            }

            let current = self.progress.current_stage;
            let outcome = self.step().await?;

            match &outcome {
                StageOutcome::Advance(LoopStage::Observe) if current == LoopStage::Checkpoint => {
                    self.progress.increment_cycle();
                    return Ok(outcome);
                }
                StageOutcome::Yield(_) => {
                    return Ok(outcome);
                }
                StageOutcome::Halt(_) => {
                    return Ok(outcome);
                }
                _ => {
                    // Continue executing next stage in cycle
                }
            }
        }
    }

    /// Layer 3: Loop ticks continuously until halt or cancellation (D-01).
    pub async fn run(&mut self) -> Result<ControllerHaltReason, ControllerError> {
        let cancel_token = self.cancellation_token.clone();
        loop {
            tokio::select! {
                biased;

                _ = cancel_token.cancelled() => {
                    let reason = ControllerHaltReason::Cancelled;
                    self.emit_event(EventType::ControllerHalted {
                        mission_id: self.mission_id,
                        reason: format!("{reason:?}"),
                    }).await;
                    return Ok(reason);
                }
                res = self.tick() => {
                    match res? {
                        StageOutcome::Halt(reason) => return Ok(reason),
                        StageOutcome::Yield(Some(dur)) => {
                            tokio::select! {
                                biased;
                                _ = cancel_token.cancelled() => {
                                    let reason = ControllerHaltReason::Cancelled;
                                    self.emit_event(EventType::ControllerHalted {
                                        mission_id: self.mission_id,
                                        reason: format!("{reason:?}"),
                                    }).await;
                                    return Ok(reason);
                                }
                                _ = tokio::time::sleep(dur) => {}
                            }
                        }
                        StageOutcome::Yield(None) => {
                            tokio::task::yield_now().await;
                        }
                        StageOutcome::Advance(LoopStage::Observe) => {
                            // Cycle finished successfully, yield to cooperative scheduler before next cycle
                            tokio::task::yield_now().await;
                        }
                        _ => {
                            tokio::task::yield_now().await;
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
    use crate::controller::harness::MockControllerHarness;
    use std::time::Duration;

    #[tokio::test]
    async fn test_single_stage_step() {
        let harness = MockControllerHarness::new();
        let mission_id = MissionId::new();
        let cancel = CancellationToken::new();
        let mut controller =
            harness.create_controller(mission_id, AutonomyMode::Autonomous, cancel);

        assert_eq!(controller.progress.current_stage, LoopStage::Observe);
        let outcome = controller.step().await.unwrap();
        assert_eq!(outcome, StageOutcome::Advance(LoopStage::IdentifyReadyWork));
        assert_eq!(
            controller.progress.current_stage,
            LoopStage::IdentifyReadyWork
        );

        let outcome2 = controller.step().await.unwrap();
        assert_eq!(
            outcome2,
            StageOutcome::Advance(LoopStage::ValidatePolicyAndResources)
        );
        assert_eq!(
            controller.progress.current_stage,
            LoopStage::ValidatePolicyAndResources
        );
    }

    #[tokio::test]
    async fn test_full_cycle_tick_and_event_publishing() {
        use crate::events::bus::EventFilter;
        use futures::StreamExt;

        let harness = MockControllerHarness::new();
        let mission_id = MissionId::new();
        let cancel = CancellationToken::new();
        let mut rx = harness.event_bus.subscribe(EventFilter::all()).await;
        let mut controller =
            harness.create_controller(mission_id, AutonomyMode::Autonomous, cancel);

        assert_eq!(controller.progress.cycle, 0);
        let outcome = controller.tick().await.unwrap();
        assert_eq!(outcome, StageOutcome::Advance(LoopStage::Observe));
        assert_eq!(controller.progress.cycle, 1);
        assert_eq!(controller.progress.current_stage, LoopStage::Observe);

        // Verify events were emitted
        let mut event_names = Vec::new();
        while let Ok(Some(Ok(env))) =
            tokio::time::timeout(Duration::from_millis(20), rx.next()).await
        {
            event_names.push(env.event_type.name().to_string());
        }
        assert!(event_names.contains(&"ControllerCycleStarted".to_string()));
        assert!(event_names.contains(&"ControllerStageTransitioned".to_string()));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_cancellation_halts_run() {
        let harness = MockControllerHarness::new();
        let mission_id = MissionId::new();
        let cancel = CancellationToken::new();
        let mut controller =
            harness.create_controller(mission_id, AutonomyMode::Autonomous, cancel.clone());

        let cancel_clone = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            cancel_clone.cancel();
        });

        let reason = controller.run().await.unwrap();
        assert_eq!(reason, ControllerHaltReason::Cancelled);
    }

    #[tokio::test]
    async fn test_pre_cancelled_run() {
        let harness = MockControllerHarness::new();
        let mission_id = MissionId::new();
        let cancel = CancellationToken::new();
        cancel.cancel();

        let mut controller =
            harness.create_controller(mission_id, AutonomyMode::Autonomous, cancel);
        let reason = controller.run().await.unwrap();
        assert_eq!(reason, ControllerHaltReason::Cancelled);
    }

    #[tokio::test]
    async fn test_step_when_cancelled_returns_halt() {
        let harness = MockControllerHarness::new();
        let mission_id = MissionId::new();
        let cancel = CancellationToken::new();
        cancel.cancel();

        let mut controller =
            harness.create_controller(mission_id, AutonomyMode::Autonomous, cancel);
        let outcome = controller.step().await.unwrap();
        assert_eq!(outcome, StageOutcome::Halt(ControllerHaltReason::Cancelled));
    }

    #[tokio::test]
    async fn test_preflight_budget_exhaustion_halts() {
        let harness = MockControllerHarness::new();
        let mission_id = MissionId::new();
        let cancel = CancellationToken::new();
        let limits = ResourceBudget {
            max_wall_clock_seconds: Some(50),
            ..Default::default()
        };

        let mut controller = AutonomyController::with_budget(
            mission_id,
            AutonomyMode::Autonomous,
            harness.dependencies(),
            limits,
            None,
            harness.event_bus.clone(),
            cancel,
        )
        .with_policy_role(crate::state_machine::agent::AgentRole::implementer())
        .with_workspace_root(std::env::temp_dir().join(format!("m31a-ctrl-{mission_id}")));

        controller.budget_tracker.record_elapsed_seconds(50);
        let outcome = controller.step().await.unwrap();
        assert_eq!(
            outcome,
            StageOutcome::Halt(ControllerHaltReason::BudgetExhausted {
                kind: BudgetKind::WallClock
            })
        );
    }

    #[tokio::test]
    async fn test_max_cycles_exceeded_halts() {
        let harness = MockControllerHarness::new();
        let mission_id = MissionId::new();
        let cancel = CancellationToken::new();

        let mut controller = AutonomyController::with_budget(
            mission_id,
            AutonomyMode::Autonomous,
            harness.dependencies(),
            ResourceBudget::default(),
            Some(1),
            harness.event_bus.clone(),
            cancel,
        )
        .with_policy_role(crate::state_machine::agent::AgentRole::implementer())
        .with_workspace_root(std::env::temp_dir().join(format!("m31a-ctrl-{mission_id}")));

        // First cycle finishes
        let outcome = controller.tick().await.unwrap();
        assert_eq!(outcome, StageOutcome::Advance(LoopStage::Observe));
        assert_eq!(controller.progress.cycle, 1);

        // Next step at cycle 1 hits limit of 1
        let outcome2 = controller.step().await.unwrap();
        assert_eq!(
            outcome2,
            StageOutcome::Halt(ControllerHaltReason::MaxCyclesExceeded { limit: 1 })
        );
    }

    #[tokio::test]
    async fn test_set_autonomy_mode_cycle_boundary() {
        use crate::events::bus::EventFilter;
        use futures::StreamExt;

        let harness = MockControllerHarness::new();
        let mission_id = MissionId::new();
        let cancel = CancellationToken::new();
        let mut rx = harness.event_bus.subscribe(EventFilter::all()).await;
        let mut controller = harness.create_controller(mission_id, AutonomyMode::Safe, cancel);

        // At Observe (cycle boundary): mode change succeeds (Edge 3)
        assert_eq!(controller.progress.current_stage, LoopStage::Observe);
        assert!(
            controller
                .set_autonomy_mode(AutonomyMode::Autonomous)
                .await
                .is_ok()
        );
        assert_eq!(controller.mode, AutonomyMode::Autonomous);

        // Verify event was emitted
        let env = tokio::time::timeout(Duration::from_millis(50), rx.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(
            env.event_type,
            EventType::ControllerDecisionModeChanged {
                mission_id,
                from: AutonomyMode::Safe,
                to: AutonomyMode::Autonomous,
            }
        );

        // Advance past Observe
        controller.step().await.unwrap();
        assert_eq!(
            controller.progress.current_stage,
            LoopStage::IdentifyReadyWork
        );

        // Mid-cycle mode change is rejected deterministically (Edge 3)
        let res = controller.set_autonomy_mode(AutonomyMode::Unattended).await;
        assert!(matches!(
            res,
            Err(ControllerError::InvalidStageTransition { .. })
        ));
        // Mode remains unchanged
        assert_eq!(controller.mode, AutonomyMode::Autonomous);
    }
}
