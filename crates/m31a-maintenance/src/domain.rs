//! Canonical Maintenance Data Model (`m31a-maintenance::domain`).
//!
//! Enforces:
//! 1. Versioned, serializable, immutable-by-default domain types.
//! 2. Strict lifecycle state machine for `MaintenanceCase` and `MaintenanceExecution`.
//! 3. Revision-bound `MaintenancePlan` with cryptographic content hashing.
//! 4. Typed multi-factor triage scoring (`TriageEvaluation`).
//! 5. Explicit autonomy tiers (0 to 3) and approval bindings.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use uuid::Uuid;

/// Source provider originating a maintenance signal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalProvider {
    GitHub,
    LocalAnalysis,
    VibeEngine,
    Scheduled,
    Custom(String),
}

impl SignalProvider {
    pub fn as_str(&self) -> &str {
        match self {
            Self::GitHub => "github",
            Self::LocalAnalysis => "local_analysis",
            Self::VibeEngine => "vibe_loop",
            Self::Scheduled => "scheduled",
            Self::Custom(s) => s.as_str(),
        }
    }
}

/// Category of maintenance work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceCategory {
    BrokenCi,
    DependencyUpdate,
    IssueBug,
    ApiMigration,
    DocDrift,
    SecurityAdvisory,
    ArchitectureDrift,
}

impl MaintenanceCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BrokenCi => "broken_ci",
            Self::DependencyUpdate => "dependency_update",
            Self::IssueBug => "issue_bug",
            Self::ApiMigration => "api_migration",
            Self::DocDrift => "doc_drift",
            Self::SecurityAdvisory => "security_advisory",
            Self::ArchitectureDrift => "architecture_drift",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "broken_ci" => Some(Self::BrokenCi),
            "dependency_update" => Some(Self::DependencyUpdate),
            "issue_bug" => Some(Self::IssueBug),
            "api_migration" => Some(Self::ApiMigration),
            "doc_drift" => Some(Self::DocDrift),
            "security_advisory" => Some(Self::SecurityAdvisory),
            "architecture_drift" => Some(Self::ArchitectureDrift),
            _ => None,
        }
    }
}

/// Severity classification of a signal or case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalSeverity {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl SignalSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

/// Trust and authenticity status for an ingested signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalTrustStatus {
    Verified,
    Unverified,
    Internal,
}

impl SignalTrustStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Unverified => "unverified",
            Self::Internal => "internal",
        }
    }
}

/// A normalized observation representing potential maintenance work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaintenanceSignal {
    pub id: String,
    pub repo_id: String,
    pub source_provider: SignalProvider,
    pub category: MaintenanceCategory,
    pub provider_event_id: Option<String>,
    pub external_url: Option<String>,
    pub event_timestamp_ms: i64,
    pub observed_revision: Option<String>,
    pub severity: SignalSeverity,
    pub dedup_fingerprint: String,
    pub trust_status: SignalTrustStatus,
    pub metadata: serde_json::Value,
    pub evidence_refs: Vec<String>,
    pub ingested_at_epoch_ms: i64,
}

impl MaintenanceSignal {
    /// Computes deterministic deduplication fingerprint for incoming signals.
    pub fn compute_dedup_fingerprint(
        repo_id: &str,
        category: MaintenanceCategory,
        provider_event_id: Option<&str>,
        observed_revision: Option<&str>,
        content_key: &str,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(repo_id.as_bytes());
        hasher.update(category.as_str().as_bytes());
        if let Some(event_id) = provider_event_id {
            hasher.update(b"event_id:");
            hasher.update(event_id.as_bytes());
        }
        if let Some(rev) = observed_revision {
            hasher.update(b"rev:");
            hasher.update(rev.as_bytes());
        }
        hasher.update(b"content:");
        hasher.update(content_key.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn new(
        repo_id: impl Into<String>,
        source_provider: SignalProvider,
        category: MaintenanceCategory,
        provider_event_id: Option<String>,
        external_url: Option<String>,
        event_timestamp_ms: i64,
        observed_revision: Option<String>,
        severity: SignalSeverity,
        content_key: &str,
        trust_status: SignalTrustStatus,
        metadata: serde_json::Value,
    ) -> Self {
        let repo_id = repo_id.into();
        let dedup_fingerprint = Self::compute_dedup_fingerprint(
            &repo_id,
            category,
            provider_event_id.as_deref(),
            observed_revision.as_deref(),
            content_key,
        );
        let id = format!("sig_{}", Uuid::now_v7());
        Self {
            id,
            repo_id,
            source_provider,
            category,
            provider_event_id,
            external_url,
            event_timestamp_ms,
            observed_revision,
            severity,
            dedup_fingerprint,
            trust_status,
            metadata,
            evidence_refs: Vec::new(),
            ingested_at_epoch_ms: Utc::now().timestamp_millis(),
        }
    }
}

/// Lifecycle status of a persistent maintenance case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseStatus {
    Open,
    Triaged,
    Investigating,
    Planning,
    PlanReady,
    AwaitingApproval,
    Executing,
    Verifying,
    ReviewPending,
    Resolved,
    Failed,
    Blocked,
    NeedsInformation,
    Cancelled,
}

impl CaseStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Triaged => "triaged",
            Self::Investigating => "investigating",
            Self::Planning => "planning",
            Self::PlanReady => "plan_ready",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Executing => "executing",
            Self::Verifying => "verifying",
            Self::ReviewPending => "review_pending",
            Self::Resolved => "resolved",
            Self::Failed => "failed",
            Self::Blocked => "blocked",
            Self::NeedsInformation => "needs_information",
            Self::Cancelled => "cancelled",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "open" => Some(Self::Open),
            "triaged" => Some(Self::Triaged),
            "investigating" => Some(Self::Investigating),
            "planning" => Some(Self::Planning),
            "plan_ready" => Some(Self::PlanReady),
            "awaiting_approval" => Some(Self::AwaitingApproval),
            "executing" => Some(Self::Executing),
            "verifying" => Some(Self::Verifying),
            "review_pending" => Some(Self::ReviewPending),
            "resolved" => Some(Self::Resolved),
            "failed" => Some(Self::Failed),
            "blocked" => Some(Self::Blocked),
            "needs_information" => Some(Self::NeedsInformation),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Resolved | Self::Failed | Self::Cancelled)
    }

    pub fn can_transition_to(&self, next: &CaseStatus) -> bool {
        if self == next {
            return true;
        }
        if self.is_terminal() {
            // Reopening a resolved or failed case on new regression evidence is allowed
            return matches!(self, Self::Resolved | Self::Failed)
                && matches!(next, Self::Open | Self::Investigating);
        }
        match self {
            Self::Open => matches!(
                next,
                Self::Triaged
                    | Self::Investigating
                    | Self::Blocked
                    | Self::NeedsInformation
                    | Self::Cancelled
            ),
            Self::Triaged => matches!(
                next,
                Self::Investigating
                    | Self::Planning
                    | Self::Blocked
                    | Self::NeedsInformation
                    | Self::Cancelled
            ),
            Self::Investigating => matches!(
                next,
                Self::Planning
                    | Self::PlanReady
                    | Self::Blocked
                    | Self::NeedsInformation
                    | Self::Failed
                    | Self::Cancelled
            ),
            Self::Planning => matches!(
                next,
                Self::PlanReady
                    | Self::AwaitingApproval
                    | Self::Blocked
                    | Self::NeedsInformation
                    | Self::Failed
                    | Self::Cancelled
            ),
            Self::PlanReady => matches!(
                next,
                Self::AwaitingApproval
                    | Self::Executing
                    | Self::Planning
                    | Self::Blocked
                    | Self::Cancelled
            ),
            Self::AwaitingApproval => matches!(
                next,
                Self::Executing
                    | Self::PlanReady
                    | Self::Blocked
                    | Self::Cancelled
                    | Self::Planning
            ),
            Self::Executing => matches!(
                next,
                Self::Verifying | Self::Planning | Self::Blocked | Self::Failed | Self::Cancelled
            ),
            Self::Verifying => matches!(
                next,
                Self::ReviewPending
                    | Self::Resolved
                    | Self::Executing
                    | Self::Planning
                    | Self::Failed
                    | Self::Blocked
                    | Self::Cancelled
            ),
            Self::ReviewPending => matches!(
                next,
                Self::Resolved
                    | Self::Executing
                    | Self::Planning
                    | Self::Blocked
                    | Self::Failed
                    | Self::Cancelled
            ),
            Self::Blocked => matches!(
                next,
                Self::Open
                    | Self::Triaged
                    | Self::Investigating
                    | Self::Planning
                    | Self::Executing
                    | Self::Cancelled
            ),
            Self::NeedsInformation => matches!(
                next,
                Self::Open | Self::Triaged | Self::Investigating | Self::Cancelled
            ),
            Self::Resolved | Self::Failed | Self::Cancelled => false,
        }
    }
}

/// Maintenance case priority ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaintenancePriority {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl MaintenancePriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

/// Human approval state for a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CaseApprovalState {
    #[default]
    None,
    Pending,
    Approved,
    Rejected,
}

impl CaseApprovalState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "none" => Some(Self::None),
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

/// Recorded attempt to remediate a maintenance issue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemediationAttempt {
    pub attempt_id: u32,
    pub timestamp_epoch_ms: i64,
    pub workflow_id: String,
    pub plan_id: String,
    pub strategy: String,
    pub success: bool,
    pub summary: String,
    pub manifest_hash: Option<String>,
}

/// Persistent unit of maintenance work surviving restarts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaintenanceCase {
    pub id: String,
    pub repo_id: String,
    pub category: MaintenanceCategory,
    pub title: String,
    pub problem_statement: String,
    pub dedup_key: String,
    pub status: CaseStatus,
    pub priority: MaintenancePriority,
    pub severity: SignalSeverity,
    pub confidence_score: f64,
    pub affected_packages: Vec<String>,
    pub affected_files: Vec<PathBuf>,
    pub affected_symbols: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub related_issues: Vec<String>,
    pub related_prs: Vec<String>,
    pub workflow_id: Option<String>,
    pub active_plan_id: Option<String>,
    pub remediation_history: Vec<RemediationAttempt>,
    pub human_approval_state: CaseApprovalState,
    pub created_at_epoch_ms: i64,
    pub updated_at_epoch_ms: i64,
    pub last_observed_at_epoch_ms: i64,
    pub resolved_at_epoch_ms: Option<i64>,
}

impl MaintenanceCase {
    pub fn new(
        repo_id: impl Into<String>,
        category: MaintenanceCategory,
        title: impl Into<String>,
        problem_statement: impl Into<String>,
        dedup_key: impl Into<String>,
        severity: SignalSeverity,
    ) -> Self {
        let now = Utc::now().timestamp_millis();
        let priority = match severity {
            SignalSeverity::Critical => MaintenancePriority::Critical,
            SignalSeverity::High => MaintenancePriority::High,
            SignalSeverity::Medium => MaintenancePriority::Medium,
            SignalSeverity::Low => MaintenancePriority::Low,
        };
        Self {
            id: format!("case_{}", Uuid::now_v7()),
            repo_id: repo_id.into(),
            category,
            title: title.into(),
            problem_statement: problem_statement.into(),
            dedup_key: dedup_key.into(),
            status: CaseStatus::Open,
            priority,
            severity,
            confidence_score: 0.5,
            affected_packages: Vec::new(),
            affected_files: Vec::new(),
            affected_symbols: Vec::new(),
            evidence_refs: Vec::new(),
            related_issues: Vec::new(),
            related_prs: Vec::new(),
            workflow_id: None,
            active_plan_id: None,
            remediation_history: Vec::new(),
            human_approval_state: CaseApprovalState::None,
            created_at_epoch_ms: now,
            updated_at_epoch_ms: now,
            last_observed_at_epoch_ms: now,
            resolved_at_epoch_ms: None,
        }
    }

    pub fn transition_to(&mut self, next_status: CaseStatus) -> anyhow::Result<()> {
        if !self.status.can_transition_to(&next_status) {
            anyhow::bail!(
                "Invalid case status transition from '{:?}' to '{:?}'",
                self.status,
                next_status
            );
        }
        self.status = next_status;
        self.updated_at_epoch_ms = Utc::now().timestamp_millis();
        if next_status == CaseStatus::Resolved {
            self.resolved_at_epoch_ms = Some(self.updated_at_epoch_ms);
        }
        Ok(())
    }
}

/// Root-cause hypothesis formulated during investigation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RootCauseHypothesis {
    pub id: String,
    pub description: String,
    pub confidence: f64,
    pub evidence_refs: Vec<String>,
    pub verified: bool,
}

/// Verifiable requirement incorporated into maintenance plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanVerificationRequirement {
    pub id: String,
    pub category: String,
    pub description: String,
    pub target_paths: Vec<PathBuf>,
    pub mandatory: bool,
}

/// Estimated risk level for maintenance plan modifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceRiskLevel {
    #[default]
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl MaintenanceRiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }
}

/// Explicit autonomy tiers governing maintenance execution authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum AutonomyTier {
    /// Tier 0: Collect signals, analyze health, create cases, no code modification, no remote write.
    #[default]
    Tier0Observe = 0,
    /// Tier 1: Plans, isolated worktrees, read-only reproduction, proposed patches, no remote publication.
    Tier1Prepare = 1,
    /// Tier 2: Guarded remediation, isolated worktrees, verified execution, draft PRs, approval for configured external actions.
    Tier2GuardedRemediation = 2,
    /// Tier 3: Restricted high-impact maintenance (major upgrades/migrations) requiring policy-bound human approval.
    Tier3RestrictedHighImpact = 3,
}

impl AutonomyTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tier0Observe => "tier0_observe",
            Self::Tier1Prepare => "tier1_prepare",
            Self::Tier2GuardedRemediation => "tier2_guarded_remediation",
            Self::Tier3RestrictedHighImpact => "tier3_restricted_high_impact",
        }
    }

    pub fn level(&self) -> u8 {
        *self as u8
    }

    pub fn from_u8(level: u8) -> Self {
        match level {
            0 => Self::Tier0Observe,
            1 => Self::Tier1Prepare,
            2 => Self::Tier2GuardedRemediation,
            _ => Self::Tier3RestrictedHighImpact,
        }
    }
}

/// Approval state for a maintenance plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PlanApprovalState {
    #[default]
    Pending,
    Approved,
    Rejected,
}

impl PlanApprovalState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

/// A typed, content-hashed maintenance plan bound to a specific repository baseline SHA.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaintenancePlan {
    pub id: String,
    pub case_id: String,
    pub repo_id: String,
    pub baseline_sha: String,
    pub category: MaintenanceCategory,
    pub problem_classification: String,
    pub selected_strategy: String,
    pub hypotheses: Vec<RootCauseHypothesis>,
    pub expected_files: Vec<PathBuf>,
    pub expected_packages: Vec<String>,
    pub relevant_tests: Vec<String>,
    pub verification_requirements: Vec<PlanVerificationRequirement>,
    pub estimated_risk: MaintenanceRiskLevel,
    pub required_autonomy_tier: AutonomyTier,
    pub requires_approval: bool,
    pub approval_state: PlanApprovalState,
    pub workflow_id: Option<String>,
    pub plan_hash: String,
    pub created_at_epoch_ms: i64,
    pub updated_at_epoch_ms: i64,
}

impl MaintenancePlan {
    /// Computes deterministic content hash over plan fields and baseline SHA.
    pub fn compute_plan_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.case_id.as_bytes());
        hasher.update(self.repo_id.as_bytes());
        hasher.update(self.baseline_sha.as_bytes());
        hasher.update(self.category.as_str().as_bytes());
        hasher.update(self.problem_classification.as_bytes());
        hasher.update(self.selected_strategy.as_bytes());
        hasher.update([self.required_autonomy_tier.level()]);
        for f in &self.expected_files {
            hasher.update(f.to_string_lossy().as_bytes());
        }
        for pkg in &self.expected_packages {
            hasher.update(pkg.as_bytes());
        }
        for t in &self.relevant_tests {
            hasher.update(t.as_bytes());
        }
        for req in &self.verification_requirements {
            hasher.update(req.id.as_bytes());
            hasher.update(req.category.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    pub fn new(
        case_id: impl Into<String>,
        repo_id: impl Into<String>,
        baseline_sha: impl Into<String>,
        category: MaintenanceCategory,
        problem_classification: impl Into<String>,
        selected_strategy: impl Into<String>,
        estimated_risk: MaintenanceRiskLevel,
        required_autonomy_tier: AutonomyTier,
        requires_approval: bool,
    ) -> Self {
        let now = Utc::now().timestamp_millis();
        let mut plan = Self {
            id: format!("plan_{}", Uuid::now_v7()),
            case_id: case_id.into(),
            repo_id: repo_id.into(),
            baseline_sha: baseline_sha.into(),
            category,
            problem_classification: problem_classification.into(),
            selected_strategy: selected_strategy.into(),
            hypotheses: Vec::new(),
            expected_files: Vec::new(),
            expected_packages: Vec::new(),
            relevant_tests: Vec::new(),
            verification_requirements: Vec::new(),
            estimated_risk,
            required_autonomy_tier,
            requires_approval,
            approval_state: if requires_approval {
                PlanApprovalState::Pending
            } else {
                PlanApprovalState::Approved
            },
            workflow_id: None,
            plan_hash: String::new(),
            created_at_epoch_ms: now,
            updated_at_epoch_ms: now,
        };
        plan.plan_hash = plan.compute_plan_hash();
        plan
    }
}

/// Lifecycle state for an active maintenance execution attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionLifecycleState {
    Created,
    Running,
    Verifying,
    Completed,
    Failed,
    Cancelled,
}

impl ExecutionLifecycleState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Running => "running",
            Self::Verifying => "verifying",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "created" => Some(Self::Created),
            "running" => Some(Self::Running),
            "verifying" => Some(Self::Verifying),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

/// Authoritative record of a maintenance workflow execution attempt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MaintenanceExecution {
    pub id: String,
    pub case_id: String,
    pub workflow_id: String,
    pub plan_id: String,
    pub manifest_hash: Option<String>,
    pub attempt_id: u32,
    pub worker_id: String,
    pub execution_state: ExecutionLifecycleState,
    pub verification_report_hash: Option<String>,
    pub promotion_state: String,
    pub pr_number: Option<u64>,
    pub pr_url: Option<String>,
    pub failure_classification: Option<String>,
    pub duration_ms: u64,
    pub tokens_used: u64,
    pub created_at_epoch_ms: i64,
    pub completed_at_epoch_ms: Option<i64>,
}

/// Recurring schedule configuration for autonomous maintenance checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MaintenanceSchedule {
    pub id: String,
    pub repo_id: String,
    pub job_kind: String,
    pub cron_or_interval_secs: i64,
    pub is_enabled: bool,
    pub last_run_epoch_ms: Option<i64>,
    pub next_run_epoch_ms: i64,
    pub consecutive_failures: u32,
    pub policy_override: Option<serde_json::Value>,
    pub created_at_epoch_ms: i64,
    pub updated_at_epoch_ms: i64,
}

/// Detailed multi-factor triage evaluation result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TriageEvaluation {
    pub case_id: String,
    pub severity: SignalSeverity,
    pub priority: MaintenancePriority,
    pub confidence_score: f64,
    pub operational_impact_score: f64,
    pub blast_radius_score: f64,
    pub reproducibility_score: f64,
    pub verification_feasibility_score: f64,
    pub human_intervention_required: bool,
    pub recommended_autonomy_tier: AutonomyTier,
    pub contributing_factors: Vec<String>,
    pub rationale: String,
}

/// Pull request delivery policy settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullRequestPolicy {
    pub open_draft_pr: bool,
    pub branch_prefix: String,
    pub require_review: bool,
    pub auto_label: Vec<String>,
}

impl Default for PullRequestPolicy {
    fn default() -> Self {
        Self {
            open_draft_pr: true,
            branch_prefix: "m31a-maint/".to_string(),
            require_review: true,
            auto_label: vec!["autonomous-maintenance".to_string()],
        }
    }
}

/// Versioned repository maintenance policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MaintenancePolicy {
    pub version: String,
    pub enabled: bool,
    pub autonomy_tier: AutonomyTier,
    pub allowed_categories: Vec<MaintenanceCategory>,
    pub allowed_branches: Vec<String>,
    pub excluded_paths: Vec<String>,
    pub risk_threshold_for_approval: MaintenanceRiskLevel,
    pub max_concurrency_per_repo: usize,
    pub max_retries_per_case: u32,
    pub max_replans_per_case: u32,
    pub timeout_per_run_secs: u64,
    pub token_budget_per_run: u64,
    pub pull_request_policy: PullRequestPolicy,
}

impl Default for MaintenancePolicy {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            enabled: true,
            autonomy_tier: AutonomyTier::Tier1Prepare,
            allowed_categories: vec![
                MaintenanceCategory::BrokenCi,
                MaintenanceCategory::DependencyUpdate,
                MaintenanceCategory::IssueBug,
                MaintenanceCategory::ApiMigration,
                MaintenanceCategory::DocDrift,
                MaintenanceCategory::SecurityAdvisory,
                MaintenanceCategory::ArchitectureDrift,
            ],
            allowed_branches: vec!["master".to_string(), "main".to_string()],
            excluded_paths: vec![
                ".git/**".to_string(),
                ".github/workflows/security.yml".to_string(),
            ],
            risk_threshold_for_approval: MaintenanceRiskLevel::Medium,
            max_concurrency_per_repo: 2,
            max_retries_per_case: 3,
            max_replans_per_case: 2,
            timeout_per_run_secs: 1800,
            token_budget_per_run: 500_000,
            pull_request_policy: PullRequestPolicy::default(),
        }
    }
}

/// Structured tamper-evident audit trail event for maintenance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditTrailEvent {
    pub id: String,
    pub case_id: Option<String>,
    pub repo_id: String,
    pub event_type: String,
    pub actor: String,
    pub policy_version: String,
    pub details: serde_json::Value,
    pub timestamp_epoch_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_case_lifecycle_state_machine() {
        let mut case = MaintenanceCase::new(
            "repo_1",
            MaintenanceCategory::BrokenCi,
            "Broken build on commit abc",
            "Compilation failure in module foo",
            "ci_run_123",
            SignalSeverity::High,
        );

        assert_eq!(case.status, CaseStatus::Open);
        assert!(case.transition_to(CaseStatus::Triaged).is_ok());
        assert!(case.transition_to(CaseStatus::Investigating).is_ok());
        assert!(case.transition_to(CaseStatus::Planning).is_ok());
        assert!(case.transition_to(CaseStatus::PlanReady).is_ok());
        assert!(case.transition_to(CaseStatus::Executing).is_ok());
        assert!(case.transition_to(CaseStatus::Verifying).is_ok());
        assert!(case.transition_to(CaseStatus::Resolved).is_ok());
        assert!(case.resolved_at_epoch_ms.is_some());

        // Terminal state cannot transition to unrelated states without reopening
        assert!(case.transition_to(CaseStatus::Executing).is_err());
        assert!(case.transition_to(CaseStatus::Investigating).is_ok()); // Reopening allowed
    }

    #[test]
    fn test_plan_hash_deterministic() {
        let plan1 = MaintenancePlan::new(
            "case_123",
            "repo_1",
            "abcdef123456",
            MaintenanceCategory::DependencyUpdate,
            "Minor patch update for serde",
            "dependency_patch_update",
            MaintenanceRiskLevel::Low,
            AutonomyTier::Tier2GuardedRemediation,
            false,
        );

        let plan2 = MaintenancePlan::new(
            "case_123",
            "repo_1",
            "abcdef123456",
            MaintenanceCategory::DependencyUpdate,
            "Minor patch update for serde",
            "dependency_patch_update",
            MaintenanceRiskLevel::Low,
            AutonomyTier::Tier2GuardedRemediation,
            false,
        );

        assert_eq!(plan1.plan_hash, plan2.plan_hash);
        assert_eq!(plan1.plan_hash.len(), 64);
    }

    #[test]
    fn test_signal_dedup_fingerprint() {
        let fp1 = MaintenanceSignal::compute_dedup_fingerprint(
            "repo_1",
            MaintenanceCategory::BrokenCi,
            Some("run_999"),
            Some("commit_abc"),
            "cargo test failure in test_foo",
        );

        let fp2 = MaintenanceSignal::compute_dedup_fingerprint(
            "repo_1",
            MaintenanceCategory::BrokenCi,
            Some("run_999"),
            Some("commit_abc"),
            "cargo test failure in test_foo",
        );

        assert_eq!(fp1, fp2);
    }
}
