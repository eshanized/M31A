//! Canonical kernel domain types for Long-Horizon Engineering Memory.
//!
//! Kernel-owned pure domain models for:
//! - Architectural Decisions
//! - Epistemic Assumptions
//! - Failure Diagnoses & Attempt Memory
//! - Durable Review Findings
//! - Revision-Aware Verification Records
//!
//! Subordinate to runtime state and repository reality.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use crate::ids::{CheckId, MissionId, TaskId};

/// Explicit status of an architectural decision (re-export or definition).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatus {
    #[default]
    Proposed,
    Accepted,
    Rejected,
    Superseded,
    NeedsOperatorDecision,
}

impl DecisionStatus {
    pub fn is_authoritative(&self) -> bool {
        matches!(self, Self::Accepted)
    }

    pub fn can_transition_to(&self, new_status: DecisionStatus) -> bool {
        matches!(
            (self, new_status),
            (Self::Proposed, Self::Accepted)
                | (Self::Proposed, Self::Rejected)
                | (Self::Proposed, Self::NeedsOperatorDecision)
                | (Self::NeedsOperatorDecision, Self::Accepted)
                | (Self::NeedsOperatorDecision, Self::Rejected)
                | (Self::Accepted, Self::Superseded)
        )
    }
}

impl fmt::Display for DecisionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Proposed => write!(f, "proposed"),
            Self::Accepted => write!(f, "accepted"),
            Self::Rejected => write!(f, "rejected"),
            Self::Superseded => write!(f, "superseded"),
            Self::NeedsOperatorDecision => write!(f, "needs_operator_decision"),
        }
    }
}

impl std::str::FromStr for DecisionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().replace(['-', ' '], "_").as_str() {
            "proposed" => Ok(Self::Proposed),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            "superseded" => Ok(Self::Superseded),
            "needs_operator_decision" | "needs_review" | "needs_operator" => {
                Ok(Self::NeedsOperatorDecision)
            }
            other => Err(format!("Unknown decision status: {}", other)),
        }
    }
}

/// Scope of engineering memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    /// Cross-mission, durable project knowledge.
    Project,
    /// Mission-bounded knowledge.
    Mission,
}

impl MemoryScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Mission => "mission",
        }
    }
}

impl fmt::Display for MemoryScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for MemoryScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "project" => Ok(Self::Project),
            "mission" => Ok(Self::Mission),
            other => Err(format!("unknown memory scope: {}", other)),
        }
    }
}

/// Durable architectural decision record stored in canonical SQLite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineeringDecision {
    pub id: String,
    pub scope: MemoryScope,
    pub mission_id: Option<MissionId>,
    pub title: String,
    pub context: String,
    pub decision: String,
    pub rationale: String,
    pub alternatives_considered: Vec<String>,
    pub consequences: Vec<String>,
    pub status: DecisionStatus,
    pub superseded_by: Option<String>,
    pub linked_requirements: Vec<String>,
    pub linked_files: Vec<String>,
    pub linked_symbols: Vec<String>,
    pub provenance_actor: String,
    pub provenance_reason: Option<String>,
    pub version: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl EngineeringDecision {
    pub fn new(
        id: impl Into<String>,
        scope: MemoryScope,
        title: impl Into<String>,
        context: impl Into<String>,
        decision: impl Into<String>,
        rationale: impl Into<String>,
        actor: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            scope,
            mission_id: None,
            title: title.into(),
            context: context.into(),
            decision: decision.into(),
            rationale: rationale.into(),
            alternatives_considered: Vec::new(),
            consequences: Vec::new(),
            status: DecisionStatus::Proposed,
            superseded_by: None,
            linked_requirements: Vec::new(),
            linked_files: Vec::new(),
            linked_symbols: Vec::new(),
            provenance_actor: actor.into(),
            provenance_reason: None,
            version: 1,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_mission_id(mut self, mission_id: MissionId) -> Self {
        self.mission_id = Some(mission_id);
        self
    }

    pub fn with_status(mut self, status: DecisionStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_alternatives(mut self, alts: Vec<String>) -> Self {
        self.alternatives_considered = alts;
        self
    }

    pub fn with_consequences(mut self, cons: Vec<String>) -> Self {
        self.consequences = cons;
        self
    }

    pub fn with_linked_files(mut self, files: Vec<String>) -> Self {
        self.linked_files = files;
        self
    }

    pub fn with_linked_requirements(mut self, reqs: Vec<String>) -> Self {
        self.linked_requirements = reqs;
        self
    }
}

/// Lifecycle status of an engineering assumption.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AssumptionStatus {
    #[default]
    Active,
    Tested,
    Confirmed,
    Invalidated,
}

impl AssumptionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Tested => "tested",
            Self::Confirmed => "confirmed",
            Self::Invalidated => "invalidated",
        }
    }

    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Active | Self::Confirmed)
    }
}

impl fmt::Display for AssumptionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for AssumptionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "tested" => Ok(Self::Tested),
            "confirmed" => Ok(Self::Confirmed),
            "invalidated" => Ok(Self::Invalidated),
            other => Err(format!("unknown assumption status: {}", other)),
        }
    }
}

/// Durable engineering assumption with epistemic lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineeringAssumption {
    pub id: Uuid,
    pub scope: MemoryScope,
    pub mission_id: MissionId,
    pub task_id: Option<TaskId>,
    pub statement: String,
    pub status: AssumptionStatus,
    pub evidence_summary: Option<String>,
    pub target_file: Option<String>,
    pub target_symbol: Option<String>,
    pub expected_hash: Option<String>,
    pub invalidated_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl EngineeringAssumption {
    pub fn new(mission_id: MissionId, statement: impl Into<String>, scope: MemoryScope) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::now_v7(),
            scope,
            mission_id,
            task_id: None,
            statement: statement.into(),
            status: AssumptionStatus::Active,
            evidence_summary: None,
            target_file: None,
            target_symbol: None,
            expected_hash: None,
            invalidated_by: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_task_id(mut self, task_id: TaskId) -> Self {
        self.task_id = Some(task_id);
        self
    }

    pub fn with_target_file(
        mut self,
        path: impl Into<String>,
        expected_hash: Option<String>,
    ) -> Self {
        self.target_file = Some(path.into());
        self.expected_hash = expected_hash;
        self
    }

    pub fn with_target_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.target_symbol = Some(symbol.into());
        self
    }
}

/// Lifecycle status of an attempted repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RepairStatus {
    #[default]
    Proposed,
    Applied,
    VerifiedSuccess,
    VerifiedFailure,
    Abandoned,
}

impl RepairStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Applied => "applied",
            Self::VerifiedSuccess => "verified_success",
            Self::VerifiedFailure => "verified_failure",
            Self::Abandoned => "abandoned",
        }
    }
}

impl fmt::Display for RepairStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for RepairStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "proposed" => Ok(Self::Proposed),
            "applied" => Ok(Self::Applied),
            "verified_success" => Ok(Self::VerifiedSuccess),
            "verified_failure" => Ok(Self::VerifiedFailure),
            "abandoned" => Ok(Self::Abandoned),
            other => Err(format!("unknown repair status: {}", other)),
        }
    }
}

/// Durable diagnosis of an engineering failure and its repair outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureDiagnosisRecord {
    pub id: Uuid,
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub failure_signature: String,
    pub failure_class: String,
    pub error_message: String,
    pub snapshot_hash: String,
    pub hypothesis: String,
    pub root_cause: String,
    pub recommended_action: String,
    pub repair_proposal_json: Option<String>,
    pub repair_status: RepairStatus,
    pub recurrence_count: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FailureDiagnosisRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mission_id: MissionId,
        task_id: TaskId,
        failure_signature: impl Into<String>,
        failure_class: impl Into<String>,
        error_message: impl Into<String>,
        snapshot_hash: impl Into<String>,
        hypothesis: impl Into<String>,
        root_cause: impl Into<String>,
        recommended_action: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::now_v7(),
            mission_id,
            task_id,
            failure_signature: failure_signature.into(),
            failure_class: failure_class.into(),
            error_message: error_message.into(),
            snapshot_hash: snapshot_hash.into(),
            hypothesis: hypothesis.into(),
            root_cause: root_cause.into(),
            recommended_action: recommended_action.into(),
            repair_proposal_json: None,
            repair_status: RepairStatus::Proposed,
            recurrence_count: 0,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_repair_proposal(mut self, proposal_json: impl Into<String>) -> Self {
        self.repair_proposal_json = Some(proposal_json.into());
        self
    }
}

/// Severity of an individual reviewer finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFindingSeverity {
    Info,
    Warning,
    Error,
    CriticalSecurity,
}

impl ReviewFindingSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::CriticalSecurity => "critical_security",
        }
    }
}

impl fmt::Display for ReviewFindingSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Status of an individual review finding across engineering turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReviewFindingStatus {
    #[default]
    Open,
    Addressed,
    Deferred,
    RejectedWithRationale,
    Escalated,
}

impl ReviewFindingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Addressed => "addressed",
            Self::Deferred => "deferred",
            Self::RejectedWithRationale => "rejected_with_rationale",
            Self::Escalated => "escalated",
        }
    }

    pub fn is_resolved(&self) -> bool {
        matches!(self, Self::Addressed | Self::RejectedWithRationale)
    }
}

impl fmt::Display for ReviewFindingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ReviewFindingStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "open" => Ok(Self::Open),
            "addressed" => Ok(Self::Addressed),
            "deferred" => Ok(Self::Deferred),
            "rejected_with_rationale" => Ok(Self::RejectedWithRationale),
            "escalated" => Ok(Self::Escalated),
            other => Err(format!("unknown review finding status: {}", other)),
        }
    }
}

/// Durable review finding preserving reviewer feedback across sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewFindingRecord {
    pub id: Uuid,
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub check_id: Option<CheckId>,
    pub file_path: String,
    pub line_start: Option<usize>,
    pub line_end: Option<usize>,
    pub severity: ReviewFindingSeverity,
    pub description: String,
    pub recommendation: String,
    pub status: ReviewFindingStatus,
    pub resolution_rationale: Option<String>,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl ReviewFindingRecord {
    pub fn new(
        mission_id: MissionId,
        task_id: TaskId,
        file_path: impl Into<String>,
        severity: ReviewFindingSeverity,
        description: impl Into<String>,
        recommendation: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            mission_id,
            task_id,
            check_id: None,
            file_path: file_path.into(),
            line_start: None,
            line_end: None,
            severity,
            description: description.into(),
            recommendation: recommendation.into(),
            status: ReviewFindingStatus::Open,
            resolution_rationale: None,
            resolved_by: None,
            resolved_at: None,
            created_at: Utc::now(),
        }
    }

    pub fn with_lines(mut self, start: usize, end: usize) -> Self {
        self.line_start = Some(start);
        self.line_end = Some(end);
        self
    }

    pub fn with_check_id(mut self, check_id: CheckId) -> Self {
        self.check_id = Some(check_id);
        self
    }
}

/// Validity condition of a prior verification check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VerificationValidity {
    #[default]
    Valid,
    StaleDueToDrift,
    InvalidatedByMutation,
}

impl VerificationValidity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::StaleDueToDrift => "stale_due_to_drift",
            Self::InvalidatedByMutation => "invalidated_by_mutation",
        }
    }

    pub fn is_authoritative(&self) -> bool {
        matches!(self, Self::Valid)
    }
}

impl fmt::Display for VerificationValidity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for VerificationValidity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "valid" => Ok(Self::Valid),
            "stale_due_to_drift" => Ok(Self::StaleDueToDrift),
            "invalidated_by_mutation" => Ok(Self::InvalidatedByMutation),
            other => Err(format!("unknown verification validity: {}", other)),
        }
    }
}

/// File and content-hash verified during a test run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileHashRecord {
    pub path: String,
    pub hash: String,
}

/// Revision-aware verification record linking requirements, checks, and code revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationRecord {
    pub id: Uuid,
    pub mission_id: MissionId,
    pub task_id: TaskId,
    pub requirement_key: String,
    pub check_id: CheckId,
    pub snapshot_hash: String,
    pub files_verified: Vec<FileHashRecord>,
    pub passed: bool,
    pub validity_status: VerificationValidity,
    pub invalidated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl VerificationRecord {
    pub fn new(
        mission_id: MissionId,
        task_id: TaskId,
        requirement_key: impl Into<String>,
        check_id: CheckId,
        snapshot_hash: impl Into<String>,
        passed: bool,
    ) -> Self {
        Self {
            id: Uuid::now_v7(),
            mission_id,
            task_id,
            requirement_key: requirement_key.into(),
            check_id,
            snapshot_hash: snapshot_hash.into(),
            files_verified: Vec::new(),
            passed,
            validity_status: VerificationValidity::Valid,
            invalidated_at: None,
            created_at: Utc::now(),
        }
    }

    pub fn with_files(mut self, files: Vec<FileHashRecord>) -> Self {
        self.files_verified = files;
        self
    }
}

/// Consolidated snapshot of relevant engineering knowledge for context compilation or session continuity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EngineeringMemorySnapshot {
    pub active_decisions: Vec<EngineeringDecision>,
    pub active_assumptions: Vec<EngineeringAssumption>,
    pub open_findings: Vec<ReviewFindingRecord>,
    pub recent_failures: Vec<FailureDiagnosisRecord>,
    pub verified_requirements: Vec<VerificationRecord>,
}

impl EngineeringMemorySnapshot {
    pub fn is_empty(&self) -> bool {
        self.active_decisions.is_empty()
            && self.active_assumptions.is_empty()
            && self.open_findings.is_empty()
            && self.recent_failures.is_empty()
            && self.verified_requirements.is_empty()
    }
}
