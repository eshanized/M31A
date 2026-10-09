//! Human-governed plan review, task review, and execution authorization lifecycle.
//!
//! Enforces: "The model proposes. The runtime decides."
//!
//! Key responsibilities:
//! - First-class Plan Review: create session, display (MD/JSON), manual edit, model revision, accept, reject.
//! - Plan Versioning: immutable history, provenance, author attribution.
//! - First-class Task Review: lowering from accepted plan, editing, adding, removing (with dependency validation), regenerating, accepting.
//! - Task Graph Validation: schema, duplicate IDs, missing prerequisites, cycle detection, capability validation.
//! - Execution Authorization: explicit operator authorization record bound to exact plan/task revision; automatic invalidation on upstream change.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

use crate::agent::intent::{FactOrigin, IntentState, IntentUnknown};
use crate::agent::intent_repository::SqliteIntentRepository;
use crate::events::bus::{BroadcastEventBus, EventBus};
use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::interaction::action::ApplicationAction;
use crate::kernel::plan::{CandidatePlan, CandidateTask, CandidateTaskKey};
use crate::persistence::sqlite::repositories::lifecycle::{
    PersistedLifecycleState, SqliteLifecycleRepository,
};
use crate::planning::risks::{Criticality, UnknownFate};
use crate::planning::validation::PlanValidator;
use crate::state_machine::lifecycle::{LifecycleEvent, LifecycleStage, transition_lifecycle};
use crate::workflow::genesis::discovery::{
    DynamicQuestion, apply_question_answer, validate_dynamic_question,
};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;

use crate::agent::model_policy::{ModelCaller, ModelProposal};
use crate::kernel::seams::context::ContextCompiler;
use crate::kernel::seams::planner::{PlanRequest, PlanService, UpstreamPlanContext};
use crate::planning::service::PlanServiceImpl;
use crate::prompt::PromptCatalog;

// ── Authorship & Status ────────────────────────────────────────────────────────

/// Author entity that produced a plan or task revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionAuthorType {
    Model,
    User,
    Runtime,
}

impl std::fmt::Display for RevisionAuthorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Model => write!(f, "model"),
            Self::User => write!(f, "user"),
            Self::Runtime => write!(f, "runtime"),
        }
    }
}

/// Status of an individual plan revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanReviewStatus {
    Draft,
    InReview,
    Accepted,
    Rejected,
    Superseded,
}

/// Status of an individual task set revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskReviewStatus {
    Draft,
    InReview,
    Accepted,
    Rejected,
    Superseded,
}

// ── Plan Revision ──────────────────────────────────────────────────────────────

/// An immutable, versioned plan proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanRevision {
    pub id: Uuid,
    pub session_id: String,
    pub revision: u32,
    pub plan_id: String,
    pub content: CandidatePlan,
    pub created_by: String,
    pub author_type: RevisionAuthorType,
    pub supersedes_revision: Option<u32>,
    pub status: PlanReviewStatus,
    pub created_at: DateTime<Utc>,
}

impl PlanRevision {
    pub fn new(
        session_id: impl Into<String>,
        revision: u32,
        plan_id: impl Into<String>,
        content: CandidatePlan,
        created_by: impl Into<String>,
        author_type: RevisionAuthorType,
        supersedes_revision: Option<u32>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            session_id: session_id.into(),
            revision,
            plan_id: plan_id.into(),
            content,
            created_by: created_by.into(),
            author_type,
            supersedes_revision,
            status: PlanReviewStatus::Draft,
            created_at: Utc::now(),
        }
    }

    /// Render machine-readable canonical representation (plan.json).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.content)
    }

    /// Compute canonical SHA-256 hash of a candidate plan content for revision integrity.
    pub fn compute_content_hash(content: &CandidatePlan) -> String {
        let serialized = serde_json::to_vec(content).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        format!("{:x}", hasher.finalize())
    }

    /// Compute canonical SHA-256 hash of this revision's plan content.
    pub fn content_hash(&self) -> String {
        Self::compute_content_hash(&self.content)
    }

    /// Render human-readable Markdown projection (PLAN.md).
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!(
            "# Plan: {} (Revision {})\n\n",
            self.content.objective, self.revision
        ));
        md.push_str(&format!(
            "> Author: {} ({}) | Status: {:?} | Created: {}\n\n",
            self.created_by, self.author_type, self.status, self.created_at
        ));

        if !self.content.assumptions.is_empty() {
            md.push_str("## Assumptions\n\n");
            for a in &self.content.assumptions {
                let mark = if a.invalidated { "~~" } else { "" };
                md.push_str(&format!("- {}{}: {}{}\n", mark, a.id, a.description, mark));
            }
            md.push('\n');
        }

        md.push_str("## Proposed Tasks\n\n");
        for (i, t) in self.content.tasks.iter().enumerate() {
            md.push_str(&format!("### {}. {} (`{}`)\n", i + 1, t.objective, t.id));
            if let Some(ref d) = t.description {
                md.push_str(&format!("{}\n\n", d));
            }
            if !t.depends_on.is_empty() {
                let deps: Vec<String> = t.depends_on.iter().map(|d| d.to_string()).collect();
                md.push_str(&format!("- **Depends on**: {}\n", deps.join(", ")));
            }
            md.push_str(&format!("- **Role**: {}\n", t.role.as_str()));
            md.push_str(&format!("- **Verification**: {:?}\n", t.verification));
            if !t.completion_criteria.is_empty() {
                md.push_str(&format!(
                    "- **Completion criteria**: {}\n",
                    t.completion_criteria.join("; ")
                ));
            }
            md.push('\n');
        }

        md
    }
}

// ── Task Revision ──────────────────────────────────────────────────────────────

/// An immutable, versioned executable task set bound to an accepted plan revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRevision {
    pub id: Uuid,
    pub session_id: String,
    pub revision: u32,
    pub plan_revision: u32,
    pub tasks: Vec<CandidateTask>,
    pub created_by: String,
    pub author_type: RevisionAuthorType,
    pub supersedes_revision: Option<u32>,
    pub status: TaskReviewStatus,
    pub created_at: DateTime<Utc>,
}

impl TaskRevision {
    pub fn new(
        session_id: impl Into<String>,
        revision: u32,
        plan_revision: u32,
        tasks: Vec<CandidateTask>,
        created_by: impl Into<String>,
        author_type: RevisionAuthorType,
        supersedes_revision: Option<u32>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            session_id: session_id.into(),
            revision,
            plan_revision,
            tasks,
            created_by: created_by.into(),
            author_type,
            supersedes_revision,
            status: TaskReviewStatus::Draft,
            created_at: Utc::now(),
        }
    }

    /// Render machine-readable canonical representation (tasks.json).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.tasks)
    }

    /// Compute canonical SHA-256 hash of a candidate task set for revision integrity.
    pub fn compute_tasks_hash(tasks: &[CandidateTask]) -> String {
        let serialized = serde_json::to_vec(tasks).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&serialized);
        format!("{:x}", hasher.finalize())
    }

    /// Compute canonical SHA-256 hash of this revision's task set.
    pub fn content_hash(&self) -> String {
        Self::compute_tasks_hash(&self.tasks)
    }

    /// Render human-readable Markdown projection (TASKS.md).
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!(
            "# Task Revision {} (Plan Revision {})\n\n",
            self.revision, self.plan_revision
        ));
        md.push_str(&format!(
            "> Author: {} ({}) | Status: {:?} | Created: {}\n\n",
            self.created_by, self.author_type, self.status, self.created_at
        ));
        md.push_str("## Proposed Tasks\n\n");
        for (i, t) in self.tasks.iter().enumerate() {
            md.push_str(&format!("### {}. {} (`{}`)\n", i + 1, t.objective, t.id));
            if let Some(ref d) = t.description {
                md.push_str(&format!("{}\n\n", d));
            }
            if !t.depends_on.is_empty() {
                let deps: Vec<String> = t.depends_on.iter().map(|d| d.to_string()).collect();
                md.push_str(&format!("- **Depends on**: {}\n", deps.join(", ")));
            }
            md.push_str(&format!("- **Role**: {}\n", t.role.as_str()));
            md.push_str(&format!("- **Verification**: {:?}\n", t.verification));
            if !t.completion_criteria.is_empty() {
                md.push_str(&format!(
                    "- **Completion criteria**: {}\n",
                    t.completion_criteria.join("; ")
                ));
            }
            md.push('\n');
        }
        md
    }
}

// ── Execution Authorization ───────────────────────────────────────────────────

/// Explicit operator launch authorization decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationDecision {
    Authorized,
    Rejected,
    Invalidated,
}

/// Cryptographically distinct launch authorization bound to specific plan/task revisions
/// and exact serialized content hashes (INVARIANT C).
///
/// The authorization binds the FULL execution surface: plan/task revisions
/// AND content hashes, the policy generation that approved it, the workspace
/// it may mutate, the role and autonomy mode it executes as, and an expiry.
/// Any drift fails closed at final execution (no stale-plan/authorization
/// reuse across graph mutation, policy change, workspace, role, mode, or
/// target-resource change).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionAuthorization {
    pub id: Uuid,
    pub session_id: String,
    pub plan_revision: u32,
    pub task_revision: u32,
    pub decision: AuthorizationDecision,
    pub authorized_by: String,
    pub authorized_at: DateTime<Utc>,
    pub invalidation_reason: Option<String>,
    pub plan_content_hash: Option<String>,
    pub task_content_hash: Option<String>,
    /// Policy generation hash active when authorized. `None` predates binding
    /// and never verifies (fail closed).
    pub policy_hash: Option<String>,
    /// Workspace root this authorization may mutate. Must equal the
    /// execution workspace at final verification.
    pub workspace_root: Option<String>,
    /// Agent role this authorization executes as.
    pub agent_role: Option<String>,
    /// Autonomy mode this authorization executes under.
    pub autonomy_mode: Option<String>,
    /// Expiry. `None` predates binding and never verifies (fail closed).
    pub expires_at: Option<DateTime<Utc>>,
}

impl ExecutionAuthorization {
    pub fn new(
        session_id: impl Into<String>,
        plan_revision: u32,
        task_revision: u32,
        authorized_by: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            session_id: session_id.into(),
            plan_revision,
            task_revision,
            decision: AuthorizationDecision::Authorized,
            authorized_by: authorized_by.into(),
            authorized_at: Utc::now(),
            invalidation_reason: None,
            plan_content_hash: None,
            task_content_hash: None,
            policy_hash: None,
            workspace_root: None,
            agent_role: None,
            autonomy_mode: None,
            expires_at: None,
        }
    }

    /// Bind the execution surface: policy generation, workspace, role, mode,
    /// and a time-to-live. Authorizations without a bound surface never
    /// verify at final execution.
    pub fn with_execution_binding(
        mut self,
        policy_hash: impl Into<String>,
        workspace_root: impl Into<String>,
        agent_role: impl Into<String>,
        autonomy_mode: impl Into<String>,
        ttl: std::time::Duration,
    ) -> Self {
        self.policy_hash = Some(policy_hash.into());
        self.workspace_root = Some(workspace_root.into());
        self.agent_role = Some(agent_role.into());
        self.autonomy_mode = Some(autonomy_mode.into());
        self.expires_at = Some(
            Utc::now() + chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::hours(1)),
        );
        self
    }

    /// Bind cryptographic SHA-256 content hashes of the authorized plan and tasks.
    pub fn with_content_hashes(
        mut self,
        plan_hash: impl Into<String>,
        task_hash: impl Into<String>,
    ) -> Self {
        self.plan_content_hash = Some(plan_hash.into());
        self.task_content_hash = Some(task_hash.into());
        self
    }

    /// Check if this authorization is valid for the current plan and task revisions.
    pub fn is_valid_for(&self, current_plan_rev: u32, current_task_rev: u32) -> bool {
        self.decision == AuthorizationDecision::Authorized
            && self.plan_revision == current_plan_rev
            && self.task_revision == current_task_rev
            && self.invalidation_reason.is_none()
    }

    /// Check if this authorization is valid for exact revisions and exact content hashes.
    pub fn is_valid_for_exact(
        &self,
        current_plan_rev: u32,
        current_task_rev: u32,
        actual_plan_hash: Option<&str>,
        actual_task_hash: Option<&str>,
    ) -> bool {
        if !self.is_valid_for(current_plan_rev, current_task_rev) {
            return false;
        }
        if let (Some(expected), Some(actual)) = (&self.plan_content_hash, actual_plan_hash) {
            if expected != actual {
                return false;
            }
        }
        if let (Some(expected), Some(actual)) = (&self.task_content_hash, actual_task_hash) {
            if expected != actual {
                return false;
            }
        }
        true
    }

    /// Invalidate this authorization record due to upstream modifications.
    pub fn invalidate(&mut self, reason: impl Into<String>) {
        self.decision = AuthorizationDecision::Invalidated;
        self.invalidation_reason = Some(reason.into());
    }
}

// ── Task Validation Errors ────────────────────────────────────────────────────

/// Validation errors for candidate task graphs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum TaskGraphValidationError {
    #[error("Task graph contains no tasks")]
    EmptyTaskGraph,

    #[error("Duplicate task ID '{id}' detected")]
    DuplicateTaskId { id: String },

    #[error("Task '{id}' has self-dependency")]
    SelfDependency { id: String },

    #[error("Task '{id}' depends on missing task '{missing_dep}'")]
    MissingDependency { id: String, missing_dep: String },

    #[error("Cycle detected in task graph: {cycle:?}")]
    CycleDetected { cycle: Vec<String> },

    #[error("Task '{id}' references empty or invalid capability '{capability}'")]
    InvalidCapability { id: String, capability: String },

    #[error("Cannot remove task '{id}' because other tasks depend on it: {dependents:?}")]
    DependentTasksBlockRemoval { id: String, dependents: Vec<String> },

    #[error("Plan validation failed: {0}")]
    PlanValidation(String),
}

// ── Task Graph Validation Engine ──────────────────────────────────────────────

/// Validates a candidate task list against graph, dependency, and capability invariants.
pub fn validate_candidate_tasks(tasks: &[CandidateTask]) -> Result<(), TaskGraphValidationError> {
    if tasks.is_empty() {
        return Err(TaskGraphValidationError::EmptyTaskGraph);
    }

    let mut ids = HashSet::new();
    for t in tasks {
        let id_str = t.id.to_string();
        if id_str.trim().is_empty() {
            return Err(TaskGraphValidationError::PlanValidation(
                "Task ID cannot be empty".to_string(),
            ));
        }
        if !ids.insert(id_str.clone()) {
            return Err(TaskGraphValidationError::DuplicateTaskId { id: id_str });
        }
    }

    // Check dependencies existence & self-dependency
    for t in tasks {
        let t_id = t.id.to_string();
        for dep in &t.depends_on {
            let dep_str = dep.to_string();
            if dep_str == t_id {
                return Err(TaskGraphValidationError::SelfDependency { id: t_id });
            }
            if !ids.contains(&dep_str) {
                return Err(TaskGraphValidationError::MissingDependency {
                    id: t_id,
                    missing_dep: dep_str,
                });
            }
        }

        // Validate capabilities
        for cap in &t.capabilities {
            if let Err(e) = cap.validate() {
                return Err(TaskGraphValidationError::InvalidCapability {
                    id: t_id.clone(),
                    capability: e,
                });
            }
        }
    }

    // Cycle detection via DFS
    let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for t in tasks {
        adj.entry(t.id.as_str())
            .or_default()
            .extend(t.depends_on.iter().map(|d| d.as_str()));
    }

    let mut visited: HashSet<&str> = HashSet::new();
    let mut rec_stack: HashSet<&str> = HashSet::new();
    let mut path: Vec<String> = Vec::new();

    for &node in adj.keys() {
        if !visited.contains(node)
            && dfs_has_cycle(node, &adj, &mut visited, &mut rec_stack, &mut path)
        {
            return Err(TaskGraphValidationError::CycleDetected { cycle: path });
        }
    }

    Ok(())
}

fn dfs_has_cycle<'a>(
    node: &'a str,
    adj: &BTreeMap<&'a str, Vec<&'a str>>,
    visited: &mut HashSet<&'a str>,
    rec_stack: &mut HashSet<&'a str>,
    path: &mut Vec<String>,
) -> bool {
    visited.insert(node);
    rec_stack.insert(node);
    path.push(node.to_string());

    if let Some(neighbors) = adj.get(node) {
        for &neighbor in neighbors {
            if !visited.contains(neighbor) {
                if dfs_has_cycle(neighbor, adj, visited, rec_stack, path) {
                    return true;
                }
            } else if rec_stack.contains(neighbor) {
                path.push(neighbor.to_string());
                return true;
            }
        }
    }

    rec_stack.remove(node);
    path.pop();
    false
}

// ── Plan Review Session ────────────────────────────────────────────────────────

/// First-class domain abstraction managing plan review lifecycle.
#[derive(Debug, Clone)]
pub struct PlanReviewSession {
    pub session_id: String,
    pub revisions: Vec<PlanRevision>,
    pub current_revision: u32,
    pub stage: LifecycleStage,
    pub validator: PlanValidator,
}

impl PlanReviewSession {
    /// Initialize a new review session from a candidate plan draft.
    pub async fn new(
        session_id: impl Into<String>,
        initial_plan: CandidatePlan,
        author: impl Into<String>,
        author_type: RevisionAuthorType,
    ) -> Result<Self, String> {
        let sid = session_id.into();
        let validator = PlanValidator::new();

        // Validate initial draft
        let report = validator.validate(&initial_plan).await;
        if !report.is_valid() {
            let errs: Vec<String> = report.errors.iter().map(|e| format!("{:?}", e)).collect();
            return Err(format!(
                "Initial plan fails validation: {}",
                errs.join(", ")
            ));
        }

        let plan_id = initial_plan.plan_id.clone();
        let rev1 = PlanRevision::new(&sid, 1, plan_id, initial_plan, author, author_type, None);

        let stage =
            transition_lifecycle(LifecycleStage::PlanDraft, LifecycleEvent::EnterPlanReview)
                .map_err(|e| e.to_string())?;

        Ok(Self {
            session_id: sid,
            revisions: vec![rev1],
            current_revision: 1,
            stage,
            validator,
        })
    }

    /// Access current active plan revision.
    pub fn current_plan_revision(&self) -> &PlanRevision {
        self.revisions
            .iter()
            .find(|r| r.revision == self.current_revision)
            .expect("Current revision must exist in revisions history")
    }

    /// Ingest a manual edit proposed by the user (untrusted input).
    ///
    /// Validates strictly with `PlanValidator`. If valid, increments revision
    /// and transitions through PlanRevision -> PlanReview. If invalid, preserves
    /// current valid plan and returns clear errors without silent repairs.
    pub async fn ingest_manual_edit(
        &mut self,
        edited_plan_json: &str,
        operator: impl Into<String>,
    ) -> Result<&PlanRevision, String> {
        let parsed_plan: CandidatePlan = serde_json::from_str(edited_plan_json)
            .map_err(|e| format!("Invalid plan JSON schema: {e}"))?;

        let report = self.validator.validate(&parsed_plan).await;
        if !report.is_valid() {
            let errs: Vec<String> = report.errors.iter().map(|e| format!("{:?}", e)).collect();
            return Err(format!(
                "Plan edit rejected by validation: {}",
                errs.join("; ")
            ));
        }

        // Transition through revision state
        self.stage = transition_lifecycle(self.stage, LifecycleEvent::PlanEditSubmitted)
            .map_err(|e| e.to_string())?;

        let new_rev = self.current_revision + 1;
        let mut new_plan = parsed_plan;
        new_plan.revision = new_rev;
        let plan_id = new_plan.plan_id.clone();

        let plan_rev = PlanRevision::new(
            &self.session_id,
            new_rev,
            &plan_id,
            new_plan,
            operator,
            RevisionAuthorType::User,
            Some(self.current_revision),
        );

        // Mark previous revision as superseded
        if let Some(prev) = self
            .revisions
            .iter_mut()
            .find(|r| r.revision == self.current_revision)
        {
            prev.status = PlanReviewStatus::Superseded;
        }

        self.revisions.push(plan_rev);
        self.current_revision = new_rev;

        self.stage = transition_lifecycle(self.stage, LifecycleEvent::PlanRevisionValidated)
            .map_err(|e| e.to_string())?;

        Ok(self.current_plan_revision())
    }

    /// Apply a model-proposed revision with user-requested change feedback.
    pub async fn apply_model_revision(
        &mut self,
        model_proposed_plan: CandidatePlan,
        model_name: impl Into<String>,
    ) -> Result<&PlanRevision, String> {
        let report = self.validator.validate(&model_proposed_plan).await;
        if !report.is_valid() {
            let errs: Vec<String> = report.errors.iter().map(|e| format!("{:?}", e)).collect();
            return Err(format!("Model plan proposal rejected: {}", errs.join("; ")));
        }

        self.stage = transition_lifecycle(self.stage, LifecycleEvent::PlanRevisionRequested)
            .map_err(|e| e.to_string())?;

        let new_rev = self.current_revision + 1;
        let mut new_plan = model_proposed_plan;
        new_plan.revision = new_rev;
        let plan_id = new_plan.plan_id.clone();

        let plan_rev = PlanRevision::new(
            &self.session_id,
            new_rev,
            &plan_id,
            new_plan,
            model_name,
            RevisionAuthorType::Model,
            Some(self.current_revision),
        );

        if let Some(prev) = self
            .revisions
            .iter_mut()
            .find(|r| r.revision == self.current_revision)
        {
            prev.status = PlanReviewStatus::Superseded;
        }

        self.revisions.push(plan_rev);
        self.current_revision = new_rev;

        self.stage = transition_lifecycle(self.stage, LifecycleEvent::PlanRevisionValidated)
            .map_err(|e| e.to_string())?;

        Ok(self.current_plan_revision())
    }

    /// Explicitly accept the current plan revision.
    pub fn accept_plan(&mut self) -> Result<(), String> {
        self.stage = transition_lifecycle(self.stage, LifecycleEvent::PlanAccepted)
            .map_err(|e| e.to_string())?;

        if let Some(cur) = self
            .revisions
            .iter_mut()
            .find(|r| r.revision == self.current_revision)
        {
            cur.status = PlanReviewStatus::Accepted;
        }

        Ok(())
    }

    /// Explicitly reject the current plan revision.
    pub fn reject_plan(&mut self) -> Result<(), String> {
        self.stage = transition_lifecycle(self.stage, LifecycleEvent::PlanRejected)
            .map_err(|e| e.to_string())?;

        if let Some(cur) = self
            .revisions
            .iter_mut()
            .find(|r| r.revision == self.current_revision)
        {
            cur.status = PlanReviewStatus::Rejected;
        }

        Ok(())
    }
}

// ── Task Review Session ────────────────────────────────────────────────────────

/// First-class domain abstraction managing task review lifecycle.
#[derive(Debug, Clone)]
pub struct TaskReviewSession {
    pub session_id: String,
    pub plan_revision: u32,
    pub revisions: Vec<TaskRevision>,
    pub current_revision: u32,
    pub stage: LifecycleStage,
}

impl TaskReviewSession {
    /// Initialize task review from accepted plan revision.
    pub fn from_accepted_plan(
        session_id: impl Into<String>,
        plan: &PlanRevision,
        creator: impl Into<String>,
    ) -> Result<Self, String> {
        if plan.status != PlanReviewStatus::Accepted {
            return Err(
                "Cannot generate executable tasks before plan is explicitly accepted".to_string(),
            );
        }

        let tasks = plan.content.tasks.clone();
        validate_candidate_tasks(&tasks).map_err(|e| e.to_string())?;

        let sid = session_id.into();
        let rev1 = TaskRevision::new(
            &sid,
            1,
            plan.revision,
            tasks,
            creator,
            RevisionAuthorType::Runtime,
            None,
        );

        let stage =
            transition_lifecycle(LifecycleStage::PlanAccepted, LifecycleEvent::TasksDrafted)
                .and_then(|s| transition_lifecycle(s, LifecycleEvent::EnterTasksReview))
                .map_err(|e| e.to_string())?;

        Ok(Self {
            session_id: sid,
            plan_revision: plan.revision,
            revisions: vec![rev1],
            current_revision: 1,
            stage,
        })
    }

    pub fn current_tasks(&self) -> &[CandidateTask] {
        &self.current_task_revision().tasks
    }

    pub fn current_task_revision(&self) -> &TaskRevision {
        self.revisions
            .iter()
            .find(|r| r.revision == self.current_revision)
            .expect("Current task revision must exist")
    }

    /// Edit an existing task in the candidate task set.
    pub fn edit_task(
        &mut self,
        updated_task: CandidateTask,
        operator: impl Into<String>,
    ) -> Result<&TaskRevision, TaskGraphValidationError> {
        let mut new_tasks = self.current_tasks().to_vec();
        let idx = new_tasks
            .iter()
            .position(|t| t.id == updated_task.id)
            .ok_or_else(|| TaskGraphValidationError::MissingDependency {
                id: updated_task.id.to_string(),
                missing_dep: "Task to edit does not exist".to_string(),
            })?;

        new_tasks[idx] = updated_task;
        validate_candidate_tasks(&new_tasks)?;

        self.apply_new_task_revision(new_tasks, operator, RevisionAuthorType::User)
    }

    /// Add a new task to the candidate task set.
    pub fn add_task(
        &mut self,
        task: CandidateTask,
        operator: impl Into<String>,
        author_type: RevisionAuthorType,
    ) -> Result<&TaskRevision, TaskGraphValidationError> {
        let mut new_tasks = self.current_tasks().to_vec();
        new_tasks.push(task);
        validate_candidate_tasks(&new_tasks)?;

        self.apply_new_task_revision(new_tasks, operator, author_type)
    }

    /// Remove a task, verifying that no dependent tasks are broken.
    pub fn remove_task(
        &mut self,
        task_id: &CandidateTaskKey,
        operator: impl Into<String>,
    ) -> Result<&TaskRevision, TaskGraphValidationError> {
        let task_id_str = task_id.to_string();

        // Check if any task depends on this task
        let dependents: Vec<String> = self
            .current_tasks()
            .iter()
            .filter(|t| t.depends_on.iter().any(|d| d == task_id))
            .map(|t| t.id.to_string())
            .collect();

        if !dependents.is_empty() {
            return Err(TaskGraphValidationError::DependentTasksBlockRemoval {
                id: task_id_str,
                dependents,
            });
        }

        let new_tasks: Vec<CandidateTask> = self
            .current_tasks()
            .iter()
            .filter(|t| &t.id != task_id)
            .cloned()
            .collect();

        validate_candidate_tasks(&new_tasks)?;

        self.apply_new_task_revision(new_tasks, operator, RevisionAuthorType::User)
    }

    /// Ingest a completely regenerated task set.
    pub fn regenerate_tasks(
        &mut self,
        regenerated_tasks: Vec<CandidateTask>,
        author: impl Into<String>,
        author_type: RevisionAuthorType,
    ) -> Result<&TaskRevision, TaskGraphValidationError> {
        validate_candidate_tasks(&regenerated_tasks)?;
        self.apply_new_task_revision(regenerated_tasks, author, author_type)
    }

    fn apply_new_task_revision(
        &mut self,
        new_tasks: Vec<CandidateTask>,
        author: impl Into<String>,
        author_type: RevisionAuthorType,
    ) -> Result<&TaskRevision, TaskGraphValidationError> {
        self.stage = transition_lifecycle(self.stage, LifecycleEvent::TaskEditSubmitted)
            .map_err(|e| TaskGraphValidationError::PlanValidation(e.to_string()))?;

        let new_rev = self.current_revision + 1;
        let task_rev = TaskRevision::new(
            &self.session_id,
            new_rev,
            self.plan_revision,
            new_tasks,
            author,
            author_type,
            Some(self.current_revision),
        );

        if let Some(prev) = self
            .revisions
            .iter_mut()
            .find(|r| r.revision == self.current_revision)
        {
            prev.status = TaskReviewStatus::Superseded;
        }

        self.revisions.push(task_rev);
        self.current_revision = new_rev;

        self.stage = transition_lifecycle(self.stage, LifecycleEvent::TaskRevisionValidated)
            .map_err(|e| TaskGraphValidationError::PlanValidation(e.to_string()))?;

        Ok(self.current_task_revision())
    }

    /// Explicitly accept the current task set.
    pub fn accept_tasks(&mut self) -> Result<(), String> {
        self.stage = transition_lifecycle(self.stage, LifecycleEvent::TasksAccepted)
            .map_err(|e| e.to_string())?;

        if let Some(cur) = self
            .revisions
            .iter_mut()
            .find(|r| r.revision == self.current_revision)
        {
            cur.status = TaskReviewStatus::Accepted;
        }

        Ok(())
    }
}

// ── Pre-Execution Lifecycle Coordinator ────────────────────────────────────────

/// Typed lifecycle response guiding interaction surfaces (CLI/TUI) without state mutation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PreExecutionResponse {
    /// Dynamic questions required from operator before plan can be drafted.
    QuestionsRequired {
        session_id: String,
        questions: Vec<DynamicQuestion>,
    },
    /// Candidate plan proposal ready for operator review.
    PlanForReview {
        session_id: String,
        revision: PlanRevision,
    },
    /// Candidate task set ready for operator review.
    TasksForReview {
        session_id: String,
        revision: TaskRevision,
    },
    /// Plan and tasks accepted; execution launch authorization requested.
    AuthorizationRequested {
        session_id: String,
        plan_revision: u32,
        task_revision: u32,
        message: String,
    },
    /// Operator explicitly authorized workspace execution.
    ReadyToExecute {
        session_id: String,
        plan: CandidatePlan,
        tasks: Vec<CandidateTask>,
        authorization: ExecutionAuthorization,
    },
    /// Operation was terminated (cancelled or rejected).
    Terminated {
        session_id: String,
        stage: LifecycleStage,
        reason: String,
    },
}

/// Deterministic prompt signal detector.
///
/// Uses keyword matching to detect the presence or absence of architectural,
/// scope, and deliverable signals in a prompt. This is NOT semantic reasoning
/// and NOT model inference — it is a deterministic Rust heuristic.
///
/// Results are attributed as `FactOrigin::RuntimeInferred`, never `ModelInferred`.
/// Unknown detection based on keyword absence should be treated as an initial
/// signal requiring clarification, not as authoritative semantic understanding.
struct PromptSemanticAnalyzer;

impl PromptSemanticAnalyzer {
    /// Analyze prompt for semantic completeness signals.
    /// Returns (has_architecture_signal, has_scope_signal, has_deliverable_signal)
    fn analyze(prompt: &str) -> (bool, bool, bool) {
        let lower = prompt.to_lowercase();

        let arch_keywords = [
            "react",
            "vue",
            "angular",
            "django",
            "flask",
            "express",
            "spring",
            "rails",
            "next",
            "nuxt",
            "svelte",
            "rust",
            "python",
            "node",
            "typescript",
            "java",
            "go",
            "api",
            "rest",
            "graphql",
            "grpc",
            "database",
            "postgres",
            "mysql",
            "mongodb",
            "sqlite",
            "redis",
            "docker",
            "kubernetes",
            "aws",
            "gcp",
            "azure",
            "serverless",
            "microservice",
            "monolith",
            "frontend",
            "backend",
            "fullstack",
            "mobile",
            "desktop",
            "cli",
            "supabase",
            "firebase",
            "cluster",
            "container",
            "gateway",
            "ingress",
            "webhook",
            "stripe",
            "service",
            "architecture",
        ];
        let has_arch = arch_keywords.iter().any(|k| lower.contains(k));

        let scope_markers = [
            "only",
            "just",
            "limit",
            "scope",
            "within",
            "exclude",
            "include",
            "between",
            "from",
            "to",
            "should not",
            "must not",
            "boundary",
            "constraint",
        ];
        let has_scope = scope_markers.iter().any(|k| lower.contains(k));

        let deliverable_markers = [
            "create",
            "build",
            "implement",
            "add",
            "fix",
            "refactor",
            "update",
            "delete",
            "remove",
            "migrate",
            "deploy",
            "test",
            "endpoint",
            "page",
            "component",
            "service",
            "module",
            "feature",
            "function",
            "class",
            "file",
        ];
        let has_deliverable = deliverable_markers.iter().any(|k| lower.contains(k));

        (has_arch, has_scope, has_deliverable)
    }
}

fn extract_json_from_text(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if let Some(start) = trimmed.find("```json") {
        let after = &trimmed[start + 7..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].trim());
        }
    } else if let Some(start) = trimmed.find("```") {
        let after = &trimmed[start + 3..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].trim());
        }
    }
    let first_brace = trimmed.find('{');
    let last_brace = trimmed.rfind('}');
    let first_bracket = trimmed.find('[');
    let last_bracket = trimmed.rfind(']');

    match (first_brace, last_brace, first_bracket, last_bracket) {
        (Some(b1), Some(b2), Some(k1), Some(k2)) if k1 < b1 && k2 > b2 => {
            return Some(&trimmed[k1..=k2]);
        }
        (Some(b1), Some(b2), _, _) if b1 < b2 => {
            return Some(&trimmed[b1..=b2]);
        }
        (_, _, Some(k1), Some(k2)) if k1 < k2 => {
            return Some(&trimmed[k1..=k2]);
        }
        _ => {}
    }
    Some(trimmed)
}

fn proposal_to_text(proposal: &ModelProposal) -> Option<String> {
    match proposal {
        ModelProposal::Complete { summary, .. } => Some(summary.clone()),
        ModelProposal::AssistantText { content } => Some(content.clone()),
        ModelProposal::ToolCalls { calls } => calls.first().map(|c| c.arguments.to_string()),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
struct DynamicQuestionsWrapper {
    #[serde(default)]
    questions: Vec<DynamicQuestion>,
}

fn parse_dynamic_questions_proposal(proposal: &ModelProposal) -> Vec<DynamicQuestion> {
    let Some(text) = proposal_to_text(proposal) else {
        return Vec::new();
    };
    let Some(json_str) = extract_json_from_text(&text) else {
        return Vec::new();
    };
    if let Ok(questions) = serde_json::from_str::<Vec<DynamicQuestion>>(json_str) {
        return questions;
    }
    if let Ok(wrapper) = serde_json::from_str::<DynamicQuestionsWrapper>(json_str) {
        return wrapper.questions;
    }
    if let Ok(single) = serde_json::from_str::<DynamicQuestion>(json_str) {
        return vec![single];
    }
    Vec::new()
}

fn parse_plan_proposal(
    proposal: &ModelProposal,
    default_objective: &str,
) -> Result<CandidatePlan, String> {
    let text = proposal_to_text(proposal).ok_or("Model emitted no text/content for plan")?;
    let json_str = extract_json_from_text(&text).ok_or("No JSON found in model proposal")?;

    let e1 = match serde_json::from_str::<CandidatePlan>(json_str) {
        Ok(mut plan) => {
            for t in &mut plan.tasks {
                crate::planning::service::align_candidate_task_role(t);
            }
            return Ok(plan);
        }
        Err(e) => e,
    };

    if let Ok(mut tasks) = serde_json::from_str::<Vec<CandidateTask>>(json_str) {
        if !tasks.is_empty() {
            for t in &mut tasks {
                crate::planning::service::align_candidate_task_role(t);
            }
            let plan_id = format!("plan-{}", Uuid::now_v7());
            return Ok(CandidatePlan::new(&plan_id, default_objective, tasks));
        }
    }

    if let Ok(mut wrapper) = serde_json::from_str::<CandidateTasksWrapper>(json_str) {
        if !wrapper.tasks.is_empty() {
            for t in &mut wrapper.tasks {
                crate::planning::service::align_candidate_task_role(t);
            }
            let plan_id = format!("plan-{}", Uuid::now_v7());
            return Ok(CandidatePlan::new(
                &plan_id,
                default_objective,
                wrapper.tasks,
            ));
        }
    }

    let e2 = match crate::planning::service::parse_decomposition_json(json_str) {
        Ok(dto) => {
            let planner = PlanServiceImpl::new(PathBuf::from("."));
            let mut tasks = planner
                .map_decomposition_to_tasks(dto)
                .map_err(|e| format!("map_decomposition_to_tasks failed: {e}"))?;
            for t in &mut tasks {
                crate::planning::service::align_candidate_task_role(t);
            }
            let plan_id = format!("plan-{}", Uuid::now_v7());
            return Ok(CandidatePlan::new(&plan_id, default_objective, tasks));
        }
        Err(e) => e,
    };

    Err(format!(
        "Could not parse candidate plan from model response (CandidatePlan err: {e1}; PlanDecompositionDto err: {e2}): text_len={}, json_len={}: {}",
        text.len(),
        json_str.len(),
        if text.len() > 1000 {
            &text[..1000]
        } else {
            &text
        }
    ))
}

#[derive(Debug, Deserialize)]
struct CandidateTasksWrapper {
    tasks: Vec<CandidateTask>,
}

fn parse_task_revision_proposal(proposal: &ModelProposal) -> Result<Vec<CandidateTask>, String> {
    let text = proposal_to_text(proposal).ok_or("Model emitted no text/content for tasks")?;
    let json_str = extract_json_from_text(&text).ok_or("No JSON found in model proposal")?;

    let e1 = match serde_json::from_str::<Vec<CandidateTask>>(json_str) {
        Ok(mut tasks) if !tasks.is_empty() => {
            for t in &mut tasks {
                crate::planning::service::align_candidate_task_role(t);
            }
            return Ok(tasks);
        }
        Ok(_) => "empty tasks array".to_string(),
        Err(e) => e.to_string(),
    };

    let e2 = match serde_json::from_str::<CandidateTasksWrapper>(json_str) {
        Ok(mut wrapper) if !wrapper.tasks.is_empty() => {
            for t in &mut wrapper.tasks {
                crate::planning::service::align_candidate_task_role(t);
            }
            return Ok(wrapper.tasks);
        }
        Ok(_) => "empty wrapper tasks array".to_string(),
        Err(e) => e.to_string(),
    };

    let e3 = match crate::planning::service::parse_decomposition_json(json_str) {
        Ok(dto) => {
            let planner = PlanServiceImpl::new(PathBuf::from("."));
            match planner.map_decomposition_to_tasks(dto) {
                Ok(mut tasks) if !tasks.is_empty() => {
                    for t in &mut tasks {
                        crate::planning::service::align_candidate_task_role(t);
                    }
                    return Ok(tasks);
                }
                Ok(_) => "empty mapped tasks".to_string(),
                Err(e) => format!("map_decomposition_to_tasks failed: {e}"),
            }
        }
        Err(e) => format!("parse_decomposition_json failed: {e}"),
    };

    Err(format!(
        "Could not parse candidate tasks from model response (Vec<CandidateTask> err: {e1}; CandidateTasksWrapper err: {e2}; PlanDecompositionDto err: {e3}): {}",
        if text.len() > 1000 {
            &text[..1000]
        } else {
            &text
        }
    ))
}

/// Central coordinator managing human-governed pre-execution lifecycle stages.
///
/// Follows: "The model proposes. The runtime decides."
pub struct PreExecutionCoordinator {
    pool: SqlitePool,
    event_bus: Option<Arc<BroadcastEventBus>>,
    model_caller: Option<Arc<dyn ModelCaller>>,
    prompt_catalog: Option<Arc<dyn PromptCatalog>>,
    prompt_compiler: Option<Arc<dyn crate::prompt::PromptCompiler>>,
    context_compiler: Option<Arc<dyn ContextCompiler>>,
    workspace_root: Option<PathBuf>,
    /// Live policy generation hash bound into minted execution
    /// authorizations. `None` fails the mint closed (unbound authorizations
    /// never verify at final execution).
    policy_hash: Option<String>,
    /// Execution role bound into minted authorizations (governed execution
    /// runs as implementer; a role change requires re-authorization).
    execution_role: Option<String>,
    /// Effective autonomy mode bound into minted authorizations. Resolved
    /// from live configuration at mint time; a profile switch between
    /// authorization and execution invalidates the authorization.
    execution_mode: Option<String>,
}

impl PreExecutionCoordinator {
    pub fn new(pool: SqlitePool, event_bus: Option<Arc<BroadcastEventBus>>) -> Self {
        Self {
            pool,
            event_bus,
            model_caller: None,
            prompt_catalog: None,
            prompt_compiler: None,
            context_compiler: None,
            workspace_root: None,
            policy_hash: None,
            execution_role: None,
            execution_mode: None,
        }
    }

    /// Create a PreExecutionCoordinator configured with deterministic test doubles
    /// for offline unit and state-machine tests.
    ///
    /// This explicitly distinguishes deterministic unit testing from live production execution.
    ///
    /// The test coordinator carries EXPLICITLY LABELED test bindings
    /// (`deterministic-test-*`, never a production hash): authorizations
    /// minted here verify only against matching test expectations, never
    /// against a live runtime (whose policy hash will differ, failing
    /// closed as designed).
    pub fn deterministic_test(pool: SqlitePool, event_bus: Option<Arc<BroadcastEventBus>>) -> Self {
        let catalog = Arc::new(crate::prompt::InMemoryPromptCatalog::with_builtins());
        let compiler = Arc::new(crate::prompt::DefaultPromptCompiler::new());
        let caller = Arc::new(crate::agent::model_policy::DeterministicLifecycleModelCaller::new());
        Self::new(pool, event_bus)
            .with_prompt_catalog(catalog)
            .with_prompt_compiler(compiler)
            .with_model_caller(caller)
            .with_policy_hash("deterministic-test-policy")
            .with_execution_role("implementer")
            .with_execution_mode("safe")
    }

    pub fn with_model_caller(mut self, caller: Arc<dyn ModelCaller>) -> Self {
        self.model_caller = Some(caller);
        self
    }

    pub fn with_prompt_catalog(mut self, catalog: Arc<dyn PromptCatalog>) -> Self {
        self.prompt_catalog = Some(catalog);
        self
    }

    /// Bind the canonical prompt compiler (runtime-shared in production).
    /// Pre-execution model prompts compile through it; a missing compiler
    /// fails the planning call closed (never template-only rendering).
    pub fn with_prompt_compiler(
        mut self,
        compiler: Arc<dyn crate::prompt::PromptCompiler>,
    ) -> Self {
        self.prompt_compiler = Some(compiler);
        self
    }

    pub fn with_context_compiler(mut self, compiler: Arc<dyn ContextCompiler>) -> Self {
        self.context_compiler = Some(compiler);
        self
    }

    /// Compile a pre-execution (discovery/planning/revision) prompt through
    /// the canonical PromptOS chain (catalog → compiler → EffectivePrompt).
    ///
    /// Both authorities are REQUIRED: a missing catalog or compiler fails
    /// the call closed — pre-execution model invocations MUST NOT happen
    /// behind template-only rendering or a substitute prompt.
    fn compile_pre_execution_prompt(
        &self,
        contract_id: &str,
        version: u32,
        scope_id: &str,
        task_objective: &str,
        params: BTreeMap<String, String>,
    ) -> Result<crate::prompt::EffectivePrompt, String> {
        let catalog = self.prompt_catalog.as_ref().ok_or_else(|| {
            "prompt catalog is required for pre-execution prompt compilation".to_string()
        })?;
        let compiler = self.prompt_compiler.as_ref().ok_or_else(|| {
            "prompt compiler is required for pre-execution prompt compilation".to_string()
        })?;
        let contract = catalog
            .resolve_canonical(contract_id, version)
            .map_err(|e| {
                format!("Failed to resolve prompt contract '{contract_id}' (v{version}): {e}")
            })?;
        crate::planning::service::compile_resolved_planning_prompt(
            catalog.as_ref(),
            compiler.as_ref(),
            contract,
            format!("pre-execution:{scope_id}"),
            format!("pre-execution:{scope_id}:task"),
            task_objective.to_string(),
            params,
        )
    }

    pub fn with_workspace_root(mut self, root: PathBuf) -> Self {
        self.workspace_root = Some(root);
        self
    }

    /// Bind the live policy generation hash into minted authorizations.
    /// Production wires the runtime's active policy hash; tests that need
    /// verifiable authorizations must wire one explicitly.
    pub fn with_policy_hash(mut self, hash: impl Into<String>) -> Self {
        self.policy_hash = Some(hash.into());
        self
    }

    /// Bind the execution role minted authorizations execute as.
    pub fn with_execution_role(mut self, role: impl Into<String>) -> Self {
        self.execution_role = Some(role.into());
        self
    }

    /// Bind the effective autonomy mode minted authorizations execute under.
    pub fn with_execution_mode(mut self, mode: impl Into<String>) -> Self {
        self.execution_mode = Some(mode.into());
        self
    }

    pub fn lifecycle_repo(&self) -> SqliteLifecycleRepository {
        SqliteLifecycleRepository::new(self.pool.clone())
    }

    fn intent_repo(&self) -> SqliteIntentRepository {
        SqliteIntentRepository::new(self.pool.clone())
    }

    fn emit_event(&self, event_type: EventType) {
        if let Some(ref bus) = self.event_bus {
            let bus = bus.clone();
            let env = EventEnvelope::new(
                0,
                None,
                None,
                "lifecycle_coordinator".to_string(),
                event_type,
            );
            tokio::spawn(async move {
                let _ = bus.publish(env).await;
            });
        }
    }

    /// Invoke the model for pre-execution reasoning (intent, discovery
    /// questions, plan/task revision) with usage tracking.
    ///
    /// Typed tool-visibility authority: pre-execution reasoning prompts are
    /// served EXPLICITLY tool-free via `call_model_tool_free_*`. Tool
    /// visibility never depends on prompt text containing magic markers.
    async fn call_model_with_usage_tracking(
        &self,
        caller: &Arc<dyn ModelCaller>,
        text: &str,
    ) -> Result<ModelProposal, String> {
        let cancel = tokio_util::sync::CancellationToken::new();
        let (proposal, usage) = caller
            .call_model_tool_free_cancellable_with_usage(text, &cancel)
            .await?;
        let inv_id = uuid::Uuid::now_v7();
        let provider = caller.provider_name();
        let model = caller.model_name();
        let (cost_usd, cost_provenance) =
            crate::model::pricing::calculate_cost_from_usage(&provider, &model, &usage);
        self.emit_event(EventType::ModelUsageUpdated {
            invocation_id: Some(inv_id),
            mission_id: None,
            task_id: None,
            provider,
            model,
            usage,
            cumulative_usage: None,
            cost_usd,
            cost_provenance,
        });
        Ok(proposal)
    }

    /// Intake user intent, detect unknowns, formulate questions or draft initial plan.
    pub async fn init_intent(
        &self,
        session_id: &str,
        raw_prompt: &str,
        _operator: &str,
    ) -> Result<PreExecutionResponse, String> {
        let repo = self.lifecycle_repo();

        // 1. Check if session already has durable lifecycle state (restart/crash recovery)
        if let Some(_existing) = repo
            .load_lifecycle_state(session_id)
            .await
            .map_err(|e| e.to_string())?
        {
            return Box::pin(self.resume_session(session_id)).await;
        }

        let mut intent = IntentState::initial_from_prompt(session_id, raw_prompt);

        let classification = crate::planning::service::classify_objective(raw_prompt);

        // Evidence-based unknown detection: semantic structure rather than word-count authority.
        // For read-only/reconnaissance tasks (e.g. "Study the codebase", "Audit"), architecture
        // choices are not missing prerequisites.
        if !classification.is_read_only() {
            let (has_arch_signal, _has_scope, _has_deliverable) =
                PromptSemanticAnalyzer::analyze(raw_prompt);

            if !has_arch_signal {
                intent.unknowns.push(IntentUnknown::from_evidence(
                    "unk_arch_choice",
                    "Application architecture and deployment boundaries are unspecified",
                    Criticality::High,
                    FactOrigin::RuntimeInferred {
                        heuristic:
                            "PromptSignalDetector: no architecture/technology keyword found in prompt"
                                .to_string(),
                    },
                    "No architecture keywords or deployment boundaries detected in prompt",
                    UnknownFate::UserDecisionRequired,
                ));
            }
        }

        self.intent_repo()
            .save(&intent)
            .await
            .map_err(|e| e.to_string())?;

        let (caller, catalog) = match (&self.model_caller, &self.prompt_catalog) {
            (Some(c), Some(cat)) => (c, cat),
            _ => {
                return Err(
                    "Model caller and prompt catalog are required for intent discovery and plan generation"
                        .to_string(),
                );
            }
        };

        let mut questions_from_model = Vec::new();
        // Skip dynamic questions if the objective is read-only and no explicit unknowns were identified
        if !classification.is_read_only() || !intent.unknowns.is_empty() {
            // Canonical PromptOS compilation: the discovery contract
            // compiles through the 7-layer PromptCompiler (same authority
            // as worker execution), not template-only rendering.
            if let Ok(contract) = catalog.resolve_canonical("genesis.dynamic_questions", 1) {
                let mut prompt_params = BTreeMap::new();
                prompt_params.insert("user_intent".to_string(), raw_prompt.to_string());
                let unk_str = intent
                    .unknowns
                    .iter()
                    .map(|u| format!("- [{}] {}: {}", u.id, u.description, u.evidence_basis))
                    .collect::<Vec<_>>()
                    .join("\n");
                prompt_params.insert(
                    "unknowns".to_string(),
                    if unk_str.is_empty() {
                        "None detected yet".to_string()
                    } else {
                        unk_str
                    },
                );
                prompt_params.insert("known_facts".to_string(), String::new());
                prompt_params.insert("resolved_decisions".to_string(), String::new());
                prompt_params.insert("previous_qa".to_string(), String::new());

                if let Ok(effective) = self.compile_pre_execution_prompt(
                    &contract.id.clone(),
                    contract.version,
                    session_id,
                    raw_prompt,
                    prompt_params,
                ) {
                    let rendered_text = effective.assembled_text;
                    let mut attempt = 0;
                    let proposal_opt = loop {
                        match self
                            .call_model_with_usage_tracking(caller, &rendered_text)
                            .await
                        {
                            Ok(p) => break Some(p),
                            Err(e) => {
                                let err_str = e.to_string();
                                let (is_transient, delay) =
                                    crate::planning::service::is_transient_provider_error(&err_str);
                                if is_transient && attempt < 3 {
                                    attempt += 1;
                                    tracing::warn!(
                                        "Transient error on dynamic question generation attempt {}/3: {}; retrying...",
                                        attempt,
                                        err_str
                                    );
                                    let sleep_dur = delay.unwrap_or_else(|| {
                                        std::time::Duration::from_millis(
                                            1000 * (1 << attempt.min(3)),
                                        )
                                    });
                                    tokio::time::sleep(sleep_dur).await;
                                    continue;
                                }
                                tracing::warn!(
                                    "Dynamic question model generation failed: {}; proceeding with direct candidate plan generation",
                                    err_str
                                );
                                break None;
                            }
                        }
                    };

                    if let Some(proposal) = proposal_opt {
                        let parsed = parse_dynamic_questions_proposal(&proposal);
                        let existing_questions = repo
                            .load_discovery_questions(session_id)
                            .await
                            .unwrap_or_default();
                        let mut answered_ids: Vec<String> = existing_questions
                            .iter()
                            .filter(|q| q.status == "answered")
                            .map(|q| q.question_id.clone())
                            .collect();

                        for q in parsed {
                            if !intent.unknowns.iter().any(|u| u.id == q.target_unknown) {
                                intent.unknowns.push(IntentUnknown::from_evidence(
                                    &q.target_unknown,
                                    &q.reason,
                                    Criticality::High,
                                    FactOrigin::ModelInferred {
                                        reasoning_summary: "Dynamic question proposed by model"
                                            .to_string(),
                                    },
                                    "Inferred from model dynamic questions proposal",
                                    UnknownFate::UserDecisionRequired,
                                ));
                            }
                            if validate_dynamic_question(&q, &intent, &answered_ids).is_ok() {
                                if let Ok(()) = repo.save_discovery_question(session_id, &q).await {
                                    answered_ids.push(q.question_id.clone());
                                    questions_from_model.push(q);
                                }
                            }
                        }
                    }
                }
            }
        }

        if !questions_from_model.is_empty() {
            self.intent_repo()
                .save(&intent)
                .await
                .map_err(|e| e.to_string())?;

            let persisted_state = PersistedLifecycleState {
                session_id: session_id.to_string(),
                stage: LifecycleStage::AwaitingInformation,
                plan_revision: 0,
                task_revision: 0,
                authorization_id: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            repo.save_lifecycle_state(&persisted_state)
                .await
                .map_err(|e| e.to_string())?;

            return Ok(PreExecutionResponse::QuestionsRequired {
                session_id: session_id.to_string(),
                questions: questions_from_model,
            });
        }

        // No dynamic questions required: generate initial CandidatePlan via real model
        // Shared prompt authorities: the planning service binds the
        // SAME catalog/compiler this coordinator owns (never divergent
        // per-call instances).
        let mut planner = PlanServiceImpl::new(
            self.workspace_root
                .clone()
                .unwrap_or_else(|| PathBuf::from(".")),
        )
        .with_model_caller(caller.clone())
        .with_prompt_catalog(catalog.clone());
        if let Some(ref compiler) = self.prompt_compiler {
            planner = planner.with_prompt_compiler(compiler.clone());
        }

        let sid_parsed = uuid::Uuid::parse_str(session_id)
            .map(crate::ids::SessionId::from)
            .map_err(|e| e.to_string())?;
        use crate::persistence::sqlite::repositories::SessionRepository;
        let session_repo = crate::persistence::sqlite::repositories::SqliteSessionRepository::new(
            self.pool.clone(),
        );
        let mission_id = if let Ok(Some(sess)) = session_repo.get(sid_parsed).await {
            sess.mission_id
        } else {
            crate::ids::MissionId::from(*sid_parsed.as_uuid())
        };
        let plan_req = PlanRequest::new(mission_id, raw_prompt);
        let resp = planner
            .generate_initial_plan(plan_req)
            .await
            .map_err(|e| format!("Model initial plan generation failed: {e}"))?;
        let plan = resp.candidate_plan;

        let plan_session =
            PlanReviewSession::new(session_id, plan, "model", RevisionAuthorType::Model).await?;
        let plan_rev = plan_session.current_plan_revision().clone();
        repo.save_plan_revision(&plan_rev)
            .await
            .map_err(|e| e.to_string())?;

        let persisted_state = PersistedLifecycleState {
            session_id: session_id.to_string(),
            stage: LifecycleStage::PlanReview,
            plan_revision: plan_rev.revision,
            task_revision: 0,
            authorization_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.save_lifecycle_state(&persisted_state)
            .await
            .map_err(|e| e.to_string())?;

        self.emit_event(EventType::PlanReviewRequired {
            session_id: session_id.to_string(),
            plan_id: plan_rev.plan_id.clone(),
            revision: plan_rev.revision,
        });

        Ok(PreExecutionResponse::PlanForReview {
            session_id: session_id.to_string(),
            revision: plan_rev,
        })
    }

    /// Submit an answer to an outstanding discovery question with UserProvided provenance.
    pub async fn submit_answer(
        &self,
        session_id: &str,
        question_id: &str,
        answer: &str,
        operator: &str,
    ) -> Result<PreExecutionResponse, String> {
        let trimmed_answer = answer.trim();
        if trimmed_answer.is_empty() {
            return Err(format!(
                "Answer to question '{question_id}' cannot be empty"
            ));
        }

        let repo = self.lifecycle_repo();
        let questions = repo
            .load_discovery_questions(session_id)
            .await
            .map_err(|e| e.to_string())?;

        let q_record = questions
            .iter()
            .find(|q| q.question_id == question_id)
            .ok_or_else(|| {
                format!("Question '{question_id}' not found for session '{session_id}'")
            })?;

        if q_record.status == "answered" || q_record.answer.is_some() {
            return Err(format!(
                "Question '{question_id}' has already been answered"
            ));
        }

        let dyn_q = DynamicQuestion::new(
            &q_record.question_id,
            &q_record.reason,
            &q_record.target_unknown,
            &q_record.text,
        )
        .with_options(q_record.options.clone())
        .with_allow_freeform(q_record.allow_freeform)
        .with_blocking(q_record.blocking);

        // Update IntentState with UserProvided provenance
        let sid_parsed = uuid::Uuid::parse_str(session_id)
            .map(crate::ids::SessionId::from)
            .map_err(|e| e.to_string())?;
        let mut intent = self
            .intent_repo()
            .load(sid_parsed)
            .await
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| IntentState::initial_from_prompt(session_id, ""));

        crate::workflow::genesis::validate_question_answer(&dyn_q, trimmed_answer, &intent)
            .map_err(|e| e.to_string())?;

        repo.record_question_answer(session_id, question_id, trimmed_answer, operator)
            .await
            .map_err(|e| format!("Failed to record question answer in persistence: {e}"))?;

        apply_question_answer(&mut intent, &dyn_q, trimmed_answer, operator);
        self.intent_repo()
            .save(&intent)
            .await
            .map_err(|e| e.to_string())?;

        // Check if more questions are pending
        let updated_questions = repo
            .load_discovery_questions(session_id)
            .await
            .map_err(|e| e.to_string())?;
        let pending: Vec<DynamicQuestion> = updated_questions
            .into_iter()
            .filter(|q| q.status != "answered" && q.blocking)
            .map(|q| {
                DynamicQuestion::new(&q.question_id, &q.reason, &q.target_unknown, &q.text)
                    .with_options(q.options)
                    .with_allow_freeform(q.allow_freeform)
                    .with_blocking(q.blocking)
            })
            .collect();

        if !pending.is_empty() {
            return Ok(PreExecutionResponse::QuestionsRequired {
                session_id: session_id.to_string(),
                questions: pending,
            });
        }

        // If an initial plan has already been generated for this session, return it without re-generating
        if let Some(existing_plan) = repo
            .load_latest_plan_revision(session_id)
            .await
            .map_err(|e| e.to_string())?
        {
            return Ok(PreExecutionResponse::PlanForReview {
                session_id: session_id.to_string(),
                revision: existing_plan,
            });
        }

        // All questions answered: generate initial CandidatePlan via real model
        let (caller, catalog) = match (&self.model_caller, &self.prompt_catalog) {
            (Some(c), Some(cat)) => (c, cat),
            _ => {
                return Err(
                    "Model caller and prompt catalog are required for plan generation".to_string(),
                );
            }
        };

        // Shared prompt authorities: the planning service binds the
        // SAME catalog/compiler this coordinator owns (never divergent
        // per-call instances).
        let mut planner = PlanServiceImpl::new(
            self.workspace_root
                .clone()
                .unwrap_or_else(|| PathBuf::from(".")),
        )
        .with_model_caller(caller.clone())
        .with_prompt_catalog(catalog.clone());
        if let Some(ref compiler) = self.prompt_compiler {
            planner = planner.with_prompt_compiler(compiler.clone());
        }

        let mut upstream = UpstreamPlanContext::default();
        for d in &intent.decisions {
            let ans = match &d.resolution {
                Some(crate::agent::intent::DecisionResolution::UserSelected {
                    raw_answer, ..
                }) => raw_answer.as_str(),
                _ => "resolved",
            };
            upstream.decisions.push(format!("{}: {}", d.question, ans));
            upstream
                .user_decisions
                .push(format!("{}: {}", d.question, ans));
        }
        for f in &intent.known_facts {
            upstream
                .requirements
                .push(format!("{}: {}", f.key, f.value));
        }

        let plan_req = PlanRequest::new(
            crate::ids::MissionId::from(Uuid::now_v7()),
            &intent.raw_prompt,
        )
        .with_upstream_context(upstream);

        let resp = planner
            .generate_initial_plan(plan_req)
            .await
            .map_err(|e| format!("Model plan generation after answers failed: {e}"))?;
        let plan = resp.candidate_plan;

        let plan_session =
            PlanReviewSession::new(session_id, plan, "model", RevisionAuthorType::Model).await?;
        let plan_rev = plan_session.current_plan_revision().clone();
        repo.save_plan_revision(&plan_rev)
            .await
            .map_err(|e| e.to_string())?;

        let persisted_state = PersistedLifecycleState {
            session_id: session_id.to_string(),
            stage: LifecycleStage::PlanReview,
            plan_revision: plan_rev.revision,
            task_revision: 0,
            authorization_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        repo.save_lifecycle_state(&persisted_state)
            .await
            .map_err(|e| e.to_string())?;

        self.emit_event(EventType::PlanReviewRequired {
            session_id: session_id.to_string(),
            plan_id: plan_rev.plan_id.clone(),
            revision: plan_rev.revision,
        });

        Ok(PreExecutionResponse::PlanForReview {
            session_id: session_id.to_string(),
            revision: plan_rev,
        })
    }

    /// Dispatch typed ApplicationAction through lifecycle review gates.
    pub async fn handle_action(
        &self,
        action: ApplicationAction,
        operator: &str,
    ) -> Result<PreExecutionResponse, String> {
        let repo = self.lifecycle_repo();

        match action {
            ApplicationAction::IntentInitRequested {
                session_id,
                raw_prompt,
            } => {
                let sid = session_id.ok_or("session_id required")?;
                self.init_intent(&sid, &raw_prompt, operator).await
            }

            ApplicationAction::QuestionAnswerSubmitted {
                session_id,
                question_id,
                answer,
            } => {
                let sid = session_id.ok_or("session_id required")?;
                self.submit_answer(&sid, &question_id, &answer, operator)
                    .await
            }

            ApplicationAction::PlanEditRequested {
                session_id,
                plan_json,
            } => {
                let sid = session_id.ok_or("session_id required")?;
                let cur_plan = repo
                    .load_latest_plan_revision(&sid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No existing plan revision to edit")?;

                let parsed_plan: CandidatePlan = serde_json::from_str(&plan_json)
                    .map_err(|e| format!("Invalid plan JSON schema: {e}"))?;

                let validator = PlanValidator::new();
                let report = validator.validate(&parsed_plan).await;
                if !report.is_valid() {
                    let errs: Vec<String> =
                        report.errors.iter().map(|e| format!("{:?}", e)).collect();
                    return Err(format!(
                        "Plan edit rejected by validation: {}",
                        errs.join("; ")
                    ));
                }

                // Invalidate downstream task acceptance and execution authorization
                repo.invalidate_authorizations(&sid, "Plan edited by operator")
                    .await
                    .map_err(|e| e.to_string())?;

                let new_rev = cur_plan.revision + 1;
                let mut new_plan = parsed_plan;
                new_plan.revision = new_rev;
                let plan_id = new_plan.plan_id.clone();

                let plan_rev = PlanRevision::new(
                    &sid,
                    new_rev,
                    &plan_id,
                    new_plan,
                    operator,
                    RevisionAuthorType::User,
                    Some(cur_plan.revision),
                );

                repo.save_plan_revision(&plan_rev)
                    .await
                    .map_err(|e| e.to_string())?;

                let state = PersistedLifecycleState {
                    session_id: sid.clone(),
                    stage: LifecycleStage::PlanReview,
                    plan_revision: new_rev,
                    task_revision: 0,
                    authorization_id: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                repo.save_lifecycle_state(&state)
                    .await
                    .map_err(|e| e.to_string())?;

                self.emit_event(EventType::PlanRevisionCreated {
                    session_id: sid.clone(),
                    plan_id: plan_rev.plan_id.clone(),
                    revision: new_rev,
                    author: operator.to_string(),
                });

                Ok(PreExecutionResponse::PlanForReview {
                    session_id: sid,
                    revision: plan_rev,
                })
            }

            ApplicationAction::PlanRevisionRequested {
                session_id,
                feedback,
            } => {
                let sid = session_id.ok_or("session_id required")?;
                let cur_plan = repo
                    .load_latest_plan_revision(&sid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No existing plan revision to revise")?;

                let (caller, catalog) = match (&self.model_caller, &self.prompt_catalog) {
                    (Some(c), Some(cat)) => (c, cat),
                    _ => {
                        return Err(
                            "Model caller and prompt catalog are required for plan revision"
                                .to_string(),
                        );
                    }
                };

                let contract = catalog
                    .resolve_canonical("planning.revision", 1)
                    .map_err(|e| format!("Failed to resolve planning.revision prompt: {e}"))?;

                let mut prompt_params = BTreeMap::new();
                prompt_params.insert(
                    "current_plan".to_string(),
                    cur_plan.to_json().unwrap_or_default(),
                );
                prompt_params.insert("user_feedback".to_string(), feedback.clone());
                prompt_params.insert("intent".to_string(), cur_plan.content.objective.clone());
                prompt_params.insert("requirements".to_string(), String::new());
                prompt_params.insert("decisions".to_string(), String::new());
                prompt_params.insert("evidence".to_string(), String::new());

                // Canonical PromptOS compilation (same authority as worker
                // execution). Fails closed on uncompilable contracts.
                let effective = self
                    .compile_pre_execution_prompt(
                        &contract.id.clone(),
                        contract.version,
                        &sid,
                        &cur_plan.content.objective,
                        prompt_params,
                    )
                    .map_err(|e| format!("Failed to compile planning.revision prompt: {e}"))?;
                let rendered_text = effective.assembled_text;

                let mut attempt = 0;
                let proposal = loop {
                    match self
                        .call_model_with_usage_tracking(caller, &rendered_text)
                        .await
                    {
                        Ok(p) => break p,
                        Err(e) => {
                            let err_str = e.to_string();
                            let (is_transient, delay) =
                                crate::planning::service::is_transient_provider_error(&err_str);
                            if is_transient && attempt < 3 {
                                attempt += 1;
                                tracing::warn!(
                                    "Transient error on plan revision attempt {}/3: {}; retrying...",
                                    attempt,
                                    err_str
                                );
                                let sleep_dur = delay.unwrap_or_else(|| {
                                    std::time::Duration::from_millis(1000 * (1 << attempt.min(3)))
                                });
                                tokio::time::sleep(sleep_dur).await;
                                continue;
                            }
                            return Err(format!("Model plan revision failed: {err_str}"));
                        }
                    }
                };

                let mut revised_plan = parse_plan_proposal(
                    &proposal,
                    &format!("{} (revised: {})", cur_plan.content.objective, feedback),
                )
                .map_err(|e| format!("Failed to parse model plan proposal: {e}"))?;

                if revised_plan == cur_plan.content {
                    return Err(
                        "Model revision produced identical plan without changes".to_string()
                    );
                }

                let validator = PlanValidator::new();
                let report = validator.validate(&revised_plan).await;
                if !report.is_valid() {
                    let errs: Vec<String> =
                        report.errors.iter().map(|e| format!("{:?}", e)).collect();
                    return Err(format!("Model plan proposal rejected: {}", errs.join("; ")));
                }

                repo.invalidate_authorizations(&sid, "Plan revised via model feedback")
                    .await
                    .map_err(|e| e.to_string())?;

                let new_rev = cur_plan.revision + 1;
                revised_plan.revision = new_rev;
                let plan_id = revised_plan.plan_id.clone();

                let plan_rev = PlanRevision::new(
                    &sid,
                    new_rev,
                    &plan_id,
                    revised_plan,
                    "model",
                    RevisionAuthorType::Model,
                    Some(cur_plan.revision),
                );

                repo.save_plan_revision(&plan_rev)
                    .await
                    .map_err(|e| e.to_string())?;

                let state = PersistedLifecycleState {
                    session_id: sid.clone(),
                    stage: LifecycleStage::PlanReview,
                    plan_revision: new_rev,
                    task_revision: 0,
                    authorization_id: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                repo.save_lifecycle_state(&state)
                    .await
                    .map_err(|e| e.to_string())?;

                self.emit_event(EventType::PlanRevisionCreated {
                    session_id: sid.clone(),
                    plan_id: plan_rev.plan_id.clone(),
                    revision: new_rev,
                    author: "model".to_string(),
                });

                Ok(PreExecutionResponse::PlanForReview {
                    session_id: sid,
                    revision: plan_rev,
                })
            }

            ApplicationAction::PlanRegenerateRequested { session_id } => {
                let sid = session_id.ok_or("session_id required")?;
                let cur_plan = repo
                    .load_latest_plan_revision(&sid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No existing plan revision to regenerate")?;

                let (caller, catalog) = match (&self.model_caller, &self.prompt_catalog) {
                    (Some(c), Some(cat)) => (c, cat),
                    _ => {
                        return Err(
                            "Model caller and prompt catalog are required for plan regeneration"
                                .to_string(),
                        );
                    }
                };

                // Shared prompt authorities: the planning service binds the
                // SAME catalog/compiler this coordinator owns (never divergent
                // per-call instances).
                let mut planner = PlanServiceImpl::new(
                    self.workspace_root
                        .clone()
                        .unwrap_or_else(|| PathBuf::from(".")),
                )
                .with_model_caller(caller.clone())
                .with_prompt_catalog(catalog.clone());
                if let Some(ref compiler) = self.prompt_compiler {
                    planner = planner.with_prompt_compiler(compiler.clone());
                }

                let plan_req = PlanRequest::new(
                    crate::ids::MissionId::from(Uuid::now_v7()),
                    &cur_plan.content.objective,
                );

                let resp = planner
                    .generate_initial_plan(plan_req)
                    .await
                    .map_err(|e| format!("Model plan regeneration failed: {e}"))?;

                let mut regen_plan = resp.candidate_plan;

                let validator = PlanValidator::new();
                let report = validator.validate(&regen_plan).await;
                if !report.is_valid() {
                    let errs: Vec<String> =
                        report.errors.iter().map(|e| format!("{:?}", e)).collect();
                    return Err(format!("Regenerated plan rejected: {}", errs.join("; ")));
                }

                repo.invalidate_authorizations(&sid, "Plan regenerated")
                    .await
                    .map_err(|e| e.to_string())?;

                let new_rev = cur_plan.revision + 1;
                regen_plan.revision = new_rev;
                let plan_id = regen_plan.plan_id.clone();

                let plan_rev = PlanRevision::new(
                    &sid,
                    new_rev,
                    &plan_id,
                    regen_plan,
                    "model",
                    RevisionAuthorType::Model,
                    Some(cur_plan.revision),
                );

                repo.save_plan_revision(&plan_rev)
                    .await
                    .map_err(|e| e.to_string())?;

                let state = PersistedLifecycleState {
                    session_id: sid.clone(),
                    stage: LifecycleStage::PlanReview,
                    plan_revision: new_rev,
                    task_revision: 0,
                    authorization_id: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                repo.save_lifecycle_state(&state)
                    .await
                    .map_err(|e| e.to_string())?;

                self.emit_event(EventType::PlanRevisionCreated {
                    session_id: sid.clone(),
                    plan_id: plan_rev.plan_id.clone(),
                    revision: new_rev,
                    author: "model".to_string(),
                });

                Ok(PreExecutionResponse::PlanForReview {
                    session_id: sid,
                    revision: plan_rev,
                })
            }

            ApplicationAction::PlanAcceptRequested {
                session_id,
                revision,
                content_hash,
            } => {
                let sid = session_id.ok_or("session_id required")?;
                let mut cur_plan = repo
                    .load_latest_plan_revision(&sid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No plan revision found to accept")?;

                if let Some(expected_rev) = revision {
                    if cur_plan.revision != expected_rev {
                        return Err(format!(
                            "Stale plan acceptance: expected revision {expected_rev}, but latest is {}",
                            cur_plan.revision
                        ));
                    }
                }

                if let Some(ref expected_hash) = content_hash {
                    let actual_hash = PlanRevision::compute_content_hash(&cur_plan.content);
                    if actual_hash != *expected_hash {
                        return Err(format!(
                            "Stale plan acceptance: content hash mismatch (expected {expected_hash}, got {actual_hash})"
                        ));
                    }
                }

                repo.update_plan_revision_status(
                    &sid,
                    cur_plan.revision,
                    PlanReviewStatus::Accepted,
                )
                .await
                .map_err(|e| e.to_string())?;
                cur_plan.status = PlanReviewStatus::Accepted;

                // Lower accepted plan directly into CandidateTasks
                let tasks = cur_plan.content.tasks.clone();
                validate_candidate_tasks(&tasks).map_err(|e| e.to_string())?;

                let task_rev = TaskRevision::new(
                    &sid,
                    1,
                    cur_plan.revision,
                    tasks.clone(),
                    &cur_plan.created_by,
                    cur_plan.author_type,
                    None,
                );

                repo.save_task_revision(&task_rev)
                    .await
                    .map_err(|e| e.to_string())?;

                let state = PersistedLifecycleState {
                    session_id: sid.clone(),
                    stage: LifecycleStage::TasksReview,
                    plan_revision: cur_plan.revision,
                    task_revision: 1,
                    authorization_id: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                repo.save_lifecycle_state(&state)
                    .await
                    .map_err(|e| e.to_string())?;

                self.emit_event(EventType::PlanAccepted {
                    session_id: sid.clone(),
                    plan_id: cur_plan.plan_id.clone(),
                    revision: cur_plan.revision,
                });

                self.emit_event(EventType::TasksReviewRequired {
                    session_id: sid.clone(),
                    plan_revision: cur_plan.revision,
                    task_revision: 1,
                    task_count: tasks.len(),
                });

                Ok(PreExecutionResponse::TasksForReview {
                    session_id: sid,
                    revision: task_rev,
                })
            }

            ApplicationAction::PlanRejectRequested { session_id, reason } => {
                let sid = session_id.ok_or("session_id required")?;
                let cur_plan = repo
                    .load_latest_plan_revision(&sid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No plan revision found to reject")?;

                repo.update_plan_revision_status(
                    &sid,
                    cur_plan.revision,
                    PlanReviewStatus::Rejected,
                )
                .await
                .map_err(|e| e.to_string())?;

                let state = PersistedLifecycleState {
                    session_id: sid.clone(),
                    stage: LifecycleStage::Rejected,
                    plan_revision: cur_plan.revision,
                    task_revision: 0,
                    authorization_id: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                repo.save_lifecycle_state(&state)
                    .await
                    .map_err(|e| e.to_string())?;

                self.emit_event(EventType::PlanRejected {
                    session_id: sid.clone(),
                    plan_id: cur_plan.plan_id.clone(),
                    revision: cur_plan.revision,
                    reason: reason.clone(),
                });

                Ok(PreExecutionResponse::Terminated {
                    session_id: sid,
                    stage: LifecycleStage::Rejected,
                    reason,
                })
            }

            ApplicationAction::TaskEditRequested {
                session_id,
                task_json,
            } => {
                let sid = session_id.ok_or("session_id required")?;
                let cur_tasks = repo
                    .load_latest_task_revision(&sid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No existing task revision to edit")?;

                let updated_task: CandidateTask = serde_json::from_str(&task_json)
                    .map_err(|e| format!("Invalid task JSON: {e}"))?;

                let mut new_tasks = cur_tasks.tasks.clone();
                let idx = new_tasks
                    .iter()
                    .position(|t| t.id == updated_task.id)
                    .ok_or_else(|| format!("Task '{}' not found", updated_task.id))?;
                new_tasks[idx] = updated_task;

                validate_candidate_tasks(&new_tasks).map_err(|e| e.to_string())?;

                // Invalidate any execution authorization
                repo.invalidate_authorizations(&sid, "Task edited by operator")
                    .await
                    .map_err(|e| e.to_string())?;

                let new_rev = cur_tasks.revision + 1;
                let task_rev = TaskRevision::new(
                    &sid,
                    new_rev,
                    cur_tasks.plan_revision,
                    new_tasks,
                    operator,
                    RevisionAuthorType::User,
                    Some(cur_tasks.revision),
                );

                repo.save_task_revision(&task_rev)
                    .await
                    .map_err(|e| e.to_string())?;

                let state = PersistedLifecycleState {
                    session_id: sid.clone(),
                    stage: LifecycleStage::TasksReview,
                    plan_revision: cur_tasks.plan_revision,
                    task_revision: new_rev,
                    authorization_id: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                repo.save_lifecycle_state(&state)
                    .await
                    .map_err(|e| e.to_string())?;

                self.emit_event(EventType::TaskRevisionCreated {
                    session_id: sid.clone(),
                    task_revision: new_rev,
                    plan_revision: cur_tasks.plan_revision,
                    author: operator.to_string(),
                });

                Ok(PreExecutionResponse::TasksForReview {
                    session_id: sid,
                    revision: task_rev,
                })
            }

            ApplicationAction::TaskAddRequested {
                session_id,
                task_json,
            } => {
                let sid = session_id.ok_or("session_id required")?;
                let cur_tasks = repo
                    .load_latest_task_revision(&sid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No existing task revision to add to")?;

                let new_task: CandidateTask = serde_json::from_str(&task_json)
                    .map_err(|e| format!("Invalid task JSON: {e}"))?;

                let mut new_tasks = cur_tasks.tasks.clone();
                new_tasks.push(new_task);

                validate_candidate_tasks(&new_tasks).map_err(|e| e.to_string())?;

                repo.invalidate_authorizations(&sid, "Task added by operator")
                    .await
                    .map_err(|e| e.to_string())?;

                let new_rev = cur_tasks.revision + 1;
                let task_rev = TaskRevision::new(
                    &sid,
                    new_rev,
                    cur_tasks.plan_revision,
                    new_tasks,
                    operator,
                    RevisionAuthorType::User,
                    Some(cur_tasks.revision),
                );

                repo.save_task_revision(&task_rev)
                    .await
                    .map_err(|e| e.to_string())?;

                let state = PersistedLifecycleState {
                    session_id: sid.clone(),
                    stage: LifecycleStage::TasksReview,
                    plan_revision: cur_tasks.plan_revision,
                    task_revision: new_rev,
                    authorization_id: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                repo.save_lifecycle_state(&state)
                    .await
                    .map_err(|e| e.to_string())?;

                self.emit_event(EventType::TaskRevisionCreated {
                    session_id: sid.clone(),
                    task_revision: new_rev,
                    plan_revision: cur_tasks.plan_revision,
                    author: operator.to_string(),
                });

                Ok(PreExecutionResponse::TasksForReview {
                    session_id: sid,
                    revision: task_rev,
                })
            }

            ApplicationAction::TaskRemoveRequested {
                session_id,
                task_id,
            } => {
                let sid = session_id.ok_or("session_id required")?;
                let cur_tasks = repo
                    .load_latest_task_revision(&sid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No existing task revision to remove from")?;

                let task_key = CandidateTaskKey::new(&task_id);

                // Dependents check
                let dependents: Vec<String> = cur_tasks
                    .tasks
                    .iter()
                    .filter(|t| t.depends_on.iter().any(|d| d == &task_key))
                    .map(|t| t.id.to_string())
                    .collect();

                if !dependents.is_empty() {
                    return Err(format!(
                        "Cannot remove task '{}' because other tasks depend on it: {:?}",
                        task_id, dependents
                    ));
                }

                let new_tasks: Vec<CandidateTask> = cur_tasks
                    .tasks
                    .into_iter()
                    .filter(|t| t.id != task_key)
                    .collect();

                validate_candidate_tasks(&new_tasks).map_err(|e| e.to_string())?;

                repo.invalidate_authorizations(&sid, "Task removed by operator")
                    .await
                    .map_err(|e| e.to_string())?;

                let new_rev = cur_tasks.revision + 1;
                let task_rev = TaskRevision::new(
                    &sid,
                    new_rev,
                    cur_tasks.plan_revision,
                    new_tasks,
                    operator,
                    RevisionAuthorType::User,
                    Some(cur_tasks.revision),
                );

                repo.save_task_revision(&task_rev)
                    .await
                    .map_err(|e| e.to_string())?;

                let state = PersistedLifecycleState {
                    session_id: sid.clone(),
                    stage: LifecycleStage::TasksReview,
                    plan_revision: cur_tasks.plan_revision,
                    task_revision: new_rev,
                    authorization_id: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                repo.save_lifecycle_state(&state)
                    .await
                    .map_err(|e| e.to_string())?;

                self.emit_event(EventType::TaskRevisionCreated {
                    session_id: sid.clone(),
                    task_revision: new_rev,
                    plan_revision: cur_tasks.plan_revision,
                    author: operator.to_string(),
                });

                Ok(PreExecutionResponse::TasksForReview {
                    session_id: sid,
                    revision: task_rev,
                })
            }

            ApplicationAction::TaskRegenerateRequested {
                session_id,
                feedback,
            } => {
                let sid = session_id.ok_or("session_id required")?;
                let cur_tasks = repo
                    .load_latest_task_revision(&sid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No existing task revision to regenerate")?;

                repo.invalidate_authorizations(&sid, "Tasks regenerated")
                    .await
                    .map_err(|e| e.to_string())?;

                let (caller, catalog) = match (&self.model_caller, &self.prompt_catalog) {
                    (Some(c), Some(cat)) => (c, cat),
                    _ => {
                        return Err(
                            "Model caller and prompt catalog are required for task regeneration"
                                .to_string(),
                        );
                    }
                };

                let contract = catalog
                    .resolve_canonical("planning.task_revision", 1)
                    .map_err(|e| format!("Failed to resolve planning.task_revision prompt: {e}"))?;

                let cur_plan = repo
                    .load_latest_plan_revision(&sid)
                    .await
                    .map_err(|e| e.to_string())?;
                let plan_objective = cur_plan
                    .as_ref()
                    .map(|p| p.content.objective.clone())
                    .unwrap_or_default();
                let plan_json = cur_plan
                    .as_ref()
                    .map(|p| p.to_json().unwrap_or_default())
                    .unwrap_or_default();
                let tasks_json = serde_json::to_string(&cur_tasks.tasks).unwrap_or_default();

                let mut prompt_params = BTreeMap::new();
                prompt_params.insert("accepted_plan".to_string(), plan_json);
                prompt_params.insert("current_tasks".to_string(), tasks_json);
                prompt_params.insert(
                    "user_feedback".to_string(),
                    feedback.clone().unwrap_or_default(),
                );
                prompt_params.insert("decisions".to_string(), String::new());

                // Canonical PromptOS compilation (same authority as worker
                // execution). Fails closed on uncompilable contracts.
                let effective = self
                    .compile_pre_execution_prompt(
                        &contract.id.clone(),
                        contract.version,
                        &sid,
                        &plan_objective,
                        prompt_params,
                    )
                    .map_err(|e| format!("Failed to compile planning.task_revision prompt: {e}"))?;
                let rendered_text = effective.assembled_text;

                let mut attempt = 0;
                let proposal = loop {
                    match self
                        .call_model_with_usage_tracking(caller, &rendered_text)
                        .await
                    {
                        Ok(p) => break p,
                        Err(e) => {
                            let err_str = e.to_string();
                            let (is_transient, delay) =
                                crate::planning::service::is_transient_provider_error(&err_str);
                            if is_transient && attempt < 3 {
                                attempt += 1;
                                tracing::warn!(
                                    "Transient error on task revision attempt {}/3: {}; retrying...",
                                    attempt,
                                    err_str
                                );
                                let sleep_dur = delay.unwrap_or_else(|| {
                                    std::time::Duration::from_millis(1000 * (1 << attempt.min(3)))
                                });
                                tokio::time::sleep(sleep_dur).await;
                                continue;
                            }
                            return Err(format!("Model task revision call failed: {err_str}"));
                        }
                    }
                };

                let regen_tasks = parse_task_revision_proposal(&proposal)
                    .map_err(|e| format!("Failed to parse task revision proposal: {e}"))?;

                if regen_tasks.is_empty() {
                    return Err("Model task revision returned empty task list".to_string());
                }

                validate_candidate_tasks(&regen_tasks).map_err(|e| e.to_string())?;

                repo.invalidate_authorizations(&sid, "Tasks regenerated")
                    .await
                    .map_err(|e| e.to_string())?;

                let new_rev = cur_tasks.revision + 1;
                let task_rev = TaskRevision::new(
                    &sid,
                    new_rev,
                    cur_tasks.plan_revision,
                    regen_tasks,
                    "model",
                    RevisionAuthorType::Model,
                    Some(cur_tasks.revision),
                );

                repo.save_task_revision(&task_rev)
                    .await
                    .map_err(|e| e.to_string())?;

                let state = PersistedLifecycleState {
                    session_id: sid.clone(),
                    stage: LifecycleStage::TasksReview,
                    plan_revision: cur_tasks.plan_revision,
                    task_revision: new_rev,
                    authorization_id: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                repo.save_lifecycle_state(&state)
                    .await
                    .map_err(|e| e.to_string())?;

                self.emit_event(EventType::TaskRevisionCreated {
                    session_id: sid.clone(),
                    task_revision: new_rev,
                    plan_revision: cur_tasks.plan_revision,
                    author: "model".to_string(),
                });

                Ok(PreExecutionResponse::TasksForReview {
                    session_id: sid,
                    revision: task_rev,
                })
            }

            ApplicationAction::TasksAcceptRequested {
                session_id,
                revision,
                content_hash,
            } => {
                let sid = session_id.ok_or("session_id required")?;
                let cur_tasks = repo
                    .load_latest_task_revision(&sid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No task revision found to accept")?;

                if let Some(expected_rev) = revision {
                    if cur_tasks.revision != expected_rev {
                        return Err(format!(
                            "Stale task acceptance: expected revision {expected_rev}, but latest is {}",
                            cur_tasks.revision
                        ));
                    }
                }

                if let Some(ref expected_hash) = content_hash {
                    let actual_hash = TaskRevision::compute_tasks_hash(&cur_tasks.tasks);
                    if actual_hash != *expected_hash {
                        return Err(format!(
                            "Stale task acceptance: content hash mismatch (expected {expected_hash}, got {actual_hash})"
                        ));
                    }
                }

                repo.update_task_revision_status(
                    &sid,
                    cur_tasks.revision,
                    TaskReviewStatus::Accepted,
                )
                .await
                .map_err(|e| e.to_string())?;

                let latest_plan = repo.load_latest_plan_revision(&sid).await.ok().flatten();
                let is_read_only = latest_plan
                    .as_ref()
                    .map(|p| {
                        crate::planning::service::classify_objective(&p.content.objective)
                            .is_read_only()
                    })
                    .unwrap_or(false);

                if is_read_only
                    && self.policy_hash.is_some()
                    && self.workspace_root.is_some()
                    && self.execution_role.is_some()
                    && self.execution_mode.is_some()
                {
                    if let Some(plan) = latest_plan {
                        let plan_hash = PlanRevision::compute_content_hash(&plan.content);
                        let task_hash = TaskRevision::compute_tasks_hash(&cur_tasks.tasks);
                        let policy_hash = self.policy_hash.clone().unwrap();
                        let workspace_root = self.workspace_root.clone().unwrap();
                        let execution_role = self.execution_role.clone().unwrap();
                        let execution_mode = self.execution_mode.clone().unwrap();
                        let auth = ExecutionAuthorization::new(
                            &sid,
                            plan.revision,
                            cur_tasks.revision,
                            "policy-auto-read-only",
                        )
                        .with_content_hashes(plan_hash, task_hash)
                        .with_execution_binding(
                            policy_hash,
                            workspace_root.display().to_string(),
                            execution_role,
                            execution_mode,
                            std::time::Duration::from_secs(3600),
                        );
                        let _ = repo.save_execution_authorization(&auth).await;

                        let next_state = PersistedLifecycleState {
                            session_id: sid.clone(),
                            stage: LifecycleStage::ExecutionAuthorized,
                            plan_revision: plan.revision,
                            task_revision: cur_tasks.revision,
                            authorization_id: Some(auth.id),
                            created_at: Utc::now(),
                            updated_at: Utc::now(),
                        };
                        let _ = repo.save_lifecycle_state(&next_state).await;

                        self.emit_event(EventType::TasksAccepted {
                            session_id: sid.clone(),
                            task_revision: cur_tasks.revision,
                            plan_revision: cur_tasks.plan_revision,
                        });
                        self.emit_event(EventType::ExecutionAuthorized {
                            session_id: sid.clone(),
                            authorization_id: auth.id,
                            authorized_by: "policy-auto-read-only".to_string(),
                        });

                        return Ok(PreExecutionResponse::ReadyToExecute {
                            session_id: sid,
                            plan: plan.content,
                            tasks: cur_tasks.tasks,
                            authorization: auth,
                        });
                    }
                }

                let message = "The plan and task list are accepted.\n\nM31A is ready to modify the workspace and execute the approved tasks.\n\nProceed with implementation?".to_string();

                let state = PersistedLifecycleState {
                    session_id: sid.clone(),
                    stage: LifecycleStage::ExecutionAwaitingAuthorization,
                    plan_revision: cur_tasks.plan_revision,
                    task_revision: cur_tasks.revision,
                    authorization_id: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };
                repo.save_lifecycle_state(&state)
                    .await
                    .map_err(|e| e.to_string())?;

                self.emit_event(EventType::TasksAccepted {
                    session_id: sid.clone(),
                    task_revision: cur_tasks.revision,
                    plan_revision: cur_tasks.plan_revision,
                });

                self.emit_event(EventType::ExecutionAuthorizationRequired {
                    session_id: sid.clone(),
                    plan_revision: cur_tasks.plan_revision,
                    task_revision: cur_tasks.revision,
                    message: message.clone(),
                });

                Ok(PreExecutionResponse::AuthorizationRequested {
                    session_id: sid,
                    plan_revision: cur_tasks.plan_revision,
                    task_revision: cur_tasks.revision,
                    message,
                })
            }

            ApplicationAction::ExecutionAuthorizationSubmitted {
                session_id,
                decision,
                reason,
            } => {
                let sid = session_id.ok_or("session_id required")?;
                let state = repo
                    .load_lifecycle_state(&sid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No active lifecycle state found")?;

                if state.stage != LifecycleStage::ExecutionAwaitingAuthorization {
                    return Err(format!(
                        "Cannot authorize execution: current stage is {:?}",
                        state.stage
                    ));
                }

                let plan = repo
                    .load_latest_plan_revision(&sid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No plan revision found")?;
                if plan.status != PlanReviewStatus::Accepted {
                    return Err(
                        "Cannot authorize execution: plan has not been accepted".to_string()
                    );
                }

                let tasks = repo
                    .load_latest_task_revision(&sid)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No task revision found")?;
                if tasks.status != TaskReviewStatus::Accepted {
                    return Err(
                        "Cannot authorize execution: tasks have not been accepted".to_string()
                    );
                }

                let plan_hash = PlanRevision::compute_content_hash(&plan.content);
                let task_hash = TaskRevision::compute_tasks_hash(&tasks.tasks);

                if decision {
                    // Full-surface binding: policy generation, workspace,
                    // role, mode, and expiry are bound NOW. Any field missing
                    // fails the authorization itself closed — an unbound
                    // authorization could never verify at final execution.
                    let policy_hash = self.policy_hash.clone().ok_or_else(|| {
                        "Cannot authorize execution: no live policy generation bound".to_string()
                    })?;
                    let workspace_root = self.workspace_root.clone().ok_or_else(|| {
                        "Cannot authorize execution: no workspace identity bound".to_string()
                    })?;
                    let execution_role = self.execution_role.clone().ok_or_else(|| {
                        "Cannot authorize execution: no execution role bound".to_string()
                    })?;
                    let execution_mode = self.execution_mode.clone().ok_or_else(|| {
                        "Cannot authorize execution: no autonomy mode bound".to_string()
                    })?;
                    let auth =
                        ExecutionAuthorization::new(&sid, plan.revision, tasks.revision, operator)
                            .with_content_hashes(plan_hash, task_hash)
                            .with_execution_binding(
                                policy_hash,
                                workspace_root.display().to_string(),
                                execution_role,
                                execution_mode,
                                std::time::Duration::from_secs(3600),
                            );
                    repo.save_execution_authorization(&auth)
                        .await
                        .map_err(|e| e.to_string())?;

                    // INVARIANT D: Persist ExecutionAuthorized, NOT Executing.
                    // The actual StartExecution transition to Executing happens at the
                    // runtime boundary (run_authorized_mission) after the TaskGraph is
                    // materialized and the AutonomyController begins. A crash between
                    // authorization and scheduler handoff must recover as ExecutionAuthorized,
                    // not Executing, because execution has not verifiably started.
                    let next_state = PersistedLifecycleState {
                        session_id: sid.clone(),
                        stage: LifecycleStage::ExecutionAuthorized,
                        plan_revision: plan.revision,
                        task_revision: tasks.revision,
                        authorization_id: Some(auth.id),
                        created_at: state.created_at,
                        updated_at: Utc::now(),
                    };
                    repo.save_lifecycle_state(&next_state)
                        .await
                        .map_err(|e| e.to_string())?;

                    self.emit_event(EventType::ExecutionAuthorized {
                        session_id: sid.clone(),
                        authorization_id: auth.id,
                        authorized_by: operator.to_string(),
                    });

                    Ok(PreExecutionResponse::ReadyToExecute {
                        session_id: sid,
                        plan: plan.content,
                        tasks: tasks.tasks,
                        authorization: auth,
                    })
                } else {
                    let mut auth =
                        ExecutionAuthorization::new(&sid, plan.revision, tasks.revision, operator)
                            .with_content_hashes(plan_hash, task_hash);
                    auth.decision = AuthorizationDecision::Rejected;
                    auth.invalidation_reason = reason.clone();
                    repo.save_execution_authorization(&auth)
                        .await
                        .map_err(|e| e.to_string())?;

                    let next_state = PersistedLifecycleState {
                        session_id: sid.clone(),
                        stage: LifecycleStage::Rejected,
                        plan_revision: plan.revision,
                        task_revision: tasks.revision,
                        authorization_id: None,
                        created_at: state.created_at,
                        updated_at: Utc::now(),
                    };
                    repo.save_lifecycle_state(&next_state)
                        .await
                        .map_err(|e| e.to_string())?;

                    self.emit_event(EventType::ExecutionAuthorizationRejected {
                        session_id: sid.clone(),
                        reason: reason.clone().unwrap_or_else(|| {
                            "Operator rejected launch authorization".to_string()
                        }),
                    });

                    Ok(PreExecutionResponse::Terminated {
                        session_id: sid,
                        stage: LifecycleStage::Rejected,
                        reason: reason.unwrap_or_else(|| {
                            "Operator rejected launch authorization".to_string()
                        }),
                    })
                }
            }

            _ => Err("Action not handled by PreExecutionCoordinator".to_string()),
        }
    }

    /// Restore exact lifecycle stage across restart / crash recovery.
    pub async fn resume_session(&self, session_id: &str) -> Result<PreExecutionResponse, String> {
        let repo = self.lifecycle_repo();
        let state = repo
            .load_lifecycle_state(session_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                format!("No persisted lifecycle state found for session '{session_id}'")
            })?;

        match state.stage {
            LifecycleStage::AwaitingInformation => {
                let questions = repo
                    .load_discovery_questions(session_id)
                    .await
                    .map_err(|e| e.to_string())?;
                let pending: Vec<DynamicQuestion> = questions
                    .into_iter()
                    .filter(|q| q.status != "answered" && q.blocking)
                    .map(|q| {
                        DynamicQuestion::new(&q.question_id, &q.reason, &q.target_unknown, &q.text)
                            .with_options(q.options)
                            .with_allow_freeform(q.allow_freeform)
                            .with_blocking(q.blocking)
                    })
                    .collect();

                Ok(PreExecutionResponse::QuestionsRequired {
                    session_id: session_id.to_string(),
                    questions: pending,
                })
            }
            LifecycleStage::PlanDraft
            | LifecycleStage::PlanReview
            | LifecycleStage::PlanRevision => {
                let plan = repo
                    .load_latest_plan_revision(session_id)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No plan revision found for active plan review stage")?;
                Ok(PreExecutionResponse::PlanForReview {
                    session_id: session_id.to_string(),
                    revision: plan,
                })
            }
            LifecycleStage::PlanAccepted
            | LifecycleStage::TasksDraft
            | LifecycleStage::TasksReview
            | LifecycleStage::TasksRevision => {
                let tasks = repo
                    .load_latest_task_revision(session_id)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No task revision found for active task review stage")?;
                Ok(PreExecutionResponse::TasksForReview {
                    session_id: session_id.to_string(),
                    revision: tasks,
                })
            }
            LifecycleStage::TasksAccepted | LifecycleStage::ExecutionAwaitingAuthorization => {
                let message = "The plan and task list are accepted.\n\nM31A is ready to modify the workspace and execute the approved tasks.\n\nProceed with implementation?".to_string();
                Ok(PreExecutionResponse::AuthorizationRequested {
                    session_id: session_id.to_string(),
                    plan_revision: state.plan_revision,
                    task_revision: state.task_revision,
                    message,
                })
            }
            LifecycleStage::ExecutionAuthorized | LifecycleStage::Executing => {
                // INVARIANT 8: Load exact authorized revisions, never 'latest whatever'.
                let plan = repo
                    .load_plan_revision(session_id, state.plan_revision)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("No plan revision {} found", state.plan_revision))?;
                let tasks = repo
                    .load_task_revision(session_id, state.task_revision)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("No task revision {} found", state.task_revision))?;
                let auth = repo
                    .load_latest_execution_authorization(session_id)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or("No execution authorization found")?;

                let plan_hash = PlanRevision::compute_content_hash(&plan.content);
                let task_hash = TaskRevision::compute_tasks_hash(&tasks.tasks);

                if !auth.is_valid_for_exact(
                    plan.revision,
                    tasks.revision,
                    Some(&plan_hash),
                    Some(&task_hash),
                ) {
                    return Ok(PreExecutionResponse::AuthorizationRequested {
                        session_id: session_id.to_string(),
                        plan_revision: plan.revision,
                        task_revision: tasks.revision,
                        message: "Previous authorization is stale, invalidated, or content hash mismatch. Re-authorization required.".to_string(),
                    });
                }

                Ok(PreExecutionResponse::ReadyToExecute {
                    session_id: session_id.to_string(),
                    plan: plan.content,
                    tasks: tasks.tasks,
                    authorization: auth,
                })
            }
            term @ (LifecycleStage::Cancelled
            | LifecycleStage::Rejected
            | LifecycleStage::Blocked
            | LifecycleStage::Failed
            | LifecycleStage::Completed) => Ok(PreExecutionResponse::Terminated {
                session_id: session_id.to_string(),
                stage: term,
                reason: format!("Session is in terminal state: {term}"),
            }),
            LifecycleStage::IntentActive => {
                let intent_repo = self.intent_repo();
                let sid_parsed = uuid::Uuid::parse_str(session_id)
                    .map(crate::ids::SessionId::from)
                    .map_err(|e| e.to_string())?;
                let intent = intent_repo
                    .load(sid_parsed)
                    .await
                    .map_err(|e| e.to_string())?;
                let prompt = intent.map(|i| i.raw_prompt).unwrap_or_default();
                Box::pin(self.init_intent(session_id, &prompt, "operator")).await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::plan::{
        CapabilityAccessMode, CapabilityRequirement, ResourceEstimate, VerificationStrategy,
    };
    use crate::state_machine::agent::AgentRole;

    fn sample_task(id: &str, deps: Vec<&str>) -> CandidateTask {
        let mut t = CandidateTask::new(
            id,
            format!("Task {}", id),
            AgentRole::implementer(),
            VerificationStrategy::Compilation,
            ResourceEstimate::default(),
        );
        t.depends_on = deps.into_iter().map(CandidateTaskKey::new).collect();
        t.capabilities = vec![CapabilityRequirement::new(
            "fs.write",
            CapabilityAccessMode::Write,
        )];
        t
    }

    #[test]
    fn test_task_graph_validation_detects_cycles() {
        let t1 = sample_task("T1", vec!["T2"]);
        let t2 = sample_task("T2", vec!["T1"]);
        let err = validate_candidate_tasks(&[t1, t2]).unwrap_err();
        assert!(matches!(
            err,
            TaskGraphValidationError::CycleDetected { .. }
        ));
    }

    #[test]
    fn test_task_graph_validation_detects_missing_dep() {
        let t1 = sample_task("T1", vec!["T99"]);
        let err = validate_candidate_tasks(&[t1]).unwrap_err();
        assert!(matches!(
            err,
            TaskGraphValidationError::MissingDependency { .. }
        ));
    }

    #[test]
    fn test_task_graph_validation_detects_duplicate_id() {
        let t1 = sample_task("T1", vec![]);
        let t2 = sample_task("T1", vec![]);
        let err = validate_candidate_tasks(&[t1, t2]).unwrap_err();
        assert!(matches!(
            err,
            TaskGraphValidationError::DuplicateTaskId { .. }
        ));
    }

    #[tokio::test]
    async fn test_task_removal_checks_dependents() {
        let t1 = sample_task("T1", vec![]);
        let t2 = sample_task("T2", vec!["T1"]);
        let t3 = sample_task("T3", vec!["T2"]);

        let plan = CandidatePlan::new("test-plan", "test", vec![t1, t2, t3]);
        let mut plan_session = PlanReviewSession::new("s1", plan, "test", RevisionAuthorType::User)
            .await
            .unwrap();
        plan_session.accept_plan().unwrap();

        let mut task_session = TaskReviewSession::from_accepted_plan(
            "s1",
            plan_session.current_plan_revision(),
            "test",
        )
        .unwrap();

        // Attempting to remove T2 when T3 depends on it must fail
        let err = task_session
            .remove_task(&CandidateTaskKey::new("T2"), "test")
            .unwrap_err();
        assert!(matches!(
            err,
            TaskGraphValidationError::DependentTasksBlockRemoval { .. }
        ));

        // Removing leaf T3 succeeds
        task_session
            .remove_task(&CandidateTaskKey::new("T3"), "test")
            .unwrap();
        assert_eq!(task_session.current_tasks().len(), 2);
    }

    #[test]
    fn test_authorization_binding_and_invalidation() {
        let mut auth = ExecutionAuthorization::new("s1", 1, 1, "operator");
        assert!(auth.is_valid_for(1, 1));

        // Invalidate on task revision change
        assert!(!auth.is_valid_for(1, 2));

        // Invalidate on plan revision change
        assert!(!auth.is_valid_for(2, 1));

        // Invalidate explicitly
        auth.invalidate("Plan revision updated");
        assert!(!auth.is_valid_for(1, 1));
        assert_eq!(auth.decision, AuthorizationDecision::Invalidated);
    }
}
