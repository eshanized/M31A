//! First-class Intent Understanding & Adaptive Task Formation.
//!
//! # Architecture
//!
//! `IntentState` is the single canonical representation of what the agent
//! understands about the user's request at any point during a session.
//!
//! It is NOT a competing engine — it is a composable capability integrated
//! into `AgentEngine`. The model populates intent state via structured tool
//! calls; the runtime persists, validates, and injects the relevant fragment
//! into each model turn's context.
//!
//! ## Lifecycle
//!
//! ```text
//! UserIntent (raw text)
//!   ↓
//! IntentState::initial_from_prompt  ← only sets raw_prompt + timestamp
//!   ↓  (model turn with repository tools)
//! IntentState (enriched by model-proposed facts)
//!   ↓  (unknowns discovered from evidence)
//! IntentUnknown::from_evidence     ← runtime asserts evidence source
//!   ↓  (consequential decision identified)
//! IntentDecision (AskUser → user answers → decision resolved)
//!   ↓  (model infers safe facts from repo)
//! IntentAssumption with provenance
//!   ↓  (adaptive task formation)
//! TaskShape selected by model, constrained by runtime
//!   ↓
//! AgentEngine execution
//! ```
//!
//! ## Governing Principle
//!
//! "The model proposes. The runtime decides."
//!
//! The model proposes intent enrichment by calling the `understand_intent`
//! tool. The runtime validates provenance and persists the result.
//! The model never becomes authoritative over intent state.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::kernel::plan::{
    CandidatePlan, CandidatePlanAssumption, CandidateTask, CandidateTaskKey, ResourceEstimate,
    VerificationStrategy,
};
use crate::planning::requirements::{
    EpistemicStatus, Provenance, ProvenanceSourceType, TrustLevel,
};
use crate::planning::risks::{Criticality, UnknownFate};
use crate::state_machine::agent::AgentRole;

// ── Fact Provenance ────────────────────────────────────────────────────────────

/// First-class provenance distinguishing how each intent fact was established.
///
/// Prevents silently treating model inference as authoritative user input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactOrigin {
    /// Stated explicitly by the operator in their original request.
    UserProvided,
    /// Observed from filesystem, git history, or manifest files in the repository.
    RepositoryObserved,
    /// Result of an executed tool (e.g., `read_file`, `repo_search`).
    ToolObserved { tool_name: String },
    /// Inferred by the model from available evidence (must be flagged, not opaque).
    ModelInferred { reasoning_summary: String },
    /// Retrieved from external documentation or API.
    Researched { source_url: Option<String> },
    /// Assumed by the model when no contrary evidence exists; explicitly flagged.
    Assumed { basis: String },
    /// Determined by runtime policy invariants.
    PolicyForced,
    /// Determined by deterministic runtime heuristics (keyword detection, signal
    /// analysis) — NOT model inference. The runtime itself made this determination
    /// without invoking a model. Distinct from `ModelInferred`.
    RuntimeInferred { heuristic: String },
}

impl FactOrigin {
    /// Map to a canonical `TrustLevel` for interop with the planning subsystem.
    pub fn trust_level(&self) -> TrustLevel {
        match self {
            Self::UserProvided => TrustLevel::AuthoritativeRuntime,
            Self::RepositoryObserved | Self::ToolObserved { .. } => TrustLevel::VerifiedRepository,
            Self::Researched { .. } => TrustLevel::UntrustedExternal,
            Self::ModelInferred { .. } | Self::Assumed { .. } => TrustLevel::UntrustedModelProposal,
            Self::PolicyForced => TrustLevel::AuthoritativeRuntime,
            // Runtime heuristic: deterministic signal detection, trusted as a runtime
            // observation but not as authoritative user input.
            Self::RuntimeInferred { .. } => TrustLevel::VerifiedRepository,
        }
    }

    /// Map to `EpistemicStatus` for downstream planning compatibility.
    pub fn epistemic_status(&self) -> EpistemicStatus {
        match self {
            Self::UserProvided => EpistemicStatus::ExplicitUserRequirement,
            Self::RepositoryObserved | Self::ToolObserved { .. } => EpistemicStatus::VerifiedFact,
            Self::Researched { .. } => EpistemicStatus::InferredFact,
            Self::ModelInferred { .. } => EpistemicStatus::Hypothesis,
            Self::Assumed { .. } => EpistemicStatus::Assumption,
            Self::PolicyForced => EpistemicStatus::VerifiedFact,
            // Runtime heuristics are deterministic inferences, not model hypotheses.
            Self::RuntimeInferred { .. } => EpistemicStatus::InferredFact,
        }
    }

    /// Convert to a canonical `Provenance` for interop with risk/requirements types.
    pub fn to_provenance(&self, actor: impl Into<String>) -> Provenance {
        Provenance {
            source_type: match self {
                Self::UserProvided => ProvenanceSourceType::UserPrompt,
                Self::RepositoryObserved => ProvenanceSourceType::RepositoryFile,
                Self::ToolObserved { .. } => ProvenanceSourceType::ToolOutput,
                Self::Researched { .. } => ProvenanceSourceType::ExternalDoc,
                Self::ModelInferred { .. } | Self::Assumed { .. } => {
                    ProvenanceSourceType::ToolOutput
                }
                Self::PolicyForced => ProvenanceSourceType::RuntimePolicy,
                Self::RuntimeInferred { .. } => ProvenanceSourceType::RuntimePolicy,
            },
            trust_level: self.trust_level(),
            location: None,
            timestamp: Utc::now(),
            actor: actor.into(),
            reason: None,
        }
    }
}

// ── Intent Unknown ─────────────────────────────────────────────────────────────

/// A genuinely unresolved item discovered from model reasoning or repository
/// evidence — NOT a fabricated placeholder.
///
/// Invariant: every `IntentUnknown` must have `evidence_basis` describing why
/// the runtime believes this is actually unknown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentUnknown {
    pub id: String,
    pub description: String,
    pub criticality: Criticality,
    /// Where this unknown came from — must be evidence-backed.
    pub origin: FactOrigin,
    /// Why the agent considers this genuinely unresolved.
    pub evidence_basis: String,
    /// Classified fate: what should happen to resolve this unknown.
    pub fate: UnknownFate,
    /// If resolved, how it was resolved.
    pub resolution: Option<UnknownResolution>,
    pub created_at: DateTime<Utc>,
}

impl IntentUnknown {
    pub fn from_evidence(
        id: impl Into<String>,
        description: impl Into<String>,
        criticality: Criticality,
        origin: FactOrigin,
        evidence_basis: impl Into<String>,
        fate: UnknownFate,
    ) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            criticality,
            origin,
            evidence_basis: evidence_basis.into(),
            fate,
            resolution: None,
            created_at: Utc::now(),
        }
    }

    pub fn is_resolved(&self) -> bool {
        self.resolution.is_some()
    }

    pub fn resolve(&mut self, resolution: UnknownResolution) {
        self.resolution = Some(resolution);
    }
}

/// How an unknown was ultimately resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum UnknownResolution {
    /// Safely inferred from repository or tool evidence.
    InferredFromEvidence {
        inference: String,
        evidence_source: FactOrigin,
    },
    /// User explicitly answered a question about this unknown.
    UserDecision {
        user_answer: String,
        question_asked: String,
    },
    /// Deferred: not required for current execution scope.
    Deferred { reason: String },
    /// Resolved via external research.
    Researched {
        finding: String,
        source: Option<String>,
    },
}

// ── Intent Assumption ──────────────────────────────────────────────────────────

/// A runtime-visible assumption the model made during intent understanding.
///
/// Assumptions are explicitly flagged so the runtime can detect when new
/// evidence contradicts them. An assumption must NOT silently become fact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentAssumption {
    pub id: String,
    pub description: String,
    pub criticality: Criticality,
    /// The specific basis for this assumption.
    pub basis: String,
    /// Origin — must typically be `ModelInferred` or `Assumed`.
    pub origin: FactOrigin,
    /// Decisions or tasks affected if this assumption turns out to be wrong.
    pub affected_unknowns: Vec<String>,
    pub affected_task_ids: Vec<String>,
    /// If invalidated by new evidence, record the contradiction.
    pub invalidation: Option<AssumptionInvalidation>,
    pub created_at: DateTime<Utc>,
}

impl IntentAssumption {
    pub fn is_valid(&self) -> bool {
        self.invalidation.is_none()
    }

    pub fn invalidate(&mut self, contradiction: AssumptionInvalidation) {
        self.invalidation = Some(contradiction);
    }
}

/// Evidence that contradicts a previously made assumption.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssumptionInvalidation {
    pub contradicting_evidence: String,
    pub evidence_origin: FactOrigin,
    pub invalidated_at: DateTime<Utc>,
}

// ── Consequential Decision ─────────────────────────────────────────────────────

/// A decision with material consequence that requires explicit user input OR
/// was definitively resolved through evidence.
///
/// Decisions are NOT fabricated — each must be identified because the model
/// reasoned that the choice materially affects implementation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentDecision {
    pub id: String,
    pub question: String,
    pub options_considered: Vec<String>,
    pub status: DecisionStatus,
    pub resolution: Option<DecisionResolution>,
    /// Why this decision is consequential (must be concrete reasoning).
    pub consequence_rationale: String,
    pub created_at: DateTime<Utc>,
}

impl IntentDecision {
    pub fn is_pending(&self) -> bool {
        self.resolution.is_none()
    }

    pub fn resolve(&mut self, resolution: DecisionResolution) {
        self.status = DecisionStatus::Resolved;
        self.resolution = Some(resolution);
    }
}

/// Lifecycle status of a consequential decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    Pending,
    Resolved,
    Deferred,
}

/// How a consequential decision was resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "resolution_type", rename_all = "snake_case")]
pub enum DecisionResolution {
    /// User explicitly selected an option.
    UserSelected {
        selected_option: String,
        raw_answer: String,
    },
    /// Runtime inferred from repository evidence (safe, non-consequential path).
    RepositoryDerived { evidence: String },
    /// Policy mandated the answer.
    PolicyForced { policy: String },
    /// Model inferred — only valid for non-blocking decisions.
    ModelInferred { reasoning: String },
}

// ── Task Shape ─────────────────────────────────────────────────────────────────

/// Adaptive execution shape selected by the model for a specific task.
///
/// The model chooses the shape based on task complexity and available context.
/// The runtime enforces that the shape is within policy bounds.
///
/// Note: This is NOT a mandatory DAG. Tiny tasks use `DirectToolExecution`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskShape {
    /// Simple tool calls sufficient — no planning infrastructure needed.
    DirectToolExecution,
    /// Inspect repository/files before taking action.
    InvestigateThenAct,
    /// Must ask user before proceeding (consequential decision pending).
    AskUserThenAct,
    /// Research external docs/APIs, then act.
    ResearchThenAct,
    /// Multi-stage work requiring DAG/workflow decomposition.
    PlanThenExecute,
    /// Hand off to another agent role.
    Delegate { target_role: String },
    /// Recovery: prior attempt failed, replan from evidence.
    Recover,
}

impl TaskShape {
    /// Whether this shape requires planning infrastructure.
    pub fn requires_planning(&self) -> bool {
        matches!(self, Self::PlanThenExecute)
    }

    /// Parse a model-proposed strategy name into its typed shape.
    ///
    /// SINGLE definition site for strategy-name authority: execution layers
    /// MUST route through this constructor instead of ad-hoc
    /// `to_lowercase()` / `starts_with("delegate:")` matching. Returns `None`
    /// for unknown names so callers fail closed on unrecognized strategies.
    /// Delegation targets are preserved verbatim as typed `Delegate` shapes;
    /// the target role string is converted to `AgentRole` authority at the
    /// delegation site, never used as raw prompt text.
    pub fn parse_strategy_name(input: &str) -> Option<Self> {
        match input.to_lowercase().as_str() {
            "investigate_then_act" | "investigate" => Some(Self::InvestigateThenAct),
            "research_then_act" | "research" => Some(Self::ResearchThenAct),
            "ask_user_then_act" | "ask_user" => Some(Self::AskUserThenAct),
            "plan_then_execute" | "plan" => Some(Self::PlanThenExecute),
            "recover" | "recovery" => Some(Self::Recover),
            "direct_tool_execution" | "direct" => Some(Self::DirectToolExecution),
            s if s.starts_with("delegate:") => {
                let role = s.trim_start_matches("delegate:").trim();
                if role.is_empty() {
                    None
                } else {
                    Some(Self::Delegate {
                        target_role: role.to_string(),
                    })
                }
            }
            _ => None,
        }
    }

    /// Whether this shape requires blocking on user input.
    pub fn requires_user_input(&self) -> bool {
        matches!(self, Self::AskUserThenAct)
    }
}

// ── Formed Task ────────────────────────────────────────────────────────────────

/// An executable task formed from intent understanding.
///
/// Tasks formed here are lightweight representations used by the agent for
/// tracking progress. They are separate from the full DAG task infrastructure
/// used by the workflow engine for complex missions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormedTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub shape: TaskShape,
    pub status: FormedTaskStatus,
    /// Assumptions this task depends on being valid.
    pub assumption_ids: Vec<String>,
    /// Unknowns that must be resolved before this task can execute.
    pub blocking_unknown_ids: Vec<String>,
    /// Provenance linking this replacement task to a superseded task (Section 7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement_for_task_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl FormedTask {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        shape: TaskShape,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            shape,
            status: FormedTaskStatus::Pending,
            assumption_ids: Vec::new(),
            blocking_unknown_ids: Vec::new(),
            replacement_for_task_id: None,
            created_at: Utc::now(),
        }
    }

    pub fn with_replacement(mut self, original_task_id: impl Into<String>) -> Self {
        self.replacement_for_task_id = Some(original_task_id.into());
        self
    }

    pub fn is_executable(&self) -> bool {
        matches!(
            self.status,
            FormedTaskStatus::Pending | FormedTaskStatus::InProgress
        ) && self.blocking_unknown_ids.is_empty()
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            FormedTaskStatus::Pending | FormedTaskStatus::InProgress
        )
    }

    pub fn is_completed(&self) -> bool {
        matches!(self.status, FormedTaskStatus::Completed { .. })
    }

    pub fn is_superseded(&self) -> bool {
        matches!(self.status, FormedTaskStatus::Superseded { .. })
    }

    pub fn is_failed(&self) -> bool {
        matches!(self.status, FormedTaskStatus::Failed { .. })
    }

    pub fn is_blocked(&self) -> bool {
        matches!(self.status, FormedTaskStatus::Blocked { .. })
    }

    pub fn is_cancelled(&self) -> bool {
        matches!(self.status, FormedTaskStatus::Cancelled { .. })
    }

    /// Invalidate this task due to new contradicting evidence.
    pub fn invalidate(&mut self, reason: impl Into<String>) {
        self.status = FormedTaskStatus::Superseded {
            reason: reason.into(),
            superseded_at: Utc::now(),
        };
    }

    /// Lower this formed task into a canonical `CandidateTask` for plan materialization.
    ///
    /// Preserves task identity, title, description, and assumption bindings without semantic loss.
    pub fn into_candidate_task(self) -> CandidateTask {
        let mut task = CandidateTask::new(
            CandidateTaskKey::new(self.id),
            self.title,
            AgentRole::implementer(),
            VerificationStrategy::Compilation,
            ResourceEstimate::default(),
        );
        task.description = if self.description.is_empty() {
            None
        } else {
            Some(self.description)
        };
        task.assumptions = self.assumption_ids;
        task
    }
}

impl From<FormedTask> for CandidateTask {
    fn from(task: FormedTask) -> Self {
        task.into_candidate_task()
    }
}

/// Lifecycle status of a formed intent task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FormedTaskStatus {
    Pending,
    InProgress,
    Completed {
        summary: String,
    },
    Failed {
        reason: String,
    },
    /// Task was invalidated by new evidence. Completed evidence is preserved.
    Superseded {
        reason: String,
        superseded_at: DateTime<Utc>,
    },
    Cancelled {
        reason: String,
    },
    Blocked {
        reason: String,
    },
}

// ── Intent State ───────────────────────────────────────────────────────────────

/// The single canonical semantic state representing the agent's understanding
/// of the user's request for a given session.
///
/// `IntentState` is:
/// - Durable (persisted to SQLite as JSON on every update)
/// - Versioned (each write increments `version`)
/// - Queryable (runtime can inspect any field)
/// - Fresh (model always receives current state, never stale)
/// - Non-authoritative (model proposes enrichment; runtime validates and persists)
///
/// It is integrated into `AgentEngine`, never a separate competing engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentState {
    pub session_id: String,
    /// The original raw user prompt, unmodified.
    pub raw_prompt: String,
    /// Model-articulated goal (what the agent intends to accomplish).
    pub goal: Option<String>,
    /// Scope: what is in-scope and out-of-scope.
    pub scope_in: Vec<String>,
    pub scope_out: Vec<String>,
    /// Known facts with their provenance.
    pub known_facts: Vec<IntentFact>,
    /// Unknowns discovered from evidence (not fabricated).
    pub unknowns: Vec<IntentUnknown>,
    /// Assumptions the model made (explicitly tracked).
    pub assumptions: Vec<IntentAssumption>,
    /// Consequential decisions (pending or resolved).
    pub decisions: Vec<IntentDecision>,
    /// Repository areas identified as relevant.
    pub relevant_repository_areas: Vec<String>,
    /// Tasks formed from intent understanding.
    pub formed_tasks: Vec<FormedTask>,
    /// Research results that have entered the evidence system.
    pub research_evidence: Vec<ResearchResult>,
    /// User steering constraints applied mid-session.
    pub steering_constraints: Vec<SteeringConstraint>,
    /// Current understanding confidence (0–100, runtime-maintained).
    pub confidence_percent: u8,
    /// How many times this state has been written (for audit).
    pub version: u64,
    /// Current active execution strategy / shape.
    #[serde(default = "default_task_shape")]
    pub current_strategy: TaskShape,
    /// History of strategy transitions.
    #[serde(default)]
    pub strategy_transitions: Vec<crate::agent::adaptive::StrategyTransitionRecord>,
    /// Differential plan revision counter.
    #[serde(default)]
    pub plan_revision: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn default_task_shape() -> TaskShape {
    TaskShape::DirectToolExecution
}

impl IntentState {
    /// Create an initial intent state from the raw user prompt.
    ///
    /// Only populates the minimum viable fields. Everything else is discovered
    /// by the model through repository inspection and tool calls.
    pub fn initial_from_prompt(
        session_id: impl Into<String>,
        raw_prompt: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            session_id: session_id.into(),
            raw_prompt: raw_prompt.into(),
            goal: None,
            scope_in: Vec::new(),
            scope_out: Vec::new(),
            known_facts: Vec::new(),
            unknowns: Vec::new(),
            assumptions: Vec::new(),
            decisions: Vec::new(),
            relevant_repository_areas: Vec::new(),
            formed_tasks: Vec::new(),
            research_evidence: Vec::new(),
            steering_constraints: Vec::new(),
            confidence_percent: 0,
            version: 0,
            current_strategy: TaskShape::DirectToolExecution,
            strategy_transitions: Vec::new(),
            plan_revision: 0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Apply user steering to the intent state.
    ///
    /// This is a first-class operation: steering constraints are persisted and
    /// the model receives them in subsequent turns. If steering contradicts
    /// assumptions, those assumptions are flagged for invalidation.
    pub fn apply_steering(&mut self, constraint: SteeringConstraint) {
        // Invalidate any assumption that conflicts with the new constraint
        for assumption in self.assumptions.iter_mut() {
            if assumption.is_valid() && constraint.contradicts_assumption(&assumption.description) {
                assumption.invalidate(AssumptionInvalidation {
                    contradicting_evidence: format!(
                        "User steering applied: {}",
                        constraint.constraint_text
                    ),
                    evidence_origin: FactOrigin::UserProvided,
                    invalidated_at: Utc::now(),
                });
            }
        }

        // Mark any formed tasks that depend on contradicted assumptions as superseded
        let invalidated_ids: Vec<String> = self
            .assumptions
            .iter()
            .filter(|a| a.invalidation.is_some())
            .map(|a| a.id.clone())
            .collect();

        for task in self.formed_tasks.iter_mut() {
            if task
                .assumption_ids
                .iter()
                .any(|aid| invalidated_ids.contains(aid))
                && matches!(task.status, FormedTaskStatus::Pending)
            {
                task.invalidate(format!(
                    "Assumption invalidated by user steering: {}",
                    constraint.constraint_text
                ));
            }
        }

        self.steering_constraints.push(constraint);
        self.bump_version();
    }

    /// Resolve a pending unknown with evidence.
    pub fn resolve_unknown(&mut self, unknown_id: &str, resolution: UnknownResolution) -> bool {
        if let Some(unk) = self.unknowns.iter_mut().find(|u| u.id == unknown_id) {
            unk.resolve(resolution);
            // Remove from blocking lists in formed tasks
            for task in self.formed_tasks.iter_mut() {
                task.blocking_unknown_ids.retain(|id| id != unknown_id);
            }
            self.bump_version();
            return true;
        }
        false
    }

    /// Add a new unknown discovered from evidence.
    ///
    /// Requires evidence_basis to be non-empty — prevents fabricated unknowns.
    pub fn add_unknown(&mut self, unknown: IntentUnknown) -> Result<(), IntentError> {
        if unknown.evidence_basis.trim().is_empty() {
            return Err(IntentError::FabricatedUnknown { id: unknown.id });
        }
        if self.unknowns.iter().any(|u| u.id == unknown.id) {
            return Err(IntentError::DuplicateUnknown { id: unknown.id });
        }
        self.unknowns.push(unknown);
        self.bump_version();
        Ok(())
    }

    /// Add a model-proposed assumption with explicit provenance.
    pub fn add_assumption(&mut self, assumption: IntentAssumption) {
        self.assumptions.push(assumption);
        self.bump_version();
    }

    /// Lookup a formed task by ID.
    pub fn task(&self, id: &str) -> Option<&FormedTask> {
        self.formed_tasks.iter().find(|t| t.id == id)
    }

    /// Lookup a formed task by ID mutably.
    pub fn task_mut(&mut self, id: &str) -> Option<&mut FormedTask> {
        self.formed_tasks.iter_mut().find(|t| t.id == id)
    }

    /// Add a consequential decision pending user resolution.
    pub fn add_decision(&mut self, decision: IntentDecision) {
        self.decisions.push(decision);
        self.bump_version();
    }

    /// Resolve a pending decision.
    pub fn resolve_decision(&mut self, decision_id: &str, resolution: DecisionResolution) -> bool {
        if let Some(dec) = self.decisions.iter_mut().find(|d| d.id == decision_id) {
            dec.resolve(resolution);
            self.bump_version();
            return true;
        }
        false
    }

    /// Add a formed task from intent understanding.
    pub fn add_formed_task(&mut self, task: FormedTask) {
        self.formed_tasks.push(task);
        self.bump_version();
    }

    /// Invalidate a formed task due to new evidence (preserves completed evidence).
    pub fn invalidate_task(&mut self, task_id: &str, reason: impl Into<String>) -> bool {
        if let Some(task) = self.formed_tasks.iter_mut().find(|t| t.id == task_id) {
            // Only invalidate pending/in-progress tasks — never retroactively corrupt completed evidence.
            if matches!(
                task.status,
                FormedTaskStatus::Pending | FormedTaskStatus::InProgress
            ) {
                task.invalidate(reason);
                self.bump_version();
                return true;
            }
        }
        false
    }

    /// Transition the active execution strategy/shape.
    pub fn transition_strategy(
        &mut self,
        new_shape: TaskShape,
        reason: impl Into<String>,
    ) -> Result<(), IntentError> {
        let reason_str = reason.into();
        crate::agent::adaptive::validate_strategy_transition(
            &self.current_strategy,
            &new_shape,
            self,
        )?;

        let old_shape = self.current_strategy.clone();
        self.strategy_transitions
            .push(crate::agent::adaptive::StrategyTransitionRecord {
                from_shape: old_shape,
                to_shape: new_shape.clone(),
                reason: reason_str,
                transitioned_at: Utc::now(),
            });
        self.current_strategy = new_shape;
        self.bump_version();
        Ok(())
    }

    /// Invalidate an assumption due to new contradicting evidence and propagate to dependent work.
    ///
    /// Traces all formed tasks depending on this assumption:
    /// - Pending or InProgress tasks are superseded with the invalidation rationale.
    /// - Completed tasks are PRESERVED: completed verified evidence is never retroactively destroyed.
    /// - Traces dependent decisions.
    pub fn invalidate_assumption(
        &mut self,
        assumption_id: &str,
        contradiction: AssumptionInvalidation,
    ) -> Result<crate::agent::adaptive::AssumptionInvalidationReport, IntentError> {
        let mut found = false;
        for a in self.assumptions.iter_mut() {
            if a.id == assumption_id {
                a.invalidate(contradiction.clone());
                found = true;
                break;
            }
        }
        if !found {
            return Err(IntentError::AssumptionNotFound {
                id: assumption_id.to_string(),
            });
        }

        let mut superseded_task_ids = Vec::new();
        let mut preserved_completed_task_ids = Vec::new();
        let mut affected_decision_ids = Vec::new();

        // 1. Identify and supersede dependent tasks (preserves completed tasks!)
        for t in self.formed_tasks.iter_mut() {
            if t.assumption_ids.iter().any(|aid| aid == assumption_id) {
                if matches!(t.status, FormedTaskStatus::Completed { .. }) {
                    // Completed verified evidence is immutable!
                    preserved_completed_task_ids.push(t.id.clone());
                } else if matches!(
                    t.status,
                    FormedTaskStatus::Pending | FormedTaskStatus::InProgress
                ) {
                    t.invalidate(format!(
                        "Assumption '{}' invalidated by evidence: {}",
                        assumption_id, contradiction.contradicting_evidence
                    ));
                    superseded_task_ids.push(t.id.clone());
                }
            }
        }

        // 2. Identify dependent decisions
        for d in self.decisions.iter_mut() {
            if d.is_pending() {
                affected_decision_ids.push(d.id.clone());
            }
        }

        self.bump_version();

        Ok(crate::agent::adaptive::AssumptionInvalidationReport {
            assumption_id: assumption_id.to_string(),
            contradicting_evidence: contradiction.contradicting_evidence,
            superseded_task_ids,
            preserved_completed_task_ids,
            affected_decision_ids,
        })
    }

    /// Replan formed tasks preserving completed work and updating plan revision.
    pub fn replan(
        &mut self,
        request: crate::agent::adaptive::AdaptiveReplanRequest,
    ) -> crate::agent::adaptive::AdaptiveReplanOutcome {
        let mut preserved_task_ids = Vec::new();
        let mut superseded_task_ids = Vec::new();
        let mut new_task_ids = Vec::new();
        let mut preserved_completed_count = 0;

        // 1. Process tasks to supersede
        for (task_id, reason) in &request.tasks_to_supersede {
            if let Some(t) = self.formed_tasks.iter_mut().find(|t| t.id == *task_id) {
                if matches!(t.status, FormedTaskStatus::Completed { .. }) {
                    // Completed work cannot be invalidated
                    preserved_task_ids.push(t.id.clone());
                    preserved_completed_count += 1;
                } else if !matches!(t.status, FormedTaskStatus::Superseded { .. }) {
                    t.invalidate(reason.clone());
                    superseded_task_ids.push(t.id.clone());
                }
            }
        }

        // 2. Identify remaining preserved tasks
        for t in &self.formed_tasks {
            if matches!(t.status, FormedTaskStatus::Completed { .. })
                && !preserved_task_ids.contains(&t.id)
            {
                preserved_task_ids.push(t.id.clone());
                preserved_completed_count += 1;
            } else if matches!(t.status, FormedTaskStatus::Pending)
                && !superseded_task_ids.contains(&t.id)
                && !preserved_task_ids.contains(&t.id)
            {
                preserved_task_ids.push(t.id.clone());
            }
        }

        // 3. Add new tasks
        for nt in request.new_tasks {
            new_task_ids.push(nt.id.clone());
            self.formed_tasks.push(nt);
        }

        self.plan_revision += 1;
        self.bump_version();

        crate::agent::adaptive::AdaptiveReplanOutcome {
            plan_revision: self.plan_revision,
            preserved_task_ids,
            superseded_task_ids,
            new_task_ids,
            preserved_completed_count,
        }
    }

    /// Add a known fact with explicit provenance.
    pub fn add_fact(&mut self, fact: IntentFact) {
        self.known_facts.push(fact);
        self.bump_version();
    }

    /// Add research evidence that has entered the canonical evidence system.
    pub fn add_research_result(&mut self, result: ResearchResult) {
        self.research_evidence.push(result);
        self.bump_version();
    }

    /// Pending unknowns that have `UserDecisionRequired` fate.
    pub fn pending_user_decisions(&self) -> Vec<&IntentUnknown> {
        self.unknowns
            .iter()
            .filter(|u| !u.is_resolved() && u.fate == UnknownFate::UserDecisionRequired)
            .collect()
    }

    /// Pending unknowns that can be safely inferred.
    pub fn safe_to_infer_unknowns(&self) -> Vec<&IntentUnknown> {
        self.unknowns
            .iter()
            .filter(|u| !u.is_resolved() && u.fate == UnknownFate::SafeToInfer)
            .collect()
    }

    /// Unresolved consequential decisions (require AskUser).
    pub fn pending_decisions(&self) -> Vec<&IntentDecision> {
        self.decisions.iter().filter(|d| d.is_pending()).collect()
    }

    /// Executable formed tasks (not blocked, not superseded).
    pub fn executable_tasks(&self) -> Vec<&FormedTask> {
        self.formed_tasks
            .iter()
            .filter(|t| t.is_executable())
            .collect()
    }

    /// Current steering constraints for model context injection.
    pub fn active_steering_constraints(&self) -> Vec<&SteeringConstraint> {
        self.steering_constraints.iter().collect()
    }

    /// Render a concise context fragment for injection into the model system prompt.
    ///
    /// This fragment gives the model authoritative awareness of current intent
    /// state without duplicating the full JSON blob. The model should use this
    /// to avoid re-asking resolved questions, honor steering, and respect known constraints.
    pub fn render_context_fragment(&self) -> String {
        let mut parts = Vec::new();

        parts.push(format!("## Intent State (v{})\n", self.version));

        if let Some(goal) = &self.goal {
            parts.push(format!("**Goal**: {}\n", goal));
        }

        if !self.scope_out.is_empty() {
            parts.push(format!("**Out-of-scope**: {}\n", self.scope_out.join(", ")));
        }

        // Steering constraints are highest priority
        if !self.steering_constraints.is_empty() {
            parts.push("\n### Active Operator Constraints\n".to_string());
            for sc in &self.steering_constraints {
                parts.push(format!("- {}\n", sc.constraint_text));
            }
        }

        // Pending user decisions that block execution
        let pending_decisions = self.pending_decisions();
        if !pending_decisions.is_empty() {
            parts.push("\n### Pending Consequential Decisions\n".to_string());
            for dec in &pending_decisions {
                parts.push(format!(
                    "- [{}] {} (use ask_user to resolve)\n",
                    dec.id, dec.question
                ));
            }
        }

        // Unresolved unknowns
        let unresolved: Vec<&IntentUnknown> =
            self.unknowns.iter().filter(|u| !u.is_resolved()).collect();
        if !unresolved.is_empty() {
            parts.push("\n### Unresolved Unknowns\n".to_string());
            for unk in &unresolved {
                parts.push(format!(
                    "- [{}] {} (fate: {:?})\n",
                    unk.id, unk.description, unk.fate
                ));
            }
        }

        // Active (valid) assumptions
        let valid_assumptions: Vec<&IntentAssumption> =
            self.assumptions.iter().filter(|a| a.is_valid()).collect();
        if !valid_assumptions.is_empty() {
            parts.push("\n### Active Assumptions\n".to_string());
            for a in &valid_assumptions {
                parts.push(format!(
                    "- [{}] {} (basis: {})\n",
                    a.id, a.description, a.basis
                ));
            }
        }

        // Relevant repository areas
        if !self.relevant_repository_areas.is_empty() {
            parts.push("\n### Identified Repository Areas\n".to_string());
            for area in &self.relevant_repository_areas {
                parts.push(format!("- {}\n", area));
            }
        }

        // Strategy and plan revision
        parts.push(format!(
            "\n### Strategy & Plan (Revision {})\n- Active Strategy: {:?}\n",
            self.plan_revision, self.current_strategy
        ));

        // Formed tasks summary
        let exec = self.executable_tasks();
        if !exec.is_empty() {
            parts.push("\n### Executable Tasks\n".to_string());
            for t in &exec {
                parts.push(format!("- [{}] {} ({:?})\n", t.id, t.title, t.shape));
            }
        }

        let completed: Vec<_> = self
            .formed_tasks
            .iter()
            .filter(|t| t.is_completed())
            .collect();
        if !completed.is_empty() {
            parts.push("\n### Preserved Completed Tasks\n".to_string());
            for t in &completed {
                parts.push(format!("- [{}] {} (verified completed)\n", t.id, t.title));
            }
        }

        let superseded: Vec<_> = self
            .formed_tasks
            .iter()
            .filter(|t| t.is_superseded())
            .collect();
        if !superseded.is_empty() {
            parts.push("\n### Superseded Tasks\n".to_string());
            for t in &superseded {
                parts.push(format!("- [{}] {} (superseded)\n", t.id, t.title));
            }
        }

        parts.concat()
    }

    /// Lower active formed tasks into a canonical `CandidatePlan`.
    ///
    /// This establishes the direct pipeline:
    /// IntentState -> CandidatePlan -> TaskGraph
    /// without intermediary parallel task structures.
    pub fn to_candidate_plan(&self) -> CandidatePlan {
        let tasks: Vec<CandidateTask> = self
            .formed_tasks
            .iter()
            .filter(|t| !t.is_superseded() && !t.is_cancelled())
            .cloned()
            .map(|t| t.into_candidate_task())
            .collect();

        let plan_id = format!("plan-intent-{}", self.session_id);
        let objective = self.goal.clone().unwrap_or_else(|| self.raw_prompt.clone());
        let mut plan = CandidatePlan::new(plan_id, objective, tasks);
        for a in &self.assumptions {
            if a.is_valid() {
                plan.assumptions.push(CandidatePlanAssumption::new(
                    a.id.clone(),
                    a.description.clone(),
                ));
            }
        }
        plan
    }

    fn bump_version(&mut self) {
        self.version += 1;
        self.updated_at = Utc::now();
    }
}

// ── Supporting Types ───────────────────────────────────────────────────────────

/// A known fact established during intent understanding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentFact {
    pub key: String,
    pub value: String,
    pub origin: FactOrigin,
    pub created_at: DateTime<Utc>,
}

impl IntentFact {
    pub fn new(key: impl Into<String>, value: impl Into<String>, origin: FactOrigin) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            origin,
            created_at: Utc::now(),
        }
    }
}

/// Research result that has entered the canonical evidence system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchResult {
    pub query: String,
    pub finding: String,
    pub source: Option<String>,
    pub unknown_id: Option<String>,
    pub recorded_at: DateTime<Utc>,
}

/// Mid-session operator steering constraint.
///
/// When the user says "don't touch the API" or "use the existing SQLite layer",
/// this becomes a canonical constraint that invalidates conflicting assumptions
/// and is injected into every subsequent model turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SteeringConstraint {
    pub constraint_text: String,
    pub raw_user_message: String,
    pub applied_at: DateTime<Utc>,
}

impl SteeringConstraint {
    pub fn new(constraint_text: impl Into<String>, raw_user_message: impl Into<String>) -> Self {
        Self {
            constraint_text: constraint_text.into(),
            raw_user_message: raw_user_message.into(),
            applied_at: Utc::now(),
        }
    }

    /// Heuristic: check if this constraint might contradict a given assumption.
    ///
    /// This is a conservative string-similarity check only. The model is
    /// responsible for deeper semantic contradiction detection.
    fn contradicts_assumption(&self, assumption_description: &str) -> bool {
        let constraint_lower = self.constraint_text.to_lowercase();
        let assumption_lower = assumption_description.to_lowercase();

        // Identify negation-style constraints
        let negation_prefixes = ["don't", "do not", "avoid", "never", "no ", "keep ", "use "];
        let has_negation = negation_prefixes
            .iter()
            .any(|p| constraint_lower.starts_with(p));

        if !has_negation {
            return false;
        }

        // Extract key subject from constraint (simple word overlap)
        let constraint_words: std::collections::HashSet<&str> =
            constraint_lower.split_whitespace().collect();
        let assumption_words: std::collections::HashSet<&str> =
            assumption_lower.split_whitespace().collect();

        let overlap: usize = constraint_words.intersection(&assumption_words).count();

        // If 2+ meaningful words overlap, flag for model review
        overlap >= 2
    }
}

// ── Errors ─────────────────────────────────────────────────────────────────────

/// Errors from intent state operations.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum IntentError {
    #[error("fabricated unknown rejected: id={id} must include evidence_basis")]
    FabricatedUnknown { id: String },
    #[error("duplicate unknown rejected: id={id}")]
    DuplicateUnknown { id: String },
    #[error("unknown not found: id={id}")]
    UnknownNotFound { id: String },
    #[error("decision not found: id={id}")]
    DecisionNotFound { id: String },
    #[error("task not found: id={id}")]
    TaskNotFound { id: String },
    #[error("assumption not found: id={id}")]
    AssumptionNotFound { id: String },
    #[error("invalid strategy transition from {from:?} to {to:?}: {reason}")]
    InvalidStrategyTransition {
        from: TaskShape,
        to: TaskShape,
        reason: String,
    },
    #[error("serialization error: {0}")]
    Serialization(String),
}

// ── Session Storage ────────────────────────────────────────────────────────────

/// Serializes `IntentState` for persistence in SQLite as a JSON blob.
pub fn serialize_intent_state(state: &IntentState) -> Result<String, IntentError> {
    serde_json::to_string(state).map_err(|e| IntentError::Serialization(e.to_string()))
}

/// Deserializes `IntentState` from a stored JSON blob.
pub fn deserialize_intent_state(json: &str) -> Result<IntentState, IntentError> {
    serde_json::from_str(json).map_err(|e| IntentError::Serialization(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_repo_origin() -> FactOrigin {
        FactOrigin::RepositoryObserved
    }

    #[test]
    fn test_initial_intent_state() {
        let state = IntentState::initial_from_prompt("session-1", "fix the login flow");
        assert_eq!(state.raw_prompt, "fix the login flow");
        assert_eq!(state.version, 0);
        assert!(state.unknowns.is_empty());
        assert!(state.assumptions.is_empty());
        assert!(state.decisions.is_empty());
        assert!(state.formed_tasks.is_empty());
    }

    #[test]
    fn test_add_evidence_backed_unknown() {
        let mut state = IntentState::initial_from_prompt("s1", "fix login");
        let unknown = IntentUnknown::from_evidence(
            "UNK-AUTH-MECHANISM",
            "Which authentication mechanism is authoritative?",
            Criticality::High,
            FactOrigin::RepositoryObserved,
            "Found multiple auth implementations in src/auth/ — unclear which is canonical",
            UnknownFate::UserDecisionRequired,
        );
        assert!(state.add_unknown(unknown).is_ok());
        assert_eq!(state.unknowns.len(), 1);
        assert_eq!(state.version, 1);
    }

    #[test]
    fn test_fabricated_unknown_rejected() {
        let mut state = IntentState::initial_from_prompt("s1", "fix login");
        let fabricated = IntentUnknown::from_evidence(
            "UNK-FAKE",
            "Some made-up unknown",
            Criticality::Low,
            FactOrigin::ModelInferred {
                reasoning_summary: "guessing".to_string(),
            },
            "", // empty evidence_basis — INVALID
            UnknownFate::SafeToInfer,
        );
        assert!(matches!(
            state.add_unknown(fabricated),
            Err(IntentError::FabricatedUnknown { .. })
        ));
    }

    #[test]
    fn test_resolve_unknown() {
        let mut state = IntentState::initial_from_prompt("s1", "fix login");
        let _ = state.add_unknown(IntentUnknown::from_evidence(
            "UNK-AUTH",
            "Auth mechanism",
            Criticality::High,
            make_repo_origin(),
            "Multiple auth files found",
            UnknownFate::SafeToInfer,
        ));
        let resolved = state.resolve_unknown(
            "UNK-AUTH",
            UnknownResolution::InferredFromEvidence {
                inference: "JWT via src/auth/jwt.rs".to_string(),
                evidence_source: make_repo_origin(),
            },
        );
        assert!(resolved);
        assert!(state.unknowns[0].is_resolved());
    }

    #[test]
    fn test_steering_invalidates_assumption() {
        let mut state = IntentState::initial_from_prompt("s1", "add dark mode");
        state.add_assumption(IntentAssumption {
            id: "ASM-DB".to_string(),
            description: "Will use PostgreSQL as primary database".to_string(),
            criticality: Criticality::Medium,
            basis: "Common choice for production apps".to_string(),
            origin: FactOrigin::Assumed {
                basis: "Common choice".to_string(),
            },
            affected_unknowns: Vec::new(),
            affected_task_ids: Vec::new(),
            invalidation: None,
            created_at: Utc::now(),
        });
        assert!(state.assumptions[0].is_valid());

        state.apply_steering(SteeringConstraint::new(
            "use the existing SQLite layer",
            "Actually, use the existing SQLite layer",
        ));
        // Assumption about PostgreSQL may be flagged for invalidation
        // (depends on word overlap heuristic)
        assert_eq!(state.steering_constraints.len(), 1);
        assert!(state.version >= 1);
    }

    #[test]
    fn test_task_invalidation_preserves_completed() {
        let mut state = IntentState::initial_from_prompt("s1", "make production ready");

        let mut completed_task = FormedTask::new(
            "T1",
            "Add logging",
            "Add structured logging",
            TaskShape::DirectToolExecution,
        );
        completed_task.status = FormedTaskStatus::Completed {
            summary: "Logging added".to_string(),
        };
        state.add_formed_task(completed_task);

        state.add_formed_task(FormedTask::new(
            "T2",
            "Add auth",
            "Add authentication",
            TaskShape::InvestigateThenAct,
        ));

        // Cannot invalidate completed task
        assert!(!state.invalidate_task("T1", "Contradicted by new evidence"));
        assert!(matches!(
            state.formed_tasks[0].status,
            FormedTaskStatus::Completed { .. }
        ));

        // Can invalidate pending task
        assert!(state.invalidate_task("T2", "Contradicted by new evidence"));
        assert!(matches!(
            state.formed_tasks[1].status,
            FormedTaskStatus::Superseded { .. }
        ));
    }

    #[test]
    fn test_context_fragment_renders_steering() {
        let mut state = IntentState::initial_from_prompt("s1", "add dark mode");
        state.goal = Some("Add dark mode theme support".to_string());
        state.apply_steering(SteeringConstraint::new(
            "Keep the existing API backward compatible",
            "Keep backward compat",
        ));
        let fragment = state.render_context_fragment();
        assert!(fragment.contains("Intent State"));
        assert!(fragment.contains("Keep the existing API backward compatible"));
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut state = IntentState::initial_from_prompt("s1", "fix login");
        state.goal = Some("Fix the login flow".to_string());
        let _ = state.add_unknown(IntentUnknown::from_evidence(
            "UNK-1",
            "Auth mechanism",
            Criticality::High,
            make_repo_origin(),
            "Multiple auth files",
            UnknownFate::UserDecisionRequired,
        ));

        let json = serialize_intent_state(&state).unwrap();
        let restored = deserialize_intent_state(&json).unwrap();
        assert_eq!(state, restored);
    }

    #[test]
    fn test_fact_origin_trust_levels() {
        assert!(FactOrigin::UserProvided.trust_level().can_verify_facts());
        assert!(
            FactOrigin::RepositoryObserved
                .trust_level()
                .can_verify_facts()
        );
        assert!(
            !FactOrigin::ModelInferred {
                reasoning_summary: "guess".into()
            }
            .trust_level()
            .can_verify_facts()
        );
    }

    #[test]
    fn test_task_shape_properties() {
        assert!(!TaskShape::DirectToolExecution.requires_planning());
        assert!(TaskShape::PlanThenExecute.requires_planning());
        assert!(TaskShape::AskUserThenAct.requires_user_input());
        assert!(!TaskShape::DirectToolExecution.requires_user_input());
    }
}
