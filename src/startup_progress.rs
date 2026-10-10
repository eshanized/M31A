//! Structured startup-stage tracing and bounded progress protocol (P0).
//!
//! The 30s assembly deadline previously produced only
//! `Runtime assembly timed out after 30.0s (deadline exceeded)` with no
//! indication of the unfinished operation. This module provides:
//!
//! - Stable stage names consumed by `tracing` and the TUI.
//! - A small typed progress update carried over a bounded
//!   [`tokio::sync::watch`] channel (single latest value, never an
//!   unbounded event queue).
//! - Timing helpers that record start/completion/elapsed per stage.
//!
//! Layering: this is a low-level (L0/L8) progress type. The TUI (L9)
//! consumes it as a projection; the runtime (L8) produces it. Lower
//! layers never depend on TUI state.
//!
//! No credentials or secret configuration values are ever included in
//! updates or trace fields.

use std::time::{Duration, Instant};

/// Stable startup stage identifiers.
///
/// Adapted to the real architecture (`AppRuntime::from_pool_workspace_and_config`
/// + `StartupCrashRecoveryScanner` + TUI hydration/bridge attachment).
///
/// Stages are coarse enough to be useful and cheap to emit.
pub mod stage {
    /// Canonical workspace/global storage directories prepared.
    pub const STORAGE: &str = "startup.storage";
    /// SQLite pool acquisition / readiness (pool is provided by caller;
    /// this stage validates acquisition of a connection).
    pub const DATABASE_POOL: &str = "startup.database.pool";
    /// SQLite migrations (executed before assembly in `main`; recorded for completeness).
    pub const DATABASE_MIGRATIONS: &str = "startup.database.migrations";
    /// Canonical workspace startup resolution (`resolve_startup`).
    pub const WORKSPACE_RESOLVE: &str = "startup.workspace.resolve";
    /// Runtime storage preparation (workspace + global dirs, artifact/telemetry dirs).
    pub const RUNTIME_STORAGE_PREPARE: &str = "runtime.storage.prepare";
    /// Telemetry forwarder setup.
    pub const RUNTIME_TELEMETRY_SETUP: &str = "runtime.telemetry.setup";
    /// Crash-recovery: enumerate in-flight missions.
    pub const CRASH_RECOVERY_ENUMERATE: &str = "runtime.crash_recovery.enumerate";
    /// Crash-recovery: database-wide integrity check (once per scan).
    pub const CRASH_RECOVERY_INTEGRITY: &str = "runtime.crash_recovery.integrity";
    /// Crash-recovery: checkpoint validation.
    pub const CRASH_RECOVERY_CHECKPOINTS: &str = "runtime.crash_recovery.checkpoints";
    /// Crash-recovery: workspace baseline capture / drift check.
    pub const CRASH_RECOVERY_WORKSPACE: &str = "runtime.crash_recovery.workspace";
    /// Crash-recovery: background job reconciliation.
    pub const CRASH_RECOVERY_JOBS: &str = "runtime.crash_recovery.jobs";
    /// Crash-recovery: task reconciliation.
    pub const CRASH_RECOVERY_TASKS: &str = "runtime.crash_recovery.tasks";
    /// Policy / budget / worktree / report authorities.
    pub const AUTHORITIES_POLICY: &str = "runtime.authorities.policy";
    /// Model catalog loading.
    pub const AUTHORITIES_CATALOG: &str = "runtime.authorities.catalog";
    /// Capability and tool registry initialization.
    pub const AUTHORITIES_CAPABILITIES: &str = "runtime.authorities.capabilities";
    /// Provider / caller construction (no network on init).
    pub const AUTHORITIES_PROVIDER: &str = "runtime.authorities.provider";
    /// Context compiler / prompt catalog / memory wiring.
    pub const AUTHORITIES_CONTEXT: &str = "runtime.authorities.context";
    /// Authority-set composition (`RuntimeAuthorities::new` + controller deps).
    pub const AUTHORITIES_COMPOSE: &str = "runtime.authorities.compose";
    /// TUI durable workspace hydration (`hydrate_from_runtime`).
    pub const TUI_WORKSPACE_HYDRATION: &str = "tui.workspace_hydration";
    /// TUI bridge attachment (`TuiRuntimeBinding::attach_runtime`).
    pub const TUI_BRIDGE_ATTACHMENT: &str = "tui.bridge_attachment";
}

/// Outcome of a single stage for tracing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageStatus {
    Started,
    Succeeded,
    Warning,
    Failed,
}

impl StageStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Succeeded => "success",
            Self::Warning => "warning",
            Self::Failed => "failure",
        }
    }
}

/// Latest observable startup progress.
///
/// Carried over a `watch` channel: receivers always observe the most
/// recent stage, never a growing backlog. `seq` distinguishes updates
/// with identical stage names (e.g. per-mission recovery progress).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupStageUpdate {
    /// Monotonic sequence number for this assembly attempt.
    pub seq: u64,
    /// Stable stage identifier (see [`stage`]).
    pub stage: String,
    /// Human-readable operator message (e.g. `Recovering unfinished missions (3/12)…`).
    /// Derived from real counters, never hardcoded percentages.
    pub message: String,
    /// Number of in-flight missions observed (when known).
    pub in_flight_missions: Option<usize>,
    /// Recovery progress numerator (missions reconciled so far).
    pub completed_missions: Option<usize>,
    /// Total missions to reconcile in this scan (when known).
    pub total_missions: Option<usize>,
}

impl Default for StartupStageUpdate {
    fn default() -> Self {
        Self {
            seq: 0,
            stage: stage::STORAGE.to_string(),
            message: "Starting runtime assembly…".to_string(),
            in_flight_missions: None,
            completed_missions: None,
            total_missions: None,
        }
    }
}

impl StartupStageUpdate {
    pub fn new(seq: u64, stage: &str, message: impl Into<String>) -> Self {
        Self {
            seq,
            stage: stage.to_string(),
            message: message.into(),
            in_flight_missions: None,
            completed_missions: None,
            total_missions: None,
        }
    }

    pub fn with_recovery_counts(mut self, completed: usize, total: usize) -> Self {
        self.completed_missions = Some(completed);
        self.total_missions = Some(total);
        self.in_flight_missions = Some(total);
        self
    }
}

/// Bounded progress channel pair for one assembly attempt.
///
/// The sender is held by the background assembly task; the TUI holds
/// the receiver and polls the latest value each frame. Capacity is
/// exactly one latest value (`watch`), so slow frames never accumulate
/// backlog and fast stages never block assembly.
pub fn progress_channel() -> (
    tokio::sync::watch::Sender<StartupStageUpdate>,
    tokio::sync::watch::Receiver<StartupStageUpdate>,
) {
    tokio::sync::watch::channel(StartupStageUpdate::default())
}

/// Reporter owned by the assembly task.
///
/// Emits both structured `tracing` spans/events (with stage, elapsed,
/// workspace/channel, and bounded counts) and the latest-value `watch`
/// update for live TUI display. `try_send` semantics: a closed receiver
/// (TUI gone) never blocks or fails assembly.
#[derive(Debug, Clone)]
pub struct StartupProgressReporter {
    sender: Option<tokio::sync::watch::Sender<StartupStageUpdate>>,
    seq: u64,
    workspace: String,
    channel: String,
}

impl StartupProgressReporter {
    /// Reporter that only emits `tracing` events (no live TUI channel).
    /// Used by non-TUI composition paths and unit tests.
    pub fn tracing_only(workspace: &str, channel: &str) -> Self {
        Self {
            sender: None,
            seq: 0,
            workspace: workspace.to_string(),
            channel: channel.to_string(),
        }
    }

    /// Reporter that also publishes live updates to the TUI.
    pub fn with_live_channel(
        sender: tokio::sync::watch::Sender<StartupStageUpdate>,
        workspace: &str,
        channel: &str,
    ) -> Self {
        Self {
            sender: Some(sender),
            seq: 0,
            workspace: workspace.to_string(),
            channel: channel.to_string(),
        }
    }

    /// Record stage start: tracing event + live TUI message.
    pub fn report_stage(&mut self, stage: &str, message: impl Into<String>) {
        self.seq += 1;
        let msg = message.into();
        tracing::info!(
            stage = stage,
            status = StageStatus::Started.as_str(),
            seq = self.seq,
            workspace = %self.workspace,
            channel = %self.channel,
            "{msg}"
        );
        if let Some(tx) = self.sender.as_ref() {
            let update = StartupStageUpdate::new(self.seq, stage, msg);
            let _ = tx.send(update);
        }
    }

    /// Record stage start with recovery counters.
    pub fn report_stage_with_counts(
        &mut self,
        stage: &str,
        message: impl Into<String>,
        completed: usize,
        total: usize,
    ) {
        self.seq += 1;
        let msg = message.into();
        tracing::info!(
            stage = stage,
            status = StageStatus::Started.as_str(),
            seq = self.seq,
            workspace = %self.workspace,
            channel = %self.channel,
            completed = completed,
            total = total,
            "{msg}"
        );
        if let Some(tx) = self.sender.as_ref() {
            let update = StartupStageUpdate::new(self.seq, stage, msg)
                .with_recovery_counts(completed, total);
            let _ = tx.send(update);
        }
    }

    /// Record stage completion with elapsed duration.
    pub fn report_complete(&mut self, stage: &str, elapsed: Duration, detail: impl Into<String>) {
        let detail = detail.into();
        tracing::info!(
            stage = stage,
            status = StageStatus::Succeeded.as_str(),
            elapsed_ms = elapsed.as_millis() as u64,
            workspace = %self.workspace,
            channel = %self.channel,
            "{detail}"
        );
    }

    /// Record stage warning (recovered, degraded, or retried).
    pub fn report_warning(&mut self, stage: &str, detail: impl Into<String>) {
        let detail = detail.into();
        tracing::warn!(
            stage = stage,
            status = StageStatus::Warning.as_str(),
            workspace = %self.workspace,
            channel = %self.channel,
            "{detail}"
        );
    }

    /// Record stage failure with elapsed duration. Never includes secrets.
    pub fn report_failure(&mut self, stage: &str, elapsed: Duration, detail: impl Into<String>) {
        let detail = detail.into();
        tracing::error!(
            stage = stage,
            status = StageStatus::Failed.as_str(),
            elapsed_ms = elapsed.as_millis() as u64,
            workspace = %self.workspace,
            channel = %self.channel,
            "{detail}"
        );
    }
}

/// RAII timer for one startup stage.
///
/// On construction records start; call [`StageTimer::success`],
/// [`StageTimer::warning`], or [`StageTimer::failure`] to record the
/// terminal event with elapsed time. If dropped without a terminal
/// call, no completion event is emitted so an unfinished stage is
/// distinguishable from a completed one in diagnostics.
pub struct StageTimer<'a> {
    stage: &'static str,
    started_at: Instant,
    reporter: &'a mut StartupProgressReporter,
    finished: bool,
}

impl<'a> StageTimer<'a> {
    pub fn start(reporter: &'a mut StartupProgressReporter, stage: &'static str) -> Self {
        Self {
            stage,
            started_at: Instant::now(),
            reporter,
            finished: false,
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }

    pub fn started_at(&self) -> Instant {
        self.started_at
    }

    pub fn success(mut self, detail: impl Into<String>) {
        self.finished = true;
        let elapsed = self.started_at.elapsed();
        self.reporter.report_complete(self.stage, elapsed, detail);
    }

    pub fn warning(mut self, detail: impl Into<String>) {
        self.finished = true;
        self.reporter.report_warning(self.stage, detail);
    }

    pub fn failure(mut self, detail: impl Into<String>) {
        self.finished = true;
        let elapsed = self.started_at.elapsed();
        self.reporter.report_failure(self.stage, elapsed, detail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_channel_holds_latest_value_only() {
        let (tx, rx) = progress_channel();
        tx.send(StartupStageUpdate::new(1, stage::STORAGE, "a"))
            .unwrap();
        tx.send(StartupStageUpdate::new(
            2,
            stage::CRASH_RECOVERY_INTEGRITY,
            "b",
        ))
        .unwrap();
        // watch holds only the latest value.
        assert_eq!(rx.borrow().seq, 2);
        assert_eq!(rx.borrow().stage, stage::CRASH_RECOVERY_INTEGRITY);
    }

    #[test]
    fn reporter_without_live_channel_never_panics() {
        let mut r = StartupProgressReporter::tracing_only("ws", "production");
        r.report_stage(stage::STORAGE, "preparing");
        r.report_complete(stage::STORAGE, Duration::from_millis(1), "done");
        r.report_warning(stage::STORAGE, "degraded");
        r.report_failure(stage::STORAGE, Duration::from_millis(1), "boom");
    }

    #[test]
    fn recovery_counts_are_real_not_hardcoded() {
        let u =
            StartupStageUpdate::new(7, stage::CRASH_RECOVERY_JOBS, "x").with_recovery_counts(3, 12);
        assert_eq!(u.completed_missions, Some(3));
        assert_eq!(u.total_missions, Some(12));
        assert_eq!(u.in_flight_missions, Some(12));
    }
}
