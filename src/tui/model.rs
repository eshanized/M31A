//! In-Memory TUI Projection ViewModel (TUI-03, D-15).
//!
//! Pure in-memory projection of runtime state. Never executes SQLite queries
//! or synchronous I/O during rendering.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::events::envelope::EventEnvelope;
use crate::events::types::EventType;
use crate::interaction::events::InteractionEvent;
use crate::interaction::session::ConversationTurn;
use crate::tui::conversation::TuiConversationItem;
use crate::tui::icons::{IconRegistry, Spinner};
use crate::tui::lifecycle::{TuiLifecycleProjection, TuiLifecycleStage};

/// Activity kind for live working/thinking indicators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ActivityKind {
    #[default]
    Idle,
    Thinking,
    Discovering,
    Planning,
    WaitingForReview,
    Executing,
    RunningTool,
    Verifying,
    Recovering,
    WaitingForApproval,
    Completed,
    Failed,
    Cancelled,
}

/// Explicit TUI runtime startup state.
///
/// The cockpit must never hide behind runtime initialization. The terminal
/// guard is acquired, a valid `TuiApplication` is created, and a startup frame is
/// rendered BEFORE expensive runtime assembly completes. Assembly then
/// proceeds asynchronously and the cockpit transitions explicitly:
///
/// `Booting -> InitializingRuntime -> HydratingSession -> HydratingWorkspace
/// -> HydratingExecution -> Ready | Failed`
///
/// `Ready` is the normal operational state. Every other state must still
/// render the full shell (header / workspace / composer / footer) with an
/// explicit startup panel — never a blank frame. Partial data is valid: the
/// UI never requires all data before it can display itself.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RuntimeStartupState {
    /// Initial `TuiApplication` constructed, no runtime work started yet.
    Booting,
    /// Canonical `AppRuntime` assembly in flight (async, off the first-frame path).
    InitializingRuntime,
    /// Hydrating durable session truth (conversation, lifecycle resume).
    HydratingSession,
    /// Hydrating workspace truth (artifacts, git, skills, catalog).
    HydratingWorkspace,
    /// Hydrating execution truth (tasks, verification, usage, telemetry).
    HydratingExecution,
    /// Normal operational cockpit.
    #[default]
    Ready,
    /// Runtime or bridge initialization failed. Carries a human-readable
    /// reason that must be rendered visibly (never a blank screen).
    Failed(String),
}

impl RuntimeStartupState {
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    pub fn is_startup(&self) -> bool {
        matches!(
            self,
            Self::Booting
                | Self::InitializingRuntime
                | Self::HydratingSession
                | Self::HydratingWorkspace
                | Self::HydratingExecution
        )
    }

    pub fn label(&self) -> String {
        match self {
            Self::Booting => "Booting".to_string(),
            Self::InitializingRuntime => "Initializing runtime…".to_string(),
            Self::HydratingSession => "Loading session state…".to_string(),
            Self::HydratingWorkspace => "Loading workspace state…".to_string(),
            Self::HydratingExecution => "Loading execution state…".to_string(),
            Self::Ready => "Ready".to_string(),
            Self::Failed(reason) => format!("Startup failed: {reason}"),
        }
    }

    pub fn failure_reason(&self) -> Option<&str> {
        match self {
            Self::Failed(reason) => Some(reason),
            _ => None,
        }
    }
}

/// Presentation-level session view mode separating Welcome onboarding from
/// the Active working session (§2, §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SessionViewMode {
    /// Shown before the first meaningful user interaction. Centered identity,
    /// centered composer, minimal chrome, no telemetry, no task dashboard, no rail.
    #[default]
    Welcome,
    /// Active interactive session workspace with continuous conversation,
    /// persistent bottom composer, contextual right rail, and quiet status bar.
    Active,
    /// Explicitly closed/ended session.
    Closed,
}

impl SessionViewMode {
    pub fn is_welcome(&self) -> bool {
        matches!(self, Self::Welcome)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }

    pub fn is_closed(&self) -> bool {
        matches!(self, Self::Closed)
    }
}

/// Single semantic presentation state for "what is M31A doing right now?"
///
/// This is the ONE authoritative helper every renderer (header, inline
/// activity line, composer, footer) must derive UI state from. It separates
/// SESSION state (alive/closed) from OPERATION state (in-flight work): a
/// session being `active` is never, by itself, work being executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiOperationState {
    /// Nothing in flight. Fresh sessions and settled requests render Ready.
    Idle,
    /// A slash command was dispatched and its result has not arrived yet.
    /// Fast commands normally skip this entirely (no visible activity).
    CommandRunning {
        command: String,
    },
    /// Genuine model/lifecycle work is in flight.
    Thinking,
    Planning,
    Executing,
    RunningTool,
    Verifying,
    Recovering,
    /// The runtime waits for operator input (discovery answers).
    WaitingForUser,
    /// Backwards compatibility alias for WaitingForUser.
    AwaitingInput,
    /// The runtime waits for an explicit operator decision
    /// (plan/task review, authorization, tool approval).
    WaitingForApproval,
    /// Backwards compatibility alias for WaitingForApproval.
    AwaitingApproval,
    /// The last operation completed. Renders quiet (Ready), not Working.
    Completed,
    /// The last operation failed. Renders Failed only via lifecycle truth;
    /// otherwise quiet — the error card in the conversation carries detail.
    Failed,
    Cancelled,
}

impl UiOperationState {
    /// True only when real in-flight work justifies a Working indicator.
    pub fn is_working(&self) -> bool {
        matches!(
            self,
            Self::CommandRunning { .. }
                | Self::Thinking
                | Self::Planning
                | Self::Executing
                | Self::RunningTool
                | Self::Verifying
                | Self::Recovering
        )
    }

    /// True when the operator must act (input or approval gates).
    pub fn is_waiting(&self) -> bool {
        matches!(
            self,
            Self::WaitingForUser
                | Self::AwaitingInput
                | Self::WaitingForApproval
                | Self::AwaitingApproval
        )
    }
}

impl ActivityKind {
    /// Get the semantic icon key for this activity kind.
    pub fn icon_key(&self) -> crate::tui::icons::IconKey {
        match self {
            Self::Idle => crate::tui::icons::IconKey::Idle,
            Self::Thinking => crate::tui::icons::IconKey::Thinking,
            Self::Discovering => crate::tui::icons::IconKey::Discovering,
            Self::Planning => crate::tui::icons::IconKey::Planning,
            Self::WaitingForReview => crate::tui::icons::IconKey::WaitingForReview,
            Self::Executing => crate::tui::icons::IconKey::Executing,
            Self::RunningTool => crate::tui::icons::IconKey::RunningTool,
            Self::Verifying => crate::tui::icons::IconKey::Verifying,
            Self::Recovering => crate::tui::icons::IconKey::Recovering,
            Self::WaitingForApproval => crate::tui::icons::IconKey::WaitingForApproval,
            Self::Completed => crate::tui::icons::IconKey::Completed,
            Self::Failed => crate::tui::icons::IconKey::Failed,
            Self::Cancelled => crate::tui::icons::IconKey::Cancelled,
        }
    }
}

/// Snapshot of an individual task in the DAG.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiTaskSnapshot {
    pub id: String,
    pub title: String,
    pub status: String,
    pub agent_role: Option<String>,
    pub progress_pct: u8,
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub assigned_agent_id: Option<String>,
    #[serde(default)]
    pub execution_result: Option<String>,
    #[serde(default)]
    pub failure_reason: Option<String>,
    #[serde(default)]
    pub retry_count: usize,
}

impl Default for TuiTaskSnapshot {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            status: "pending".to_string(),
            agent_role: None,
            progress_pct: 0,
            dependencies: Vec::new(),
            assigned_agent_id: None,
            execution_result: None,
            failure_reason: None,
            retry_count: 0,
        }
    }
}

impl TuiTaskSnapshot {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        status: impl Into<String>,
        agent_role: Option<String>,
        progress_pct: u8,
        dependencies: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status: status.into(),
            agent_role,
            progress_pct,
            dependencies,
            assigned_agent_id: None,
            execution_result: None,
            failure_reason: None,
            retry_count: 0,
        }
    }
}

/// Snapshot of an active agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TuiAgentSnapshot {
    pub id: String,
    pub role: String,
    pub state: String,
    pub current_task: Option<String>,
    pub total_tokens: u64,
}

/// Snapshot of tool execution telemetry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiToolSnapshot {
    pub name: String,
    pub executions_count: usize,
    pub errors_count: usize,
}

/// Execution state of a live tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiveToolState {
    Running,
    Completed,
    Failed,
}

/// Real-time live tool operation telemetry for in-flight display.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LiveToolOperation {
    pub call_id: String,
    pub tool_name: String,
    pub parameters: String,
    pub state: LiveToolState,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub output_preview: Option<String>,
}

/// Snapshot of job execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiJobSnapshot {
    pub id: String,
    pub name: String,
    pub status: String,
    pub duration_ms: u64,
}

/// Individual log entry displayed in TUI log pane.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiLogLine {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
    pub source: String,
}

/// Pending interactive approval request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiApprovalRequest {
    pub id: String,
    pub tool_name: String,
    pub agent_role: String,
    pub justification: String,
    pub parameters_summary: String,
    pub risk_tier: String,
    pub timestamp: DateTime<Utc>,
}

/// Projection state of the Task Execution DAG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TaskGraphProjectionState {
    #[default]
    Unknown,
    Loading,
    Loaded,
}

/// Model usage telemetry.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiModelUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_cost_cents: Option<u64>,
    pub api_calls: u64,
    #[serde(default)]
    pub in_flight_prompt_tokens: u64,
    #[serde(default)]
    pub in_flight_completion_tokens: u64,
    #[serde(skip)]
    pub processed_invocations: std::collections::HashSet<String>,
    #[serde(skip)]
    pub priced_invocations: std::collections::HashSet<String>,
}

impl TuiModelUsage {
    pub fn effective_prompt_tokens(&self) -> u64 {
        self.prompt_tokens + self.in_flight_prompt_tokens
    }

    pub fn effective_completion_tokens(&self) -> u64 {
        self.completion_tokens + self.in_flight_completion_tokens
    }

    pub fn effective_total_tokens(&self) -> u64 {
        self.effective_prompt_tokens() + self.effective_completion_tokens()
    }
}

/// System health and performance telemetry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiSystemStats {
    pub uptime_secs: u64,
    pub events_processed: u64,
    pub memory_rss_mb: u64,
}

impl Default for TuiSystemStats {
    fn default() -> Self {
        Self {
            uptime_secs: 0,
            events_processed: 0,
            memory_rss_mb: 64,
        }
    }
}

/// Snapshot of a canonical artifact record (P0, ART-01, ART-02).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TuiArtifactSnapshot {
    pub id: String,
    pub name: String,
    pub logical_path: String,
    pub content_hash: String,
    pub size_bytes: u64,
    pub extension: String,
    pub version: u32,
    pub status: String,
    pub mission_id: Option<String>,
    pub task_id: Option<String>,
    pub producer_role: Option<String>,
    pub parent_artifact_ids: Vec<String>,
    pub verification_check_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub storage_state: String,
    #[serde(default)]
    pub quarantine_reason: Option<String>,
}

impl From<&crate::persistence::artifacts::ArtifactRecord> for TuiArtifactSnapshot {
    fn from(r: &crate::persistence::artifacts::ArtifactRecord) -> Self {
        Self {
            id: r.id.to_string(),
            name: r.name.clone(),
            logical_path: r.logical_path.display().to_string(),
            content_hash: r.content_hash.clone(),
            size_bytes: r.size_bytes,
            extension: r.extension.clone(),
            version: r.version,
            status: r.status.to_string(),
            mission_id: r.provenance.mission_id.map(|m| m.to_string()),
            task_id: r.provenance.task_id.map(|t| t.to_string()),
            producer_role: r.provenance.producer_role.clone(),
            parent_artifact_ids: r
                .provenance
                .parent_artifact_ids
                .iter()
                .map(|id| id.to_string())
                .collect(),
            verification_check_ids: r
                .provenance
                .verification_check_ids
                .iter()
                .map(|id| id.to_string())
                .collect(),
            created_at: r.created_at,
            storage_state: "stored".to_string(),
            quarantine_reason: r.provenance.quarantine_reason.clone(),
        }
    }
}

/// Snapshot of an individual verification check (P0, VER-01, VER-04, Law 6).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiVerificationCheck {
    pub check_id: String,
    pub mission_id: String,
    pub task_id: String,
    pub tier_num: u8,
    pub tier_name: String,
    pub status: String, // "passed", "failed", "blocked", "not_run", "skipped_with_reason", "pending", "unknown"
    pub command_or_tool: String,
    pub inputs_normalized: String,
    pub evidence_artifact_id: Option<String>,
    pub summary: String,
    pub failure_class: Option<String>,
    pub snapshot_hash: String,
    pub created_at: DateTime<Utc>,
}

impl From<&crate::verification::types::VerificationCheck> for TuiVerificationCheck {
    fn from(c: &crate::verification::types::VerificationCheck) -> Self {
        Self {
            check_id: c.check_id.to_string(),
            mission_id: c.mission_id.to_string(),
            task_id: c.task_id.to_string(),
            tier_num: c.tier.as_u8(),
            tier_name: c.tier.name().to_string(),
            status: c.status.as_str().to_string(),
            command_or_tool: c.command_or_tool.clone(),
            inputs_normalized: c.inputs_normalized.clone(),
            evidence_artifact_id: c.evidence_artifact_id.map(|a| a.to_string()),
            summary: c.summary.clone(),
            failure_class: c.failure_class.clone(),
            snapshot_hash: c.snapshot_hash.clone(),
            created_at: c.created_at,
        }
    }
}

/// Verification summary metrics across active checks.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiVerificationSummary {
    pub total_checks: usize,
    pub passed_count: usize,
    pub failed_count: usize,
    pub blocked_count: usize,
    pub skipped_count: usize,
    pub not_run_count: usize,
    pub overall_status: String,
    pub last_evaluated: Option<DateTime<Utc>>,
}

/// Snapshot of a failure recovery attempt (P1, FLC-05, D-06).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiRecoverySnapshot {
    pub attempt_id: String,
    pub failure_class: String,
    pub strategy: String,
    pub attempt_number: usize,
    pub checkpoint_id: Option<String>,
    pub affected_mission_id: Option<String>,
    pub affected_task_id: Option<String>,
    pub outcome: String,
    pub budget_consumed: usize,
    pub backoff_delay_ms: u64,
    pub action_taken: String,
}

impl From<&crate::recovery::budget::RecoveryAttemptRecord> for TuiRecoverySnapshot {
    fn from(r: &crate::recovery::budget::RecoveryAttemptRecord) -> Self {
        Self {
            attempt_id: format!("{}-{}-{}", r.task_id, r.strategy, r.attempt_number),
            failure_class: format!("{:?}", r.failure_class),
            strategy: r.strategy.clone(),
            attempt_number: r.attempt_number,
            checkpoint_id: None,
            affected_mission_id: Some(r.mission_id.to_string()),
            affected_task_id: Some(r.task_id.to_string()),
            outcome: r.result.clone(),
            budget_consumed: r.budget_consumed,
            backoff_delay_ms: r.backoff_delay_ms,
            action_taken: r.action_taken.clone(),
        }
    }
}

/// Status of an individual doctor probe evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoctorStatus {
    Pass,
    Fail,
    Warning,
    Unverified,
    Unavailable,
    NotRun,
}

impl DoctorStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Warning => "WARN",
            Self::Unverified => "UNVERIFIED",
            Self::Unavailable => "UNAVAILABLE",
            Self::NotRun => "NOT RUN",
        }
    }
}

/// Truthful diagnostic check record for the Doctor surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiDoctorCheck {
    pub category: String,
    pub name: String,
    pub status: DoctorStatus,
    pub detail: String,
    pub remediation: Option<String>,
    pub evaluated_at: Option<DateTime<Utc>>,
}

/// State of quarantine containment data projection.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum QuarantineProjectionState {
    #[default]
    Loading,
    Ready,
    Failed(String),
}

/// A containment breach / quarantine record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiQuarantineItem {
    pub id: String,
    pub name: String,
    pub reason: String,
    pub quarantined_at: Option<DateTime<Utc>>,
}

/// Canonical helper to construct a `TuiQuarantineItem` with authoritative reason or fail-closed fallback (AUD-17).
pub fn map_quarantine_item(
    id: impl Into<String>,
    name: impl Into<String>,
    reason: Option<String>,
    quarantined_at: Option<DateTime<Utc>>,
) -> TuiQuarantineItem {
    let reason_str = reason
        .filter(|r| !r.trim().is_empty())
        .unwrap_or_else(|| "Reason unavailable".to_string());
    TuiQuarantineItem {
        id: id.into(),
        name: name.into(),
        reason: reason_str,
        quarantined_at,
    }
}

impl From<crate::interaction::events::QuarantineItemRecord> for TuiQuarantineItem {
    fn from(r: crate::interaction::events::QuarantineItemRecord) -> Self {
        map_quarantine_item(r.id, r.name, Some(r.reason), r.quarantined_at)
    }
}

/// Typed lifecycle state for independently hydrated data sections (M31A-UI-09, M31A-UI-22).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SectionProjectionState {
    #[default]
    Loading,
    Ready,
    Stale(String),
    Failed(String),
}

/// Masked credential status safe for display without secrets (M31A-UI-05).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TuiCredentialStatus {
    #[default]
    Loading,
    Configured,
    NotConfigured,
    Unavailable(String),
}

impl TuiCredentialStatus {
    pub fn is_configured(&self) -> bool {
        matches!(self, Self::Configured)
    }
}

/// Authoritative sandbox and execution isolation status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TuiSandboxStatus {
    pub isolation_active: bool,
    pub quotas_enforced: bool,
    pub watchdog_active: bool,
    pub mode: String,
    pub details: Option<String>,
    #[serde(default)]
    pub network_restricted: bool,
    #[serde(default)]
    pub enforcement: Option<crate::sandbox::enforcement::ProcessEnforcementRecord>,
}

/// Snapshot of a discovered/available skill (P2, SKL-01, SKL-03).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiSkillSnapshot {
    pub id: String,
    pub name: String,
    pub description: String,
    pub origin_tier: String,
    pub capabilities: Vec<String>,
    pub verification_tier: String,
    pub risk_tier: String,
    pub status: String,
}

impl From<&crate::skill::discovery::SkillPackage> for TuiSkillSnapshot {
    fn from(p: &crate::skill::discovery::SkillPackage) -> Self {
        Self {
            id: p.manifest.id.clone(),
            name: p.manifest.name.clone(),
            description: p.manifest.description.clone(),
            origin_tier: p.origin.to_string(),
            capabilities: p.manifest.required_capabilities.clone(),
            verification_tier: p.manifest.verification.tier.to_string(),
            risk_tier: p.manifest.risk_profile.level.to_string(),
            status: "available".to_string(),
        }
    }
}

/// Snapshot of a canonical tool definition from ToolRegistry (P2, TL-01, D-05).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiToolDefinitionSnapshot {
    pub name: String,
    pub description: String,
    pub capability_requirements: Vec<String>,
    pub risk_level: String,
    pub is_read_only: bool,
    pub available: bool,
}

impl From<&dyn crate::tools::definition::AnyTool> for TuiToolDefinitionSnapshot {
    fn from(t: &dyn crate::tools::definition::AnyTool) -> Self {
        Self {
            name: t.id().to_string(),
            description: t.description().to_string(),
            capability_requirements: t
                .required_capabilities()
                .iter()
                .map(|c| c.to_string())
                .collect(),
            risk_level: format!("{:?}", t.base_risk()),
            is_read_only: t.base_risk() == crate::tools::risk::RiskClass::ReadOnly,
            available: true,
        }
    }
}

/// Canonical mission timeline entry (P1, EVT-01, EVT-02).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiTimelineEntry {
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub category: String,
    pub title: String,
    pub details: String,
    pub level: String,
}

/// End-to-end 7-step traceability chain (P0).
/// user decision → plan revision → task revision → execution/action → source change → verification → evidence
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiTraceabilityChain {
    pub user_decision: Option<String>,
    pub plan_revision: Option<u32>,
    pub plan_hash: Option<String>,
    pub task_revision: Option<u32>,
    pub task_id: Option<String>,
    pub execution_id: Option<String>,
    pub source_change: Option<String>,
    pub verification_id: Option<String>,
    pub verification_status: Option<String>,
    pub evidence_artifact_id: Option<String>,
    pub checkpoint_id: Option<String>,
}

/// Telemetry heartbeat for tracking execution progress and detecting stalls.
#[derive(Debug, Clone, Default)]
pub struct ExecutionHeartbeat {
    pub execution_started_at: Option<DateTime<Utc>>,
    pub last_runtime_event_at: Option<DateTime<Utc>>,
    pub last_task_event_at: Option<DateTime<Utc>>,
    pub last_model_event_at: Option<DateTime<Utc>>,
    pub last_tool_event_at: Option<DateTime<Utc>>,
    pub active_task_id: Option<String>,
    pub active_agent_id: Option<String>,
    pub active_tool_id: Option<String>,
    pub current_activity: Option<String>,
    pub current_activity_started_at: Option<DateTime<Utc>>,
}

/// Pure in-memory projection model for the TUI cockpit.
#[derive(Debug, Clone)]
pub struct TuiViewModel {
    pub mission_id: Option<String>,
    pub mission_name: String,
    pub mission_status: String,
    pub objective: String,
    pub tasks: Vec<TuiTaskSnapshot>,
    pub agents: Vec<TuiAgentSnapshot>,
    pub tools: Vec<TuiToolSnapshot>,
    pub jobs: Vec<TuiJobSnapshot>,
    pub logs: VecDeque<TuiLogLine>,
    pub approvals: Vec<TuiApprovalRequest>,
    pub model_usage: TuiModelUsage,
    pub system_stats: TuiSystemStats,
    pub is_dirty: bool,
    pub max_logs: usize,
    /// Atomic counter tracking any forbidden SQLite operations during render.
    pub sqlite_access_counter: Arc<AtomicUsize>,

    // --- Interactive Session & Cockpit Extensions ---
    pub session_id: Option<String>,
    pub session_status: String,
    pub session_view_mode: SessionViewMode,
    pub active_model: String,
    pub active_provider: String,
    pub active_profile: String,
    pub workspace_path: String,
    pub conversation: Vec<TuiConversationItem>,
    pub live_activity: Option<String>,
    pub live_tools: Vec<LiveToolOperation>,
    pub active_stream_message_id: Option<String>,
    pub catalog_models: Vec<crate::model::router::resolver::ModelCandidate>,
    pub git_diff_text: Option<String>,
    pub git_branch: String,
    pub execution_worktree_branch: Option<String>,
    pub task_graph_state: TaskGraphProjectionState,
    pub active_graph_id: Option<String>,
    pub active_graph_revision: u32,
    pub scroll_offset: usize,
    /// Authoritative conversation viewport state (one per TUI, §15).
    ///
    /// `scroll_offset` counts lines from the BOTTOM (0 = pinned to bottom).
    /// `follow` is true while the viewport tracks the newest message.
    /// `unseen_count` counts conversation items that arrived while the
    /// operator was reading older content (`follow == false`).
    /// `viewport_height` / `content_height` are measured in rendered lines
    /// at the last frame; `last_total_lines` lets the renderer preserve the
    /// viewport when new lines arrive while `follow == false`.
    pub follow: bool,
    pub unseen_count: usize,
    pub viewport_height: usize,
    pub content_height: usize,
    pub(crate) last_total_lines: usize,
    /// Request correlation: the in-flight user submission, if any.
    ///
    /// Stamped at submit time (`begin_request`), settled by the terminal
    /// interaction event for that request (`settle_request`). The bridge
    /// processes actions sequentially, so terminal events settle the request
    /// they belong to; a stale activity from request N can never be mistaken
    /// for request N+1 because every terminal event settles unconditionally.
    /// Durable runtime IDs (session/mission/plan/task) are untouched.
    pub active_request_id: Option<String>,
    pub last_settled_request_id: Option<String>,
    /// The slash command currently awaiting its result, if any. Fast
    /// commands leave this `None` (no visible activity for trivial work).
    pub active_command: Option<String>,
    /// Governed lifecycle projection. Presentation only; the
    /// runtime owns lifecycle truth. Updated from trusted runtime events.
    pub lifecycle: TuiLifecycleProjection,

    /// Semantic icon registry (Nerd Font / Unicode / ASCII).
    pub icons: IconRegistry,

    /// Animated activity spinner driven by TUI ticks.
    pub spinner: Spinner,

    /// Current activity kind (thinking, executing, verifying, etc.).
    pub activity_kind: ActivityKind,
    /// Human-readable activity message.
    pub activity_message: Option<String>,
    /// When the current activity started (for duration display).
    pub activity_started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Active model invocation currently being projected into the thinking UI.
    pub live_model_invocation_id: Option<String>,
    pub live_model_provider: Option<String>,
    pub live_model_name: Option<String>,
    /// Character/token heuristic only; rendered with a tilde until provider usage arrives.
    pub live_model_reasoning_tokens_estimated: Option<u64>,
    /// Provider usage observed while streaming (never inferred from model names).
    pub live_model_usage: Option<crate::model::types::TokenUsage>,
    /// Bounded tombstones prevent delayed stream telemetry from reviving a completed turn.
    settled_model_invocations: VecDeque<String>,

    // --- Evidence Plane & Observability Extensions ---
    pub artifacts: Vec<TuiArtifactSnapshot>,
    pub verification_checks: Vec<TuiVerificationCheck>,
    pub verification_summary: TuiVerificationSummary,
    pub recovery_attempts: Vec<TuiRecoverySnapshot>,
    pub skills: Vec<TuiSkillSnapshot>,
    pub canonical_tools: Vec<TuiToolDefinitionSnapshot>,
    pub timeline: VecDeque<TuiTimelineEntry>,
    pub max_timeline: usize,
    pub traceability: Vec<TuiTraceabilityChain>,
    /// Heartbeat metrics tracking execution activity and detecting stalls.
    pub heartbeat: ExecutionHeartbeat,
    /// Deduplication set of seen event envelope IDs.
    pub seen_events: std::collections::HashSet<crate::ids::event::EventId>,
    /// Explicit runtime startup state.
    ///
    /// Rendered through the normal shell: header, workspace startup panel,
    /// composer, and footer all remain visible while the runtime initializes.
    /// Defaults to `Ready` so existing projections (tests, headless consumers)
    /// keep their established rendering; production TUI startup sets this to
    /// `InitializingRuntime` BEFORE the first frame.
    pub runtime_status: RuntimeStartupState,
    /// Human-readable startup detail shown in the startup panel (e.g. current
    /// init step). Pure presentation state, never authoritative.
    pub runtime_status_detail: Option<String>,
    /// Typed classification of the current failure, if any (Principle 11).
    /// Kept separate from the `Failed(String)` message so existing callers
    /// keep working while new code can report Runtime/Bridge/Hydration/Model
    /// /Workspace errors explicitly.
    pub runtime_error_kind: Option<crate::tui::errors::TuiErrorKind>,
    /// Duration recorded per startup stage for diagnostic tracking.
    pub startup_stage_durations: Vec<(String, std::time::Duration)>,
    /// Instant when the current startup stage began.
    pub startup_stage_started_at: Option<std::time::Instant>,
    /// Authoritative diagnostic checks for the Doctor surface.
    pub doctor_checks: Vec<TuiDoctorCheck>,
    /// Currently active interactive decision awaiting operator selection (if any).
    pub pending_decision: Option<crate::interaction::decision::InteractiveDecision>,
    /// Quarantine containment projection state.
    pub quarantine_state: QuarantineProjectionState,
    /// Quarantined artifacts / containment breaches.
    pub quarantine_items: Vec<TuiQuarantineItem>,
    /// Authoritative sandbox and execution isolation status.
    pub sandbox_status: Option<TuiSandboxStatus>,
    /// Live draft for modal dialog text inputs (e.g. MissionCreation).
    pub modal_draft: String,
    /// Authoritative active checkpoint ID for rollback and recovery (M31A-UI-01).
    pub checkpoint_id: Option<String>,
    /// Whether a checkpoint restore operation is actively in-flight.
    pub rollback_in_progress: bool,
    /// Pending in-flight rollback target checkpoint (AUD-02, AUD-27).
    pub pending_rollback_target: Option<String>,
    /// Authoritative set of known valid checkpoint IDs eligible for restoration (AUD-01).
    pub authoritative_checkpoints: Option<std::collections::HashSet<String>>,
    /// Pending in-flight approval requests awaiting resolution from runtime.
    pub pending_approvals: Vec<TuiApprovalRequest>,
    /// Configured fast/auxiliary model identifier from runtime configuration.
    pub fast_auxiliary_model: Option<String>,
    /// Monotonic generation counter for quarantine queries to reject stale responses (M31A-UI-03).
    pub quarantine_generation: u64,
    /// Monotonic generation counter for model catalog refresh queries (AUD-26).
    pub catalog_generation: u64,
    /// Projected credential status without render-time I/O (M31A-UI-05).
    pub credential_status: TuiCredentialStatus,
    /// Verification checks section projection state (M31A-UI-22).
    pub verification_section_state: SectionProjectionState,
    /// Skills section projection state (AUD-29).
    pub skills_section_state: SectionProjectionState,
    /// Recovery attempts section projection state (M31A-UI-22).
    pub recovery_section_state: SectionProjectionState,
    /// Git status section projection state (M31A-UI-22).
    pub git_section_state: SectionProjectionState,
    /// Task graph section projection state (M31A-UI-22).
    pub task_graph_section_state: SectionProjectionState,
    /// Model usage telemetry section projection state (M31A-UI-22).
    pub usage_section_state: SectionProjectionState,
    /// Model catalog section projection state (M31A-UI-22).
    pub catalog_section_state: SectionProjectionState,
    /// Jobs list selection index, decoupled from context rail scroll (M31A-UI-07).
    pub jobs_selected_idx: usize,
    /// Detail inspector vertical scroll offset (M31A-UI-14).
    pub detail_scroll_offset: usize,
    /// Static environment overrides captured once at startup boundary (AUD-10).
    pub env_overrides: std::collections::HashMap<String, bool>,
}

impl Default for TuiViewModel {
    fn default() -> Self {
        Self::new()
    }
}

impl TuiViewModel {
    pub fn new() -> Self {
        Self {
            mission_id: None,
            mission_name: "M31A Autonomous Cockpit".to_string(),
            mission_status: "idle".to_string(),
            objective: "Awaiting mission start...".to_string(),
            tasks: Vec::new(),
            agents: Vec::new(),
            tools: Vec::new(),
            jobs: Vec::new(),
            logs: VecDeque::with_capacity(1000),
            approvals: Vec::new(),
            model_usage: TuiModelUsage::default(),
            system_stats: TuiSystemStats::default(),
            is_dirty: true,
            max_logs: 1000,
            sqlite_access_counter: Arc::new(AtomicUsize::new(0)),

            session_id: None,
            session_status: "idle".to_string(),
            session_view_mode: SessionViewMode::Welcome,
            active_model: "none".to_string(),
            active_provider: "none".to_string(),
            active_profile: "autonomous".to_string(),
            workspace_path: ".".to_string(),
            conversation: Vec::new(),
            live_activity: None,
            live_tools: Vec::new(),
            active_stream_message_id: None,
            catalog_models: Vec::new(),
            git_diff_text: None,
            git_branch: "N/A".to_string(),
            execution_worktree_branch: None,
            task_graph_state: TaskGraphProjectionState::Unknown,
            active_graph_id: None,
            active_graph_revision: 0,
            scroll_offset: 0,
            follow: true,
            unseen_count: 0,
            viewport_height: 0,
            content_height: 0,
            last_total_lines: 0,
            active_request_id: None,
            last_settled_request_id: None,
            active_command: None,
            lifecycle: TuiLifecycleProjection::new(),
            icons: IconRegistry::from_theme(crate::tui::theme::ThemeMode::default()),
            spinner: Spinner::new(),
            activity_kind: ActivityKind::default(),
            activity_message: None,
            activity_started_at: None,
            live_model_invocation_id: None,
            live_model_provider: None,
            live_model_name: None,
            live_model_reasoning_tokens_estimated: None,
            live_model_usage: None,
            settled_model_invocations: VecDeque::with_capacity(128),

            artifacts: Vec::new(),
            verification_checks: Vec::new(),
            verification_summary: TuiVerificationSummary::default(),
            recovery_attempts: Vec::new(),
            skills: Vec::new(),
            canonical_tools: Vec::new(),
            timeline: VecDeque::with_capacity(1000),
            max_timeline: 1000,
            traceability: Vec::new(),
            heartbeat: ExecutionHeartbeat::default(),
            seen_events: std::collections::HashSet::new(),
            runtime_status: RuntimeStartupState::Ready,
            runtime_status_detail: None,
            runtime_error_kind: None,
            startup_stage_durations: Vec::new(),
            startup_stage_started_at: Some(std::time::Instant::now()),
            doctor_checks: Vec::new(),
            pending_decision: None,
            quarantine_state: QuarantineProjectionState::Loading,
            quarantine_items: Vec::new(),
            sandbox_status: None,
            modal_draft: String::new(),
            checkpoint_id: None,
            rollback_in_progress: false,
            pending_rollback_target: None,
            authoritative_checkpoints: None,
            pending_approvals: Vec::new(),
            fast_auxiliary_model: None,
            quarantine_generation: 0,
            catalog_generation: 0,
            credential_status: TuiCredentialStatus::Loading,
            verification_section_state: SectionProjectionState::Loading,
            skills_section_state: SectionProjectionState::Loading,
            recovery_section_state: SectionProjectionState::Loading,
            git_section_state: SectionProjectionState::Loading,
            task_graph_section_state: SectionProjectionState::Loading,
            usage_section_state: SectionProjectionState::Loading,
            catalog_section_state: SectionProjectionState::Loading,
            jobs_selected_idx: 0,
            detail_scroll_offset: 0,
            env_overrides: std::collections::HashMap::new(),
        }
    }

    /// Check if an interactive decision is currently pending operator input.
    pub fn has_pending_decision(&self) -> bool {
        self.pending_decision
            .as_ref()
            .map(|d| d.state.is_pending())
            .unwrap_or(false)
    }

    /// Select the next decision option in the pending prompt.
    pub fn select_next_decision_option(&mut self) {
        if let Some(ref mut d) = self.pending_decision {
            d.select_next();
            self.is_dirty = true;
        }
    }

    /// Select the previous decision option in the pending prompt.
    pub fn select_prev_decision_option(&mut self) {
        if let Some(ref mut d) = self.pending_decision {
            d.select_prev();
            self.is_dirty = true;
        }
    }

    /// Select a decision option by zero-based index in the pending prompt.
    pub fn select_decision_option_index(&mut self, idx: usize) {
        if let Some(ref mut d) = self.pending_decision {
            if d.select_index(idx) {
                self.is_dirty = true;
            }
        }
    }

    /// Check if in-flight execution or animations require continuous frame rendering.
    pub fn has_active_animation(&self) -> bool {
        self.runtime_status.is_startup()
            || self.operation_state().is_working()
            || self.active_stream_message_id.is_some()
            || self
                .live_tools
                .iter()
                .any(|t| t.state == LiveToolState::Running)
    }

    /// The ONE authoritative UI activity derivation (§22).
    ///
    /// Every renderer (header, inline activity line, composer placeholder,
    /// footer) must use this. Session liveness (`session_status == active`)
    /// is deliberately NOT an input: an alive session with no in-flight
    /// operation is `Idle`, never `Working`.
    pub fn operation_state(&self) -> UiOperationState {
        use crate::tui::lifecycle::TuiLifecycleStage as L;
        // Terminal failure truth first (lifecycle or mission).
        if self.lifecycle.stage == L::Failed || self.mission_status == "failed" {
            return UiOperationState::Failed;
        }
        if self.lifecycle.stage == L::Cancelled || self.mission_status == "cancelled" {
            return UiOperationState::Cancelled;
        }
        // Governance gates and pending approvals genuinely need the operator.
        if !self.approvals.is_empty() {
            return UiOperationState::AwaitingApproval;
        }
        match self.lifecycle.stage {
            L::DiscoveryRequired | L::Blocked => return UiOperationState::AwaitingInput,
            L::PlanReviewRequired
            | L::PlanRevisionAvailable
            | L::TasksReviewRequired
            | L::TaskRevisionAvailable
            | L::ExecutionAuthorizationRequired => return UiOperationState::AwaitingApproval,
            _ => {}
        }
        // Explicit activity state owned by the request lifecycle.
        match self.activity_kind {
            ActivityKind::Thinking => return UiOperationState::Thinking,
            ActivityKind::Discovering => return UiOperationState::AwaitingInput,
            ActivityKind::Planning => return UiOperationState::Planning,
            ActivityKind::WaitingForReview | ActivityKind::WaitingForApproval => {
                return UiOperationState::AwaitingApproval;
            }
            ActivityKind::Executing => return UiOperationState::Executing,
            ActivityKind::RunningTool => return UiOperationState::RunningTool,
            ActivityKind::Verifying => return UiOperationState::Verifying,
            ActivityKind::Recovering => return UiOperationState::Recovering,
            ActivityKind::Completed => return UiOperationState::Completed,
            ActivityKind::Failed => return UiOperationState::Failed,
            ActivityKind::Cancelled => return UiOperationState::Cancelled,
            ActivityKind::Idle => {}
        }
        // Lifecycle execution truth without a stale activity label.
        match self.lifecycle.stage {
            L::IntentActive | L::PlanDraft | L::TasksDraft => {
                return UiOperationState::Planning;
            }
            L::PlanAccepted | L::TasksAccepted | L::ExecutionAuthorized | L::Executing => {
                return UiOperationState::Executing;
            }
            L::Verifying => return UiOperationState::Verifying,
            _ => {}
        }
        // Belt-and-braces: live streams/tools imply work even if the
        // activity label was missed. A live `live_activity` string alone is
        // NOT trusted here — it must always be paired with an activity kind
        // or an in-flight tool/stream, otherwise it is stale text.
        if self.active_stream_message_id.is_some() {
            return UiOperationState::Executing;
        }
        if self
            .live_tools
            .iter()
            .any(|t| t.state == LiveToolState::Running)
        {
            return UiOperationState::RunningTool;
        }
        if let Some(ref command) = self.active_command {
            return UiOperationState::CommandRunning {
                command: command.clone(),
            };
        }
        if self.heartbeat.active_task_id.is_some() {
            return UiOperationState::Executing;
        }
        if self.lifecycle.stage == L::Completed || self.mission_status == "completed" {
            return UiOperationState::Completed;
        }
        UiOperationState::Idle
    }

    /// Check whether there is an active failure or inspectable error in the conversation.
    pub fn has_inspectable_error(&self) -> bool {
        self.lifecycle.failure_reason.is_some()
            || !self.recovery_attempts.is_empty()
            || self
                .conversation
                .iter()
                .rev()
                .take(10)
                .any(|item| match item {
                    TuiConversationItem::Error { .. } | TuiConversationItem::Failure { .. } => true,
                    TuiConversationItem::Verification { passed, .. } => !*passed,
                    TuiConversationItem::ToolResult { success, .. } => !*success,
                    _ => false,
                })
    }

    /// Return a human-readable summary of the most recent failure or error.
    pub fn latest_error_summary(&self) -> Option<String> {
        if let Some(ref reason) = self.lifecycle.failure_reason {
            return Some(reason.clone());
        }
        for item in self.conversation.iter().rev().take(10) {
            match item {
                TuiConversationItem::Error { message, .. } => return Some(message.clone()),
                TuiConversationItem::Failure {
                    reason, context, ..
                } => {
                    return Some(format!("[{context}] {reason}"));
                }
                TuiConversationItem::Verification {
                    summary, passed, ..
                } if !*passed => {
                    return Some(format!("Verification failed: {summary}"));
                }
                TuiConversationItem::ToolResult {
                    output_preview,
                    success,
                    tool_name,
                    ..
                } if !*success => {
                    return Some(format!("Tool '{tool_name}' failed: {output_preview}"));
                }
                _ => {}
            }
        }
        None
    }

    /// Semantic "is M31A actually working right now?" predicate.
    ///
    /// Derived SOLELY from [`Self::operation_state`]. Historical records
    /// (completed tasks, idle agents, resolved approvals) and mere session
    /// liveness (`session_status == "active"`) must NOT force an active-work
    /// layout. Only live runtime state counts.
    pub fn is_semantically_active(&self) -> bool {
        let state = self.operation_state();
        state.is_working() || state.is_waiting()
    }

    /// Quiet one-line execution summary for inline display.
    /// Returns None when idle.
    pub fn execution_summary(&self) -> Option<String> {
        let now = Utc::now();
        let is_executing = self.lifecycle.stage
            == crate::tui::lifecycle::TuiLifecycleStage::Executing
            || self.mission_status == "running";

        // 1. Inactivity / stall detection: if executing and no runtime activity for >= 60s
        if is_executing {
            if let Some(last_event) = self.heartbeat.last_runtime_event_at {
                let silent_secs = (now - last_event).num_seconds();
                if silent_secs >= 60 {
                    let mins = silent_secs / 60;
                    return Some(format!("Stalled? No runtime activity for {mins}m"));
                }
            }
        }

        // 2. Running tool
        if let Some(tool) = self
            .live_tools
            .iter()
            .rev()
            .find(|t| t.state == LiveToolState::Running)
        {
            let dur = (now - tool.started_at).num_seconds().max(0);
            return Some(format!("Running tool `{}` ({}s)", tool.tool_name, dur));
        }

        // 3. Active running task from authoritative heartbeat / runtime event
        if let Some(ref tid) = self.heartbeat.active_task_id {
            if let Some(task) = self.tasks.iter().find(|t| &t.id == tid) {
                let total = self.tasks.len();
                let idx = self
                    .tasks
                    .iter()
                    .position(|t| &t.id == tid)
                    .map(|p| p + 1)
                    .unwrap_or(1);
                let role_str = task
                    .agent_role
                    .as_deref()
                    .map(|r| format!(" · {r}"))
                    .unwrap_or_default();
                return Some(format!("Task {idx}/{total} · {}{role_str}", task.title));
            } else if let Some(activity) = self.heartbeat.current_activity.as_deref() {
                return Some(activity.to_string());
            }
        }

        // 4. Explicit activity message (e.g. model streaming, thinking)
        if let Some(msg) = self.activity_message.as_ref() {
            if !msg.is_empty() {
                return Some(msg.clone());
            }
        }

        // 5. Executing state without dispatched task: truthful scheduler explanation
        if is_executing {
            let total = self.tasks.len();
            let completed = self
                .tasks
                .iter()
                .filter(|t| {
                    t.status.eq_ignore_ascii_case("completed")
                        || t.status.eq_ignore_ascii_case("succeeded")
                })
                .count();
            let blocked = self
                .tasks
                .iter()
                .filter(|t| t.status.eq_ignore_ascii_case("blocked"))
                .count();

            if blocked > 0 {
                return Some(format!("Waiting · {blocked} tasks blocked"));
            }
            if completed == total && total > 0 {
                return Some("Verifying mission completion...".to_string());
            }
            return Some("Waiting for scheduler dispatch...".to_string());
        }

        let stage = &self.lifecycle.stage;
        if *stage == crate::tui::lifecycle::TuiLifecycleStage::ExecutionAuthorized {
            return Some("Execution authorized · starting...".to_string());
        }
        if *stage != crate::tui::lifecycle::TuiLifecycleStage::Idle {
            return Some(stage.label().to_lowercase());
        }
        if self.operation_state().is_working() {
            Some("working".to_string())
        } else {
            None
        }
    }

    /// Compact thought/activity indicator with live elapsed duration (e.g. "Thought · 5.8s" or "Planning · 2.1s").
    pub fn thought_duration_label(&self) -> Option<String> {
        if !self.operation_state().is_working() {
            return None;
        }
        let dur = self.activity_started_at.map(|s| {
            let elapsed = Utc::now() - s;
            let secs = (elapsed.num_milliseconds().max(0) as f32) / 1000.0;
            format!("{secs:.1}s")
        });
        match self.activity_kind {
            ActivityKind::Thinking => {
                let mut label = dur
                    .map(|d| format!("Thought · {d}"))
                    .unwrap_or_else(|| "Thought".to_string());
                if let Some(model) = self.live_model_name.as_deref() {
                    label.push_str(&format!(" · {model}"));
                }
                if let Some(usage) = self.live_model_usage.as_ref() {
                    if usage.total_tokens > 0 {
                        let provenance = if usage.source
                            == crate::model::types::UsageSource::AuthoritativeProvider
                        {
                            "provider"
                        } else {
                            "estimated"
                        };
                        label.push_str(&format!(
                            " · {} tokens ({provenance})",
                            usage.total_tokens
                        ));
                    }
                    if usage.reasoning_tokens > 0 {
                        label.push_str(&format!(" · {} reasoning", usage.reasoning_tokens));
                    } else if let Some(tokens) = self.live_model_reasoning_tokens_estimated {
                        label.push_str(&format!(" · ~{tokens} reasoning tokens"));
                    }
                } else if let Some(tokens) = self.live_model_reasoning_tokens_estimated {
                    label.push_str(&format!(" · ~{tokens} reasoning tokens"));
                }
                Some(label)
            },
            ActivityKind::Planning => Some(
                dur.map(|d| format!("Planning · {d}"))
                    .unwrap_or_else(|| "Planning".to_string()),
            ),
            ActivityKind::RunningTool => {
                if let Some(tool) = self
                    .live_tools
                    .iter()
                    .rev()
                    .find(|t| t.state == LiveToolState::Running)
                {
                    Some(
                        dur.map(|d| format!("Tool · {} · {d}", tool.tool_name))
                            .unwrap_or_else(|| format!("Tool · {}", tool.tool_name)),
                    )
                } else {
                    Some("Running tool".to_string())
                }
            }
            _ => self.execution_summary(),
        }
    }

    /// Transition from Welcome to Active mode upon first prompt or meaningful interaction (§3, §4).
    pub fn enter_active_session(&mut self) {
        if self.session_view_mode != SessionViewMode::Closed {
            self.session_view_mode = SessionViewMode::Active;
            self.is_dirty = true;
        }
    }

    /// Whether the view is in the minimal Welcome mode.
    pub fn is_welcome(&self) -> bool {
        self.session_view_mode.is_welcome()
    }

    /// Whether the view is in the Active Session Workspace.
    pub fn is_active(&self) -> bool {
        self.session_view_mode.is_active()
    }

    /// Record transition between startup stages and measure elapsed duration.
    fn record_startup_stage_transition(&mut self) {
        let now = std::time::Instant::now();
        if let Some(started) = self.startup_stage_started_at {
            if self.runtime_status != RuntimeStartupState::Ready
                || !self.startup_stage_durations.is_empty()
            {
                let prev_name = self.runtime_status.label();
                self.startup_stage_durations
                    .push((prev_name, now.duration_since(started)));
            }
        }
        self.startup_stage_started_at = Some(now);
    }

    /// Retrieve recorded startup duration for a stage matching the given label prefix.
    pub fn startup_stage_duration(&self, stage_label_prefix: &str) -> Option<std::time::Duration> {
        self.startup_stage_durations
            .iter()
            .find(|(name, _)| name.starts_with(stage_label_prefix))
            .map(|(_, dur)| *dur)
    }

    /// Mark the projection as booting (initial app, before runtime work).
    pub fn set_runtime_booting(&mut self) {
        self.record_startup_stage_transition();
        self.runtime_status = RuntimeStartupState::Booting;
        self.runtime_status_detail = None;
        self.runtime_error_kind = None;
        self.is_dirty = true;
    }

    /// Mark the projection as initializing the canonical runtime.
    pub fn set_runtime_initializing(&mut self, detail: Option<String>) {
        self.record_startup_stage_transition();
        self.runtime_status = RuntimeStartupState::InitializingRuntime;
        self.runtime_status_detail = detail;
        self.runtime_error_kind = None;
        self.is_dirty = true;
    }

    /// Mark the projection as hydrating durable session truth.
    pub fn set_runtime_hydrating_session(&mut self, detail: Option<String>) {
        self.record_startup_stage_transition();
        self.runtime_status = RuntimeStartupState::HydratingSession;
        self.runtime_status_detail = detail;
        self.runtime_error_kind = None;
        self.is_dirty = true;
    }

    /// Mark the projection as hydrating workspace truth.
    pub fn set_runtime_hydrating_workspace(&mut self, detail: Option<String>) {
        self.record_startup_stage_transition();
        self.runtime_status = RuntimeStartupState::HydratingWorkspace;
        self.runtime_status_detail = detail;
        self.runtime_error_kind = None;
        self.is_dirty = true;
    }

    /// Mark the projection as hydrating execution truth.
    pub fn set_runtime_hydrating_execution(&mut self, detail: Option<String>) {
        self.record_startup_stage_transition();
        self.runtime_status = RuntimeStartupState::HydratingExecution;
        self.runtime_status_detail = detail;
        self.runtime_error_kind = None;
        self.is_dirty = true;
    }

    /// Mark the projection as ready for normal operation.
    pub fn set_runtime_ready(&mut self) {
        self.record_startup_stage_transition();
        self.runtime_status = RuntimeStartupState::Ready;
        self.runtime_status_detail = None;
        self.runtime_error_kind = None;
        self.is_dirty = true;
    }

    /// Mark runtime startup as failed with a visible reason.
    ///
    /// The failure is also recorded as an error conversation item so the
    /// operator sees it in the stream, not just in the startup panel.
    pub fn set_runtime_failed(&mut self, reason: impl Into<String>) {
        self.set_runtime_failed_with_kind(crate::tui::errors::TuiErrorKind::Runtime, reason);
    }

    /// Mark startup as failed with an explicit error kind (Principle 11).
    pub fn set_runtime_failed_with_kind(
        &mut self,
        kind: crate::tui::errors::TuiErrorKind,
        reason: impl Into<String>,
    ) {
        self.record_startup_stage_transition();
        let reason = reason.into();
        let err = crate::tui::errors::TuiError::new(kind, reason.clone());
        self.runtime_status = RuntimeStartupState::Failed(reason.clone());
        self.runtime_status_detail = Some(reason.clone());
        self.runtime_error_kind = Some(kind);
        self.add_conversation_item(TuiConversationItem::Error {
            message: format!("{}: {reason}", kind.title()),
            timestamp: chrono::Utc::now(),
        });
        let _ = err;
        self.is_dirty = true;
    }

    /// True when the cockpit is in normal operational state.
    pub fn is_runtime_ready(&self) -> bool {
        self.runtime_status.is_ready()
    }

    /// Begin tracking a new user submission that performs real work.
    ///
    /// Returns the fresh request correlation id. Must be called AFTER input
    /// classification, only for inputs that actually start an operation —
    /// never unconditionally on submit.
    pub fn begin_request(&mut self, message: impl Into<String>) -> String {
        let request_id = uuid::Uuid::now_v7().to_string();
        self.active_request_id = Some(request_id.clone());
        self.active_command = None;
        self.activity_kind = ActivityKind::Thinking;
        self.activity_message = Some(message.into());
        self.activity_started_at = Some(chrono::Utc::now());
        self.live_activity = self.activity_message.clone();
        self.spinner.reset();
        self.is_dirty = true;
        request_id
    }

    /// Begin tracking a slash command WITHOUT any visible activity.
    ///
    /// Fast commands must not animate the UI. The request id still guards
    /// stale-activity association; the terminal `CommandOutput`/`Error`
    /// event settles it.
    pub fn begin_request_silent(&mut self) -> String {
        let request_id = uuid::Uuid::now_v7().to_string();
        self.active_request_id = Some(request_id.clone());
        self.is_dirty = true;
        request_id
    }

    /// Mark a slash command as genuinely long-running (diagnostics, etc.).
    /// Renders `Running /<command>…`, never generic model reasoning.
    pub fn begin_command_running(&mut self, command: impl Into<String>) {
        let command = command.into();
        let now = chrono::Utc::now();
        self.active_command = Some(command.clone());
        self.activity_kind = ActivityKind::Idle;
        self.activity_message = Some(format!("Running /{command}…"));
        self.activity_started_at = Some(now);
        self.live_activity = None;
        self.spinner.reset();
        self.is_dirty = true;
    }

    /// Settle the in-flight request: clear every activity field.
    ///
    /// Called by EVERY terminal interaction event (`CommandOutput`, `Error`,
    /// `AssistantFinished`, `AssistantOutput`, `AssistantFailed`, lifecycle
    /// responses, …) unless the event explicitly represents a still-running
    /// operation. Never rely on the next unrelated event to clean up.
    pub fn settle_request(&mut self) {
        self.last_settled_request_id = self.active_request_id.take();
        self.active_command = None;
        self.live_activity = None;
        self.activity_kind = ActivityKind::Idle;
        self.activity_message = None;
        self.activity_started_at = None;
        self.is_dirty = true;
    }

    /// Settle into an explicit failure. The error card in the conversation
    /// carries detail; the header stays quiet (no permanent Working).
    pub fn fail_request(&mut self, message: impl Into<String>) {
        self.last_settled_request_id = self.active_request_id.take();
        self.active_command = None;
        self.live_activity = None;
        self.activity_kind = ActivityKind::Failed;
        self.activity_message = Some(message.into());
        self.activity_started_at = None;
        self.is_dirty = true;
    }

    /// Mark state as updated requiring re-render.
    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    /// Clear dirty flag after rendering frame.
    pub fn clear_dirty(&mut self) {
        self.is_dirty = false;
    }

    /// Append log line preserving ring-buffer capacity.
    pub fn add_log(
        &mut self,
        level: impl Into<String>,
        message: impl Into<String>,
        source: impl Into<String>,
    ) {
        if self.logs.len() >= self.max_logs {
            self.logs.pop_front();
        }
        self.logs.push_back(TuiLogLine {
            timestamp: Utc::now(),
            level: level.into(),
            message: message.into(),
            source: source.into(),
        });
        self.is_dirty = true;
    }

    /// Append a timeline entry preserving ring buffer capacity.
    pub fn add_timeline_entry(
        &mut self,
        sequence: u64,
        timestamp: DateTime<Utc>,
        category: impl Into<String>,
        title: impl Into<String>,
        details: impl Into<String>,
        level: impl Into<String>,
    ) {
        if self.timeline.len() >= self.max_timeline {
            self.timeline.pop_front();
        }
        self.timeline.push_back(TuiTimelineEntry {
            sequence,
            timestamp,
            category: category.into(),
            title: title.into(),
            details: details.into(),
            level: level.into(),
        });
        self.is_dirty = true;
    }

    /// Load or replace canonical task snapshots into projection.
    pub fn load_tasks(&mut self, tasks: Vec<TuiTaskSnapshot>) {
        self.tasks = tasks;
        self.task_graph_state = TaskGraphProjectionState::Loaded;
        self.is_dirty = true;
    }

    /// Load or replace canonical artifact snapshots into projection.
    pub fn load_artifacts(&mut self, artifacts: Vec<TuiArtifactSnapshot>) {
        let mut q_items = Vec::new();
        for a in &artifacts {
            if a.status.eq_ignore_ascii_case("quarantined") {
                q_items.push(map_quarantine_item(
                    &a.id,
                    &a.name,
                    a.quarantine_reason.clone(),
                    Some(a.created_at),
                ));
            }
        }
        self.quarantine_items = q_items;
        self.quarantine_state = QuarantineProjectionState::Ready;
        self.artifacts = artifacts;
        self.rebuild_traceability();
        self.is_dirty = true;
    }

    /// Return the authoritative rollback target checkpoint if valid, non-empty, and eligible (AUD-01).
    pub fn valid_rollback_target(&self) -> Option<&str> {
        let is_valid = |s: &str| {
            if s.parse::<crate::ids::CheckpointId>().is_err() {
                return false;
            }
            match self.authoritative_checkpoints {
                Some(ref set) => set.contains(s),
                None => false, // Fail closed if authoritative checkpoint information has not loaded (AUD-01)
            }
        };
        if let Some(ref cid) = self.checkpoint_id {
            if is_valid(cid) {
                return Some(cid.as_str());
            }
        }
        for t in &self.traceability {
            if let Some(ref cid) = t.checkpoint_id {
                if is_valid(cid) {
                    return Some(cid.as_str());
                }
            }
        }
        for r in &self.recovery_attempts {
            if let Some(ref cid) = r.checkpoint_id {
                if is_valid(cid) {
                    return Some(cid.as_str());
                }
            }
        }
        None
    }

    /// Set authoritative rollback checkpoint target.
    pub fn set_rollback_checkpoint_target(&mut self, checkpoint_id: impl Into<String>) {
        let cid = checkpoint_id.into();
        let is_valid = cid.parse::<crate::ids::CheckpointId>().is_ok()
            && self
                .authoritative_checkpoints
                .as_ref()
                .map(|set| set.contains(&cid))
                .unwrap_or(false); // Fail closed if authoritative checkpoints have not loaded (AUD-01)
        if is_valid {
            self.checkpoint_id = Some(cid.clone());
            if let Some(t) = self.traceability.first_mut() {
                t.checkpoint_id = Some(cid);
            }
        }
        self.is_dirty = true;
    }

    /// Set authoritative eligible checkpoints from storage (AUD-01).
    pub fn set_authoritative_checkpoints(&mut self, checkpoints: Vec<String>) {
        self.authoritative_checkpoints = Some(checkpoints.into_iter().collect());
        self.is_dirty = true;
    }

    /// Record a newly created checkpoint into authoritative checkpoints (AUD-01).
    pub fn add_authoritative_checkpoint(&mut self, checkpoint_id: impl Into<String>) {
        let cid = checkpoint_id.into();
        if let Some(ref mut set) = self.authoritative_checkpoints {
            set.insert(cid);
        } else {
            let mut set = std::collections::HashSet::new();
            set.insert(cid);
            self.authoritative_checkpoints = Some(set);
        }
        self.is_dirty = true;
    }

    /// Whether rollback can proceed with a valid target.
    pub fn can_rollback(&self) -> bool {
        self.valid_rollback_target().is_some()
    }

    /// Set quarantine items directly from authoritative source.
    pub fn set_quarantine_items(&mut self, items: Vec<TuiQuarantineItem>) {
        self.quarantine_items = items;
        self.quarantine_state = QuarantineProjectionState::Ready;
        self.is_dirty = true;
    }

    /// Mark quarantine retrieval failed with reason.
    pub fn set_quarantine_failed(&mut self, reason: impl Into<String>) {
        self.quarantine_state = QuarantineProjectionState::Failed(reason.into());
        self.is_dirty = true;
    }

    /// Set authoritative sandbox status.
    pub fn set_sandbox_status(&mut self, status: TuiSandboxStatus) {
        self.sandbox_status = Some(status);
        self.is_dirty = true;
    }

    /// Load or replace canonical verification checks and recompute summary.
    pub fn load_verification_checks(&mut self, checks: Vec<TuiVerificationCheck>) {
        self.verification_checks = checks;
        self.recompute_verification_summary();
        self.rebuild_traceability();
        self.is_dirty = true;
    }

    /// Recompute verification summary counters from canonical checks.
    pub fn recompute_verification_summary(&mut self) {
        let total = self.verification_checks.len();
        let passed = self
            .verification_checks
            .iter()
            .filter(|c| c.status == "passed")
            .count();
        let failed = self
            .verification_checks
            .iter()
            .filter(|c| c.status == "failed")
            .count();
        let blocked = self
            .verification_checks
            .iter()
            .filter(|c| c.status == "blocked")
            .count();
        let skipped = self
            .verification_checks
            .iter()
            .filter(|c| c.status == "skipped_with_reason" || c.status == "skipped")
            .count();
        let not_run = self
            .verification_checks
            .iter()
            .filter(|c| c.status == "not_run")
            .count();

        let overall = if total == 0 {
            "not_run".to_string()
        } else if failed > 0 {
            "failed".to_string()
        } else if blocked > 0 {
            "blocked".to_string()
        } else if passed == total {
            "passed".to_string()
        } else {
            "pending".to_string()
        };

        self.verification_summary = TuiVerificationSummary {
            total_checks: total,
            passed_count: passed,
            failed_count: failed,
            blocked_count: blocked,
            skipped_count: skipped,
            not_run_count: not_run,
            overall_status: overall,
            last_evaluated: self.verification_checks.last().map(|c| c.created_at),
        };
    }

    /// Load recovery attempt records into projection.
    pub fn load_recovery_attempts(&mut self, attempts: Vec<TuiRecoverySnapshot>) {
        self.recovery_attempts = attempts;
        self.is_dirty = true;
    }

    /// Load discovered skill packages.
    pub fn load_skills(&mut self, skills: Vec<TuiSkillSnapshot>) {
        self.skills = skills;
        self.is_dirty = true;
    }

    /// Load canonical tool definitions from ToolRegistry.
    pub fn load_canonical_tools(&mut self, tools: Vec<TuiToolDefinitionSnapshot>) {
        self.canonical_tools = tools;
        self.is_dirty = true;
    }

    /// Rebuild end-to-end 7-step traceability chains from canonical projection state.
    /// user decision → plan revision → task revision → execution/action → source change → verification → evidence
    pub fn rebuild_traceability(&mut self) {
        let mut chains = Vec::new();

        let plan_rev = self.lifecycle.plan_revision;
        let plan_hash = self.lifecycle.plan_hash.clone();
        let task_rev = self.lifecycle.task_revision;

        if !self.tasks.is_empty() {
            for task in &self.tasks {
                let matching_checks: Vec<&TuiVerificationCheck> = self
                    .verification_checks
                    .iter()
                    .filter(|c| c.task_id == task.id)
                    .collect();

                let matching_artifacts: Vec<&TuiArtifactSnapshot> = self
                    .artifacts
                    .iter()
                    .filter(|a| a.task_id.as_deref() == Some(&task.id))
                    .collect();

                let first_check = matching_checks.first();
                let first_art = matching_artifacts.first();

                chains.push(TuiTraceabilityChain {
                    user_decision: if plan_rev.is_some() {
                        Some("PlanAccepted / ExecutionAuthorized".to_string())
                    } else {
                        None
                    },
                    plan_revision: plan_rev,
                    plan_hash: plan_hash.clone(),
                    task_revision: task_rev,
                    task_id: Some(task.id.clone()),
                    execution_id: task.agent_role.clone(),
                    source_change: self
                        .git_diff_text
                        .as_ref()
                        .map(|_| "workspace diff present".to_string()),
                    verification_id: first_check.map(|c| c.check_id.clone()),
                    verification_status: first_check.map(|c| c.status.clone()),
                    evidence_artifact_id: first_art
                        .map(|a| a.id.clone())
                        .or_else(|| first_check.and_then(|c| c.evidence_artifact_id.clone())),
                    checkpoint_id: self.checkpoint_id.clone(),
                });
            }
        } else if plan_rev.is_some() {
            chains.push(TuiTraceabilityChain {
                user_decision: Some("PlanUnderReview".to_string()),
                plan_revision: plan_rev,
                plan_hash,
                task_revision: task_rev,
                task_id: None,
                execution_id: None,
                source_change: None,
                verification_id: None,
                verification_status: None,
                evidence_artifact_id: None,
                checkpoint_id: self.checkpoint_id.clone(),
            });
        }

        self.traceability = chains;
    }

    /// Apply runtime event envelope into in-memory projection in O(1) time.
    pub fn apply_event(&mut self, event: &EventEnvelope) {
        if !self.seen_events.insert(event.id) {
            return;
        }
        self.system_stats.events_processed += 1;
        self.is_dirty = true;
        let seq = event.sequence;
        let ts = event.timestamp;
        self.heartbeat.last_runtime_event_at = Some(ts);

        match &event.event_type {
            EventType::MissionPaused { reason, .. } => {
                self.mission_status = "paused".to_string();
                self.add_log("WARN", format!("Mission paused: {reason}"), "runtime");
                self.add_timeline_entry(seq, ts, "lifecycle", "Mission paused", reason, "WARN");
            }
            EventType::MissionResumed { .. } => {
                self.mission_status = "running".to_string();
                self.add_log("INFO", "Mission resumed", "runtime");
                self.add_timeline_entry(
                    seq,
                    ts,
                    "lifecycle",
                    "Mission resumed",
                    "Mission resumed",
                    "INFO",
                );
            }
            EventType::TaskCompleted {
                task_id, result, ..
            } => {
                let tid_str = task_id.to_string();
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == tid_str) {
                    t.status = "completed".to_string();
                    t.progress_pct = 100;
                }
                self.heartbeat.last_task_event_at = Some(ts);
                if self.heartbeat.active_task_id.as_deref() == Some(&tid_str) {
                    self.heartbeat.active_task_id = None;
                    self.heartbeat.active_agent_id = None;
                    self.heartbeat.current_activity = None;
                    self.heartbeat.current_activity_started_at = None;
                }
                self.add_log("INFO", format!("Task completed: {tid_str}"), "scheduler");
                self.add_timeline_entry(
                    seq,
                    ts,
                    "task",
                    format!("Task completed: {tid_str}"),
                    result,
                    "INFO",
                );
                self.rebuild_traceability();
            }
            EventType::TaskFailed { task_id, error, .. } => {
                let tid_str = task_id.to_string();
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == tid_str) {
                    t.status = "failed".to_string();
                }
                self.heartbeat.last_task_event_at = Some(ts);
                if self.heartbeat.active_task_id.as_deref() == Some(&tid_str) {
                    self.heartbeat.active_task_id = None;
                    self.heartbeat.active_agent_id = None;
                    self.heartbeat.current_activity = None;
                    self.heartbeat.current_activity_started_at = None;
                }
                self.add_log(
                    "ERROR",
                    format!("Task {tid_str} failed: {error}"),
                    "scheduler",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "task",
                    format!("Task {tid_str} failed"),
                    error,
                    "ERROR",
                );
                self.rebuild_traceability();
            }
            EventType::AgentSpawned { agent_id, role, .. } => {
                self.agents.push(TuiAgentSnapshot {
                    id: agent_id.to_string(),
                    role: role.clone(),
                    state: "active".to_string(),
                    current_task: None,
                    total_tokens: 0,
                });
                self.add_log("INFO", format!("Agent spawned: {role}"), "agents");
                self.add_timeline_entry(
                    seq,
                    ts,
                    "agent",
                    format!("Agent spawned: {role}"),
                    agent_id.to_string(),
                    "INFO",
                );
            }
            EventType::ToolRequested { tool_name, .. } => {
                if let Some(t) = self.tools.iter_mut().find(|t| t.name == *tool_name) {
                    t.executions_count += 1;
                } else {
                    self.tools.push(TuiToolSnapshot {
                        name: tool_name.clone(),
                        executions_count: 1,
                        errors_count: 0,
                    });
                }
                self.heartbeat.last_tool_event_at = Some(ts);
                self.heartbeat.active_tool_id = Some(tool_name.clone());
                self.add_log("DEBUG", format!("Tool requested: {tool_name}"), "tools");
                self.add_timeline_entry(
                    seq,
                    ts,
                    "tool",
                    format!("Tool requested: {tool_name}"),
                    "",
                    "DEBUG",
                );
            }
            EventType::ToolFailed {
                tool_call_id,
                error,
                ..
            } => {
                self.heartbeat.last_tool_event_at = Some(ts);
                self.heartbeat.active_tool_id = None;
                self.add_log(
                    "ERROR",
                    format!("Tool call {tool_call_id} failed: {error}"),
                    "tools",
                );
                self.add_timeline_entry(seq, ts, "tool", "Tool failed", error, "ERROR");
            }
            EventType::PolicyEvaluated {
                policy_id,
                decision,
                reason,
            } => {
                self.add_log(
                    "INFO",
                    format!("Policy {policy_id} evaluated: {decision} ({reason})"),
                    "policy",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "policy",
                    format!("Policy evaluated: {decision}"),
                    format!("{policy_id}: {reason}"),
                    "INFO",
                );
            }
            EventType::PolicyDenied { policy_id, reason } => {
                self.approvals.retain(|a| a.id != *policy_id);
                self.add_log(
                    "WARN",
                    format!("Policy denied {policy_id}: {reason}"),
                    "policy",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "policy",
                    format!("Policy denied: {policy_id}"),
                    reason,
                    "WARN",
                );
            }
            // ── Governed lifecycle projection ──
            EventType::PlanReviewRequired {
                plan_id, revision, ..
            } => {
                self.lifecycle.apply_event(event);
                self.upsert_plan_review_card(
                    *revision,
                    plan_id.clone(),
                    None,
                    None,
                    None,
                    None,
                    Vec::new(),
                );
                self.add_log(
                    "INFO",
                    format!("Plan R{revision} ready for review ({plan_id})"),
                    "lifecycle",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "plan",
                    format!("Plan R{revision} review required"),
                    plan_id,
                    "INFO",
                );
                self.rebuild_traceability();
            }
            EventType::PlanRevisionCreated {
                revision, author, ..
            } => {
                self.lifecycle.apply_event(event);
                self.add_log(
                    "INFO",
                    format!("Plan revision {revision} created"),
                    "lifecycle",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "plan",
                    format!("Plan revision {revision} created"),
                    format!("Author: {author}"),
                    "INFO",
                );
                self.rebuild_traceability();
            }
            EventType::PlanAccepted { revision, .. } => {
                self.lifecycle.apply_event(event);
                self.add_log("INFO", format!("Plan R{revision} accepted"), "lifecycle");
                self.add_timeline_entry(
                    seq,
                    ts,
                    "plan",
                    format!("Plan R{revision} accepted"),
                    "",
                    "INFO",
                );
                self.rebuild_traceability();
            }
            EventType::PlanRejected { reason, .. } => {
                self.lifecycle.apply_event(event);
                self.add_conversation_item(TuiConversationItem::Failure {
                    context: "plan rejected".to_string(),
                    reason: reason.clone(),
                    timestamp: Utc::now(),
                });
                self.add_log("WARN", format!("Plan rejected: {reason}"), "lifecycle");
                self.add_timeline_entry(seq, ts, "plan", "Plan rejected", reason, "WARN");
                self.rebuild_traceability();
            }
            EventType::TasksReviewRequired {
                task_revision,
                task_count,
                ..
            } => {
                self.lifecycle.apply_event(event);
                self.upsert_task_review_card(
                    *task_revision,
                    None,
                    Some(*task_count),
                    None,
                    Vec::new(),
                );
                self.add_log(
                    "INFO",
                    format!("Task revision {task_revision} ready for review"),
                    "lifecycle",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "task",
                    format!("Task revision {task_revision} review required"),
                    format!("{task_count} tasks proposed"),
                    "INFO",
                );
                self.rebuild_traceability();
            }
            EventType::TaskRevisionCreated {
                task_revision,
                author,
                ..
            } => {
                self.lifecycle.apply_event(event);
                self.add_log(
                    "INFO",
                    format!("Task revision {task_revision} created"),
                    "lifecycle",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "task",
                    format!("Task revision {task_revision} created"),
                    format!("Author: {author}"),
                    "INFO",
                );
                self.rebuild_traceability();
            }
            EventType::TasksAccepted { task_revision, .. } => {
                self.lifecycle.apply_event(event);
                self.add_log("INFO", "Task set accepted", "lifecycle");
                self.add_timeline_entry(
                    seq,
                    ts,
                    "task",
                    format!("Tasks accepted (R{task_revision})"),
                    "",
                    "INFO",
                );
                self.rebuild_traceability();
            }
            EventType::ExecutionAuthorizationRequired { message, .. } => {
                self.lifecycle.apply_event(event);
                self.upsert_auth_required_card(message.clone());
                self.add_log("WARN", "Execution authorization required", "lifecycle");
                self.add_timeline_entry(
                    seq,
                    ts,
                    "governance",
                    "Execution authorization required",
                    message,
                    "WARN",
                );
            }
            EventType::ExecutionAuthorized {
                authorization_id,
                authorized_by,
                ..
            } => {
                self.lifecycle.apply_event(event);
                self.upsert_auth_granted_card(authorization_id.to_string());
                self.add_log(
                    "INFO",
                    format!("Execution authorized: {authorization_id}"),
                    "lifecycle",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "governance",
                    format!("Execution authorized: {authorization_id}"),
                    format!("Authorized by: {authorized_by}"),
                    "INFO",
                );
                self.rebuild_traceability();
            }
            EventType::ExecutionAuthorizationRejected { reason, .. } => {
                self.lifecycle.apply_event(event);
                self.add_conversation_item(TuiConversationItem::Failure {
                    context: "authorization rejected".to_string(),
                    reason: reason.clone(),
                    timestamp: Utc::now(),
                });
                self.add_log(
                    "WARN",
                    format!("Execution authorization rejected: {reason}"),
                    "lifecycle",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "governance",
                    "Execution authorization rejected",
                    reason,
                    "WARN",
                );
                self.rebuild_traceability();
            }
            EventType::TaskGraphMaterialized {
                graph_id,
                revision,
                task_count,
                tasks,
                ..
            } => {
                let before = self.lifecycle.stage.clone();
                self.lifecycle.apply_event(event);
                if before == TuiLifecycleStage::ExecutionAuthorized
                    && self.lifecycle.stage == TuiLifecycleStage::Executing
                {
                    self.add_conversation_item(TuiConversationItem::System {
                        text: format!("Started execution · {task_count} tasks"),
                        timestamp: Utc::now(),
                    });
                }
                self.heartbeat.execution_started_at = Some(ts);
                if !tasks.is_empty() {
                    self.tasks = tasks
                        .iter()
                        .map(|t| TuiTaskSnapshot {
                            id: t.id.to_string(),
                            title: t.title.clone(),
                            status: t.status.to_string(),
                            agent_role: Some(t.role.to_string()),
                            progress_pct: if t.status
                                == crate::state_machine::task::TaskState::Succeeded
                            {
                                100
                            } else {
                                0
                            },
                            dependencies: t.dependencies.iter().map(|d| d.to_string()).collect(),
                            assigned_agent_id: None,
                            execution_result: None,
                            failure_reason: None,
                            retry_count: 0,
                        })
                        .collect();
                    self.task_graph_state = TaskGraphProjectionState::Loaded;
                } else if *task_count > 0 {
                    self.task_graph_state = TaskGraphProjectionState::Loading;
                } else {
                    self.tasks.clear();
                    self.task_graph_state = TaskGraphProjectionState::Loaded;
                }
                self.add_log(
                    "INFO",
                    format!("TaskGraph materialized: {graph_id} rev {revision}"),
                    "scheduler",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "scheduler",
                    format!("TaskGraph materialized ({task_count} tasks)"),
                    format!("{graph_id} rev {revision}"),
                    "INFO",
                );
                self.rebuild_traceability();
            }
            EventType::TaskStarted { .. } | EventType::MissionStarted { .. } => {
                let governed = matches!(
                    self.lifecycle.stage,
                    TuiLifecycleStage::ExecutionAuthorized | TuiLifecycleStage::Executing
                );
                if governed {
                    self.lifecycle.apply_event(event);
                }
                match &event.event_type {
                    EventType::MissionStarted {
                        objective,
                        mission_id,
                    } => {
                        self.mission_id = Some(mission_id.to_string());
                        self.mission_status = "running".to_string();
                        self.objective = objective.clone();
                        self.add_log("INFO", format!("Mission started: {objective}"), "runtime");
                        self.add_timeline_entry(
                            seq,
                            ts,
                            "lifecycle",
                            "Mission started",
                            objective,
                            "INFO",
                        );
                        self.rebuild_traceability();
                    }
                    EventType::TaskStarted {
                        task_id, agent_id, ..
                    } => {
                        let tid_str = task_id.to_string();
                        let aid_str = agent_id.to_string();
                        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == tid_str) {
                            t.status = "running".to_string();
                            t.assigned_agent_id = Some(aid_str.clone());
                        } else {
                            self.tasks.push(TuiTaskSnapshot {
                                id: tid_str.clone(),
                                title: format!("Task {tid_str}"),
                                status: "running".to_string(),
                                agent_role: None,
                                progress_pct: 0,
                                dependencies: Vec::new(),
                                assigned_agent_id: Some(aid_str.clone()),
                                execution_result: None,
                                failure_reason: None,
                                retry_count: 0,
                            });
                        }
                        self.heartbeat.last_task_event_at = Some(ts);
                        self.heartbeat.active_task_id = Some(tid_str.clone());
                        self.heartbeat.active_agent_id = Some(aid_str.clone());
                        self.heartbeat.current_activity = Some(format!("Running task {tid_str}"));
                        self.heartbeat.current_activity_started_at = Some(ts);
                        self.add_log("INFO", format!("Task started: {tid_str}"), "scheduler");
                        self.add_timeline_entry(
                            seq,
                            ts,
                            "task",
                            format!("Task started: {tid_str}"),
                            format!("Agent: {aid_str}"),
                            "INFO",
                        );
                        self.rebuild_traceability();
                    }
                    _ => {}
                }
            }
            EventType::VerificationStarted {
                verification_id,
                target,
                ..
            } => {
                self.lifecycle.apply_event(event);
                self.add_log("INFO", "Verification started", "verification");
                self.add_timeline_entry(
                    seq,
                    ts,
                    "verification",
                    format!("Verification started: {target}"),
                    verification_id,
                    "INFO",
                );
                if !self
                    .verification_checks
                    .iter()
                    .any(|c| c.check_id == *verification_id)
                {
                    self.verification_checks.push(TuiVerificationCheck {
                        check_id: verification_id.clone(),
                        mission_id: self.mission_id.clone().unwrap_or_default(),
                        task_id: target.clone(),
                        tier_num: 1,
                        tier_name: "deterministic".to_string(),
                        status: "pending".to_string(),
                        command_or_tool: "verification".to_string(),
                        inputs_normalized: target.clone(),
                        evidence_artifact_id: None,
                        summary: format!("Verification running on target: {target}"),
                        failure_class: None,
                        snapshot_hash: "".to_string(),
                        created_at: ts,
                    });
                    self.recompute_verification_summary();
                    self.rebuild_traceability();
                }
            }
            EventType::VerificationCompleted {
                verification_id,
                passed,
                evidence,
                ..
            } => {
                self.lifecycle.apply_event(event);
                let level = if *passed { "INFO" } else { "ERROR" };
                let title = if *passed {
                    "Verification passed"
                } else {
                    "Verification failed"
                };
                self.add_log(level, format!("{title}: {evidence}"), "verification");
                self.add_timeline_entry(seq, ts, "verification", title, evidence, level);

                if let Some(c) = self
                    .verification_checks
                    .iter_mut()
                    .find(|c| c.check_id == *verification_id)
                {
                    c.status = if *passed {
                        "passed".to_string()
                    } else {
                        "failed".to_string()
                    };
                    c.summary = evidence.clone();
                } else {
                    self.verification_checks.push(TuiVerificationCheck {
                        check_id: verification_id.clone(),
                        mission_id: self.mission_id.clone().unwrap_or_default(),
                        task_id: "".to_string(),
                        tier_num: 0,
                        tier_name: "unverified".to_string(),
                        status: if *passed {
                            "missing_evidence".to_string()
                        } else {
                            "failed".to_string()
                        },
                        command_or_tool: "unresolved_check".to_string(),
                        inputs_normalized: "authoritative_record_unavailable".to_string(),
                        evidence_artifact_id: None,
                        summary: evidence.clone(),
                        failure_class: if *passed {
                            Some("MissingVerificationEvidence".to_string())
                        } else {
                            Some("VerificationFailure".to_string())
                        },
                        snapshot_hash: "unavailable".to_string(),
                        created_at: ts,
                    });
                }
                self.recompute_verification_summary();
                self.rebuild_traceability();
            }
            EventType::QualityGateEvaluated {
                step_key,
                tier,
                passed,
                ..
            } => {
                let status_str = if *passed { "passed" } else { "failed" };
                let level = if *passed { "INFO" } else { "ERROR" };
                self.add_log(
                    level,
                    format!("Quality gate {status_str} for step {step_key} (tier {tier})"),
                    "verification",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "quality_gate",
                    format!("Quality gate {status_str}: {step_key}"),
                    format!("Tier {tier}"),
                    level,
                );
                self.verification_checks.push(TuiVerificationCheck {
                    check_id: format!("gate-{step_key}-{tier}"),
                    mission_id: self.mission_id.clone().unwrap_or_default(),
                    task_id: step_key.clone(),
                    tier_num: 1,
                    tier_name: tier.clone(),
                    status: status_str.to_string(),
                    command_or_tool: format!("quality_gate_{tier}"),
                    inputs_normalized: step_key.clone(),
                    evidence_artifact_id: None,
                    summary: format!("Quality gate evaluation for step {step_key} on tier {tier}"),
                    failure_class: if *passed {
                        None
                    } else {
                        Some("QualityGateFailure".to_string())
                    },
                    snapshot_hash: "".to_string(),
                    created_at: ts,
                });
                self.recompute_verification_summary();
                self.rebuild_traceability();
            }
            EventType::RecoveryAttempted {
                recovery_id,
                strategy,
                success,
                ..
            } => {
                let level = if *success { "INFO" } else { "WARN" };
                self.add_log(
                    level,
                    format!("Recovery attempt {recovery_id} ({strategy}): success={success}"),
                    "recovery",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "recovery",
                    format!("Recovery attempt: {strategy}"),
                    format!("ID: {recovery_id}, success={success}"),
                    level,
                );
                self.recovery_attempts.push(TuiRecoverySnapshot {
                    attempt_id: recovery_id.clone(),
                    failure_class: "ExecutionFailure".to_string(),
                    strategy: strategy.clone(),
                    attempt_number: self.recovery_attempts.len() + 1,
                    checkpoint_id: None,
                    affected_mission_id: self.mission_id.clone(),
                    affected_task_id: None,
                    outcome: if *success {
                        "success".to_string()
                    } else {
                        "failed".to_string()
                    },
                    budget_consumed: 1,
                    backoff_delay_ms: 0,
                    action_taken: format!("Executed recovery strategy: {strategy}"),
                });
            }
            EventType::CheckpointCreated {
                checkpoint_id,
                description,
                ..
            } => {
                self.add_log(
                    "INFO",
                    format!("Checkpoint created: {checkpoint_id}"),
                    "checkpoint",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "checkpoint",
                    "Checkpoint created",
                    format!("{checkpoint_id}: {description}"),
                    "INFO",
                );
            }
            EventType::CheckpointRestored { checkpoint_id, .. } => {
                self.add_log(
                    "WARN",
                    format!("Checkpoint restored: {checkpoint_id}"),
                    "checkpoint",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "checkpoint",
                    "Checkpoint restored",
                    checkpoint_id.to_string(),
                    "WARN",
                );
            }
            EventType::MissionCompleted { .. } => {
                self.lifecycle.apply_event(event);
                self.mission_status = "completed".to_string();
                self.add_log("INFO", "Mission completed successfully", "runtime");
                self.add_timeline_entry(
                    seq,
                    ts,
                    "lifecycle",
                    "Mission completed",
                    "Completed successfully",
                    "INFO",
                );
                self.rebuild_traceability();
            }
            EventType::MissionFailed { reason, .. } => {
                self.lifecycle.apply_event(event);
                self.mission_status = "failed".to_string();
                self.add_conversation_item(TuiConversationItem::Failure {
                    context: "runtime failure".to_string(),
                    reason: reason.clone(),
                    timestamp: Utc::now(),
                });
                self.add_log("ERROR", format!("Mission failed: {reason}"), "runtime");
                self.add_timeline_entry(seq, ts, "lifecycle", "Mission failed", reason, "ERROR");
                self.rebuild_traceability();
            }
            EventType::MissionCancelled { reason, .. } => {
                self.lifecycle.apply_event(event);
                self.mission_status = "cancelled".to_string();
                self.add_log("WARN", format!("Mission cancelled: {reason}"), "runtime");
                self.add_timeline_entry(seq, ts, "lifecycle", "Mission cancelled", reason, "WARN");
                self.rebuild_traceability();
            }
            EventType::TaskBlocked {
                task_id, reason, ..
            } => {
                self.lifecycle.apply_event(event);
                let tid_str = task_id.to_string();
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == tid_str) {
                    t.status = "blocked".to_string();
                }
                self.add_conversation_item(TuiConversationItem::Failure {
                    context: "task blocked".to_string(),
                    reason: reason.clone(),
                    timestamp: Utc::now(),
                });
                self.add_log("WARN", format!("Task blocked: {reason}"), "scheduler");
                self.add_timeline_entry(seq, ts, "task", "Task blocked", reason, "WARN");
            }
            EventType::TaskUnblocked { task_id, .. } => {
                self.lifecycle.apply_event(event);
                let tid_str = task_id.to_string();
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == tid_str) {
                    t.status = "ready".to_string();
                }
                self.add_log("INFO", "Task unblocked", "scheduler");
                self.add_timeline_entry(seq, ts, "task", "Task unblocked", "", "INFO");
            }
            EventType::TaskCancelled {
                task_id, reason, ..
            } => {
                let tid_str = task_id.to_string();
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == tid_str) {
                    t.status = "cancelled".to_string();
                }
                self.add_log(
                    "WARN",
                    format!("Task {tid_str} cancelled: {reason}"),
                    "scheduler",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "task",
                    format!("Task {tid_str} cancelled"),
                    reason,
                    "WARN",
                );
                self.rebuild_traceability();
            }
            EventType::TaskSkipped {
                task_id, reason, ..
            } => {
                let tid_str = task_id.to_string();
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == tid_str) {
                    t.status = "skipped".to_string();
                }
                self.add_log(
                    "WARN",
                    format!("Task {tid_str} skipped: {reason}"),
                    "scheduler",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "task",
                    format!("Task {tid_str} skipped"),
                    reason,
                    "WARN",
                );
                self.rebuild_traceability();
            }
            EventType::TaskNeedsReview {
                task_id, reason, ..
            } => {
                let tid_str = task_id.to_string();
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == tid_str) {
                    t.status = "needs_review".to_string();
                }
                self.add_log(
                    "INFO",
                    format!("Task {tid_str} needs review: {reason}"),
                    "scheduler",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "task",
                    format!("Task {tid_str} needs review"),
                    reason,
                    "INFO",
                );
                self.rebuild_traceability();
            }
            EventType::GitStateChanged {
                workspace_branch,
                execution_branch,
                ..
            } => {
                self.git_branch = workspace_branch.clone();
                self.execution_worktree_branch = execution_branch.clone();
                self.is_dirty = true;
            }
            EventType::ModelStreamProgress {
                invocation_id,
                provider,
                model,
                elapsed_ms,
                reasoning_tokens_estimated,
                usage,
            } => {
                self.apply_interaction_event(&InteractionEvent::ModelStreamProgress {
                    invocation_id: invocation_id.to_string(),
                    provider: provider.clone(),
                    model: model.clone(),
                    elapsed_ms: *elapsed_ms,
                    reasoning_tokens_estimated: *reasoning_tokens_estimated,
                    usage: usage.clone(),
                });
            }
            EventType::ModelStreamFinished {
                invocation_id,
                failed,
            } => {
                self.apply_interaction_event(&InteractionEvent::ModelStreamFinished {
                    invocation_id: invocation_id.to_string(),
                    failed: *failed,
                });
            }
            EventType::ModelUsageUpdated {
                invocation_id,
                usage,
                cost_usd,
                ..
            } => {
                if let Some(id) = invocation_id {
                    self.finalize_live_model_progress(&id.to_string(), Some(usage));
                }
                let cost_cents = cost_usd.map(|usd| (usd * 100.0).round() as u64);
                self.record_model_usage(
                    invocation_id.map(|id| id.to_string()),
                    usage.prompt_tokens as u64,
                    usage.completion_tokens as u64,
                    cost_cents,
                );
            }
            EventType::ArtifactCreated {
                artifact_id, path, ..
            } => {
                let aid_str = artifact_id.to_string();
                self.add_log(
                    "INFO",
                    format!("Artifact created: {path} ({aid_str})"),
                    "artifacts",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "artifact",
                    "Artifact created",
                    format!("{path} ({aid_str})"),
                    "INFO",
                );
                if !self.artifacts.iter().any(|a| a.id == aid_str) {
                    let file_name = std::path::Path::new(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(path)
                        .to_string();
                    let ext = std::path::Path::new(path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_string();
                    self.artifacts.push(TuiArtifactSnapshot {
                        id: aid_str,
                        name: file_name,
                        logical_path: path.clone(),
                        content_hash: "hash_pending".to_string(),
                        size_bytes: 0,
                        extension: ext,
                        version: 1,
                        status: "valid".to_string(),
                        mission_id: self.mission_id.clone(),
                        task_id: None,
                        producer_role: None,
                        parent_artifact_ids: Vec::new(),
                        verification_check_ids: Vec::new(),
                        created_at: ts,
                        storage_state: "stored".to_string(),
                        quarantine_reason: None,
                    });
                    self.rebuild_traceability();
                }
            }
            EventType::WorkflowArtifactRecorded {
                artifact_id,
                name,
                path,
                ..
            } => {
                let aid_str = artifact_id.to_string();
                self.add_log(
                    "INFO",
                    format!("Workflow artifact recorded: {name} at {path} ({aid_str})"),
                    "artifacts",
                );
                self.add_timeline_entry(
                    seq,
                    ts,
                    "artifact",
                    format!("Workflow artifact: {name}"),
                    format!("{path} ({aid_str})"),
                    "INFO",
                );
                if let Some(existing) = self.artifacts.iter_mut().find(|a| a.id == aid_str) {
                    existing.name = name.clone();
                    existing.logical_path = path.clone();
                } else {
                    let ext = std::path::Path::new(path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_string();
                    self.artifacts.push(TuiArtifactSnapshot {
                        id: aid_str,
                        name: name.clone(),
                        logical_path: path.clone(),
                        content_hash: "hash_pending".to_string(),
                        size_bytes: 0,
                        extension: ext,
                        version: 1,
                        status: "valid".to_string(),
                        mission_id: self.mission_id.clone(),
                        task_id: None,
                        producer_role: None,
                        parent_artifact_ids: Vec::new(),
                        verification_check_ids: Vec::new(),
                        created_at: ts,
                        storage_state: "stored".to_string(),
                        quarantine_reason: None,
                    });
                    self.rebuild_traceability();
                }
            }
            _ => {
                self.add_log("DEBUG", format!("Event: {:?}", event.event_type), "bus");
                self.add_timeline_entry(
                    seq,
                    ts,
                    "bus",
                    format!("{:?}", event.event_type),
                    format!("Seq {seq}"),
                    "DEBUG",
                );
            }
        }
    }

    /// Append conversation item to the timeline.
    ///
    /// Follow-mode aware: while following, the viewport stays pinned to the
    /// bottom; while the operator reads older content, the item counts as
    /// unseen and the viewport is preserved at render time.
    pub fn add_conversation_item(&mut self, item: TuiConversationItem) {
        self.enter_active_session();
        self.conversation.push(item);
        if self.follow {
            self.scroll_offset = 0;
            self.unseen_count = 0;
        } else {
            self.unseen_count = self.unseen_count.saturating_add(1);
        }
        self.is_dirty = true;
    }

    /// Report an operator-visible error through conversation feed and internal logs.
    pub fn set_error(&mut self, message: impl Into<String>) {
        let msg = message.into();
        self.add_conversation_item(TuiConversationItem::Error {
            message: msg.clone(),
            timestamp: chrono::Utc::now(),
        });
        self.add_log("ERROR", msg, "tui");
        self.mark_dirty();
    }

    /// Insert or enrich the plan review card for a revision.
    ///
    /// Envelope events carry identity only (revision, plan id); coordinator
    /// responses later enrich the same card with hash, objective, and counts.
    /// Exactly one card exists per revision; content is never fabricated.
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_plan_review_card(
        &mut self,
        revision: u32,
        plan_id: String,
        content_hash: Option<String>,
        objective: Option<String>,
        task_count: Option<usize>,
        plan_markdown: Option<String>,
        tasks: Vec<crate::kernel::plan::CandidateTask>,
    ) {
        for item in self.conversation.iter_mut().rev() {
            if let TuiConversationItem::PlanReview {
                revision: r,
                content_hash: h,
                objective: o,
                task_count: c,
                plan_markdown: pm,
                tasks: ts,
                timestamp,
                ..
            } = item
            {
                if *r == revision {
                    if h.is_none() {
                        *h = content_hash;
                    }
                    if o.is_empty() {
                        if let Some(obj) = objective {
                            *o = obj;
                        }
                    }
                    if *c == 0 {
                        if let Some(n) = task_count {
                            *c = n;
                        }
                    }
                    if pm.is_none() {
                        *pm = plan_markdown;
                    }
                    if ts.is_empty() && !tasks.is_empty() {
                        *ts = tasks;
                    }
                    *timestamp = Utc::now();
                    self.is_dirty = true;
                    return;
                }
            }
        }
        self.add_conversation_item(TuiConversationItem::PlanReview {
            revision,
            plan_id,
            content_hash,
            objective: objective.unwrap_or_default(),
            task_count: task_count.unwrap_or(0),
            plan_markdown,
            tasks,
            timestamp: Utc::now(),
        });
    }

    /// Insert or enrich the task review card for a revision.
    pub fn upsert_task_review_card(
        &mut self,
        task_revision: u32,
        content_hash: Option<String>,
        task_count: Option<usize>,
        task_markdown: Option<String>,
        tasks: Vec<crate::kernel::plan::CandidateTask>,
    ) {
        let plan_revision = self.lifecycle.plan_revision.unwrap_or(0);
        for item in self.conversation.iter_mut().rev() {
            if let TuiConversationItem::TaskReview {
                task_revision: r,
                content_hash: h,
                task_count: c,
                plan_revision: pr,
                task_markdown: tm,
                tasks: ts,
                timestamp,
                ..
            } = item
            {
                if *r == task_revision {
                    if h.is_none() {
                        *h = content_hash;
                    }
                    if *c == 0 {
                        if let Some(n) = task_count {
                            *c = n;
                        }
                    }
                    if tm.is_none() {
                        *tm = task_markdown;
                    }
                    if ts.is_empty() && !tasks.is_empty() {
                        *ts = tasks;
                    }
                    *pr = plan_revision;
                    *timestamp = Utc::now();
                    self.is_dirty = true;
                    return;
                }
            }
        }
        self.add_conversation_item(TuiConversationItem::TaskReview {
            plan_revision,
            task_revision,
            content_hash,
            task_count: task_count.unwrap_or(0),
            task_markdown,
            tasks,
            timestamp: Utc::now(),
        });
    }

    /// Insert or refresh the execution authorization request card.
    pub fn upsert_auth_required_card(&mut self, message: String) {
        let plan_revision = self.lifecycle.plan_revision.unwrap_or(0);
        let task_revision = self.lifecycle.task_revision.unwrap_or(0);
        for item in self.conversation.iter_mut().rev() {
            if let TuiConversationItem::AuthRequired {
                message: m,
                timestamp,
                ..
            } = item
            {
                *m = message.clone();
                *timestamp = Utc::now();
                self.is_dirty = true;
                return;
            }
        }
        self.add_conversation_item(TuiConversationItem::AuthRequired {
            plan_revision,
            task_revision,
            message,
            timestamp: Utc::now(),
        });
    }

    /// Insert or refresh the authorization-granted card (authorized, not executing).
    pub fn upsert_auth_granted_card(&mut self, authorization_id: String) {
        let plan_revision = self.lifecycle.plan_revision.unwrap_or(0);
        let task_revision = self.lifecycle.task_revision.unwrap_or(0);
        for item in self.conversation.iter_mut().rev() {
            if let TuiConversationItem::AuthGranted {
                authorization_id: a,
                timestamp,
                ..
            } = item
            {
                if *a == authorization_id {
                    *timestamp = Utc::now();
                    self.is_dirty = true;
                    return;
                }
            }
        }
        self.add_conversation_item(TuiConversationItem::AuthGranted {
            authorization_id,
            plan_revision,
            task_revision,
            timestamp: Utc::now(),
        });
    }

    /// True when a failure card with the exact reason is already on the timeline.
    fn recent_failure_with_reason(&self, reason: &str) -> bool {
        self.conversation.iter().rev().take(6).any(|item| {
            matches!(
                item,
                TuiConversationItem::Failure { reason: r, .. } if r == reason
            )
        })
    }

    /// Remember recently settled invocation IDs to reject late progress events.
    fn mark_model_invocation_settled(&mut self, invocation_id: &str) {
        if self
            .settled_model_invocations
            .iter()
            .any(|settled| settled == invocation_id)
        {
            return;
        }
        self.settled_model_invocations
            .push_back(invocation_id.to_string());
        if self.settled_model_invocations.len() > 128 {
            self.settled_model_invocations.pop_front();
        }
    }

    /// Add the compact post-invocation thought summary and clear its live-only projection.
    /// Raw reasoning content is never stored in the TUI conversation.
    fn finalize_live_model_progress(
        &mut self,
        invocation_id: &str,
        usage: Option<&crate::model::types::TokenUsage>,
    ) {
        if self.live_model_invocation_id.as_deref() != Some(invocation_id) {
            return;
        }

        let elapsed_ms = self
            .activity_started_at
            .map(|started| (Utc::now() - started).num_milliseconds().max(0) as u64)
            .unwrap_or(0);
        let elapsed_secs = elapsed_ms as f64 / 1000.0;
        let tokens = match usage {
            Some(usage) if usage.reasoning_tokens > 0 => {
                format!("{} tokens", usage.reasoning_tokens)
            }
            Some(usage) if usage.total_tokens > 0 => {
                if usage.source == crate::model::types::UsageSource::AuthoritativeProvider {
                    format!("{} tokens", usage.total_tokens)
                } else {
                    format!("~{} tokens (estimated)", usage.total_tokens)
                }
            }
            _ => self
                .live_model_reasoning_tokens_estimated
                .map(|estimated| format!("~{estimated} reasoning tokens (estimated)"))
                .unwrap_or_else(|| "token count unavailable".to_string()),
        };

        self.add_conversation_item(TuiConversationItem::System {
            text: format!("Thought for {elapsed_secs:.0}s, {tokens}"),
            timestamp: Utc::now(),
        });
        self.mark_model_invocation_settled(invocation_id);
        self.live_model_invocation_id = None;
        self.live_model_provider = None;
        self.live_model_name = None;
        self.live_model_reasoning_tokens_estimated = None;
        self.live_model_usage = None;
        if self.activity_kind == ActivityKind::Thinking {
            self.activity_kind = ActivityKind::Idle;
            self.activity_message = None;
            self.activity_started_at = None;
            self.live_activity = None;
        }
        self.is_dirty = true;
    }

    /// Record model usage for an invocation idempotently and truthfully.
    /// Deduplicates token counts, api calls, and cost without losing late-arriving pricing.
    pub fn record_model_usage(
        &mut self,
        invocation_id: Option<String>,
        prompt_tokens: u64,
        completion_tokens: u64,
        cost_cents: Option<u64>,
    ) {
        if let Some(inv_id) = invocation_id {
            let is_new_tokens = self
                .model_usage
                .processed_invocations
                .insert(inv_id.clone());
            if is_new_tokens {
                self.model_usage.prompt_tokens += prompt_tokens;
                self.model_usage.completion_tokens += completion_tokens;
                self.model_usage.in_flight_prompt_tokens = 0;
                self.model_usage.in_flight_completion_tokens = 0;
                self.model_usage.api_calls += 1;
            }

            let mut is_new_cost = false;
            if let Some(cents) = cost_cents {
                if self.model_usage.priced_invocations.insert(inv_id) {
                    *self.model_usage.total_cost_cents.get_or_insert(0) += cents;
                    is_new_cost = true;
                }
            }

            if is_new_tokens || is_new_cost {
                self.is_dirty = true;
            }
        } else {
            // Provisional in-flight update from streaming chunks
            self.model_usage.in_flight_prompt_tokens = prompt_tokens;
            self.model_usage.in_flight_completion_tokens = completion_tokens;
            self.is_dirty = true;
        }
    }

    /// Reset session-scoped state when the active session changes.
    /// Invalidate prior session-scoped tasks, graph ID/revision, assignment info,
    /// and caches, resetting session-scoped telemetry cleanly (M31A-UI-21).
    pub fn reset_session_projection(&mut self, new_session_id: &str) {
        self.session_id = Some(new_session_id.to_string());
        self.session_status = "active".to_string();
        self.tasks.clear();
        self.active_graph_id = None;
        self.active_graph_revision = 0;
        self.task_graph_state = TaskGraphProjectionState::Loading;
        self.mission_id = None;
        self.execution_worktree_branch = None;
        self.conversation.clear();
        self.scroll_offset = 0;
        self.follow = true;
        self.unseen_count = 0;
        self.checkpoint_id = None;
        self.model_usage = TuiModelUsage::default();
        self.live_model_invocation_id = None;
        self.live_model_provider = None;
        self.live_model_name = None;
        self.live_model_reasoning_tokens_estimated = None;
        self.live_model_usage = None;
        self.usage_section_state = SectionProjectionState::Loading;
        self.verification_section_state = SectionProjectionState::Loading;
        self.recovery_section_state = SectionProjectionState::Loading;
        self.git_section_state = SectionProjectionState::Loading;
        self.task_graph_section_state = SectionProjectionState::Loading;
        self.rebuild_traceability();
        self.is_dirty = true;
    }

    /// Hydrate historical model usage idempotently from persisted records (M31A-UI-21).
    pub fn hydrate_persisted_usage(
        &mut self,
        records: &[crate::model::persistence::invocation::ModelInvocationRecord],
    ) {
        for r in records {
            let cost_cents = r.cost_usd.map(|usd| (usd * 100.0).round() as u64);
            self.record_model_usage(
                Some(r.id.to_string()),
                r.prompt_tokens as u64,
                r.completion_tokens as u64,
                cost_cents,
            );
        }
        self.usage_section_state = SectionProjectionState::Ready;
        self.is_dirty = true;
    }

    /// Load durable session history into the conversation timeline.
    pub fn load_session_history(&mut self, turns: &[ConversationTurn]) {
        self.conversation.clear();
        for turn in turns {
            self.conversation.push(TuiConversationItem::from_turn(turn));
        }
        if !self.conversation.is_empty() {
            self.enter_active_session();
        }
        self.scroll_to_bottom();
    }

    /// Authoritatively reconcile the in-memory task graph with an incoming snapshot.
    ///
    /// Preserves existing task identity, runtime execution status, and progress where valid,
    /// removes obsolete tasks absent from the incoming snapshot, preserves snapshot display order,
    /// and drops stale revisions that arrive out of order.
    pub fn reconcile_task_graph(
        &mut self,
        graph_id: &str,
        revision: u32,
        tasks: &[crate::events::types::TaskSummary],
    ) -> bool {
        if self.active_graph_id.as_deref() == Some(graph_id)
            && revision < self.active_graph_revision
        {
            return false;
        }
        self.active_graph_id = Some(graph_id.to_string());
        self.active_graph_revision = revision;

        let mut reconciled = Vec::with_capacity(tasks.len());
        for t in tasks {
            let tid_str = t.id.to_string();
            let initial_pct = if t.status == crate::state_machine::task::TaskState::Succeeded {
                100
            } else {
                0
            };
            if let Some(existing) = self.tasks.iter().find(|task| task.id == tid_str) {
                let mut updated = existing.clone();
                updated.title = t.title.clone();
                updated.agent_role = Some(t.role.to_string());
                updated.dependencies = t.dependencies.iter().map(|d| d.to_string()).collect();
                if updated.status == "pending" || updated.status.is_empty() {
                    updated.status = t.status.to_string();
                    updated.progress_pct = initial_pct;
                }
                reconciled.push(updated);
            } else {
                reconciled.push(TuiTaskSnapshot {
                    id: tid_str,
                    title: t.title.clone(),
                    status: t.status.to_string(),
                    agent_role: Some(t.role.to_string()),
                    progress_pct: initial_pct,
                    dependencies: t.dependencies.iter().map(|d| d.to_string()).collect(),
                    assigned_agent_id: None,
                    execution_result: None,
                    failure_reason: None,
                    retry_count: 0,
                });
            }
        }
        self.tasks = reconciled;
        self.task_graph_state = TaskGraphProjectionState::Loaded;
        self.is_dirty = true;
        true
    }

    /// Maximum valid scroll offset for the last measured frame.
    pub fn max_scroll(&self) -> usize {
        self.content_height.saturating_sub(self.viewport_height)
    }

    /// Viewport-sized page step for PageUp/PageDown.
    pub fn page_step(&self) -> usize {
        if self.viewport_height >= 2 {
            self.viewport_height.saturating_sub(1).max(1)
        } else {
            10
        }
    }

    fn clamp_scroll(&mut self) {
        let max = self.max_scroll();
        if self.scroll_offset > max {
            self.scroll_offset = max;
        }
        if max == 0 || self.scroll_offset == 0 {
            // Content fits, or pinned to the bottom: following again.
            self.scroll_offset = 0;
            self.follow = true;
            self.unseen_count = 0;
        }
        self.is_dirty = true;
    }

    /// Scroll conversation viewport up by N lines (toward older content).
    /// Manual scrolling leaves follow mode; the viewport is preserved.
    pub fn scroll_up(&mut self, lines: usize) {
        if self.max_scroll() == 0 {
            self.scroll_offset = 0;
            self.follow = true;
            self.unseen_count = 0;
            self.is_dirty = true;
            return;
        }
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
        let max = self.max_scroll();
        if self.scroll_offset > max {
            self.scroll_offset = max;
        }
        if self.scroll_offset != 0 {
            self.follow = false;
        } else {
            self.follow = true;
            self.unseen_count = 0;
        }
        self.is_dirty = true;
    }

    /// Scroll conversation viewport down by N lines (toward newer content).
    /// Reaching the bottom restores follow mode.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        self.clamp_scroll();
    }

    /// Reset scroll offset to track bottom of conversation.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.follow = true;
        self.unseen_count = 0;
        self.is_dirty = true;
    }

    /// Jump to the oldest conversation content.
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = self.max_scroll();
        if self.max_scroll() > 0 {
            self.follow = false;
        }
        self.is_dirty = true;
    }

    /// Scroll by mouse wheel steps. `up == true` moves toward older content.
    pub fn scroll_wheel(&mut self, up: bool) {
        if up {
            self.scroll_up(3);
        } else {
            self.scroll_down(3);
        }
    }

    /// Record the measured frame geometry after rendering the conversation.
    ///
    /// Called by the conversation renderer with real content/viewport
    /// heights. When new lines arrived while the operator reads older
    /// content, the offset grows by the delta so the viewport is preserved
    /// instead of being yanked to the bottom.
    pub fn note_rendered_frame(&mut self, total_lines: usize, viewport_height: usize) {
        self.viewport_height = viewport_height;
        self.content_height = total_lines;
        if !self.follow && total_lines > self.last_total_lines {
            self.scroll_offset = self
                .scroll_offset
                .saturating_add(total_lines.saturating_sub(self.last_total_lines));
        }
        if self.follow {
            self.scroll_offset = 0;
        }
        self.last_total_lines = total_lines;
        let max = self.max_scroll();
        if self.scroll_offset > max {
            self.scroll_offset = max;
        }
        if max == 0 || self.scroll_offset == 0 {
            self.scroll_offset = 0;
            self.follow = true;
            self.unseen_count = 0;
        }
        self.is_dirty = true;
    }

    /// Apply decoupled InteractionEvent stream item to view model state.
    pub fn apply_interaction_event(&mut self, event: &InteractionEvent) {
        self.system_stats.events_processed += 1;
        self.is_dirty = true;

        let now = Utc::now();

        match event {
            InteractionEvent::SessionStarted { session_id } => {
                let sid_str = session_id.to_string();
                if self.session_id.as_deref() != Some(&sid_str) {
                    self.reset_session_projection(&sid_str);
                } else {
                    self.session_status = "active".to_string();
                }
                self.settle_request();
                self.scroll_to_bottom();
                self.add_conversation_item(TuiConversationItem::System {
                    text: format!("Session started: {session_id}"),
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::SessionResumed { session_id } => {
                let sid_str = session_id.to_string();
                if self.session_id.as_deref() != Some(&sid_str) {
                    self.reset_session_projection(&sid_str);
                } else {
                    self.session_status = "active".to_string();
                }
                self.enter_active_session();
                self.settle_request();
                self.scroll_to_bottom();
                self.add_conversation_item(TuiConversationItem::System {
                    text: format!("Session resumed: {session_id}"),
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::SessionHistoryLoaded { session_id, turns } => {
                let sid_str = session_id.to_string();
                if self.session_id.as_deref() == Some(&sid_str) {
                    self.session_status = "active".to_string();
                    self.load_session_history(turns);
                }
            }
            InteractionEvent::SessionConversationCleared { .. } => {
                self.conversation.clear();
                self.scroll_offset = 0;
                self.follow = true;
                self.unseen_count = 0;
                self.is_dirty = true;
            }
            InteractionEvent::ModelActivity { text } => {
                self.live_activity = Some(text.clone());
                self.activity_kind = ActivityKind::Thinking;
                self.activity_message = Some(text.clone());
                self.activity_started_at = Some(now);
                self.add_log("INFO", format!("Model: {text}"), "model");
            }
            InteractionEvent::ModelStreamProgress {
                invocation_id,
                provider,
                model,
                elapsed_ms,
                reasoning_tokens_estimated,
                usage,
            } => {
                if self
                    .settled_model_invocations
                    .iter()
                    .any(|settled| settled == invocation_id)
                {
                    return;
                }
                if self.live_model_invocation_id.as_deref() != Some(invocation_id.as_str()) {
                    self.live_model_invocation_id = Some(invocation_id.clone());
                    self.activity_started_at = Some(
                        now - chrono::Duration::milliseconds(
                            (*elapsed_ms).min(i64::MAX as u64) as i64,
                        ),
                    );
                    self.live_model_usage = None;
                }
                self.live_model_provider = Some(provider.clone());
                self.live_model_name = Some(model.clone());
                self.live_model_reasoning_tokens_estimated = *reasoning_tokens_estimated;
                if let Some(usage) = usage {
                    self.live_model_usage = Some(usage.clone());
                }
                self.activity_kind = ActivityKind::Thinking;
                self.activity_message = Some(format!("Reasoning with {provider}/{model}"));
                self.live_activity = Some(self.activity_message.clone().unwrap_or_default());
            }
            InteractionEvent::ModelStreamFinished {
                invocation_id,
                failed,
            } => {
                let was_active =
                    self.live_model_invocation_id.as_deref() == Some(invocation_id.as_str());
                self.mark_model_invocation_settled(invocation_id);
                if was_active {
                    self.live_model_invocation_id = None;
                    self.live_model_provider = None;
                    self.live_model_name = None;
                    self.live_model_reasoning_tokens_estimated = None;
                    self.live_model_usage = None;
                    self.activity_started_at = None;
                    self.live_activity = None;
                    if *failed {
                        self.activity_kind = ActivityKind::Failed;
                        self.activity_message = Some("Model invocation failed".to_string());
                    } else if self.activity_kind == ActivityKind::Thinking {
                        self.activity_kind = ActivityKind::Idle;
                        self.activity_message = None;
                    }
                    self.is_dirty = true;
                }
            }
            InteractionEvent::AssistantStarted { message_id } => {
                self.enter_active_session();
                self.live_activity = Some("Receiving assistant response...".to_string());
                self.activity_kind = ActivityKind::Thinking;
                self.activity_message = Some("Streaming response...".to_string());
                self.activity_started_at = Some(now);
                self.active_stream_message_id = Some(message_id.clone());
                self.add_conversation_item(TuiConversationItem::Assistant {
                    id: message_id.clone(),
                    sequence: self.conversation.len() as u64 + 1,
                    text: String::new(),
                    streaming: true,
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::AssistantDelta { message_id, delta } => {
                if let Some(item) = self.conversation.iter_mut().rev().find(|i| match i {
                    TuiConversationItem::Assistant { id, streaming, .. } => {
                        id == message_id && *streaming
                    }
                    _ => false,
                }) {
                    if let TuiConversationItem::Assistant { text, .. } = item {
                        text.push_str(delta);
                    }
                } else {
                    self.active_stream_message_id = Some(message_id.clone());
                    self.add_conversation_item(TuiConversationItem::Assistant {
                        id: message_id.clone(),
                        sequence: self.conversation.len() as u64 + 1,
                        text: delta.clone(),
                        streaming: true,
                        timestamp: Utc::now(),
                    });
                }
            }
            InteractionEvent::AssistantFinished { message_id } => {
                self.last_settled_request_id = self.active_request_id.take();
                self.active_command = None;
                self.live_activity = None;
                self.activity_kind = ActivityKind::Idle;
                self.activity_message = None;
                self.activity_started_at = None;
                self.live_model_invocation_id = None;
                self.live_model_provider = None;
                self.live_model_name = None;
                self.live_model_reasoning_tokens_estimated = None;
                self.live_model_usage = None;
                self.active_stream_message_id = None;
                if let Some(TuiConversationItem::Assistant { streaming, .. }) =
                    self.conversation.iter_mut().rev().find(|i| match i {
                        TuiConversationItem::Assistant { id, .. } => id == message_id,
                        _ => false,
                    })
                {
                    *streaming = false;
                }
            }
            InteractionEvent::AssistantFailed { message_id, error } => {
                self.last_settled_request_id = self.active_request_id.take();
                self.active_command = None;
                self.live_activity = None;
                self.activity_kind = ActivityKind::Failed;
                self.activity_message = Some(format!("Assistant failed: {error}"));
                self.activity_started_at = None;
                self.live_model_invocation_id = None;
                self.live_model_provider = None;
                self.live_model_name = None;
                self.live_model_reasoning_tokens_estimated = None;
                self.live_model_usage = None;
                self.active_stream_message_id = None;
                if let Some(TuiConversationItem::Assistant {
                    text, streaming, ..
                }) = self.conversation.iter_mut().rev().find(|i| match i {
                    TuiConversationItem::Assistant { id, .. } => id == message_id,
                    _ => false,
                }) {
                    *streaming = false;
                    if text.is_empty() {
                        *text = format!("[Error: {error}]");
                    }
                } else {
                    self.add_conversation_item(TuiConversationItem::Error {
                        message: format!("Assistant error: {error}"),
                        timestamp: Utc::now(),
                    });
                }
            }
            InteractionEvent::ToolStarted {
                call_id,
                tool_name,
                parameters,
            } => {
                self.enter_active_session();
                self.live_activity = Some(format!("Running tool `{tool_name}`..."));
                self.activity_kind = ActivityKind::RunningTool;
                self.activity_message = Some(format!("Running `{tool_name}`..."));
                self.activity_started_at = Some(now);
                let params_str = serde_json::to_string(parameters).unwrap_or_default();
                self.live_tools.push(LiveToolOperation {
                    call_id: call_id.clone(),
                    tool_name: tool_name.clone(),
                    parameters: params_str.clone(),
                    state: LiveToolState::Running,
                    started_at: now,
                    completed_at: None,
                    duration_ms: None,
                    output_preview: None,
                });
                self.add_conversation_item(TuiConversationItem::ToolActivity {
                    call_id: call_id.clone(),
                    tool_name: tool_name.clone(),
                    parameters: params_str,
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::ToolCompleted {
                call_id,
                tool_name,
                success,
                output_preview,
            } => {
                self.enter_active_session();
                let (duration_ms, resolved_name) = if let Some(live_op) = self
                    .live_tools
                    .iter_mut()
                    .rev()
                    .find(|t| t.call_id == *call_id)
                {
                    live_op.state = if *success {
                        LiveToolState::Completed
                    } else {
                        LiveToolState::Failed
                    };
                    live_op.completed_at = Some(now);
                    let dur = (now - live_op.started_at).num_milliseconds().max(0) as u64;
                    live_op.duration_ms = Some(dur);
                    live_op.output_preview = Some(output_preview.clone());
                    (Some(dur), live_op.tool_name.clone())
                } else {
                    (None, tool_name.clone())
                };

                let name = if tool_name.is_empty() {
                    resolved_name
                } else {
                    tool_name.clone()
                };

                // The tool finished: no stale "Running tool…" may survive.
                self.live_activity = None;
                self.last_settled_request_id = self.active_request_id.take();
                self.active_command = None;

                let dur_str = duration_ms.map(|d| format!(" ({d}ms)")).unwrap_or_default();
                if *success {
                    self.activity_kind = ActivityKind::Completed;
                    self.activity_message = Some(format!("Tool `{name}` completed{dur_str}"));
                } else {
                    self.activity_kind = ActivityKind::Failed;
                    self.activity_message = Some(format!("Tool `{name}` failed{dur_str}"));
                }
                self.add_conversation_item(TuiConversationItem::ToolResult {
                    call_id: call_id.clone(),
                    tool_name: name,
                    success: *success,
                    output_preview: output_preview.clone(),
                    expanded: false,
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::AssistantOutput { text } => {
                self.enter_active_session();
                self.last_settled_request_id = self.active_request_id.take();
                self.active_command = None;
                self.live_activity = None;
                self.activity_kind = ActivityKind::Idle;
                self.activity_message = None;
                self.activity_started_at = None;
                self.active_stream_message_id = None;

                let already_reconciled = if let Some(TuiConversationItem::Assistant {
                    text: existing_text,
                    streaming,
                    ..
                }) = self
                    .conversation
                    .iter_mut()
                    .rev()
                    .find(|i| matches!(i, TuiConversationItem::Assistant { .. }))
                {
                    if *streaming {
                        *streaming = false;
                        if !text.is_empty() {
                            *existing_text = text.clone();
                        }
                        true
                    } else {
                        existing_text == text
                    }
                } else {
                    false
                };

                if !already_reconciled {
                    self.add_conversation_item(TuiConversationItem::Assistant {
                        id: uuid::Uuid::now_v7().to_string(),
                        sequence: self.conversation.len() as u64 + 1,
                        text: text.clone(),
                        streaming: false,
                        timestamp: Utc::now(),
                    });
                }
            }
            InteractionEvent::VerificationStarted {
                verification_id,
                mission_id,
                target,
            } => {
                if !self
                    .verification_checks
                    .iter()
                    .any(|c| c.check_id == *verification_id)
                {
                    self.verification_checks.push(TuiVerificationCheck {
                        check_id: verification_id.clone(),
                        mission_id: mission_id.clone(),
                        task_id: target.clone(),
                        tier_num: 1,
                        tier_name: "deterministic".to_string(),
                        status: "pending".to_string(),
                        command_or_tool: "verification".to_string(),
                        inputs_normalized: target.clone(),
                        evidence_artifact_id: None,
                        summary: format!("Verification running on target: {target}"),
                        failure_class: None,
                        snapshot_hash: "".to_string(),
                        created_at: Utc::now(),
                    });
                    self.recompute_verification_summary();
                    self.rebuild_traceability();
                }
            }
            InteractionEvent::VerificationPassed {
                summary,
                verification_id,
                check,
            } => {
                self.enter_active_session();
                self.last_settled_request_id = self.active_request_id.take();
                self.active_command = None;
                self.live_activity = None;
                self.activity_kind = ActivityKind::Completed;
                self.activity_message = Some(format!("Verification passed: {summary}"));
                if !self.lifecycle.stage.is_terminal() {
                    self.lifecycle.stage = TuiLifecycleStage::Verifying;
                    self.lifecycle.verification_summary = Some(summary.clone());
                }
                self.add_conversation_item(TuiConversationItem::Verification {
                    sequence: self.conversation.len() as u64 + 1,
                    passed: true,
                    summary: summary.clone(),
                    timestamp: Utc::now(),
                });
                if let Some(c) = check {
                    if let Some(existing) = self
                        .verification_checks
                        .iter_mut()
                        .find(|k| k.check_id == c.check_id)
                    {
                        *existing = c.clone();
                    } else {
                        self.verification_checks.push(c.clone());
                    }
                } else if let Some(vid) = verification_id {
                    if let Some(existing) = self
                        .verification_checks
                        .iter_mut()
                        .find(|k| k.check_id == *vid)
                    {
                        existing.status = "passed".to_string();
                        existing.summary = summary.clone();
                    } else {
                        self.verification_checks.push(TuiVerificationCheck {
                            check_id: vid.clone(),
                            mission_id: self.mission_id.clone().unwrap_or_default(),
                            task_id: "".to_string(),
                            tier_num: 0,
                            tier_name: "unverified".to_string(),
                            status: "missing_evidence".to_string(),
                            command_or_tool: "unresolved_check".to_string(),
                            inputs_normalized: "authoritative_record_unavailable".to_string(),
                            evidence_artifact_id: None,
                            summary: format!("Unresolved verification outcome without authoritative evidence: {summary}"),
                            failure_class: Some("MissingVerificationEvidence".to_string()),
                            snapshot_hash: "unavailable".to_string(),
                            created_at: Utc::now(),
                        });
                    }
                }
                self.recompute_verification_summary();
                self.rebuild_traceability();
            }
            InteractionEvent::VerificationFailed {
                summary,
                verification_id,
                check,
            } => {
                self.enter_active_session();
                self.last_settled_request_id = self.active_request_id.take();
                self.active_command = None;
                self.live_activity = None;
                self.activity_kind = ActivityKind::Failed;
                self.activity_message = Some(format!("Verification failed: {summary}"));
                self.lifecycle.stage = TuiLifecycleStage::Failed;
                self.lifecycle.failure_reason = Some(summary.clone());
                self.add_conversation_item(TuiConversationItem::Verification {
                    sequence: self.conversation.len() as u64 + 1,
                    passed: false,
                    summary: summary.clone(),
                    timestamp: Utc::now(),
                });
                if let Some(c) = check {
                    if let Some(existing) = self
                        .verification_checks
                        .iter_mut()
                        .find(|k| k.check_id == c.check_id)
                    {
                        *existing = c.clone();
                    } else {
                        self.verification_checks.push(c.clone());
                    }
                } else if let Some(vid) = verification_id {
                    if let Some(existing) = self
                        .verification_checks
                        .iter_mut()
                        .find(|k| k.check_id == *vid)
                    {
                        existing.status = "failed".to_string();
                        existing.summary = summary.clone();
                        existing.failure_class = Some("VerificationFailure".to_string());
                    } else {
                        self.verification_checks.push(TuiVerificationCheck {
                            check_id: vid.clone(),
                            mission_id: self.mission_id.clone().unwrap_or_default(),
                            task_id: "".to_string(),
                            tier_num: 0,
                            tier_name: "unverified".to_string(),
                            status: "failed".to_string(),
                            command_or_tool: "unresolved_check".to_string(),
                            inputs_normalized: "authoritative_record_unavailable".to_string(),
                            evidence_artifact_id: None,
                            summary: summary.clone(),
                            failure_class: Some("VerificationFailure".to_string()),
                            snapshot_hash: "unavailable".to_string(),
                            created_at: Utc::now(),
                        });
                    }
                }
                self.recompute_verification_summary();
                self.rebuild_traceability();
            }
            // Generic tool/policy approvals are distinct from plan acceptance,
            // task acceptance, and execution authorization. They must never
            // move the governed lifecycle projection.
            InteractionEvent::ApprovalRequested {
                request_id,
                tool_name,
                details,
                risk_tier,
                parameters_summary,
                agent_role,
                timeout_seconds: _,
            } => {
                self.enter_active_session();
                self.activity_kind = ActivityKind::WaitingForApproval;
                let display_tool = if tool_name.is_empty() {
                    "Unavailable"
                } else {
                    tool_name.as_str()
                };
                self.activity_message = Some(format!("Approval required: {display_tool}"));
                self.activity_started_at = Some(now);

                let req = TuiApprovalRequest {
                    id: request_id.clone(),
                    tool_name: display_tool.to_string(),
                    agent_role: agent_role
                        .clone()
                        .unwrap_or_else(|| "Unavailable".to_string()),
                    justification: details.clone(),
                    parameters_summary: parameters_summary.clone().unwrap_or_default(),
                    risk_tier: risk_tier
                        .clone()
                        .unwrap_or_else(|| "Unavailable".to_string()),
                    timestamp: Utc::now(),
                };

                if let Some(existing) = self.approvals.iter_mut().find(|a| a.id == *request_id) {
                    *existing = req;
                } else {
                    self.approvals.push(req);
                }

                self.add_conversation_item(TuiConversationItem::Approval {
                    request_id: request_id.clone(),
                    tool_name: display_tool.to_string(),
                    details: details.clone(),
                    decision: None,
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::ApprovalResolved {
                request_id,
                approved,
            } => {
                let already_resolved = self.conversation.iter().any(|item| {
                    if let TuiConversationItem::Approval {
                        request_id: id,
                        decision,
                        ..
                    } = item
                    {
                        id == request_id && decision.is_some()
                    } else {
                        false
                    }
                });
                if already_resolved {
                    return;
                }

                self.live_activity = None;
                self.last_settled_request_id = self.active_request_id.take();
                self.active_command = None;
                self.activity_kind = if *approved {
                    ActivityKind::Completed
                } else {
                    ActivityKind::Failed
                };
                self.activity_message = Some(if *approved {
                    "Action approved".to_string()
                } else {
                    "Action denied".to_string()
                });
                self.approvals.retain(|a| a.id != *request_id);
                self.pending_approvals.retain(|a| a.id != *request_id);
                let decision_str = if *approved { "Approved" } else { "Denied" };
                self.add_conversation_item(TuiConversationItem::Approval {
                    request_id: request_id.clone(),
                    tool_name: "".to_string(),
                    details: format!("Decision: {decision_str}"),
                    decision: Some(decision_str.to_string()),
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::MissionStateChanged { mission_id, status } => {
                self.mission_id = Some(mission_id.to_string());
                self.mission_status = status.to_lowercase();
                self.add_log(
                    "INFO",
                    format!("Mission {mission_id} state changed to {status}"),
                    "runtime",
                );
            }
            InteractionEvent::CommandOutput { text } => {
                // A command result is terminal for the dispatched command:
                // clear every activity field so no stale spinner survives.
                // The result itself is the first-class conversation record.
                self.enter_active_session();
                self.settle_request();
                self.add_conversation_item(TuiConversationItem::System {
                    text: text.clone(),
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::ApprovalFailed { request_id, error } => {
                if let Some(pos) = self
                    .pending_approvals
                    .iter()
                    .position(|a| a.id == *request_id)
                {
                    let req = self.pending_approvals.remove(pos);
                    if !self.approvals.iter().any(|a| a.id == req.id) {
                        self.approvals.push(req);
                    }
                }
                self.add_log(
                    "WARN",
                    format!("Approval {request_id} failed: {error}"),
                    "approval",
                );
                self.is_dirty = true;
            }
            InteractionEvent::Error { message } => {
                self.enter_active_session();
                self.fail_request(message.clone());
                // Generic runtime errors must not clear pending rollback target (AUD-02, AUD-27)
                if let Some(ref mut d) = self.pending_decision {
                    if d.state.is_submitting() {
                        d.state = crate::interaction::decision::DecisionState::Pending;
                    }
                }
                self.add_conversation_item(TuiConversationItem::Error {
                    message: message.clone(),
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::Completion { summary } => {
                self.enter_active_session();
                self.last_settled_request_id = self.active_request_id.take();
                self.active_command = None;
                self.live_activity = None;
                self.activity_kind = ActivityKind::Completed;
                self.activity_message = Some(format!("Completed: {summary}"));
                // Legacy agent-turn completion. Terminal lifecycle truth comes
                // from MissionCompleted; never overwrite a sealed stage here.
                if !self.lifecycle.stage.is_terminal() {
                    self.lifecycle.stage = TuiLifecycleStage::Completed;
                }
                self.add_conversation_item(TuiConversationItem::System {
                    text: format!("Completed: {summary}"),
                    timestamp: Utc::now(),
                });
            }
            // ── Governed lifecycle responses ──
            // Produced by the TUI runtime bridge from canonical coordinator
            // responses. Each carries authoritative revision identity; hashes
            // come from the runtime's canonical artifact serialization.
            InteractionEvent::DiscoveryRequired {
                session_id,
                questions,
            } => {
                self.enter_active_session();
                self.last_settled_request_id = self.active_request_id.take();
                self.active_command = None;
                self.live_activity = None;
                self.activity_kind = ActivityKind::Discovering;
                self.activity_message = Some(format!("Discovery: {} questions", questions.len()));
                self.activity_started_at = Some(now);
                if !self.lifecycle.stage.is_terminal() {
                    self.lifecycle.session_id = Some(session_id.clone());
                    self.lifecycle.stage = TuiLifecycleStage::DiscoveryRequired;
                    self.lifecycle.pending_discovery_questions = questions.clone();
                    self.lifecycle.pending_questions =
                        questions.iter().map(|q| q.text.clone()).collect();
                    self.lifecycle.selected_option_index = questions
                        .first()
                        .and_then(|q| if q.options.is_empty() { None } else { Some(0) });
                }
                self.add_conversation_item(TuiConversationItem::Discovery {
                    questions: questions.iter().map(|q| q.text.clone()).collect(),
                    structured_questions: questions.clone(),
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::DecisionRequested { decision } => {
                self.enter_active_session();
                self.last_settled_request_id = self.active_request_id.take();
                self.live_activity = None;
                self.activity_kind = ActivityKind::WaitingForApproval;
                self.activity_message = Some(decision.title.clone());
                self.activity_started_at = Some(now);
                if !self.lifecycle.stage.is_terminal() {
                    self.lifecycle.session_id = Some(decision.session_id.clone());
                }
                self.pending_decision = Some(decision.clone());
                self.is_dirty = true;
                self.scroll_to_bottom();
            }
            InteractionEvent::DecisionResolved {
                decision_id,
                session_id: _,
                outcome,
            } => {
                if let Some(ref d) = self.pending_decision {
                    if d.id == *decision_id {
                        self.pending_decision = None;
                    }
                }
                self.activity_kind = ActivityKind::Executing;
                self.activity_message = Some(outcome.clone());
                self.activity_started_at = Some(now);
                self.add_conversation_item(TuiConversationItem::System {
                    text: format!("Decision resolved: {outcome}"),
                    timestamp: Utc::now(),
                });
                self.is_dirty = true;
                self.scroll_to_bottom();
            }
            InteractionEvent::PlanningChoiceRequired {
                session_id,
                prompt: _,
                question,
            } => {
                self.enter_active_session();
                self.last_settled_request_id = self.active_request_id.take();
                self.live_activity = None;
                self.activity_kind = ActivityKind::Discovering;
                self.activity_message = Some("Planning choice required".to_string());
                self.activity_started_at = Some(now);
                if !self.lifecycle.stage.is_terminal() {
                    self.lifecycle.session_id = Some(session_id.clone());
                }
                self.pending_decision = Some(
                    crate::interaction::decision::InteractiveDecision::planning_choice(
                        session_id, "", question,
                    ),
                );
                let mut dq = crate::workflow::genesis::DynamicQuestion::new(
                    "planning_choice",
                    "Operator decision on project-level planning documentation",
                    "project_planning_choice",
                    question.as_str(),
                );
                dq.options = vec![
                    "Create a plan first".into(),
                    "Implement directly".into(),
                    "Cancel request".into(),
                ];
                self.add_conversation_item(TuiConversationItem::Discovery {
                    questions: vec![question.clone()],
                    structured_questions: vec![dq],
                    timestamp: Utc::now(),
                });
                self.scroll_to_bottom();
            }
            InteractionEvent::PlanForReview {
                session_id,
                revision,
                plan_id,
                objective,
                task_count,
                content_hash,
                plan_markdown,
                tasks,
            } => {
                self.last_settled_request_id = self.active_request_id.take();
                self.active_command = None;
                self.live_activity = None;
                self.activity_kind = ActivityKind::WaitingForReview;
                self.activity_message = Some(format!(
                    "Plan R{revision} ready for review ({task_count} tasks)"
                ));
                self.activity_started_at = Some(now);
                if !self.lifecycle.stage.is_terminal() {
                    self.lifecycle.session_id = Some(session_id.clone());
                    self.lifecycle.stage = TuiLifecycleStage::PlanReviewRequired;
                    self.lifecycle.plan_revision = Some(*revision);
                    if content_hash.is_some() {
                        self.lifecycle.plan_hash = content_hash.clone();
                    }
                }
                self.pending_decision = Some(
                    crate::interaction::decision::InteractiveDecision::plan_review(
                        session_id,
                        *revision,
                        objective,
                        *task_count,
                        content_hash.clone(),
                    ),
                );
                self.upsert_plan_review_card(
                    *revision,
                    plan_id.clone(),
                    content_hash.clone(),
                    Some(objective.clone()),
                    Some(*task_count),
                    plan_markdown.clone(),
                    tasks.clone(),
                );
            }
            InteractionEvent::TasksForReview {
                session_id,
                plan_revision,
                task_revision,
                task_count,
                content_hash,
                task_markdown,
                tasks,
            } => {
                self.last_settled_request_id = self.active_request_id.take();
                self.active_command = None;
                self.live_activity = None;
                self.activity_kind = ActivityKind::WaitingForReview;
                self.activity_message = Some(format!(
                    "Task set R{task_revision} ready for review ({task_count} tasks)"
                ));
                self.activity_started_at = Some(now);
                if !self.lifecycle.stage.is_terminal() {
                    self.lifecycle.session_id = Some(session_id.clone());
                    self.lifecycle.stage = TuiLifecycleStage::TasksReviewRequired;
                    self.lifecycle.plan_revision = Some(*plan_revision);
                    self.lifecycle.task_revision = Some(*task_revision);
                    if content_hash.is_some() {
                        self.lifecycle.task_hash = content_hash.clone();
                    }
                }
                self.pending_decision = Some(
                    crate::interaction::decision::InteractiveDecision::tasks_review(
                        session_id,
                        *plan_revision,
                        *task_revision,
                        *task_count,
                        content_hash.clone(),
                    ),
                );
                self.upsert_task_review_card(
                    *task_revision,
                    content_hash.clone(),
                    Some(*task_count),
                    task_markdown.clone(),
                    tasks.clone(),
                );
            }
            InteractionEvent::AuthorizationRequired {
                session_id,
                plan_revision,
                task_revision,
                message,
            } => {
                self.last_settled_request_id = self.active_request_id.take();
                self.active_command = None;
                self.live_activity = None;
                if !self.lifecycle.stage.is_terminal() {
                    self.lifecycle.session_id = Some(session_id.clone());
                    self.lifecycle.stage = TuiLifecycleStage::ExecutionAuthorizationRequired;
                    self.lifecycle.plan_revision = Some(*plan_revision);
                    self.lifecycle.task_revision = Some(*task_revision);
                }
                self.activity_kind = ActivityKind::WaitingForApproval;
                self.activity_message = Some(message.clone());
                self.activity_started_at = Some(now);
                self.pending_decision = Some(
                    crate::interaction::decision::InteractiveDecision::execution_authorization(
                        session_id,
                        *plan_revision,
                        *task_revision,
                        message,
                    ),
                );
                self.upsert_auth_required_card(message.clone());
            }
            InteractionEvent::ExecutionReady {
                session_id,
                authorization_id,
                plan_revision,
                task_revision,
            } => {
                self.last_settled_request_id = self.active_request_id.take();
                self.active_command = None;
                self.live_activity = None;
                self.pending_decision = None;
                self.activity_kind = ActivityKind::Executing;
                self.activity_message = Some("Execution authorized — starting...".to_string());
                self.activity_started_at = Some(now);
                if !self.lifecycle.stage.is_terminal() {
                    self.lifecycle.session_id = Some(session_id.clone());
                    self.lifecycle.stage = TuiLifecycleStage::ExecutionAuthorized;
                    self.lifecycle.plan_revision = Some(*plan_revision);
                    self.lifecycle.task_revision = Some(*task_revision);
                    self.lifecycle.authorization_id = Some(authorization_id.clone());
                }
                self.upsert_auth_granted_card(authorization_id.clone());
            }
            InteractionEvent::LifecycleTerminated {
                session_id,
                stage,
                reason,
            } => {
                self.last_settled_request_id = self.active_request_id.take();
                self.active_command = None;
                self.live_activity = None;
                self.pending_decision = None;
                self.activity_kind = match stage.as_str() {
                    "Completed" => ActivityKind::Completed,
                    "Failed" => ActivityKind::Failed,
                    "Cancelled" => ActivityKind::Cancelled,
                    "Rejected" => ActivityKind::Failed,
                    "Blocked" => ActivityKind::Failed,
                    _ => ActivityKind::Idle,
                };
                self.activity_message = Some(format!("Lifecycle terminated: {stage} — {reason}"));
                self.lifecycle.session_id = Some(session_id.clone());
                let terminal = match stage.as_str() {
                    "Rejected" => TuiLifecycleStage::Rejected,
                    "Cancelled" => TuiLifecycleStage::Cancelled,
                    "Failed" => TuiLifecycleStage::Failed,
                    "Completed" => TuiLifecycleStage::Completed,
                    "Blocked" => TuiLifecycleStage::Blocked,
                    _ => self.lifecycle.stage.clone(),
                };
                self.lifecycle.stage = terminal.clone();
                self.lifecycle.failure_reason = Some(reason.clone());
                if terminal == TuiLifecycleStage::Completed {
                    self.add_conversation_item(TuiConversationItem::System {
                        text: format!("Lifecycle terminated in stage '{stage}': {reason}"),
                        timestamp: Utc::now(),
                    });
                } else if terminal == TuiLifecycleStage::Rejected
                    && self.recent_failure_with_reason(reason)
                {
                    // The authoritative rejection event (PlanRejected /
                    // ExecutionAuthorizationRejected) already recorded this
                    // exact failure; do not double the timeline.
                } else {
                    self.add_conversation_item(TuiConversationItem::Failure {
                        context: format!("lifecycle terminated ({stage})"),
                        reason: reason.clone(),
                        timestamp: Utc::now(),
                    });
                }
            }
            InteractionEvent::ModelUsageUpdated {
                invocation_id,
                prompt_tokens,
                completion_tokens,
                total_tokens: _,
                cost_cents,
                cost_usd: _,
                cost_provenance: _,
            } => {
                if let Some(id) = invocation_id.as_deref() {
                    // This interaction projection does not carry token-usage provenance.
                    // Only use actual stream usage if it arrived; otherwise leave the
                    // final summary at the reasoning estimate or "unavailable".
                    let usage = self.live_model_usage.clone();
                    self.finalize_live_model_progress(id, usage.as_ref());
                }
                self.record_model_usage(
                    invocation_id.clone(),
                    *prompt_tokens,
                    *completion_tokens,
                    *cost_cents,
                );
            }
            InteractionEvent::GitStateChanged {
                workspace_branch,
                execution_branch,
                ..
            } => {
                self.git_branch = workspace_branch.clone();
                self.execution_worktree_branch = execution_branch.clone();
                self.is_dirty = true;
            }
            InteractionEvent::TasksMaterialized {
                graph_id,
                revision,
                tasks,
                session_id,
            } => {
                if let Some(sid) = session_id {
                    if self.session_id.as_deref() != Some(sid) {
                        return;
                    }
                }
                self.reconcile_task_graph(graph_id, *revision, tasks);
            }
            InteractionEvent::TaskStarted {
                mission_id: _,
                task_id,
                agent_id,
            } => {
                let tid_str = task_id.clone();
                let aid_str = agent_id.clone();
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == tid_str) {
                    t.status = "running".to_string();
                    t.assigned_agent_id = Some(aid_str.clone());
                } else {
                    self.tasks.push(TuiTaskSnapshot {
                        id: tid_str.clone(),
                        title: format!("Task {tid_str}"),
                        status: "running".to_string(),
                        agent_role: None,
                        progress_pct: 0,
                        dependencies: Vec::new(),
                        assigned_agent_id: Some(aid_str.clone()),
                        execution_result: None,
                        failure_reason: None,
                        retry_count: 0,
                    });
                }
                self.heartbeat.last_task_event_at = Some(now);
                self.heartbeat.active_task_id = Some(tid_str.clone());
                self.heartbeat.active_agent_id = Some(aid_str);
                self.heartbeat.current_activity = Some(format!("Running task {tid_str}"));
                self.heartbeat.current_activity_started_at = Some(now);
                self.add_log("INFO", format!("Task started: {tid_str}"), "scheduler");
                self.rebuild_traceability();
            }
            InteractionEvent::TaskCompleted {
                mission_id: _,
                task_id,
                result,
            } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.status = "completed".to_string();
                    t.progress_pct = 100;
                    t.execution_result = Some(result.clone());
                }
                self.heartbeat.last_task_event_at = Some(now);
                if self.heartbeat.active_task_id.as_deref() == Some(task_id.as_str()) {
                    self.heartbeat.active_task_id = None;
                    self.heartbeat.active_agent_id = None;
                    self.heartbeat.current_activity = None;
                    self.heartbeat.current_activity_started_at = None;
                }
                self.add_log("INFO", format!("Task completed: {task_id}"), "scheduler");
                self.rebuild_traceability();
            }
            InteractionEvent::TaskFailed {
                mission_id: _,
                task_id,
                error,
            } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.status = "failed".to_string();
                    t.failure_reason = Some(error.clone());
                    t.retry_count = t.retry_count.saturating_add(1);
                }
                self.heartbeat.last_task_event_at = Some(now);
                if self.heartbeat.active_task_id.as_deref() == Some(task_id.as_str()) {
                    self.heartbeat.active_task_id = None;
                    self.heartbeat.active_agent_id = None;
                    self.heartbeat.current_activity = None;
                    self.heartbeat.current_activity_started_at = None;
                }
                self.add_log(
                    "ERROR",
                    format!("Task {task_id} failed: {error}"),
                    "scheduler",
                );
                self.rebuild_traceability();
            }
            InteractionEvent::TaskCancelled {
                mission_id: _,
                task_id,
                reason,
            } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.status = "cancelled".to_string();
                    t.failure_reason = Some(reason.clone());
                }
                self.heartbeat.last_task_event_at = Some(now);
                if self.heartbeat.active_task_id.as_deref() == Some(task_id.as_str()) {
                    self.heartbeat.active_task_id = None;
                    self.heartbeat.active_agent_id = None;
                    self.heartbeat.current_activity = None;
                    self.heartbeat.current_activity_started_at = None;
                }
                self.add_log(
                    "WARN",
                    format!("Task {task_id} cancelled: {reason}"),
                    "scheduler",
                );
                self.rebuild_traceability();
            }
            InteractionEvent::AgentSpawned {
                agent_id,
                mission_id: _,
                role,
            } => {
                if let Some(a) = self.agents.iter_mut().find(|a| a.id == *agent_id) {
                    a.role = role.clone();
                } else {
                    self.agents.push(TuiAgentSnapshot {
                        id: agent_id.clone(),
                        role: role.clone(),
                        state: "spawned".to_string(),
                        current_task: None,
                        total_tokens: 0,
                    });
                }
                self.add_log(
                    "INFO",
                    format!("Agent spawned: {agent_id} ({role})"),
                    "agent",
                );
                self.is_dirty = true;
            }
            InteractionEvent::AgentStarted {
                agent_id,
                mission_id: _,
                task_id,
            } => {
                if let Some(a) = self.agents.iter_mut().find(|a| a.id == *agent_id) {
                    a.state = "running".to_string();
                    a.current_task = Some(task_id.clone());
                } else {
                    self.agents.push(TuiAgentSnapshot {
                        id: agent_id.clone(),
                        role: "agent".to_string(),
                        state: "running".to_string(),
                        current_task: Some(task_id.clone()),
                        total_tokens: 0,
                    });
                }
                self.add_log(
                    "INFO",
                    format!("Agent started: {agent_id} on task {task_id}"),
                    "agent",
                );
                self.is_dirty = true;
            }
            InteractionEvent::AgentStepCompleted {
                agent_id,
                task_id,
                step_number,
                steps_remaining,
            } => {
                if let Some(a) = self.agents.iter_mut().find(|a| a.id == *agent_id) {
                    a.current_task = Some(task_id.clone());
                }
                self.add_log(
                    "INFO",
                    format!("Agent {agent_id} completed step {step_number} ({steps_remaining} remaining)"),
                    "agent",
                );
                self.is_dirty = true;
            }
            InteractionEvent::AgentHandoffRecorded {
                handoff_id,
                mission_id: _,
                source_agent_id,
                target_role,
                reason,
            } => {
                self.add_log(
                    "INFO",
                    format!("Agent handoff {handoff_id} from {source_agent_id} to {target_role}: {reason}"),
                    "agent",
                );
                self.is_dirty = true;
            }
            InteractionEvent::AgentCompleted {
                agent_id,
                mission_id: _,
                summary,
            } => {
                if let Some(a) = self.agents.iter_mut().find(|a| a.id == *agent_id) {
                    a.state = "completed".to_string();
                    a.current_task = None;
                }
                self.add_log(
                    "INFO",
                    format!("Agent {agent_id} completed: {summary}"),
                    "agent",
                );
                self.is_dirty = true;
            }
            InteractionEvent::AgentFailed {
                agent_id,
                mission_id: _,
                error,
            } => {
                if let Some(a) = self.agents.iter_mut().find(|a| a.id == *agent_id) {
                    a.state = "failed".to_string();
                }
                self.add_log(
                    "ERROR",
                    format!("Agent {agent_id} failed: {error}"),
                    "agent",
                );
                self.is_dirty = true;
            }
            InteractionEvent::AgentCancelled {
                agent_id,
                mission_id: _,
                reason,
            } => {
                if let Some(a) = self.agents.iter_mut().find(|a| a.id == *agent_id) {
                    a.state = "cancelled".to_string();
                }
                self.add_log(
                    "WARN",
                    format!("Agent {agent_id} cancelled: {reason}"),
                    "agent",
                );
                self.is_dirty = true;
            }
            InteractionEvent::JobStarted {
                job_id,
                mission_id: _,
                job_type,
            } => {
                if let Some(j) = self.jobs.iter_mut().find(|j| j.id == *job_id) {
                    j.status = "running".to_string();
                    j.name = job_type.clone();
                } else {
                    self.jobs.push(TuiJobSnapshot {
                        id: job_id.clone(),
                        name: job_type.clone(),
                        status: "running".to_string(),
                        duration_ms: 0,
                    });
                }
                if let Some(ref mut sb) = self.sandbox_status {
                    // Do not infer isolation_active or quotas_enforced from mode string;
                    // enforcement state must derive strictly from concrete ProcessEnforcementRecord (AUD-04).
                    if !sb.mode.contains("disabled") && !sb.mode.starts_with("unsupported") {
                        sb.watchdog_active = true;
                    }
                }
                self.add_log("INFO", format!("Job {job_id} ({job_type}) started"), "job");
                self.is_dirty = true;
            }
            InteractionEvent::JobCompleted {
                job_id,
                mission_id: _,
                artifacts: _,
            } => {
                if let Some(j) = self.jobs.iter_mut().find(|j| j.id == *job_id) {
                    j.status = "completed".to_string();
                }
                let has_other_running = self.jobs.iter().any(|j| {
                    j.id != *job_id && (j.status == "running" || j.status == "in_progress")
                });
                if !has_other_running {
                    if let Some(ref mut sb) = self.sandbox_status {
                        sb.isolation_active = false;
                        sb.quotas_enforced = false;
                        sb.watchdog_active = false;
                        sb.network_restricted = false;
                        sb.enforcement = None;
                    }
                }
                self.add_log("INFO", format!("Job {job_id} completed"), "job");
                self.is_dirty = true;
            }
            InteractionEvent::JobFailed {
                job_id,
                mission_id: _,
                error,
            } => {
                if let Some(j) = self.jobs.iter_mut().find(|j| j.id == *job_id) {
                    j.status = "failed".to_string();
                }
                let has_other_running = self.jobs.iter().any(|j| {
                    j.id != *job_id && (j.status == "running" || j.status == "in_progress")
                });
                if !has_other_running {
                    if let Some(ref mut sb) = self.sandbox_status {
                        sb.isolation_active = false;
                        sb.quotas_enforced = false;
                        sb.watchdog_active = false;
                        sb.network_restricted = false;
                        sb.enforcement = None;
                    }
                }
                self.add_log("ERROR", format!("Job {job_id} failed: {error}"), "job");
                self.is_dirty = true;
            }
            InteractionEvent::SandboxEnforcementReported { record } => {
                if let Some(ref mut sb) = self.sandbox_status {
                    sb.isolation_active = record.isolation.is_active();
                    sb.quotas_enforced = record.resource_limits.is_active();
                    sb.watchdog_active = record.watchdog.is_active();
                    sb.network_restricted = record.network.is_active();
                    sb.enforcement = Some(record.clone());
                    self.is_dirty = true;
                }
            }
            InteractionEvent::CatalogRefreshed { generation, .. } => {
                if *generation >= self.catalog_generation {
                    self.catalog_generation = *generation;
                    self.catalog_section_state = SectionProjectionState::Ready;
                    self.is_dirty = true;
                }
            }
            InteractionEvent::CatalogRefreshFailed { error, generation } => {
                if *generation >= self.catalog_generation {
                    self.catalog_generation = *generation;
                    self.catalog_section_state = SectionProjectionState::Failed(error.clone());
                    self.is_dirty = true;
                }
            }
            InteractionEvent::CheckpointCreated {
                checkpoint_id,
                mission_id: _,
                description,
            } => {
                self.checkpoint_id = Some(checkpoint_id.clone());
                self.add_authoritative_checkpoint(checkpoint_id.clone());
                if let Some(t) = self.traceability.first_mut() {
                    t.checkpoint_id = Some(checkpoint_id.clone());
                }
                self.add_log(
                    "INFO",
                    format!("Checkpoint {checkpoint_id} created: {description}"),
                    "checkpoint",
                );
                self.is_dirty = true;
            }
            InteractionEvent::CheckpointRestored {
                checkpoint_id,
                mission_id: _,
            } => {
                if let Some(ref pending) = self.pending_rollback_target {
                    if pending != checkpoint_id {
                        return;
                    }
                    self.rollback_in_progress = false;
                    self.pending_rollback_target = None;
                }
                self.checkpoint_id = Some(checkpoint_id.clone());
                self.add_authoritative_checkpoint(checkpoint_id.clone());
                if let Some(t) = self.traceability.first_mut() {
                    t.checkpoint_id = Some(checkpoint_id.clone());
                }
                self.add_log(
                    "INFO",
                    format!("Checkpoint {checkpoint_id} restored"),
                    "checkpoint",
                );
                self.is_dirty = true;
            }
            InteractionEvent::CheckpointRestoreFailed {
                checkpoint_id,
                error,
            } => {
                let matches_pending =
                    self.pending_rollback_target.as_deref() == Some(checkpoint_id.as_str());
                if matches_pending {
                    self.rollback_in_progress = false;
                    self.pending_rollback_target = None;
                    self.set_error(format!(
                        "Checkpoint restore to {checkpoint_id} failed: {error}"
                    ));
                    self.add_log(
                        "ERROR",
                        format!("Checkpoint restore {checkpoint_id} failed: {error}"),
                        "checkpoint",
                    );
                    self.is_dirty = true;
                }
            }
            InteractionEvent::QuarantineQueried { items, generation } => {
                if *generation >= self.quarantine_generation {
                    self.quarantine_generation = *generation;
                    self.quarantine_items =
                        items.iter().cloned().map(TuiQuarantineItem::from).collect();
                    self.quarantine_state = QuarantineProjectionState::Ready;
                    self.is_dirty = true;
                }
            }
            InteractionEvent::QuarantineQueryFailed { error, generation } => {
                if *generation >= self.quarantine_generation {
                    self.quarantine_generation = *generation;
                    self.quarantine_state = QuarantineProjectionState::Failed(error.clone());
                    self.is_dirty = true;
                }
            }
            InteractionEvent::RecoveryAttempted {
                recovery_id,
                mission_id,
                strategy,
                success,
            } => {
                let outcome_str = if *success { "succeeded" } else { "failed" };
                if let Some(r) = self
                    .recovery_attempts
                    .iter_mut()
                    .find(|r| r.attempt_id == *recovery_id)
                {
                    r.outcome = outcome_str.to_string();
                    r.strategy = strategy.clone();
                } else {
                    let attempt_number = self.recovery_attempts.len() + 1;
                    self.recovery_attempts.push(TuiRecoverySnapshot {
                        attempt_id: recovery_id.clone(),
                        failure_class: "ExecutionFailure".to_string(),
                        strategy: strategy.clone(),
                        attempt_number,
                        checkpoint_id: None,
                        affected_mission_id: Some(mission_id.clone()),
                        affected_task_id: None,
                        outcome: outcome_str.to_string(),
                        budget_consumed: 0,
                        backoff_delay_ms: 0,
                        action_taken: format!("Recovery strategy: {strategy}"),
                    });
                }
                self.add_log(
                    if *success { "INFO" } else { "WARN" },
                    format!("Recovery attempt {recovery_id} ({strategy}): {outcome_str}"),
                    "recovery",
                );
                self.is_dirty = true;
            }
            InteractionEvent::TaskBlocked {
                task_id, reason, ..
            } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.status = "blocked".to_string();
                    t.failure_reason = Some(reason.clone());
                }
                self.add_log(
                    "WARN",
                    format!("Task {task_id} blocked: {reason}"),
                    "scheduler",
                );
                self.is_dirty = true;
            }
            InteractionEvent::TaskUnblocked { task_id, .. } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.status = "ready".to_string();
                    t.failure_reason = None;
                }
                self.add_log("INFO", format!("Task {task_id} unblocked"), "scheduler");
                self.is_dirty = true;
            }
            InteractionEvent::TaskRetryScheduled {
                task_id,
                attempt,
                retry_delay_ms,
                ..
            } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.status = "retry_scheduled".to_string();
                    t.retry_count = *attempt as usize;
                }
                self.add_log(
                    "INFO",
                    format!("Task {task_id} retry #{attempt} scheduled in {retry_delay_ms}ms"),
                    "scheduler",
                );
                self.is_dirty = true;
            }
            InteractionEvent::TaskNeedsReview {
                task_id, reason, ..
            } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.status = "needs_review".to_string();
                    t.failure_reason = Some(reason.clone());
                }
                self.add_log(
                    "WARN",
                    format!("Task {task_id} needs review: {reason}"),
                    "scheduler",
                );
                self.is_dirty = true;
            }
            InteractionEvent::TaskAttemptFailed {
                task_id,
                attempt,
                error,
                ..
            } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.retry_count = *attempt as usize;
                    t.failure_reason = Some(error.clone());
                }
                self.add_log(
                    "WARN",
                    format!("Task {task_id} attempt #{attempt} failed: {error}"),
                    "scheduler",
                );
                self.is_dirty = true;
            }
            InteractionEvent::TaskSkipped {
                task_id, reason, ..
            } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.status = "skipped".to_string();
                    t.failure_reason = Some(reason.clone());
                }
                self.add_log(
                    "INFO",
                    format!("Task {task_id} skipped: {reason}"),
                    "scheduler",
                );
                self.is_dirty = true;
            }
            InteractionEvent::TaskCreated {
                task_id,
                description,
                ..
            } => {
                if !self.tasks.iter().any(|t| t.id == *task_id) {
                    self.tasks.push(TuiTaskSnapshot {
                        id: task_id.clone(),
                        title: description.clone(),
                        status: "pending".to_string(),
                        agent_role: None,
                        progress_pct: 0,
                        dependencies: Vec::new(),
                        assigned_agent_id: None,
                        execution_result: None,
                        failure_reason: None,
                        retry_count: 0,
                    });
                }
                self.add_log(
                    "INFO",
                    format!("Task created: {task_id} — {description}"),
                    "scheduler",
                );
                self.is_dirty = true;
            }
            InteractionEvent::TaskReviewed {
                task_id,
                approved,
                reviewer,
                ..
            } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == *task_id) {
                    if *approved {
                        t.status = "ready".to_string();
                    }
                }
                self.add_log(
                    "INFO",
                    format!(
                        "Task {task_id} {} by {reviewer}",
                        if *approved { "approved" } else { "rejected" }
                    ),
                    "scheduler",
                );
                self.is_dirty = true;
            }
            InteractionEvent::TaskSuperseded {
                task_id,
                replacement_task_id,
                ..
            } => {
                if let Some(t) = self.tasks.iter_mut().find(|t| t.id == *task_id) {
                    t.status = "superseded".to_string();
                }
                self.add_log(
                    "INFO",
                    format!(
                        "Task {task_id} superseded{}",
                        replacement_task_id
                            .as_ref()
                            .map(|r| format!(" by {r}"))
                            .unwrap_or_default()
                    ),
                    "scheduler",
                );
                self.is_dirty = true;
            }
            InteractionEvent::DoctorReportUpdated { checks } => {
                self.doctor_checks = checks.clone();
                self.is_dirty = true;
            }
            InteractionEvent::CriticalPathRecalculated {
                graph_id,
                critical_tasks,
                projected_duration_secs,
            } => {
                self.add_log("INFO", format!("Critical path recalculated for {graph_id}: {} tasks, {projected_duration_secs}s projected", critical_tasks.len()), "scheduler");
                self.is_dirty = true;
            }
            InteractionEvent::ResourceLeased {
                task_id,
                resource_key,
                lock_mode,
                ..
            } => {
                self.add_log(
                    "DEBUG",
                    format!("Resource {resource_key} leased to task {task_id} ({lock_mode})"),
                    "concurrency",
                );
                self.is_dirty = true;
            }
            InteractionEvent::ResourceReleased {
                task_id,
                resource_key,
                ..
            } => {
                self.add_log(
                    "DEBUG",
                    format!("Resource {resource_key} released by task {task_id}"),
                    "concurrency",
                );
                self.is_dirty = true;
            }
            InteractionEvent::ResourceRevoked {
                task_id,
                resource_key,
                reason,
                ..
            } => {
                self.add_log(
                    "WARN",
                    format!("Resource {resource_key} revoked from task {task_id}: {reason}"),
                    "concurrency",
                );
                self.is_dirty = true;
            }
            InteractionEvent::ConfigurationUpdated {
                model,
                provider,
                profile,
            } => {
                self.active_model = model.clone();
                self.active_provider = provider.clone();
                if let Some(p) = profile {
                    self.active_profile = p.clone();
                }
                self.add_log(
                    "INFO",
                    format!("Configuration updated: provider={provider}, model={model}"),
                    "runtime",
                );
                self.is_dirty = true;
            }
            InteractionEvent::WorkflowSnapshotUpdated { snapshot } => {
                self.add_log(
                    "INFO",
                    format!(
                        "Workflow run {} updated: status={:?}",
                        snapshot.run.id, snapshot.run.status
                    ),
                    "workflow",
                );
                self.is_dirty = true;
            }
        }
    }

    /// Reconstruct state from durable event history (TUI-05).
    pub fn reconstruct_from_events(&mut self, events: &[EventEnvelope]) {
        for ev in events {
            self.apply_event(ev);
        }
    }

    /// Record an illegal SQLite query attempt during render frame (for test assertion).
    pub fn record_sqlite_render_access(&self) {
        self.sqlite_access_counter.fetch_add(1, Ordering::SeqCst);
    }

    /// Get count of illegal SQLite queries during render.
    pub fn sqlite_render_access_count(&self) -> usize {
        self.sqlite_access_counter.load(Ordering::SeqCst)
    }

    /// Count completed tasks according to canonical terminal state (M31A-UI-08, M31A-UI-19).
    pub fn count_successful_tasks(&self) -> usize {
        count_successful_tasks(&self.tasks)
    }

    /// Count active agents according to canonical lifecycle state (M31A-UI-08).
    pub fn count_active_agents(&self) -> usize {
        count_active_agents(&self.agents)
    }
}

/// Canonical task terminal outcome classification (Law 6, M31A-UI-08, M31A-UI-19).
/// Only authoritative terminal states count as completed. Progress 100% on a non-terminal
/// state (e.g. running, blocked, cancelled, failed) is NEVER considered completed.
pub fn is_task_terminal_success(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "succeeded" | "completed"
    )
}

/// Count completed tasks according to canonical terminal state (M31A-UI-08, M31A-UI-19).
pub fn count_successful_tasks(tasks: &[TuiTaskSnapshot]) -> usize {
    tasks
        .iter()
        .filter(|t| is_task_terminal_success(&t.status))
        .count()
}

/// Canonical agent lifecycle activity classification (M31A-UI-08).
/// Only agents in active execution states count as active.
/// Waiting, completed, failed, cancelled, blocked, and unknown states do NOT count as active.
pub fn is_agent_active(state: &str) -> bool {
    matches!(
        state.to_ascii_lowercase().as_str(),
        "running" | "active" | "busy" | "executing" | "in_progress"
    )
}

/// Count active agents according to canonical lifecycle state (M31A-UI-08).
pub fn count_active_agents(agents: &[TuiAgentSnapshot]) -> usize {
    agents.iter().filter(|a| is_agent_active(&a.state)).count()
}
