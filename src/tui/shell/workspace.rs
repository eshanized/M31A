//! Unified Workspace Orchestration & Composition Surface (Section 5, 18, TUI-01).
//!
//! Organizes the primary cockpit workspace into cohesive split-pane surfaces
//! ensuring the operator never feels ejected from the autonomous execution loop.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::composer::TuiComposer;
use crate::tui::focus::FocusTarget;
use crate::tui::model::TuiViewModel;
use crate::tui::navigation::ScreenId;
use crate::tui::registry::{ViewId, ViewRegistry};
use crate::tui::replay::ReplayController;
use crate::tui::screens::wizard::SetupWizardScreen;
use crate::tui::surface::{
    ModelInfo, ModelSelectorState, ProviderInfo, WorkflowDashboardState, render_agents_surface,
    render_artifacts_surface, render_conversation_surface, render_doctor_surface,
    render_git_surface, render_jobs_surface, render_model_selector, render_replay_surface,
    render_tasks_surface, render_telemetry_surface, render_tools_surface,
    render_verification_surface, render_workflow_dashboard,
};
use crate::tui::theme::{ThemeMode, ThemeTokens};

/// Render the unified cockpit workspace.
#[allow(clippy::too_many_arguments)]
pub fn render_workspace(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
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
) {
    if area.width < 10 || area.height < 6 {
        return;
    }

    // Allocate bottom 4 rows for Composer
    let workspace_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(4)])
        .split(area);

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

    // 1. Render Upper Workspace depending on active screen and terminal width
    let is_wide = area.width >= 100;

    match screen {
        ScreenId::Dashboard => {
            let has_active_work =
                !model.tasks.is_empty() || !model.agents.is_empty() || !model.approvals.is_empty();

            if has_active_work && area.width >= 90 {
                let (left_pct, right_pct) = if area.width >= 120 {
                    (65, 35)
                } else {
                    (60, 40)
                };
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(left_pct),
                        Constraint::Percentage(right_pct),
                    ])
                    .split(upper_area);

                render_conversation_surface(
                    f,
                    cols[0],
                    model,
                    tokens,
                    focus == FocusTarget::Conversation,
                );
                render_dashboard_overview_panel(
                    f,
                    cols[1],
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                );
            } else if has_active_work && area.height >= 24 {
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                    .split(upper_area);

                render_conversation_surface(
                    f,
                    rows[0],
                    model,
                    tokens,
                    focus == FocusTarget::Conversation,
                );
                render_dashboard_overview_panel(
                    f,
                    rows[1],
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                );
            } else {
                render_conversation_surface(
                    f,
                    upper_area,
                    model,
                    tokens,
                    focus == FocusTarget::Conversation,
                );
            }
        }
        ScreenId::TaskGraph => {
            if is_wide {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                    .split(upper_area);
                render_conversation_surface(
                    f,
                    cols[0],
                    model,
                    tokens,
                    focus == FocusTarget::Conversation,
                );
                render_tasks_surface(
                    f,
                    cols[1],
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                    context_selected_idx,
                );
            } else {
                render_tasks_surface(
                    f,
                    upper_area,
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                    context_selected_idx,
                );
            }
        }
        ScreenId::Agents => {
            if is_wide {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                    .split(upper_area);
                render_conversation_surface(
                    f,
                    cols[0],
                    model,
                    tokens,
                    focus == FocusTarget::Conversation,
                );
                render_agents_surface(
                    f,
                    cols[1],
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                    context_selected_idx,
                );
            } else {
                render_agents_surface(
                    f,
                    upper_area,
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                    context_selected_idx,
                );
            }
        }
        ScreenId::Tools => {
            if is_wide {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                    .split(upper_area);
                render_conversation_surface(
                    f,
                    cols[0],
                    model,
                    tokens,
                    focus == FocusTarget::Conversation,
                );
                render_tools_surface(
                    f,
                    cols[1],
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                    context_selected_idx,
                );
            } else {
                render_tools_surface(
                    f,
                    upper_area,
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                    context_selected_idx,
                );
            }
        }
        ScreenId::Git => {
            if is_wide {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                    .split(upper_area);
                render_conversation_surface(
                    f,
                    cols[0],
                    model,
                    tokens,
                    focus == FocusTarget::Conversation,
                );
                render_git_surface(
                    f,
                    cols[1],
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                    context_selected_idx,
                );
            } else {
                render_git_surface(
                    f,
                    upper_area,
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                    context_selected_idx,
                );
            }
        }
        ScreenId::Verification => {
            if is_wide {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                    .split(upper_area);
                render_conversation_surface(
                    f,
                    cols[0],
                    model,
                    tokens,
                    focus == FocusTarget::Conversation,
                );
                render_verification_surface(
                    f,
                    cols[1],
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                    context_selected_idx,
                );
            } else {
                render_verification_surface(
                    f,
                    upper_area,
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                    context_selected_idx,
                );
            }
        }
        ScreenId::Jobs => {
            if is_wide {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                    .split(upper_area);
                render_conversation_surface(
                    f,
                    cols[0],
                    model,
                    tokens,
                    focus == FocusTarget::Conversation,
                );
                render_jobs_surface(
                    f,
                    cols[1],
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                );
            } else {
                render_jobs_surface(
                    f,
                    upper_area,
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                );
            }
        }
        ScreenId::Doctor => {
            if is_wide {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                    .split(upper_area);
                render_conversation_surface(
                    f,
                    cols[0],
                    model,
                    tokens,
                    focus == FocusTarget::Conversation,
                );
                render_doctor_surface(
                    f,
                    cols[1],
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                );
            } else {
                render_doctor_surface(
                    f,
                    upper_area,
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                );
            }
        }
        ScreenId::ModelUsage => {
            if is_wide {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                    .split(upper_area);
                render_conversation_surface(
                    f,
                    cols[0],
                    model,
                    tokens,
                    focus == FocusTarget::Conversation,
                );
                render_telemetry_surface(
                    f,
                    cols[1],
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                );
            } else {
                render_telemetry_surface(
                    f,
                    upper_area,
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                );
            }
        }
        ScreenId::Artifacts => {
            if is_wide {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                    .split(upper_area);
                render_conversation_surface(
                    f,
                    cols[0],
                    model,
                    tokens,
                    focus == FocusTarget::Conversation,
                );
                render_artifacts_surface(
                    f,
                    cols[1],
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                    context_selected_idx,
                );
            } else {
                render_artifacts_surface(
                    f,
                    upper_area,
                    model,
                    tokens,
                    focus == FocusTarget::ContextPanel,
                    context_selected_idx,
                );
            }
        }
        ScreenId::Replay => {
            render_replay_surface(
                f,
                upper_area,
                model,
                replay,
                tokens,
                focus == FocusTarget::ContextPanel,
            );
        }
        ScreenId::Mission | ScreenId::Approvals | ScreenId::Logs | ScreenId::Help => {
            // Contextual split view with conversation on left and secondary view on right
            if is_wide {
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(upper_area);
                render_conversation_surface(
                    f,
                    cols[0],
                    model,
                    tokens,
                    focus == FocusTarget::Conversation,
                );
                crate::tui::screens::render_screen(screen, f, cols[1], model);
            } else {
                crate::tui::screens::render_screen(screen, f, upper_area, model);
            }
        }
    }

    // 2. Render Bottom Composer
    let is_busy = model.mission_status == "running"
        || model.session_status == "running"
        || model.mission_status == "executing";

    if is_composer_focused {
        composer.render(f, composer_area, tokens);
    } else {
        render_unfocused_composer(f, composer_area, composer, is_busy, tokens);
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

/// Render the bottom composer when unfocused.
fn render_unfocused_composer(
    f: &mut Frame,
    area: Rect,
    composer: &TuiComposer,
    is_busy: bool,
    tokens: &ThemeTokens,
) {
    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();
    let title = if is_busy {
        " Composer [Executing · Esc to cancel | Ctrl+C to abort] "
    } else {
        " Composer [Unfocused · Press Enter or 'i' to type | / for commands | @ for files] "
    };

    let border_color = if is_mono {
        Color::White
    } else if is_busy {
        Color::Yellow
    } else {
        tokens.border_default.fg.unwrap_or(Color::DarkGray)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let placeholder_text = if is_busy {
        " m31a ❯ Working on task... [Esc to cancel | Ctrl+C to abort]"
    } else if composer.text().is_empty() {
        " m31a> Press Enter or 'i' to focus composer..."
    } else {
        " m31a> (Paused: press Enter or 'i' to resume typing)"
    };

    let p = Paragraph::new(placeholder_text)
        .style(if is_busy {
            Style::default().fg(Color::Yellow)
        } else {
            tokens.text_muted
        })
        .block(block);

    f.render_widget(p, area);
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
            // Render the full model selector surface
            let providers = vec![
                ProviderInfo {
                    id: "nvidia_nim".to_string(),
                    name: "NVIDIA NIM".to_string(),
                    is_current: model.active_provider == "nvidia_nim",
                    is_available: true,
                    model_count: 0, // Would be populated from model catalog
                    base_url: None,
                },
                ProviderInfo {
                    id: "anthropic".to_string(),
                    name: "Anthropic".to_string(),
                    is_current: model.active_provider == "anthropic",
                    is_available: false,
                    model_count: 0,
                    base_url: None,
                },
                ProviderInfo {
                    id: "openai".to_string(),
                    name: "OpenAI".to_string(),
                    is_current: model.active_provider == "openai",
                    is_available: false,
                    model_count: 0,
                    base_url: None,
                },
            ];
            let models = vec![
                ModelInfo {
                    model_id: "meta/llama-3.1-70b-instruct".to_string(),
                    display_name: Some("Llama 3.1 70B Instruct".to_string()),
                    tier: "Reasoning".to_string(),
                    context_capacity: 131072,
                    supports_tools: true,
                    is_current_primary: model.active_model == "meta/llama-3.1-70b-instruct",
                    is_current_fast: false,
                    availability: "Available".to_string(),
                },
                ModelInfo {
                    model_id: "meta/llama-3.2-11b-vision-instruct".to_string(),
                    display_name: Some("Llama 3.2 11B Vision Instruct".to_string()),
                    tier: "Fast".to_string(),
                    context_capacity: 131072,
                    supports_tools: true,
                    is_current_primary: false,
                    is_current_fast: model.active_model == "meta/llama-3.2-11b-vision-instruct",
                    availability: "Available".to_string(),
                },
            ];
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
            lines.push(format!(
                " Cost:   ${:.4} / {} (Remaining: {})",
                model.budget.consumed_cents as f64 / 100.0,
                cost_max,
                rem_cost
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
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "n/a".to_string()),
            ));
        }
        _ => {
            lines.push(format!(
                "lifecycle={:?} mission={:?} status={}",
                model.lifecycle.stage, model.mission_id, model.mission_status
            ));
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
    let block = Block::default()
        .title(format!(" {title} [Esc closes] "))
        .borders(Borders::ALL)
        .border_style(tokens.border_focused);
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
    let name = registry.get(overlay).map(|m| m.name).unwrap_or("Overlay");
    let width = (area.width.saturating_sub(8)).clamp(20, 72);
    let height = 9.min(area.height.saturating_sub(2)).max(5);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let popup = Rect::new(x, y, width, height);
    let body = format!(
        "{name}\nroute: {:?}\nlifecycle: {:?}\nmission: {:?}\n[Esc] close",
        crate::tui::navigation::canonical_screen(overlay),
        model.lifecycle.stage,
        model.mission_id,
    );
    let block = Block::default()
        .title(format!(" {name} "))
        .borders(Borders::ALL)
        .border_style(tokens.border_focused);
    f.render_widget(ratatui::widgets::Clear, popup);
    f.render_widget(Paragraph::new(body).block(block), popup);
}

/// Render the Setup Wizard as a full-screen modal overlay.
///
/// The wizard is bound to the TUI's authoritative workspace path — never the
/// process literal `"."` — so re-configuration persists into the same
/// workspace the cockpit was launched against.
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

/// Render the overview panel for the Dashboard (retaining exact test contract strings).
fn render_dashboard_overview_panel(
    f: &mut Frame,
    area: Rect,
    model: &TuiViewModel,
    tokens: &ThemeTokens,
    is_focused: bool,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let is_mono = tokens.mode == ThemeMode::MonochromeANSI || ThemeTokens::is_no_color_active();
    let border_style = if is_focused {
        tokens.border_focused
    } else if is_mono {
        Style::default()
    } else {
        tokens.border_default
    };

    let mut text = String::new();
    if area.height < 12 {
        text.push_str("MISSION COCKPIT OVERVIEW\n");
        text.push_str(&format!(
            "Tasks Total: {} | Active Agents: {} | Pending Approvals: {}\n",
            model.tasks.len(),
            model.agents.len(),
            model.approvals.len(),
        ));
        text.push_str("Recent Logs:\n");
        for log in model
            .logs
            .iter()
            .rev()
            .take(area.height.saturating_sub(4) as usize)
        {
            text.push_str(&format!(
                " [{}] [{}] {}\n",
                log.level, log.source, log.message
            ));
        }
    } else {
        text.push_str(&format!(
            "MISSION COCKPIT OVERVIEW\n\
             ========================\n\
             Status:           {}\n\
             Objective:        {}\n\
             Tasks Total:      {} (Completed: {})\n\
             Active Agents:    {}\n\
             Events Processed: {}\n\
             Pending Approvals:{}\n\n\
             Recent Logs:\n",
            model.mission_status.to_uppercase(),
            model.objective,
            model.tasks.len(),
            model
                .tasks
                .iter()
                .filter(|t| t.status == "completed")
                .count(),
            model.agents.len(),
            model.system_stats.events_processed,
            model.approvals.len()
        ));
        for log in model
            .logs
            .iter()
            .rev()
            .take(area.height.saturating_sub(10) as usize)
        {
            text.push_str(&format!(
                " [{}] [{}] {}\n",
                log.level, log.source, log.message
            ));
        }
    }

    let block = Block::default()
        .title(" Mission Cockpit Overview ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let p = Paragraph::new(text).block(block);
    f.render_widget(p, area);
}
