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
use crate::interaction::state::SessionPromptState;
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
}

/// Snapshot of an active agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

/// Atomic snapshot of canonical budget enforcement state (P1, BGT-01, D-06).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuiBudgetSnapshot {
    pub scope: String,
    pub allocated_cents: u64,
    pub consumed_cents: u64,
    pub remaining_cents: u64,
    pub max_tokens: u64,
    pub consumed_tokens: u64,
    pub remaining_tokens: u64,
    pub max_tool_calls: u64,
    pub consumed_tool_calls: u64,
    pub is_exhausted: bool,
    pub is_constrained: bool,
}

impl Default for TuiBudgetSnapshot {
    fn default() -> Self {
        Self {
            scope: "global".to_string(),
            allocated_cents: 0,
            consumed_cents: 0,
            remaining_cents: 0,
            max_tokens: 0,
            consumed_tokens: 0,
            remaining_tokens: 0,
            max_tool_calls: 0,
            consumed_tool_calls: 0,
            is_exhausted: false,
            is_constrained: false,
        }
    }
}

impl From<&crate::budget::BudgetSnapshot> for TuiBudgetSnapshot {
    fn from(b: &crate::budget::BudgetSnapshot) -> Self {
        let max_cost_cents = (b.max_cost_usd.unwrap_or(0.0) * 100.0) as u64;
        let consumed_cost_cents = (b.cost_consumed_usd * 100.0) as u64;
        let remaining_cost_cents = max_cost_cents.saturating_sub(consumed_cost_cents);
        let max_tok = b.max_tokens.unwrap_or(0);
        let consumed_tok = b.tokens_consumed;
        let remaining_tok = max_tok.saturating_sub(consumed_tok);
        let max_steps = b.max_agent_steps.unwrap_or(0) as u64;
        let consumed_steps = b.agent_steps_consumed as u64;
        let is_exhausted = (max_tok > 0 && consumed_tok >= max_tok)
            || (max_cost_cents > 0 && consumed_cost_cents >= max_cost_cents);
        let is_constrained = (max_tok > 0 && remaining_tok < max_tok / 5)
            || (max_cost_cents > 0 && remaining_cost_cents < max_cost_cents / 5);

        Self {
            scope: "global".to_string(),
            allocated_cents: max_cost_cents,
            consumed_cents: consumed_cost_cents,
            remaining_cents: remaining_cost_cents,
            max_tokens: max_tok,
            consumed_tokens: consumed_tok,
            remaining_tokens: remaining_tok,
            max_tool_calls: max_steps,
            consumed_tool_calls: consumed_steps,
            is_exhausted,
            is_constrained,
        }
    }
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
    pub prompt_state: SessionPromptState,
    pub scroll_offset: usize,
    pub selected_tool_output: Option<String>,
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

    // --- Evidence Plane & Observability Extensions ---
    pub artifacts: Vec<TuiArtifactSnapshot>,
    pub verification_checks: Vec<TuiVerificationCheck>,
    pub verification_summary: TuiVerificationSummary,
    pub recovery_attempts: Vec<TuiRecoverySnapshot>,
    pub budget: TuiBudgetSnapshot,
    pub skills: Vec<TuiSkillSnapshot>,
    pub canonical_tools: Vec<TuiToolDefinitionSnapshot>,
    pub timeline: VecDeque<TuiTimelineEntry>,
    pub max_timeline: usize,
    pub traceability: Vec<TuiTraceabilityChain>,
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
            prompt_state: SessionPromptState::Idle,
            scroll_offset: 0,
            selected_tool_output: None,
            lifecycle: TuiLifecycleProjection::new(),
            icons: IconRegistry::from_theme(crate::tui::theme::ThemeMode::default()),
            spinner: Spinner::new(),
            activity_kind: ActivityKind::default(),
            activity_message: None,
            activity_started_at: None,

            artifacts: Vec::new(),
            verification_checks: Vec::new(),
            verification_summary: TuiVerificationSummary::default(),
            recovery_attempts: Vec::new(),
            budget: TuiBudgetSnapshot::default(),
            skills: Vec::new(),
            canonical_tools: Vec::new(),
            timeline: VecDeque::with_capacity(1000),
            max_timeline: 1000,
            traceability: Vec::new(),
        }
    }

    /// Check if in-flight execution or animations require continuous frame rendering.
    pub fn has_active_animation(&self) -> bool {
        matches!(
            self.activity_kind,
            ActivityKind::Thinking
                | ActivityKind::Discovering
                | ActivityKind::Planning
                | ActivityKind::Executing
                | ActivityKind::RunningTool
                | ActivityKind::Verifying
                | ActivityKind::Recovering
        ) || self.active_stream_message_id.is_some()
            || self
                .live_tools
                .iter()
                .any(|t| t.state == LiveToolState::Running)
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
        self.artifacts = artifacts;
        self.rebuild_traceability();
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

    /// Update canonical budget state.
    pub fn update_budget(&mut self, snapshot: TuiBudgetSnapshot) {
        self.budget = snapshot;
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
            });
        }

        self.traceability = chains;
    }

    /// Apply runtime event envelope into in-memory projection in O(1) time.
    pub fn apply_event(&mut self, event: &EventEnvelope) {
        self.system_stats.events_processed += 1;
        self.is_dirty = true;
        let seq = event.sequence;
        let ts = event.timestamp;

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
                self.upsert_plan_review_card(*revision, plan_id.clone(), None, None, None);
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
                self.upsert_task_review_card(*task_revision, None, Some(*task_count));
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
                        text: format!(
                            "Execution started via StartExecution boundary: graph {graph_id} rev {revision} with {task_count} tasks."
                        ),
                        timestamp: Utc::now(),
                    });
                }
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
                        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == tid_str) {
                            t.status = "running".to_string();
                            if t.agent_role.is_none() {
                                t.agent_role = Some(agent_id.to_string());
                            }
                            if t.progress_pct < 10 {
                                t.progress_pct = 10;
                            }
                        } else {
                            self.tasks.push(TuiTaskSnapshot {
                                id: tid_str.clone(),
                                title: format!("Task {tid_str}"),
                                status: "running".to_string(),
                                agent_role: Some(agent_id.to_string()),
                                progress_pct: 10,
                                dependencies: Vec::new(),
                            });
                        }
                        self.add_log("INFO", format!("Task started: {tid_str}"), "scheduler");
                        self.add_timeline_entry(
                            seq,
                            ts,
                            "task",
                            format!("Task started: {tid_str}"),
                            format!("Agent: {agent_id}"),
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
                        tier_num: 1,
                        tier_name: "deterministic".to_string(),
                        status: if *passed {
                            "passed".to_string()
                        } else {
                            "failed".to_string()
                        },
                        command_or_tool: "verification".to_string(),
                        inputs_normalized: "".to_string(),
                        evidence_artifact_id: None,
                        summary: evidence.clone(),
                        failure_class: if *passed {
                            None
                        } else {
                            Some("VerificationFailure".to_string())
                        },
                        snapshot_hash: "".to_string(),
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
            EventType::GitStateChanged {
                workspace_branch,
                execution_branch,
                ..
            } => {
                self.git_branch = workspace_branch.clone();
                self.execution_worktree_branch = execution_branch.clone();
                self.is_dirty = true;
            }
            EventType::ModelUsageUpdated {
                invocation_id,
                usage,
                ..
            } => {
                let inv_str = invocation_id.map(|id| id.to_string());
                if let Some(ref id) = inv_str {
                    if !self.model_usage.processed_invocations.insert(id.clone()) {
                        return;
                    }
                }
                self.model_usage.prompt_tokens += usage.prompt_tokens as u64;
                self.model_usage.completion_tokens += usage.completion_tokens as u64;
                self.model_usage.api_calls += 1;
                self.is_dirty = true;
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
    pub fn add_conversation_item(&mut self, item: TuiConversationItem) {
        self.conversation.push(item);
        self.is_dirty = true;
    }

    /// Insert or enrich the plan review card for a revision.
    ///
    /// Envelope events carry identity only (revision, plan id); coordinator
    /// responses later enrich the same card with hash, objective, and counts.
    /// Exactly one card exists per revision; content is never fabricated.
    pub fn upsert_plan_review_card(
        &mut self,
        revision: u32,
        plan_id: String,
        content_hash: Option<String>,
        objective: Option<String>,
        task_count: Option<usize>,
    ) {
        for item in self.conversation.iter_mut().rev() {
            if let TuiConversationItem::PlanReview {
                revision: r,
                content_hash: h,
                objective: o,
                task_count: c,
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
            timestamp: Utc::now(),
        });
    }

    /// Insert or enrich the task review card for a revision.
    pub fn upsert_task_review_card(
        &mut self,
        task_revision: u32,
        content_hash: Option<String>,
        task_count: Option<usize>,
    ) {
        let plan_revision = self.lifecycle.plan_revision.unwrap_or(0);
        for item in self.conversation.iter_mut().rev() {
            if let TuiConversationItem::TaskReview {
                task_revision: r,
                content_hash: h,
                task_count: c,
                plan_revision: pr,
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

    /// Load durable session history into the conversation timeline.
    pub fn load_session_history(&mut self, turns: &[ConversationTurn]) {
        self.conversation.clear();
        for turn in turns {
            self.conversation.push(TuiConversationItem::from_turn(turn));
        }
        self.is_dirty = true;
    }

    /// Scroll conversation viewport up by N lines.
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
        self.is_dirty = true;
    }

    /// Scroll conversation viewport down by N lines.
    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        self.is_dirty = true;
    }

    /// Reset scroll offset to track bottom of conversation.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.is_dirty = true;
    }

    /// Apply decoupled InteractionEvent stream item to view model state.
    pub fn apply_interaction_event(&mut self, event: &InteractionEvent) {
        self.system_stats.events_processed += 1;
        self.is_dirty = true;

        let now = Utc::now();

        match event {
            InteractionEvent::SessionStarted { session_id } => {
                self.session_id = Some(session_id.to_string());
                self.session_status = "active".to_string();
                self.activity_kind = ActivityKind::Idle;
                self.activity_message = None;
                self.activity_started_at = None;
                self.add_conversation_item(TuiConversationItem::System {
                    text: format!("Session started: {session_id}"),
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::SessionResumed { session_id } => {
                self.session_id = Some(session_id.to_string());
                self.session_status = "active".to_string();
                self.activity_kind = ActivityKind::Idle;
                self.activity_message = None;
                self.activity_started_at = None;
                self.add_conversation_item(TuiConversationItem::System {
                    text: format!("Session resumed: {session_id}"),
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::ModelActivity { text } => {
                self.live_activity = Some(text.clone());
                self.activity_kind = ActivityKind::Thinking;
                self.activity_message = Some(text.clone());
                self.activity_started_at = Some(now);
                self.add_log("INFO", format!("Model: {text}"), "model");
            }
            InteractionEvent::AssistantStarted { message_id } => {
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
                self.live_activity = None;
                self.activity_kind = ActivityKind::Idle;
                self.activity_message = None;
                self.activity_started_at = None;
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
                self.live_activity = None;
                self.activity_kind = ActivityKind::Failed;
                self.activity_message = Some(format!("Assistant failed: {error}"));
                self.activity_started_at = None;
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
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::AssistantOutput { text } => {
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
            InteractionEvent::VerificationPassed { summary } => {
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
                self.verification_checks.push(TuiVerificationCheck {
                    check_id: format!("check-{}", self.verification_checks.len() + 1),
                    mission_id: self.mission_id.clone().unwrap_or_default(),
                    task_id: "".to_string(),
                    tier_num: 1,
                    tier_name: "deterministic".to_string(),
                    status: "passed".to_string(),
                    command_or_tool: "verification".to_string(),
                    inputs_normalized: "".to_string(),
                    evidence_artifact_id: None,
                    summary: summary.clone(),
                    failure_class: None,
                    snapshot_hash: "".to_string(),
                    created_at: Utc::now(),
                });
                self.recompute_verification_summary();
                self.rebuild_traceability();
            }
            InteractionEvent::VerificationFailed { summary } => {
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
                self.verification_checks.push(TuiVerificationCheck {
                    check_id: format!("check-{}", self.verification_checks.len() + 1),
                    mission_id: self.mission_id.clone().unwrap_or_default(),
                    task_id: "".to_string(),
                    tier_num: 1,
                    tier_name: "deterministic".to_string(),
                    status: "failed".to_string(),
                    command_or_tool: "verification".to_string(),
                    inputs_normalized: "".to_string(),
                    evidence_artifact_id: None,
                    summary: summary.clone(),
                    failure_class: Some("VerificationFailure".to_string()),
                    snapshot_hash: "".to_string(),
                    created_at: Utc::now(),
                });
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
            } => {
                self.prompt_state = SessionPromptState::AwaitingApproval;
                self.activity_kind = ActivityKind::WaitingForApproval;
                self.activity_message = Some(format!("Approval required: {tool_name}"));
                self.activity_started_at = Some(now);
                self.approvals.push(TuiApprovalRequest {
                    id: request_id.clone(),
                    tool_name: tool_name.clone(),
                    agent_role: "Implementer".to_string(),
                    justification: details.clone(),
                    parameters_summary: "".to_string(),
                    risk_tier: "HighRiskMutation".to_string(),
                    timestamp: Utc::now(),
                });
                self.add_conversation_item(TuiConversationItem::Approval {
                    request_id: request_id.clone(),
                    tool_name: tool_name.clone(),
                    details: details.clone(),
                    decision: None,
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::ApprovalResolved {
                request_id,
                approved,
            } => {
                self.prompt_state = SessionPromptState::Executing;
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
                self.add_conversation_item(TuiConversationItem::System {
                    text: text.clone(),
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::Error { message } => {
                self.live_activity = None;
                self.activity_kind = ActivityKind::Failed;
                self.activity_message = Some(message.clone());
                self.add_conversation_item(TuiConversationItem::Error {
                    message: message.clone(),
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::Completion { summary } => {
                self.live_activity = None;
                self.activity_kind = ActivityKind::Completed;
                self.activity_message = Some(format!("Completed: {summary}"));
                // Legacy agent-turn completion. Terminal lifecycle truth comes
                // from MissionCompleted; never overwrite a sealed stage here.
                if !self.lifecycle.stage.is_terminal() {
                    self.lifecycle.stage = TuiLifecycleStage::Completed;
                }
                self.prompt_state = SessionPromptState::Idle;
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
                self.live_activity = None;
                self.activity_kind = ActivityKind::Discovering;
                self.activity_message = Some(format!("Discovery: {} questions", questions.len()));
                self.activity_started_at = Some(now);
                if !self.lifecycle.stage.is_terminal() {
                    self.lifecycle.session_id = Some(session_id.clone());
                    self.lifecycle.stage = TuiLifecycleStage::DiscoveryRequired;
                    self.lifecycle.pending_questions = questions.clone();
                }
                self.prompt_state = SessionPromptState::WaitingForUser;
                self.add_conversation_item(TuiConversationItem::Discovery {
                    questions: questions.clone(),
                    timestamp: Utc::now(),
                });
            }
            InteractionEvent::PlanForReview {
                session_id,
                revision,
                plan_id,
                objective,
                task_count,
                content_hash,
            } => {
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
                self.upsert_plan_review_card(
                    *revision,
                    plan_id.clone(),
                    content_hash.clone(),
                    Some(objective.clone()),
                    Some(*task_count),
                );
            }
            InteractionEvent::TasksForReview {
                session_id,
                plan_revision,
                task_revision,
                task_count,
                content_hash,
            } => {
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
                self.upsert_task_review_card(
                    *task_revision,
                    content_hash.clone(),
                    Some(*task_count),
                );
            }
            InteractionEvent::AuthorizationRequired {
                session_id,
                plan_revision,
                task_revision,
                message,
            } => {
                self.live_activity = None;
                if !self.lifecycle.stage.is_terminal() {
                    self.lifecycle.session_id = Some(session_id.clone());
                    self.lifecycle.stage = TuiLifecycleStage::ExecutionAuthorizationRequired;
                    self.lifecycle.plan_revision = Some(*plan_revision);
                    self.lifecycle.task_revision = Some(*task_revision);
                }
                self.prompt_state = SessionPromptState::AwaitingApproval;
                self.activity_kind = ActivityKind::WaitingForApproval;
                self.activity_message = Some(message.clone());
                self.activity_started_at = Some(now);
                self.upsert_auth_required_card(message.clone());
            }
            InteractionEvent::ExecutionReady {
                session_id,
                authorization_id,
                plan_revision,
                task_revision,
            } => {
                self.live_activity = None;
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
                self.live_activity = None;
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
                self.prompt_state = SessionPromptState::Idle;
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
            } => {
                if let Some(inv_id) = invocation_id {
                    if !self
                        .model_usage
                        .processed_invocations
                        .insert(inv_id.clone())
                    {
                        return;
                    }
                    self.model_usage.prompt_tokens += prompt_tokens;
                    self.model_usage.completion_tokens += completion_tokens;
                    self.model_usage.in_flight_prompt_tokens = 0;
                    self.model_usage.in_flight_completion_tokens = 0;
                    self.model_usage.api_calls += 1;
                    if let Some(cents) = cost_cents {
                        *self.model_usage.total_cost_cents.get_or_insert(0) += cents;
                    }
                } else {
                    // Provisional in-flight update from streaming chunks
                    self.model_usage.in_flight_prompt_tokens = *prompt_tokens;
                    self.model_usage.in_flight_completion_tokens = *completion_tokens;
                }
                self.is_dirty = true;
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
            InteractionEvent::TasksMaterialized { tasks, .. } => {
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
                    })
                    .collect();
                self.task_graph_state = TaskGraphProjectionState::Loaded;
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
}
