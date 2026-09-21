//! Adaptive Execution, Recovery & Replanning Subsystem.
//!
//! "The model proposes. The runtime decides."
//!
//! Provides canonical runtime structures and algorithms for:
//! - Structured diagnostic evidence flow from execution
//! - Evidence-based stagnation and loop detection across workspace mutations
//! - Adaptive strategy transitions between TaskShapes
//! - Plan-aware replanning with task completion preservation
//! - Assumption invalidation propagation to dependent work
//! - Dynamic unknown tracking during execution
//! - Finite runtime-owned execution budgets
//! - Multi-class failure categorization

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::agent::intent::{FormedTask, FormedTaskStatus, IntentError, IntentState, TaskShape};
use crate::kernel::seams::recovery::FailureClassification;

// ── Diagnostic Evidence ──────────────────────────────────────────────────────

/// Structured evidence produced by a tool execution or verification attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticEvidence {
    pub tool_name: String,
    pub target: Option<String>,
    pub failure_class: FailureClassification,
    pub error_summary: String,
    pub detailed_output: String,
    pub exit_code: Option<i32>,
    pub affected_files: Vec<String>,
    pub is_policy_denial: bool,
    pub timestamp: DateTime<Utc>,
    pub category: Option<ExecutionFailureCategory>,
    pub subagent_failure: Option<SubagentFailureEvidence>,
}

impl DiagnosticEvidence {
    pub fn new(
        tool_name: impl Into<String>,
        failure_class: FailureClassification,
        error_summary: impl Into<String>,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            target: None,
            failure_class,
            error_summary: error_summary.into(),
            detailed_output: String::new(),
            exit_code: None,
            affected_files: Vec::new(),
            is_policy_denial: false,
            timestamp: Utc::now(),
            category: None,
            subagent_failure: None,
        }
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    pub fn with_detailed_output(mut self, output: impl Into<String>) -> Self {
        self.detailed_output = output.into();
        self
    }

    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = Some(code);
        self
    }

    pub fn with_affected_files(mut self, files: Vec<String>) -> Self {
        self.affected_files = files;
        self
    }

    pub fn with_policy_denial(mut self, is_denial: bool) -> Self {
        self.is_policy_denial = is_denial;
        self
    }

    pub fn with_category(mut self, category: ExecutionFailureCategory) -> Self {
        self.category = Some(category);
        self
    }

    pub fn with_subagent_failure(mut self, failure: SubagentFailureEvidence) -> Self {
        self.subagent_failure = Some(failure);
        self
    }
}

// ── Failure Categorization ───────────────────────────────────────────────────

/// Distinct canonical categories of execution failures (Section 16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionFailureCategory {
    /// Operation blocked by runtime policy.
    PolicyDenied,
    /// Operation paused awaiting human operator authorization.
    WaitingForApproval,
    /// Tool or capability not available in the current environment.
    CapabilityUnavailable,
    /// Tool executed and failed (compilation, syntax, exit code).
    ExecutionFailed,
    /// External environment or dependency infrastructure failure.
    EnvironmentFailed,
    /// Model provider network, transport, timeout, or schema failure.
    ModelFailure,
    /// Tests or linters failed to verify the changes.
    VerificationFailure,
    /// Deterministically unrecoverable failure under runtime bounds.
    UnrecoverableFailure,
}

impl ExecutionFailureCategory {
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::PolicyDenied | Self::UnrecoverableFailure => false,
            Self::WaitingForApproval
            | Self::CapabilityUnavailable
            | Self::ExecutionFailed
            | Self::EnvironmentFailed
            | Self::ModelFailure
            | Self::VerificationFailure => true,
        }
    }
}

// ── Evidence-Based Stall / Loop Protection ───────────────────────────────────

/// An execution observation recorded for canonical stall and progress detection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionObservation {
    pub tool_name: String,
    pub input_fingerprint: String,
    pub workspace_state_fingerprint: String,
    pub success: bool,
    pub failure_signature: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Outcome of evaluating an action proposal against past execution evidence.
#[derive(Debug, Clone, PartialEq)]
pub enum StallEvaluation {
    /// Action is progressing or valid to execute.
    Progressing,
    /// The action failed previously with the identical workspace state.
    /// Rejected to prevent blind repetition without file inspection or parameter change.
    RepeatedActionRejected {
        tool_name: String,
        consecutive_count: usize,
        advice: String,
    },
    /// Identical failing action repeated 2+ times with no workspace change; hard stagnation loop.
    StagnationLoopDetected {
        tool_name: String,
        consecutive_count: usize,
        error: String,
    },
}

/// Canonical, evidence-based stall and stagnation detector (Section 13).
///
/// Uses: action name, inputs, workspace state (git status / mutation hash),
/// and failure signature.
/// A changed workspace state legitimately permits a repeated action (e.g. `run_tests`
/// running again after editing a file).
#[derive(Debug, Clone, Default)]
pub struct StallDetector {
    observations: Vec<ExecutionObservation>,
    max_consecutive_identical_failures: usize,
}

impl StallDetector {
    pub fn new() -> Self {
        Self {
            observations: Vec::new(),
            max_consecutive_identical_failures: 2,
        }
    }

    pub fn with_max_failures(mut self, max: usize) -> Self {
        self.max_consecutive_identical_failures = max;
        self
    }

    /// Evaluate an action proposal before execution against recent failure history.
    pub fn evaluate_proposal(
        &self,
        tool_name: &str,
        input_fingerprint: &str,
        current_workspace_fingerprint: &str,
    ) -> StallEvaluation {
        // Count consecutive failures with identical tool, input, and workspace state
        let consecutive_identical_failures = self
            .observations
            .iter()
            .rev()
            .take_while(|obs| {
                !obs.success
                    && obs.tool_name == tool_name
                    && obs.input_fingerprint == input_fingerprint
                    && obs.workspace_state_fingerprint == current_workspace_fingerprint
            })
            .count();

        if consecutive_identical_failures >= self.max_consecutive_identical_failures {
            StallEvaluation::StagnationLoopDetected {
                tool_name: tool_name.to_string(),
                consecutive_count: consecutive_identical_failures + 1,
                error: format!(
                    "Non-progress loop detected: identical failing action '{}' repeated {} times with identical workspace state",
                    tool_name,
                    consecutive_identical_failures + 1
                ),
            }
        } else if consecutive_identical_failures == 1 {
            StallEvaluation::RepeatedActionRejected {
                tool_name: tool_name.to_string(),
                consecutive_count: 1,
                advice: format!(
                    "Repeated non-progress action rejected: This identical '{}' tool call failed on the previous step with the same workspace state. You MUST inspect files with 'read_file', modify your changes, or alter your parameters before retrying.",
                    tool_name
                ),
            }
        } else {
            StallEvaluation::Progressing
        }
    }

    /// Record a completed execution observation.
    pub fn record_observation(&mut self, observation: ExecutionObservation) {
        self.observations.push(observation);
        if self.observations.len() > 50 {
            self.observations.remove(0);
        }
    }

    /// Reset recent observations on external steering or full strategy shift.
    pub fn clear(&mut self) {
        self.observations.clear();
    }
}

// ── Adaptive Execution Budget ────────────────────────────────────────────────

/// Finite runtime-owned budget bounds (Section 12).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveBudget {
    pub max_turns: u32,
    pub max_recovery_attempts: u32,
    pub max_plan_revisions: u32,
    pub max_delegation_depth: u32,
    pub recovery_attempts_consumed: u32,
    pub plan_revisions_consumed: u32,
}

impl Default for AdaptiveBudget {
    fn default() -> Self {
        Self {
            max_turns: 50,
            max_recovery_attempts: 5,
            max_plan_revisions: 5,
            max_delegation_depth: 2,
            recovery_attempts_consumed: 0,
            plan_revisions_consumed: 0,
        }
    }
}

impl AdaptiveBudget {
    pub fn new(max_turns: u32, max_recovery_attempts: u32, max_plan_revisions: u32) -> Self {
        Self {
            max_turns,
            max_recovery_attempts,
            max_plan_revisions,
            max_delegation_depth: 2,
            recovery_attempts_consumed: 0,
            plan_revisions_consumed: 0,
        }
    }

    pub fn can_attempt_recovery(&self) -> bool {
        self.recovery_attempts_consumed < self.max_recovery_attempts
    }

    pub fn record_recovery_attempt(&mut self) -> Result<u32, String> {
        if self.recovery_attempts_consumed >= self.max_recovery_attempts {
            return Err(format!(
                "Recovery budget exhausted: {}/{} attempts consumed",
                self.recovery_attempts_consumed, self.max_recovery_attempts
            ));
        }
        self.recovery_attempts_consumed += 1;
        Ok(self.recovery_attempts_consumed)
    }

    pub fn can_replan(&self) -> bool {
        self.plan_revisions_consumed < self.max_plan_revisions
    }

    pub fn record_replan(&mut self) -> Result<u32, String> {
        if self.plan_revisions_consumed >= self.max_plan_revisions {
            return Err(format!(
                "Plan revision budget exhausted: {}/{} revisions consumed",
                self.plan_revisions_consumed, self.max_plan_revisions
            ));
        }
        self.plan_revisions_consumed += 1;
        Ok(self.plan_revisions_consumed)
    }

    pub fn remaining_recovery_budget(&self) -> u32 {
        self.max_recovery_attempts
            .saturating_sub(self.recovery_attempts_consumed)
    }

    pub fn remaining_replan_budget(&self) -> u32 {
        self.max_plan_revisions
            .saturating_sub(self.plan_revisions_consumed)
    }
}

// ── Strategy Transitions ─────────────────────────────────────────────────────

/// Audit record of an adaptive strategy transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyTransitionRecord {
    pub from_shape: TaskShape,
    pub to_shape: TaskShape,
    pub reason: String,
    pub transitioned_at: DateTime<Utc>,
}

/// Validate whether a strategy transition is legal under runtime invariants (Section 11).
pub fn validate_strategy_transition(
    from: &TaskShape,
    to: &TaskShape,
    _intent: &IntentState,
) -> Result<(), IntentError> {
    if from == to {
        return Ok(());
    }

    // Rules:
    // 1. Transitioning to AskUserThenAct is valid if there are pending consequential decisions or unknowns requiring user input,
    //    or if unexpected blocking ambiguity arose.
    // 2. Transitioning to Recover is valid when prior attempts failed or assumptions were invalidated.
    // 3. Transitioning to ResearchThenAct is valid if external documentation or API uncertainty exists.
    // 4. Any transition must preserve runtime safety invariants.
    match (from, to) {
        (_, TaskShape::AskUserThenAct) => {
            // Permitted when user input is needed
            Ok(())
        }
        (_, TaskShape::Recover) => {
            // Permitted after failure or invalidation
            Ok(())
        }
        (TaskShape::Recover, TaskShape::PlanThenExecute)
        | (TaskShape::Recover, TaskShape::InvestigateThenAct)
        | (TaskShape::Recover, TaskShape::DirectToolExecution) => {
            // Recovery leads to revised execution
            Ok(())
        }
        (TaskShape::DirectToolExecution, TaskShape::InvestigateThenAct)
        | (TaskShape::DirectToolExecution, TaskShape::ResearchThenAct)
        | (TaskShape::DirectToolExecution, TaskShape::PlanThenExecute) => {
            // Escalation from tiny direct to investigation or planning
            Ok(())
        }
        (TaskShape::InvestigateThenAct, TaskShape::ResearchThenAct)
        | (TaskShape::InvestigateThenAct, TaskShape::PlanThenExecute)
        | (TaskShape::InvestigateThenAct, TaskShape::DirectToolExecution) => {
            // Investigation concludes into research, plan, or action
            Ok(())
        }
        (TaskShape::ResearchThenAct, TaskShape::InvestigateThenAct)
        | (TaskShape::ResearchThenAct, TaskShape::DirectToolExecution)
        | (TaskShape::ResearchThenAct, TaskShape::PlanThenExecute) => {
            // Research concludes into action or plan
            Ok(())
        }
        (TaskShape::PlanThenExecute, TaskShape::InvestigateThenAct)
        | (TaskShape::PlanThenExecute, TaskShape::DirectToolExecution) => {
            // Plan simplification or targeted investigation
            Ok(())
        }
        (TaskShape::AskUserThenAct, _) => {
            // User provided response; transitioning to chosen action or plan is permitted
            Ok(())
        }
        (TaskShape::Delegate { .. }, _) => {
            // Subagent execution concluded; returning to parent execution shape is permitted
            Ok(())
        }
        (_, TaskShape::Delegate { target_role }) => {
            if target_role.trim().is_empty() {
                return Err(IntentError::InvalidStrategyTransition {
                    from: from.clone(),
                    to: to.clone(),
                    reason: "Delegation target role cannot be empty".to_string(),
                });
            }
            Ok(())
        }
        (from, to) => Err(IntentError::InvalidStrategyTransition {
            from: from.clone(),
            to: to.clone(),
            reason: format!(
                "Unsupported or invalid strategy transition from {:?} to {:?}",
                from, to
            ),
        }),
    }
}

// ── Plan Revision ────────────────────────────────────────────────────────────

/// Request payload driving an adaptive plan revision (Section 7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveReplanRequest {
    pub reason: String,
    /// Tasks to supersede with reason: (task_id, reason)
    pub tasks_to_supersede: Vec<(String, String)>,
    /// New tasks to introduce into the plan
    pub new_tasks: Vec<FormedTask>,
    /// Assumptions invalidated by this replan
    pub invalidated_assumption_ids: Vec<String>,
}

/// Outcome of an adaptive plan revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveReplanOutcome {
    pub plan_revision: u32,
    pub preserved_task_ids: Vec<String>,
    pub superseded_task_ids: Vec<String>,
    pub new_task_ids: Vec<String>,
    pub preserved_completed_count: usize,
}

// ── Assumption Invalidation Report ───────────────────────────────────────────

/// Structured report of an assumption invalidation and its downstream effects (Section 9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssumptionInvalidationReport {
    pub assumption_id: String,
    pub contradicting_evidence: String,
    pub superseded_task_ids: Vec<String>,
    pub preserved_completed_task_ids: Vec<String>,
    pub affected_decision_ids: Vec<String>,
}

// ── Subagent Failure Evidence ────────────────────────────────────────────────

/// Structured causal evidence returned to the parent when a subagent fails (Section 19).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubagentFailureEvidence {
    pub subagent_role: String,
    pub error_message: String,
    pub completed_work_summary: Option<String>,
    pub unresolved_unknowns: Vec<String>,
    pub invalidated_assumptions: Vec<String>,
}

// ── Recovery Context Rendering ───────────────────────────────────────────────

/// Helper for rendering a compact recovery context fragment into the model prompt (Section 14).
pub fn render_recovery_context(
    intent: &IntentState,
    budget: &AdaptiveBudget,
    recent_diagnostics: &[DiagnosticEvidence],
) -> String {
    let mut parts = Vec::new();

    parts.push(format!(
        "### Adaptive Execution & Recovery Status (v{}, rev {})\n",
        intent.version, intent.plan_revision
    ));
    parts.push(format!(
        "- Current Strategy: {:?}\n",
        intent.current_strategy
    ));
    parts.push(format!(
        "- Remaining Budgets: Recovery Attempts: {}/{}, Plan Revisions: {}/{}\n",
        budget.remaining_recovery_budget(),
        budget.max_recovery_attempts,
        budget.remaining_replan_budget(),
        budget.max_plan_revisions
    ));

    if let Some(diag) = recent_diagnostics.last() {
        parts.push("\n### Recent Diagnostic Evidence\n".to_string());
        parts.push(format!("- Tool: {}\n", diag.tool_name));
        parts.push(format!("- Failure Class: {:?}\n", diag.failure_class));
        parts.push(format!("- Error Summary: {}\n", diag.error_summary));
        if !diag.affected_files.is_empty() {
            parts.push(format!(
                "- Affected Files: {}\n",
                diag.affected_files.join(", ")
            ));
        }
        if diag.is_policy_denial {
            parts.push(
                "- POLICY DENIAL: Do not retry the identical unauthorized action without operator permission.\n"
                    .to_string(),
            );
        }
    }

    // Invalidated assumptions
    let invalidated: Vec<_> = intent
        .assumptions
        .iter()
        .filter(|a| !a.is_valid())
        .collect();
    if !invalidated.is_empty() {
        parts.push("\n### Invalidated Assumptions (DO NOT RELY ON THESE)\n".to_string());
        for a in &invalidated {
            if let Some(inv) = &a.invalidation {
                parts.push(format!(
                    "- [{}] {} (Invalidated by: {})\n",
                    a.id, a.description, inv.contradicting_evidence
                ));
            }
        }
    }

    // Work items breakdown
    let completed: Vec<_> = intent
        .formed_tasks
        .iter()
        .filter(|t| t.is_completed())
        .collect();
    let superseded: Vec<_> = intent
        .formed_tasks
        .iter()
        .filter(|t| t.is_superseded())
        .collect();
    let pending: Vec<_> = intent
        .formed_tasks
        .iter()
        .filter(|t| {
            matches!(
                t.status,
                FormedTaskStatus::Pending | FormedTaskStatus::InProgress
            )
        })
        .collect();

    parts.push("\n### Work Items Status\n".to_string());
    if !completed.is_empty() {
        parts.push(format!(
            "- Preserved Completed Work ({} tasks): {}\n",
            completed.len(),
            completed
                .iter()
                .map(|t| format!("[{}] {}", t.id, t.title))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !superseded.is_empty() {
        parts.push(format!(
            "- Superseded / Stale Work ({} tasks): {}\n",
            superseded.len(),
            superseded
                .iter()
                .map(|t| format!("[{}] {}", t.id, t.title))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !pending.is_empty() {
        parts.push(format!(
            "- Active Pending Work ({} tasks): {}\n",
            pending.len(),
            pending
                .iter()
                .map(|t| format!("[{}] {}", t.id, t.title))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    parts.concat()
}
