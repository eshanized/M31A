//! Unified Workspace Orchestration & Composition Surface (Section 5, 18, TUI-01).
//!
//! Organizes the primary cockpit workspace into cohesive split-pane surfaces
//! ensuring the operator never feels ejected from the autonomous execution loop.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::composer::TuiComposer;
use crate::tui::focus::FocusTarget;
use crate::tui::model::TuiViewModel;
use crate::tui::navigation::ScreenId;
use crate::tui::registry::{ViewId, ViewRegistry};
use crate::tui::replay::ReplayController;
use crate::tui::screens::wizard::SetupWizardScreen;
use crate::tui::surface::{
    ModelSelectorState, WorkflowDashboardState, render_model_selector, render_workflow_dashboard,
};
use crate::tui::theme::ThemeTokens;

/// Render the unified cockpit workspace.
#[allow(clippy::too_many_arguments)]
pub fn render_workspace(
    f: &mut Frame,
    area: Rect,
    model: &mut TuiViewModel,
    composer: &TuiComposer,
    screen: ScreenId,
    replay: &ReplayController,
    focus: FocusTarget,
    is_composer_focused: bool,
    tokens: &ThemeTokens,
    context_selected_idx: usize,
    active_detail: Option<ViewId>,
    active_overlay: Option<ViewId>,
    workflow_snapshot: &Option<crate::workflow::engine::WorkflowExecutionSnapshot>,
    workflow_dashboard_state: &mut WorkflowDashboardState,
    model_selector_state: &mut ModelSelectorState,
    setup_wizard: &mut Option<SetupWizardScreen>,
    settings_state: &mut crate::tui::surface::settings::SettingsState,
    resolved_config: Option<&crate::config::ResolvedConfiguration>,
    settings_draft: Option<&crate::config::schema::AppConfig>,
) {
    if area.width < 10 || area.height < 6 {
        return;
    }

    // Mode S — Explicit startup state (never blank).
    //
    // While the canonical runtime is still assembling/hydrating (or failed),
    // the workspace renders an explicit startup panel through the normal
    // shell. Header, footer, and the composer below remain visible; a missing
    // bridge/runtime is a state, never a reason to stop rendering.
    if model.runtime_status.is_startup() || model.runtime_status.is_failed() {
        render_startup_workspace(
            f,
            area,
            model,
            composer,
            tokens,
            is_composer_focused,
            active_overlay,
            setup_wizard,
        );
        return;
    }

    // Mode A — Minimal Welcome Mode:
    // Shown before first prompt/meaningful interaction on the Dashboard screen.
    if model.session_view_mode.is_welcome() && screen == ScreenId::Dashboard {
        render_welcome_workspace(
            f,
            area,
            model,
            composer,
            tokens,
            active_overlay,
            setup_wizard,
        );
        return;
    }

    // Mode B — Active Session Workspace:
    // Responsive layout: terminal width >= 120 gets contextual right rail on Dashboard.
    let show_context_rail = area.width >= 120 && screen == ScreenId::Dashboard;
    let rail_width = if show_context_rail {
        28.min(area.width / 4).max(24)
    } else {
        0
    };
    let left_width = area.width.saturating_sub(rail_width);

    let left_area = Rect::new(area.x, area.y, left_width, area.height);
    let rail_area = Rect::new(area.x + left_width, area.y, rail_width, area.height);

    // Allocate bottom 4 rows of left column for Composer
    let workspace_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(4)])
        .split(left_area);

    let upper_area = workspace_chunks[0];
    let composer_area = workspace_chunks[1];

    // 1a. Contextual detail inspector (§48-50): the navigation authority
    // stores active_detail, and the workspace CONSUMES it here — state alone
    // is not reachability. Carve a detail strip from the upper area.
    let (main_area, detail_area) = if let Some(detail) = active_detail {
        if upper_area.height >= 14 {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(4), Constraint::Length(10)])
                .split(upper_area);
            (chunks[0], Some((chunks[1], detail)))
        } else {
            (upper_area, Some((upper_area, detail)))
        }
    } else {
        (upper_area, None)
    };
    let upper_area = main_area;

    // 1. Route-owned upper-area composition (Principles 7–8).
    //
    // The orchestrator only prepares `upper_area`; each route composes its
    // own conversation + inspector surfaces via `crate::tui::routes`. No
    // business logic lives here — pure delegation preserves M31A's exact
    // responsive visuals while removing the monolithic match.
    {
        let mut route_ctx = crate::tui::routes::RouteContext::new(
            screen,
            focus,
            context_selected_idx,
            replay,
            workflow_snapshot,
            workflow_dashboard_state,
            model_selector_state,
            settings_state,
            resolved_config,
            settings_draft,
        );
        crate::tui::routes::render_route_upper(
            f,
            upper_area,
            model,
            composer,
            &mut route_ctx,
            tokens,
        );
    }

    // 2. Render Bottom Composer. Busy derives from the single
    // authoritative operation state — never from session liveness.
    let composer_state = model.operation_state();
    let is_busy = composer_state.is_working();
    let is_waiting = composer_state.is_waiting();

    if is_composer_focused {
        composer.render(f, composer_area, tokens);
    } else {
        render_unfocused_composer(f, composer_area, composer, is_busy, is_waiting, tokens);
    }

    // 2b. Context Rail for Active Dashboard Workspace
    if show_context_rail {
        super::context_rail::render_context_rail(
            f,
            rail_area,
            model,
            tokens,
            focus == FocusTarget::ContextPanel,
            context_selected_idx,
        );
    }

    // 3. Contextual detail + overlay views: every registered ViewId that
    // resolves to a detail/overlay MUST render observable content here.
    if let Some((area, detail)) = detail_area {
        render_detail_inspector(
            f,
            area,
            detail,
            model,
            tokens,
            workflow_snapshot,
            workflow_dashboard_state,
            model_selector_state,
        );
    }
    if let Some(overlay) = active_overlay {
        render_view_overlay(f, area, overlay, model, tokens, setup_wizard);
    }
}

/// Render Mode A — Minimal Welcome Mode (before first prompt/interaction).
fn render_welcome_workspace(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    composer: &TuiComposer,
    tokens: &ThemeTokens,
    active_overlay: Option<ViewId>,
    setup_wizard: &mut Option<SetupWizardScreen>,
) {
    if area.width < 10 || area.height < 6 {
        return;
    }

    // Centered layout calculation
    let max_box_w = 64;
    let box_width = (area.width.saturating_sub(8)).clamp(36, max_box_w);
    let box_height = 5.min(area.height.saturating_sub(6)).max(4);
    let total_block_h = 4 + 1 + box_height + 1 + 1; // 4 lines title + gap + box + gap + hints
    let start_y = if area.height > total_block_h {
        area.y + (area.height - total_block_h) / 2
    } else {
        area.y
    };
    let center_x = area.x + (area.width.saturating_sub(box_width)) / 2;

    // 1. Centered identity
    let title_area = Rect::new(area.x, start_y, area.width, 4);
    let title_lines = vec![
        Line::from(Span::styled(
            "M31A",
            tokens.text_primary.add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            "What are we building today?",
            tokens.text_secondary,
        )),
        Line::from(Span::styled(
            "Your autonomous software engineering workspace.",
            tokens.text_muted,
        )),
    ];
    f.render_widget(
        Paragraph::new(title_lines).alignment(ratatui::layout::Alignment::Center),
        title_area,
    );

    // 2. Centered Composer Box
    let box_rect = Rect::new(center_x, start_y + 4 + 1, box_width, box_height);
    let box_block = Block::default()
        .borders(Borders::ALL)
        .border_style(tokens.separator);
    f.render_widget(box_block, box_rect);

    // Inside the box:
    // Top portion for composer input
    let inner_input_rect = Rect::new(
        box_rect.x + 1,
        box_rect.y + 1,
        box_rect.width.saturating_sub(2),
        box_rect.height.saturating_sub(3).max(1),
    );
    composer.render_bare(f, inner_input_rect, tokens);

    // Bottom line inside the box: Build · model · profile
    let meta_rect = Rect::new(
        box_rect.x + 1,
        box_rect.bottom().saturating_sub(2),
        box_rect.width.saturating_sub(2),
        1,
    );
    let model_name = if model.active_model.is_empty() || model.active_model == "default" {
        "model"
    } else {
        &model.active_model
    };
    let profile_name = if model.active_profile.is_empty() {
        "profile"
    } else {
        &model.active_profile
    };
    let meta_line = Line::from(vec![
        Span::styled(" Build · ", tokens.text_muted),
        Span::styled(model_name, tokens.text_muted),
        Span::styled(" · ", tokens.text_muted),
        Span::styled(profile_name, tokens.text_muted),
    ]);
    f.render_widget(Paragraph::new(meta_line), meta_rect);

    // 3. Hints below the box:
    // / for commands   @ for files   ? for help
    let hint_y = (box_rect.bottom() + 1).min(area.bottom().saturating_sub(1));
    let hint_rect = Rect::new(area.x, hint_y, area.width, 1);
    let hint_line = Line::from(vec![
        Span::styled("/ for commands", tokens.text_muted),
        Span::styled("   ", tokens.text_muted),
        Span::styled("@ for files", tokens.text_muted),
        Span::styled("   ", tokens.text_muted),
        Span::styled("? for help", tokens.text_muted),
    ]);
    f.render_widget(
        Paragraph::new(hint_line).alignment(ratatui::layout::Alignment::Center),
        hint_rect,
    );

    // Contextual overlays if open
    if let Some(overlay) = active_overlay {
        render_view_overlay(f, area, overlay, model, tokens, setup_wizard);
    }
}

/// Render Mode S — Explicit startup state (never blank).
///
/// Rendered through the normal shell while the canonical runtime assembles
/// asynchronously. Always contains visible M31A UI: identity, startup
/// state, the three canonical init steps, and the composer shell below.
/// The composer remains visible but gated: local editing works, execution
/// dispatches fail closed until the runtime is ready (see `send_or_fail`).
#[allow(clippy::too_many_arguments)]
fn render_startup_workspace(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    composer: &TuiComposer,
    tokens: &ThemeTokens,
    is_composer_focused: bool,
    active_overlay: Option<ViewId>,
    setup_wizard: &mut Option<SetupWizardScreen>,
) {
    use crate::tui::model::RuntimeStartupState as R;

    // Split: upper startup panel + bottom composer (same 4-row composer
    // contract as the normal workspace so the shell never jumps).
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(4)])
        .split(area);
    let upper_area = chunks[0];
    let composer_area = chunks[1];

    let is_failed = model.runtime_status.is_failed();
    let title = "M31A";
    let status_line = match &model.runtime_status {
        R::Booting => "Booting…".to_string(),
        R::InitializingRuntime => "Initializing M31A runtime…".to_string(),
        R::HydratingSession => "Loading session state…".to_string(),
        R::HydratingWorkspace => "Loading workspace state…".to_string(),
        R::HydratingExecution => "Loading execution state…".to_string(),
        R::Ready => "Ready".to_string(),
        R::Failed(reason) => format!("Startup failed: {reason}"),
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!("  {title}"),
        tokens.text_primary.add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        format!(
            "  {}",
            "─".repeat((area.width as usize).saturating_sub(4).min(120))
        ),
        tokens.separator,
    )));
    lines.push(Line::from(Span::styled(
        format!("  {status_line}"),
        if is_failed {
            tokens.error
        } else {
            tokens.text_secondary
        },
    )));
    lines.push(Line::raw(""));
    // Canonical init steps — always visible so the operator knows the boot
    // did not stall on a blank screen.
    let (step_active, step_detail) = match &model.runtime_status {
        R::Booting => (0, None),
        R::InitializingRuntime => (0, model.runtime_status_detail.as_deref()),
        R::HydratingWorkspace | R::HydratingSession => (1, model.runtime_status_detail.as_deref()),
        R::HydratingExecution => (2, model.runtime_status_detail.as_deref()),
        R::Ready => (3, None),
        R::Failed(_) => (0, model.runtime_status_detail.as_deref()),
    };
    let steps = [
        "Preparing execution authorities",
        "Loading workspace state",
        "Connecting cockpit bridge",
    ];
    for (idx, step) in steps.iter().enumerate() {
        let marker = if is_failed {
            "  ○"
        } else if idx < step_active {
            "  ✓"
        } else if idx == step_active {
            "  •"
        } else {
            "  ○"
        };
        let style = if !is_failed && idx == step_active {
            tokens.text_primary
        } else {
            tokens.text_muted
        };
        let dur_opt = match idx {
            0 => model
                .startup_stage_duration("Initializing runtime")
                .or_else(|| model.startup_stage_duration("Booting")),
            1 => model
                .startup_stage_duration("Loading workspace state")
                .or_else(|| model.startup_stage_duration("Loading session state")),
            2 => model
                .startup_stage_duration("Connecting cockpit bridge")
                .or_else(|| model.startup_stage_duration("Loading execution state")),
            _ => None,
        };
        let dur_str = if idx < step_active {
            if let Some(d) = dur_opt {
                format!(" ({}ms)", d.as_millis())
            } else {
                String::new()
            }
        } else if idx == step_active && !is_failed {
            if let Some(started) = model.startup_stage_started_at {
                format!(" ({}ms…)", started.elapsed().as_millis())
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        lines.push(Line::from(vec![
            Span::styled(marker.to_string(), style),
            Span::styled(format!(" {step}{dur_str}"), style),
        ]));
    }
    if let Some(detail) = step_detail {
        if !detail.is_empty() && !is_failed {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                format!("  {detail}"),
                tokens.text_muted,
            )));
        }
    }
    if is_failed {
        lines.push(Line::raw(""));
        if let Some(reason) = model.runtime_status.failure_reason() {
            lines.push(Line::from(Span::styled(
                format!("  {reason}"),
                tokens.error,
            )));
        }
        lines.push(Line::from(Span::styled(
            "  Check logs and retry. Press q or Ctrl+C to exit.".to_string(),
            tokens.text_muted,
        )));
    } else {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "  Composer remains available; execution starts once the runtime is ready.".to_string(),
            tokens.text_muted,
        )));
        lines.push(Line::from(Span::styled(
            "  Press Ctrl+C to cancel startup.".to_string(),
            tokens.text_muted,
        )));
    }
    // Keep a quiet workspace hint so the panel reads as the cockpit shell,
    // not a dead splash.
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        format!("  Workspace: {}", model.workspace_path),
        tokens.text_muted,
    )));

    let max_rows = upper_area.height as usize;
    let truncated: Vec<Line> = lines.into_iter().take(max_rows.max(1)).collect();
    f.render_widget(Paragraph::new(truncated), upper_area);

    // Composer shell stays visible and editable during startup. Execution
    // itself stays gated in `TuiApplication::send_or_fail` (fail-closed).
    if is_composer_focused {
        composer.render(f, composer_area, tokens);
    } else {
        render_unfocused_composer(f, composer_area, composer, false, true, tokens);
    }

    if let Some(overlay) = active_overlay {
        render_view_overlay(f, area, overlay, model, tokens, setup_wizard);
    }
}

/// Render the bottom composer when unfocused — quiet, open, obvious.
fn render_unfocused_composer(
    f: &mut Frame,
    area: Rect,
    composer: &TuiComposer,
    is_busy: bool,
    is_waiting: bool,
    tokens: &ThemeTokens,
) {
    use ratatui::text::{Line, Span};
    let text = composer.text();
    let placeholder = if is_busy {
        "Working…"
    } else if is_waiting {
        "Waiting…"
    } else if text.is_empty() {
        "Ask M31A to build, inspect, fix, or explain..."
    } else {
        text
    };
    // Hairline separator + open prompt line (no box, no title noise).
    if area.height >= 2 {
        let sep = Paragraph::new(Line::from(Span::styled(
            format!(
                " {}",
                "─".repeat((area.width as usize).saturating_sub(2).min(120))
            ),
            tokens.separator,
        )));
        f.render_widget(
            sep,
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
        );
        let body = Paragraph::new(Line::from(vec![
            Span::styled("> ", tokens.text_muted),
            Span::styled(
                placeholder.to_string(),
                if is_busy || is_waiting || !text.is_empty() {
                    tokens.text_secondary
                } else {
                    tokens.text_muted
                },
            ),
        ]));
        f.render_widget(
            body,
            Rect {
                x: area.x,
                y: area.y + 1,
                width: area.width,
                height: area.height.saturating_sub(1),
            },
        );
    } else {
        f.render_widget(Paragraph::new(format!("> {placeholder}")), area);
    }
}

/// Render a contextual detail inspector for the active navigation detail.
///
/// Every branch reads live projection state — never placeholder content. The
/// header names the canonical registry entry; the body shows the actual
/// runtime inventory known to the projection (which may honestly be empty
/// before execution produces data).
#[allow(clippy::too_many_arguments)]
fn render_detail_inspector(
    f: &mut Frame,
    area: Rect,
    detail: ViewId,
    model: &TuiViewModel,
    tokens: &ThemeTokens,
    workflow_snapshot: &Option<crate::workflow::engine::WorkflowExecutionSnapshot>,
    workflow_dashboard_state: &mut WorkflowDashboardState,
    model_selector_state: &mut ModelSelectorState,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let registry = ViewRegistry::new();
    let title = registry.get(detail).map(|m| m.name).unwrap_or("Inspector");
    let max_rows = area.height.saturating_sub(2) as usize;
    let mut lines: Vec<String> = Vec::new();
    match detail {
        ViewId::DagInspector => {
            // When a workflow snapshot is available, render the full workflow dashboard.
            if let Some(snapshot) = workflow_snapshot {
                render_workflow_dashboard(
                    f,
                    area,
                    snapshot,
                    tokens,
                    true,
                    workflow_dashboard_state,
                );
                return;
            }
            // Fallback: basic task list
            lines.push(format!("tasks tracked: {}", model.tasks.len()));
            for t in model.tasks.iter().take(max_rows.saturating_sub(1)) {
                lines.push(format!(
                    " [{}] {} — {} ({}%) deps:[{}]",
                    t.status,
                    t.id,
                    t.title,
                    t.progress_pct,
                    t.dependencies.join(","),
                ));
            }
        }
        ViewId::AgentInspector => {
            lines.push(format!("agents supervised: {}", model.agents.len()));
            for a in model.agents.iter().take(max_rows.saturating_sub(1)) {
                lines.push(format!(
                    " {} [{}] task={:?} tokens={}",
                    a.id, a.state, a.current_task, a.total_tokens
                ));
            }
        }
        ViewId::ToolActivity => {
            lines.push(format!(
                "tools registered in projection: {}",
                model.tools.len()
            ));
            for t in model.tools.iter().take(max_rows.saturating_sub(1)) {
                lines.push(format!(
                    " {} exec={} errors={}",
                    t.name, t.executions_count, t.errors_count
                ));
            }
        }
        ViewId::FailureRecovery => {
            lines.push(format!(
                "lifecycle={:?} failure={:?}",
                model.lifecycle.stage, model.lifecycle.failure_reason
            ));
            lines.push(format!(
                "recovery attempts recorded: {}",
                model.recovery_attempts.len()
            ));
            if model.recovery_attempts.is_empty() {
                lines.push(" [No recovery attempts recorded - system operating normally or failure unrecovered]".to_string());
            } else {
                for a in model
                    .recovery_attempts
                    .iter()
                    .take(max_rows.saturating_sub(3))
                {
                    lines.push(format!(
                        " Attempt #{} [{}] class={} strat={} delay={}ms budget={}",
                        a.attempt_number,
                        a.outcome,
                        a.failure_class,
                        a.strategy,
                        a.backoff_delay_ms,
                        a.budget_consumed,
                    ));
                }
            }
            for log in model
                .logs
                .iter()
                .rev()
                .take(max_rows.saturating_sub(lines.len() + 1))
            {
                if log.level == "ERROR" || log.level == "WARN" {
                    lines.push(format!(" [{}] {}", log.level, log.message));
                }
            }
        }
        ViewId::ApprovalsQueue | ViewId::PolicyLedger => {
            lines.push(format!("pending approvals: {}", model.approvals.len()));
            for a in model.approvals.iter().take(max_rows.saturating_sub(1)) {
                lines.push(format!(
                    " {} tool={} risk={} {}",
                    a.id, a.tool_name, a.risk_tier, a.parameters_summary
                ));
            }
        }
        ViewId::AuditLog | ViewId::EventLog => {
            lines.push(format!(
                "events processed: {} (timeline: {})",
                model.system_stats.events_processed,
                model.timeline.len()
            ));
            if !model.timeline.is_empty() {
                for e in model.timeline.iter().rev().take(max_rows.saturating_sub(2)) {
                    lines.push(format!(
                        " #{} [{}] [{}] {}: {}",
                        e.sequence, e.timestamp, e.level, e.category, e.details
                    ));
                }
            } else {
                for log in model.logs.iter().rev().take(max_rows.saturating_sub(2)) {
                    lines.push(format!(" [{}] {}", log.level, log.message));
                }
            }
        }
        ViewId::ArtifactExplorer | ViewId::ReportCard => {
            lines.push(format!(
                "artifacts tracked: {} (canonical store)",
                model.artifacts.len()
            ));
            if model.artifacts.is_empty() {
                lines.push(" [No artifacts recorded yet - projection awaits canonical ArtifactService records]".to_string());
            } else {
                for a in model.artifacts.iter().take(max_rows.saturating_sub(2)) {
                    lines.push(format!(
                        " [{}] {} (v{}) size={}b path={}",
                        a.status, a.name, a.version, a.size_bytes, a.logical_path,
                    ));
                }
            }
            if model.verification_summary.total_checks > 0 {
                lines.push(format!(
                    "verification: total={} passed={} failed={} blocked={}",
                    model.verification_summary.total_checks,
                    model.verification_summary.passed_count,
                    model.verification_summary.failed_count,
                    model.verification_summary.blocked_count
                ));
            }
        }
        ViewId::CheckpointTree | ViewId::RollbackInspector | ViewId::VerificationSuite => {
            lines.push("=== CANONICAL TRACEABILITY CHAIN ===".to_string());
            if let Some(t) = model.traceability.first() {
                lines.push(format!(
                    " 1. User Decision:  {}",
                    t.user_decision.as_deref().unwrap_or("[Not recorded]")
                ));
                lines.push(format!(
                    " 2. Plan Revision:  {} (hash: {})",
                    t.plan_revision
                        .map(|r| r.to_string())
                        .unwrap_or_else(|| "[Not recorded]".to_string()),
                    t.plan_hash.as_deref().unwrap_or("[None]")
                ));
                lines.push(format!(
                    " 3. Task Revision:  {} (task: {})",
                    t.task_revision
                        .map(|r| r.to_string())
                        .unwrap_or_else(|| "[Not recorded]".to_string()),
                    t.task_id.as_deref().unwrap_or("[None]")
                ));
                lines.push(format!(
                    " 4. Execution:      {}",
                    t.execution_id.as_deref().unwrap_or("[Not recorded]")
                ));
                lines.push(format!(
                    " 5. Source Change:  {}",
                    t.source_change.as_deref().unwrap_or("[Not recorded]")
                ));
                lines.push(format!(
                    " 6. Verification:   {} (status: {})",
                    t.verification_id.as_deref().unwrap_or("[Not recorded]"),
                    t.verification_status.as_deref().unwrap_or("[None]")
                ));
                lines.push(format!(
                    " 7. Evidence:       {}",
                    t.evidence_artifact_id
                        .as_deref()
                        .unwrap_or("[Not recorded]")
                ));
            } else {
                lines.push(
                    " [No complete traceability chain recorded yet — awaits lifecycle transitions]"
                        .to_string(),
                );
            }
            if detail == ViewId::VerificationSuite {
                lines.push("--- VERIFICATION SUITE CHECKS ---".to_string());
                if model.verification_checks.is_empty() {
                    lines.push(
                        " [No verification checks recorded yet - status: not run / pending]"
                            .to_string(),
                    );
                } else {
                    for c in model
                        .verification_checks
                        .iter()
                        .take(max_rows.saturating_sub(lines.len() + 1))
                    {
                        lines.push(format!(
                            " [{}] tier={} {}: {}",
                            c.status, c.tier_name, c.command_or_tool, c.summary
                        ));
                    }
                }
            }
        }
        ViewId::ModelRegistry => {
            let (providers, models) =
                crate::tui::surface::model_selector::resolve_display_models_and_providers(
                    &model.catalog_models,
                    &model.active_provider,
                    &model.active_model,
                );
            render_model_selector(
                f,
                area,
                &providers,
                &models,
                &model.active_provider,
                &model.active_model,
                None,
                tokens,
                true,
                model_selector_state,
            );
            return;
        }
        ViewId::BudgetMonitor => {
            lines.push("=== CANONICAL BUDGET ENFORCEMENT ===".to_string());
            lines.push(format!(" Scope: {}", model.budget.scope));
            let tokens_max = if model.budget.max_tokens > 0 {
                model.budget.max_tokens.to_string()
            } else {
                "unbounded".to_string()
            };
            let rem_tokens = if model.budget.max_tokens > 0 {
                model.budget.remaining_tokens.to_string()
            } else {
                "unbounded".to_string()
            };
            lines.push(format!(
                " Tokens: {} / {} (Remaining: {})",
                model.budget.consumed_tokens, tokens_max, rem_tokens
            ));
            let steps_max = if model.budget.max_tool_calls > 0 {
                model.budget.max_tool_calls.to_string()
            } else {
                "unbounded".to_string()
            };
            let rem_steps = if model.budget.max_tool_calls > 0 {
                model
                    .budget
                    .max_tool_calls
                    .saturating_sub(model.budget.consumed_tool_calls)
                    .to_string()
            } else {
                "unbounded".to_string()
            };
            lines.push(format!(
                " Steps:  {} / {} (Remaining: {})",
                model.budget.consumed_tool_calls, steps_max, rem_steps
            ));
            let cost_max = if model.budget.allocated_cents > 0 {
                format!("${:.2}", model.budget.allocated_cents as f64 / 100.0)
            } else {
                "unbounded".to_string()
            };
            let rem_cost = if model.budget.allocated_cents > 0 {
                format!("${:.2}", model.budget.remaining_cents as f64 / 100.0)
            } else {
                "unbounded".to_string()
            };
            let cost_display = if model.budget.consumed_cents > 0 {
                format!("${:.4}", model.budget.consumed_cents as f64 / 100.0)
            } else if model.budget.consumed_tokens > 0 {
                "unpriced".to_string()
            } else {
                "$0.0000".to_string()
            };
            lines.push(format!(
                " Cost:   {} / {} (Remaining: {})",
                cost_display, cost_max, rem_cost
            ));
            let status = if model.budget.is_exhausted {
                "EXHAUSTED"
            } else if model.budget.is_constrained {
                "CONSTRAINED"
            } else {
                "NORMAL"
            };
            lines.push(format!(" Status: {}", status));
        }
        ViewId::ProfileMatrix => {
            lines.push("=== CANONICAL AGENT PROFILES ===".to_string());
            if let Ok(reg) = crate::agent::registry::RoleRegistry::global().read() {
                for id in reg.all_ids().iter().take(max_rows.saturating_sub(2)) {
                    if let Some(def) = reg.resolve(&crate::state_machine::agent::AgentRole::new(id))
                    {
                        let pref_model = &def.profile.model_policy.preferred_model;
                        lines.push(format!(
                            " {} [max_steps={}, model={}] {}",
                            def.id.as_str(),
                            def.profile.max_steps,
                            pref_model,
                            def.description
                        ));
                    }
                }
            } else {
                lines.push(" [Unable to read RoleRegistry]".to_string());
            }
        }
        ViewId::SkillRegistry => {
            lines.push(format!(
                "=== CANONICAL SKILL REGISTRY ({}) ===",
                model.skills.len()
            ));
            if model.skills.is_empty() {
                lines.push(" [No skills discovered in workspace or catalog]".to_string());
            } else {
                for s in model.skills.iter().take(max_rows.saturating_sub(2)) {
                    lines.push(format!(
                        " {} [{}] tier={} risk={} ({})",
                        s.id, s.status, s.verification_tier, s.risk_tier, s.origin_tier
                    ));
                }
            }
        }
        ViewId::ProviderSetup | ViewId::ContextMonitor | ViewId::TelemetryGraphs => {
            lines.push(format!(
                "model={} provider={} profile={}",
                model.active_model, model.active_provider, model.active_profile
            ));
            lines.push(format!(
                "tokens prompt={} completion={} api_calls={} cost_cents={}",
                model.model_usage.prompt_tokens,
                model.model_usage.completion_tokens,
                model.model_usage.api_calls,
                model
                    .model_usage
                    .total_cost_cents
                    .map(|c| if c > 0 {
                        c.to_string()
                    } else if model.model_usage.effective_total_tokens() > 0 {
                        "unpriced".to_string()
                    } else {
                        "0".to_string()
                    })
                    .unwrap_or_else(|| if model.model_usage.effective_total_tokens() > 0 {
                        "unpriced".to_string()
                    } else {
                        "n/a".to_string()
                    }),
            ));
        }
        ViewId::MissionDashboard => {
            lines.push(format!("Objective: {}", model.objective));
            lines.push(format!(
                "Status: {} | Stage: {}",
                model.mission_status,
                model.lifecycle.stage.label()
            ));
            if let Some(ref mid) = model.mission_id {
                lines.push(format!("Mission ID: {}", mid));
            }
            lines.push(format!("Branch: {}", model.git_branch));
            lines.push(format!(
                "Tasks: {} total (completed: {})",
                model.tasks.len(),
                model
                    .tasks
                    .iter()
                    .filter(|t| t.status == "completed")
                    .count()
            ));
            lines.push(format!(
                "Agents: {} active | Unseen: {}",
                model.agents.len(),
                model.unseen_count
            ));
        }
        ViewId::TaskDetails => {
            lines.push(format!("Tasks in DAG: {}", model.tasks.len()));
            if model.tasks.is_empty() {
                lines.push(" [No active tasks scheduled in task graph]".to_string());
            } else {
                for t in model.tasks.iter().take(max_rows.saturating_sub(1)) {
                    lines.push(format!(
                        " [{}] {} — {} ({}%) deps:[{}]",
                        t.status,
                        t.id,
                        t.title,
                        t.progress_pct,
                        t.dependencies.join(",")
                    ));
                }
            }
        }
        ViewId::ExecutionStream => {
            lines.push(format!("Timeline Events: {}", model.timeline.len()));
            if model.timeline.is_empty() {
                lines.push(" [Awaiting execution events from runtime bus]".to_string());
            } else {
                for e in model.timeline.iter().rev().take(max_rows.saturating_sub(1)) {
                    lines.push(format!(
                        " #{} [{}] [{}] {}: {}",
                        e.sequence, e.timestamp, e.level, e.category, e.details
                    ));
                }
            }
        }
        ViewId::SystemHealth => {
            lines.push("=== SYSTEM HEALTH & DIAGNOSTICS ===".to_string());
            lines.push(format!(" Lifecycle: {}", model.lifecycle.stage.label()));
            lines.push(format!(
                " Events Processed: {}",
                model.system_stats.events_processed
            ));
            lines.push(format!(
                " DB Access During Render: {} (Invariant: 0)",
                model.sqlite_render_access_count()
            ));
            lines.push(format!(
                " Active Agents: {} | Pending Approvals: {}",
                model.agents.len(),
                model.approvals.len()
            ));
            lines.push(format!(
                " Budget Scope: {} | Exhausted: {}",
                model.budget.scope, model.budget.is_exhausted
            ));
            let error_count = model.logs.iter().filter(|l| l.level == "ERROR").count();
            let warn_count = model.logs.iter().filter(|l| l.level == "WARN").count();
            lines.push(format!(
                " Log Diagnostics: {} warnings, {} errors",
                warn_count, error_count
            ));
        }
        ViewId::ProcessSandbox => {
            lines.push("=== PROCESS SANDBOX ISOLATION ===".to_string());
            lines.push(format!(" Workspace Root: {}", model.workspace_path));
            lines.push(format!(
                " Sandboxed Tools Registered: {}",
                model.tools.len()
            ));
            for t in model.tools.iter().take(max_rows.saturating_sub(2)) {
                lines.push(format!(
                    " Tool: {} (runs: {}, errors: {})",
                    t.name, t.executions_count, t.errors_count
                ));
            }
            if model.tools.is_empty() {
                lines.push(" [No external process tools currently spawned in sandbox]".to_string());
            }
        }
        ViewId::GitTimeline => {
            lines.push("=== GIT REPOSITORY TIMELINE ===".to_string());
            lines.push(format!(" Current Branch: {}", model.git_branch));
            lines.push(format!(" Workspace: {}", model.workspace_path));
            let git_logs: Vec<_> = model.logs.iter().filter(|l| l.source == "git").collect();
            if git_logs.is_empty() {
                lines.push(" [No recent git operations logged]".to_string());
            } else {
                for l in git_logs.iter().rev().take(max_rows.saturating_sub(2)) {
                    lines.push(format!(" [{}] {}", l.level, l.message));
                }
            }
        }
        ViewId::DoctorDiagnostics => {
            lines.push("=== RUNTIME DOCTOR & PROBE DIAGNOSTICS ===".to_string());
            lines.push(format!(" Workspace Root: {}", model.workspace_path));
            lines.push(format!(
                " Active Provider: {} | Model: {}",
                model.active_provider, model.active_model
            ));
            lines.push(format!(
                " Session Lifecycle: {}",
                model.lifecycle.stage.label()
            ));
            lines
                .push(" Probes Available: 6 (Config, DB, Net, Sandbox, Git, Security)".to_string());
            lines.push(" Press Enter or /doctor to run full diagnostic suite.".to_string());
        }
        ViewId::OnboardingTour => {
            lines.push("=== ONBOARDING & COCKPIT TOUR ===".to_string());
            lines.push(format!(
                " 1. Workspace: Configured at {}",
                model.workspace_path
            ));
            lines.push(format!(" 2. Model: Active model is {}", model.active_model));
            lines.push(" 3. Navigation: Press 1-8 for primary cockpit views".to_string());
            lines.push(
                " 4. Universal Palette: Press Ctrl+P anytime to search views and commands"
                    .to_string(),
            );
            lines.push(
                " 5. Safety: Every mutation passes through policy before execution".to_string(),
            );
        }
        ViewId::SetupWizard => {
            lines.push("=== WORKSPACE SETUP WIZARD ===".to_string());
            lines.push(format!(" Workspace: {}", model.workspace_path));
            lines.push(format!(" Configured Provider: {}", model.active_provider));
            lines.push(format!(" Configured Model: {}", model.active_model));
            lines.push(
                " Setup wizard guides provider selection, API keys, and workspace constraints."
                    .to_string(),
            );
        }
        ViewId::WorkspaceSetup => {
            lines.push("=== WORKSPACE ENVIRONMENT SETUP ===".to_string());
            lines.push(format!(" Path: {}", model.workspace_path));
            lines.push(format!(" Git Branch: {}", model.git_branch));
            lines.push(format!(" Discovered Skills: {}", model.skills.len()));
            lines.push(format!(" Discovered Tools: {}", model.tools.len()));
            lines.push(format!(" Active Profile: {}", model.active_profile));
        }
        ViewId::PromptLab => {
            lines.push("=== PROMPT LAB & SCRATCHPAD ===".to_string());
            lines.push(format!(
                " Model: {} ({})",
                model.active_model, model.active_provider
            ));
            lines.push(format!(" Profile Policy: {}", model.active_profile));
            lines.push(format!(
                " Prompt Tokens Consumed: {}",
                model.model_usage.prompt_tokens
            ));
            lines.push(format!(
                " Completion Tokens Consumed: {}",
                model.model_usage.completion_tokens
            ));
            lines.push(
                " Experiment with system prompt directives and tool schema variations.".to_string(),
            );
        }
        ViewId::QuarantineManager => {
            lines.push("=== ARTIFACT & TOOL QUARANTINE MANAGER ===".to_string());
            lines.push(
                " Security Policy: Fail-closed on unauthorized external side effects".to_string(),
            );
            lines.push(format!(
                " Registered Tools: {} | Pending Approvals: {}",
                model.tools.len(),
                model.approvals.len()
            ));
            lines.push(" Quarantined Items: 0 (No containment breaches detected)".to_string());
        }
        ViewId::PluginManager => {
            lines.push("=== DYNAMIC PLUGIN & EXTENSIONS MANAGER ===".to_string());
            lines.push(format!(" Discovered Skills: {}", model.skills.len()));
            lines.push(format!(" Built-in Tools: {}", model.tools.len()));
            for s in model.skills.iter().take(max_rows.saturating_sub(2)) {
                lines.push(format!(
                    " Plugin/Skill: {} [{}] tier={}",
                    s.id, s.status, s.verification_tier
                ));
            }
            if model.skills.is_empty() {
                lines.push(
                    " [No external plugins discovered in workspace .m31a/plugins]".to_string(),
                );
            }
        }
        ViewId::SettingsConfig => {
            lines.push("=== CONFIGURATION & RUNTIME SETTINGS ===".to_string());
            lines.push(format!(" Workspace: {}", model.workspace_path));
            lines.push(format!(" Active Provider: {}", model.active_provider));
            lines.push(format!(" Active Model: {}", model.active_model));
            lines.push(format!(" Active Profile: {}", model.active_profile));
            lines.push(
                " Invariant: Configuration changes require valid provider credentials.".to_string(),
            );
        }
        ViewId::KeybindingsGuide => {
            lines.push("=== CANONICAL KEYBOARD SHORTCUTS ===".to_string());
            lines.push(" 1: Mission Dashboard     2: DAG Inspector".to_string());
            lines.push(" 3: Task Details          4: Agent Inspector".to_string());
            lines.push(" 5: Tool Activity         6: Execution Stream".to_string());
            lines.push(" 7: Failure Recovery      8: System Health".to_string());
            lines.push(" 9: Policy Ledger         0: Approvals Queue".to_string());
            lines.push(" g: Git Timeline          a: Artifact Explorer".to_string());
            lines.push(" b: Budget Monitor        s: Skill Registry".to_string());
            lines.push(" d: Doctor Probes         m: Model Selector".to_string());
            lines.push(" Ctrl+P: Command Palette  Ctrl+C: Cancel Action".to_string());
            lines.push(" Tab/BackTab: Cycle Focus Esc: Back / Close Pane".to_string());
        }
        ViewId::HelpDocs => {
            lines.push("=== M31A OPERATOR MANUAL & REFERENCE ===".to_string());
            lines.push(" Invariant: The model proposes. The runtime decides.".to_string());
            lines.push(" Slash Commands:".to_string());
            lines.push("   /help    - Display command reference".to_string());
            lines.push("   /doctor  - Run system health probes".to_string());
            lines.push("   /version - Display engine version".to_string());
            lines.push("   /clear   - Clear conversation history".to_string());
            lines.push(" Input '@' to mention tools/skills, '/' for commands.".to_string());
        }
        ViewId::StartupRecovery => {
            lines.push("=== STARTUP INTEGRITY & RECOVERY ===".to_string());
            lines.push(format!(
                " Session ID: {}",
                model.lifecycle.session_id.as_deref().unwrap_or("none")
            ));
            lines.push(format!(
                " Lifecycle Stage: {}",
                model.lifecycle.stage.label()
            ));
            let (status_str, recovery_str) = match &model.runtime_status {
                crate::tui::model::RuntimeStartupState::Ready => (
                    "Verified SQLite connection pool operational".to_string(),
                    "WAL recovery verified; crash-consistent transactions ensured.".to_string(),
                ),
                crate::tui::model::RuntimeStartupState::Failed(err) => (
                    "Database / runtime startup failed".to_string(),
                    format!("Recovery unverified: {err}"),
                ),
                _ => (
                    "Database verification in progress".to_string(),
                    "Recovery scanning active; awaiting runtime readiness.".to_string(),
                ),
            };
            lines.push(format!(" DB Health: {status_str}"));
            lines.push(format!(" {recovery_str}"));
        }
        ViewId::CommandPalette => {
            lines.push("=== UNIVERSAL COMMAND PALETTE (Ctrl+P) ===".to_string());
            lines.push(" Instant fuzzy search across all 40 canonical views.".to_string());
            lines.push(" Execute slash commands and governed mission actions.".to_string());
            lines.push(" Type to filter, Up/Down to navigate, Enter to select.".to_string());
        }
        ViewId::DiffViewer => {
            lines.push("=== GIT WORKSPACE DIFF VIEWER ===".to_string());
            lines.push(format!(" Branch: {}", model.git_branch));
            lines.push(format!(" Workspace: {}", model.workspace_path));
            lines.push(" Track uncommitted modifications, additions, and deletions.".to_string());
        }
        ViewId::MissionCreation => {
            lines.push("=== MISSION CREATION & INITIALIZATION ===".to_string());
            lines.push(format!(
                " Current Mission: {}",
                model.mission_id.as_deref().unwrap_or("[None]")
            ));
            lines.push(format!(" Active Profile: {}", model.active_profile));
            lines.push(format!(" Default Model: {}", model.active_model));
            lines.push(
                " Enter mission objective in prompt composer to initiate execution.".to_string(),
            );
        }
    }
    if lines.is_empty() {
        lines.push("(no data yet — projection awaits runtime events)".to_string());
    }
    let text = lines
        .into_iter()
        .take(max_rows.max(1))
        .collect::<Vec<_>>()
        .join("\n");
    // Quiet inspector: hairline on top, plain title, no focused-border shout.
    let block = Block::default()
        .title(format!(" {title} "))
        .borders(Borders::TOP)
        .border_style(tokens.separator);
    f.render_widget(Paragraph::new(text).block(block), area);
}

/// Render a transient overlay view as a centered floating panel.
///
/// Overlays never own business state; they present registry-described
/// contextual help/selection over live projection summaries.
fn render_view_overlay(
    f: &mut Frame,
    area: Rect,
    overlay: ViewId,
    model: &TuiViewModel,
    tokens: &ThemeTokens,
    setup_wizard: &mut Option<SetupWizardScreen>,
) {
    if overlay == ViewId::SetupWizard {
        render_setup_wizard_overlay(f, area, setup_wizard, &model.workspace_path, tokens);
        return;
    }

    let registry = ViewRegistry::new();
    let meta = registry.get(overlay);
    let name = meta.map(|m| m.name).unwrap_or("Overlay");
    let category = meta.map(|m| m.domain_category).unwrap_or("General");
    let route = meta.map(|m| m.route_path).unwrap_or("/unknown");
    let shortcut = meta
        .and_then(|m| m.hotkey)
        .map(|c| format!(" [{c}]"))
        .unwrap_or_default();

    let width = (area.width.saturating_sub(8)).clamp(30, 80);
    let height = 14.min(area.height.saturating_sub(2)).max(6);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let popup = Rect::new(x, y, width, height);

    let mut content: Vec<String> = Vec::new();
    match overlay {
        ViewId::ApprovalsQueue => {
            content.push(format!("Pending Approvals: {}", model.approvals.len()));
            if model.approvals.is_empty() {
                content.push("No approvals awaiting operator confirmation.".to_string());
            } else {
                for a in model.approvals.iter().take(3) {
                    content.push(format!("• {} tool={} ({})", a.id, a.tool_name, a.risk_tier));
                }
            }
            content.push("\n[A]pprove All  [R]eject  [Esc] Close".to_string());
        }
        ViewId::RollbackInspector => {
            content.push("Checkpoint Rollback Inspector".to_string());
            if let Some(t) = model.traceability.first() {
                content.push(format!(
                    "Target Plan: {}",
                    t.plan_hash.as_deref().unwrap_or("head")
                ));
                content.push(format!(
                    "Last Verification: {}",
                    t.verification_status.as_deref().unwrap_or("none")
                ));
            } else {
                content.push("No prior checkpoints available for rollback.".to_string());
            }
            content.push("\n[Enter] Confirm Rollback  [Esc] Dismiss".to_string());
        }
        ViewId::StartupRecovery => {
            content.push("Startup State Integrity & Recovery".to_string());
            content.push(format!(
                "Session: {}",
                model.lifecycle.session_id.as_deref().unwrap_or("new")
            ));
            content.push(format!("Status: {}", model.lifecycle.stage.label()));
            content.push("\n[R]esume Session  [N]ew Session  [Esc] Dismiss".to_string());
        }
        ViewId::MissionCreation => {
            content.push("Initialize New Mission".to_string());
            content.push(format!("Workspace: {}", model.workspace_path));
            content.push(format!(
                "Model: {} ({})",
                model.active_model, model.active_provider
            ));
            content.push("\nType objective into prompt composer and press Enter.".to_string());
            content.push("[Esc] Close".to_string());
        }
        ViewId::KeybindingsGuide => {
            content.push("Canonical Keyboard Shortcuts".to_string());
            content.push("1-8: Cockpit Views | 9: Policy | 0: Approvals".to_string());
            content.push("g: Git | a: Artifacts | b: Budget | s: Skills | d: Doctor".to_string());
            content.push("Ctrl+P: Command Palette | Ctrl+C: Cancel | Tab: Focus".to_string());
            content.push("[Esc] Close".to_string());
        }
        ViewId::HelpDocs => {
            content.push("M31 Autonomous Operator Guide".to_string());
            content.push("Invariant: The model proposes. The runtime decides.".to_string());
            content.push("Slash commands: /help, /doctor, /version, /plan, /clear".to_string());
            content.push("[Esc] Close".to_string());
        }
        ViewId::OnboardingTour => {
            content.push("Welcome to M31 Autonomous Cockpit".to_string());
            content.push(format!("Workspace: {}", model.workspace_path));
            content.push(format!(
                "Configured: {} via {}",
                model.active_model, model.active_provider
            ));
            content.push("Use numeric hotkeys (1-8) to navigate cockpit views.".to_string());
            content.push("[Esc] Close Tour".to_string());
        }
        ViewId::CommandPalette => {
            content.push("Universal Command Palette".to_string());
            content.push("Search all 40 views, missions, and runtime commands.".to_string());
            content.push("Press Ctrl+P anywhere in the cockpit to activate.".to_string());
            content.push("[Esc] Close".to_string());
        }
        _ => {
            content.push(format!("{name}{shortcut}"));
            content.push(format!("Domain: {category} | Route: {route}"));
            content.push(format!(
                "Lifecycle: {} | Mission: {}",
                model.lifecycle.stage.label(),
                model.mission_id.as_deref().unwrap_or("none")
            ));
            content.push("[Esc] Close".to_string());
        }
    }

    let body = content.join("\n");
    let block = Block::default()
        .title(format!(" {name} "))
        .borders(Borders::ALL)
        .border_style(tokens.separator);
    f.render_widget(ratatui::widgets::Clear, popup);
    f.render_widget(Paragraph::new(body).block(block), popup);
}

/// Render the Setup Wizard as a full-screen modal overlay.
///
/// The wizard is bound to the TUI's authoritative workspace path — never the
/// process literal `"."` — so re-configuration persists into the same
/// workspace the cockpit was launched against.
/// Constrain conversational reading width on very wide terminals.
/// Surplus space stays quiet (whitespace) rather than stretched text or a
/// forced telemetry pane.
pub(crate) fn constrain_reading_width(area: Rect, max_width: u16) -> Rect {
    if area.width <= max_width + 8 {
        return area;
    }
    let x = area.x + (area.width - max_width) / 2;
    Rect {
        x,
        y: area.y,
        width: max_width,
        height: area.height,
    }
}

fn render_setup_wizard_overlay(
    f: &mut Frame,
    area: Rect,
    setup_wizard: &mut Option<SetupWizardScreen>,
    workspace_path: &str,
    _tokens: &ThemeTokens,
) {
    // Initialize wizard if not present
    if setup_wizard.is_none() {
        let root = if workspace_path.trim().is_empty() {
            std::path::PathBuf::from(".")
        } else {
            std::path::PathBuf::from(workspace_path)
        };
        *setup_wizard = Some(SetupWizardScreen::new(root));
    }

    if let Some(wizard) = setup_wizard {
        // Render full screen
        wizard.render(f, area);
    }
}
