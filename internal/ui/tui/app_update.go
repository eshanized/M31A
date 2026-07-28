package tui

import (
	"fmt"
	"log/slog"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/workflow"
)

// Update implements tea.Model. It is the single dispatch point for all messages.
// CRITICAL: Never mutate AppState from a goroutine. All mutations go here.
//
// Each case delegates to a handler function in the corresponding handler_*.go
// file. This keeps Update() as a thin dispatcher while preserving all behavior.
func (m *AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	var cmds []tea.Cmd

	switch msg := msg.(type) {
	// ── Window resize ──────────────────────────────────────────────────────────
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		cmds = append(cmds, m.handleWindowResize(msg))

	// ── Keyboard ───────────────────────────────────────────────────────────────
	case tea.KeyMsg:
		switch msg.String() {
		case "ctrl+c":
			if m.streamCancelFn != nil {
				m.streamCancelFn()
				m.streamCancelFn = nil
				m.lastCtrlCTime = time.Time{}
				cmds = append(cmds, m.addToastCmd("Response cancelled", "warning", 3*time.Second))
				return m, tea.Batch(cmds...)
			}
			if !m.lastCtrlCTime.IsZero() && time.Since(m.lastCtrlCTime) < 2*time.Second {
				m.lastCtrlCTime = time.Time{}
				cw, ch := m.contentDimensions()
				m.confirmQuitModel = NewConfirmQuitModel(m.themeManager.Current(), cw, ch)
				m.switchScreen(ScreenConfirmQuit)
				return m, nil
			}
			m.lastCtrlCTime = time.Now()
			cmds = append(cmds, m.addToastCmd("Press ctrl+c again to exit (2s window)", "info", 2*time.Second))
			return m, tea.Batch(cmds...)
		default:
			cmds = append(cmds, m.routeKeyMsg(msg))
		}

	// ── Mouse ─────────────────────────────────────────────────────────────────
	case tea.MouseMsg:
		if c := m.forwardMouseToScreen(msg); c != nil {
			cmds = append(cmds, c)
		}

	// ── Screen routing ─────────────────────────────────────────────────────────
	case AppMsg:
		cmds = append(cmds, m.handleAppMsg(msg))

	// ── Pop screen (esc back navigation) ──────────────────────────────────────
	case PopScreenMsg:
		cmds = append(cmds, m.popScreen())

	// ── Key action ────────────────────────────────────────────────────────────
	case KeyActionMsg:
		cmds = append(cmds, m.handleKeyAction(msg.Action))

	// ── Leader timeout ────────────────────────────────────────────────────────
	case LeaderTimeoutMsg:
		_, cmd := handleLeaderTimeoutMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Slash command ─────────────────────────────────────────────────────────
	case SlashCommandMsg:
		_, cmd := handleSlashCommandMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Home screen submit ───────────────────────────────────────────────────
	case HomeSubmitMsg:
		_, cmd := handleHomeSubmitMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Intent classification result ──────────────────────────────────────────
	case IntentClassifiedMsg:
		_, cmd := handleIntentClassifiedMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Streaming ─────────────────────────────────────────────────────────────
	case StreamMsg:
		_, cmd := handleStreamMsg(m, msg)
		cmds = append(cmds, cmd)
	case StreamDoneMsg:
		_, cmd := handleStreamDoneMsg(m, msg)
		cmds = append(cmds, cmd)
	case StreamErrorMsg:
		_, cmd := handleStreamErrorMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Stream chunk from workflow phases ──────────────────────────────────
	case StreamChunkMsg:
		if m.replModel != nil && msg.Chunk != nil {
			m.replModel.AppendStreamChunk(msg.Chunk)
		}

	// ── Agent loop ────────────────────────────────────────────────────────
	case AgentStreamMsg, AgentThinkingMsg, AgentToolStartMsg, AgentToolProgressMsg,
		AgentToolDoneMsg, AgentIterationDoneMsg, AgentIterationMsg,
		AgentDoneMsg, AgentErrorMsg, AgentCompressedMsg:
		cmds = append(cmds, m.handleAgentMsg(msg))

	case TickMsg:
		cmds = append(cmds, m.handleTickMsg(msg)...)

	// ── Health ────────────────────────────────────────────────────────────────
	case HealthCheckTickMsg:
		_, cmd := handleHealthCheckTickMsg(m, msg)
		cmds = append(cmds, cmd)
	case HealthCheckResultMsg:
		_, cmd := handleHealthCheckResultMsg(m, msg)
		cmds = append(cmds, cmd)
	case EmitterDropLogTickMsg:
		cmds = append(cmds, m.handleEmitterDropLogTick(msg))

	// ── Cache refresh ─────────────────────────────────────────────────────────
	case RefreshCacheMsg:
		_, cmd := handleRefreshCacheMsg(m, msg)
		cmds = append(cmds, cmd)
	case CacheRefreshResultMsg:
		_, cmd := handleCacheRefreshResultMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Permission modal ──────────────────────────────────────────────────────
	case PermissionRequestMsg:
		_, cmd := handlePermissionRequestMsg(m, msg)
		cmds = append(cmds, cmd)
	case PermissionResponseMsg:
		_, cmd := handlePermissionResponseMsg(m, msg)
		cmds = append(cmds, cmd)
	case PermissionTickMsg:
		_, cmd := handlePermissionTickMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Question modal ────────────────────────────────────────────────────────
	case QuestionRequestMsg:
		_, cmd := handleQuestionRequestMsg(m, msg)
		cmds = append(cmds, cmd)
	case QuestionResponseMsg:
		_, cmd := handleQuestionResponseMsg(m, msg)
		cmds = append(cmds, cmd)
	case types.QuestionResponse:
		_, cmd := handleToolsQuestionResponse(m, msg)
		cmds = append(cmds, cmd)

	// ── Discuss Q&A ────────────────────────────────────────────────────────
	case DiscussAnswerMsg:
		_, cmd := handleDiscussAnswerMsg(m, msg)
		cmds = append(cmds, cmd)
	case DiscussCompleteMsg:
		_, cmd := handleDiscussCompleteMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Workflow phase result ─────────────────────────────────────────────────
	case PhaseResultMsg:
		_, cmd := handlePhaseResultMsg(m, msg)
		cmds = append(cmds, cmd)
	case PhaseTransitionMsg:
		_, cmd := m.handlePhaseTransitionDecision(msg)
		cmds = append(cmds, cmd)
	case PlanReadyMsg:
		_, cmd := handlePlanReadyMsg(m, msg)
		cmds = append(cmds, cmd)
	case PlanApproveMsg:
		_, cmd := handlePlanApproveMsg(m, msg)
		cmds = append(cmds, cmd)
	case PlanRefineMsg:
		_, cmd := handlePlanRefineMsg(m, msg)
		cmds = append(cmds, cmd)
	case workflow.DemonstrationReadyMsg:
		_, cmd := handleDemonstrationReadyMsg(m, msg)
		cmds = append(cmds, cmd)
	case ExecutePauseMsg:
		_, cmd := handleExecutePauseMsg(m, msg)
		cmds = append(cmds, cmd)
	case HealResultMsg:
		_, cmd := handleHealResultMsg(m, msg)
		cmds = append(cmds, cmd)

	case workflow.TaskStartMsg:
		_, cmd := handleTaskStartWorkflowMsg(m, msg)
		cmds = append(cmds, cmd)
	case workflow.TaskUpdateMsg:
		_, cmd := handleTaskUpdateWorkflowMsg(m, msg)
		cmds = append(cmds, cmd)
	case workflow.ToolStartMsg:
		_, cmd := handleToolStartWorkflowMsg(m, msg)
		cmds = append(cmds, cmd)
	case workflow.ToolCompleteMsg:
		_, cmd := handleToolCompleteWorkflowMsg(m, msg)
		cmds = append(cmds, cmd)
	case workflow.SelfHealStartMsg:
		_, cmd := handleSelfHealStartWorkflowMsg(m, msg)
		cmds = append(cmds, cmd)
	case workflow.SelfHealCompleteMsg:
		_, cmd := handleSelfHealCompleteWorkflowMsg(m, msg)
		cmds = append(cmds, cmd)
	case workflow.RuntimeCheckCompleteMsg:
		if m.runtimeModel != nil {
			m.runtimeModel.SetSummary(msg.Summary)
		}
		cmds = append(cmds, m.drainAdaptiveCmd())
	case workflow.PhaseTransitionStartMsg:
		cmds = append(cmds, m.handlePhaseTransitionStart(msg)...)
	case workflow.PhaseTransitionCompleteMsg:
		cmds = append(cmds, m.handlePhaseTransitionComplete(msg)...)
	case workflow.IntermediateProgressMsg,
		workflow.ThinkingStartMsg, workflow.ThinkingCompleteMsg:
		cmds = append(cmds, m.drainAdaptiveCmd())
	case workflow.DecisionsSnapshotMsg:
		cmds = append(cmds, m.handleDecisionsSnapshot(msg))
	// ── W7: Newly wired workflow events ──────────────────────────────────────
	case workflow.InitAnalysisMsg:
		cmds = append(cmds, m.handleInitAnalysis(msg)...)
	case workflow.InitPreflightMsg:
		cmds = append(cmds, m.handleInitPreflight(msg)...)
	case workflow.ResearchProgressMsg:
		cmds = append(cmds, m.handleResearchProgress(msg)...)
	case workflow.PlanCheckMsg:
		cmds = append(cmds, m.handlePlanCheck(msg)...)
	case workflow.PlanRevisionMsg:
		cmds = append(cmds, m.handlePlanRevision(msg)...)
	case workflow.PlanChunkProgressMsg:
		cmds = append(cmds, m.handlePlanChunkProgress(msg)...)
	case workflow.DiscussQualityMsg:
		cmds = append(cmds, m.handleDiscussQuality(msg)...)
	case workflow.DiscussCompletenessMsg:
		cmds = append(cmds, m.handleDiscussCompleteness(msg)...)
	case workflow.ExecutePreflightMsg:
		cmds = append(cmds, m.handleExecutePreflight(msg)...)
	case workflow.ExecuteQualityGateMsg:
		cmds = append(cmds, m.handleExecuteQualityGate(msg)...)
	case workflow.ExecuteLoopDetectMsg:
		cmds = append(cmds, m.handleExecuteLoopDetect(msg)...)
	case workflow.VerifyReportMsg:
		cmds = append(cmds, m.handleVerifyReport(msg)...)
	case workflow.ShipPreflightMsg:
		cmds = append(cmds, m.handleShipPreflight(msg)...)
	case workflow.ShipChangelogMsg:
		cmds = append(cmds, m.handleShipChangelog(msg)...)
	case workflow.CompactionCompleteMsg:
		cmds = append(cmds, m.handleCompactionComplete(msg)...)
	case workflow.TaskDiffSummaryMsg:
		cmds = append(cmds, m.handleTaskDiffSummary(msg)...)
	case workflow.AgentSwitchMsg:
		cmds = append(cmds, m.handleAgentSwitch(msg)...)

	// ── Narrative engine ─────────────────────────────────────────────────
	case NarrativeBubbleMsg:
		cmds = append(cmds, m.handleNarrativeBubble(msg)...)

	// ── Batched emitter drain ────────────────────────────────────────────────
	case DrainBatchMsg:
		// Process each message in the batch through the same handlers.
		// This preserves ordering and re-uses existing logic.
		for _, sub := range msg.Messages {
			switch subMsg := sub.(type) {
			case workflow.TaskStartMsg:
				cmds = append(cmds, m.handleWorkflowTaskStart(subMsg)...)
			case workflow.TaskUpdateMsg:
				cmds = append(cmds, m.handleWorkflowTaskUpdate(subMsg)...)
			case workflow.ToolStartMsg:
				cmds = append(cmds, m.handleWorkflowToolStart(subMsg)...)
			case workflow.ToolCompleteMsg:
				cmds = append(cmds, m.handleWorkflowToolComplete(subMsg)...)
			case workflow.SelfHealStartMsg:
				cmds = append(cmds, m.handleWorkflowSelfHealStart(subMsg)...)
			case workflow.SelfHealCompleteMsg:
				cmds = append(cmds, m.handleWorkflowSelfHealComplete(subMsg)...)
			case workflow.PhaseTransitionStartMsg:
				cmds = append(cmds, m.handlePhaseTransitionStart(subMsg)...)
			case workflow.PhaseTransitionCompleteMsg:
				cmds = append(cmds, m.handlePhaseTransitionComplete(subMsg)...)
			case workflow.RuntimeCheckCompleteMsg:
				if m.runtimeModel != nil {
					m.runtimeModel.SetSummary(subMsg.Summary)
				}
			case workflow.IntermediateProgressMsg,
				workflow.ThinkingStartMsg, workflow.ThinkingCompleteMsg:
				// No specific handler needed; drain continues below.
			// ── W7: Newly wired workflow events in batch drain ──────────────────
			case workflow.InitAnalysisMsg:
				cmds = append(cmds, m.handleInitAnalysis(subMsg)...)
			case workflow.InitPreflightMsg:
				cmds = append(cmds, m.handleInitPreflight(subMsg)...)
			case workflow.ResearchProgressMsg:
				cmds = append(cmds, m.handleResearchProgress(subMsg)...)
			case workflow.PlanCheckMsg:
				cmds = append(cmds, m.handlePlanCheck(subMsg)...)
			case workflow.PlanRevisionMsg:
				cmds = append(cmds, m.handlePlanRevision(subMsg)...)
			case workflow.PlanChunkProgressMsg:
				cmds = append(cmds, m.handlePlanChunkProgress(subMsg)...)
			case workflow.DiscussQualityMsg:
				cmds = append(cmds, m.handleDiscussQuality(subMsg)...)
			case workflow.DiscussCompletenessMsg:
				cmds = append(cmds, m.handleDiscussCompleteness(subMsg)...)
			case workflow.ExecutePreflightMsg:
				cmds = append(cmds, m.handleExecutePreflight(subMsg)...)
			case workflow.ExecuteQualityGateMsg:
				cmds = append(cmds, m.handleExecuteQualityGate(subMsg)...)
			case workflow.ExecuteLoopDetectMsg:
				cmds = append(cmds, m.handleExecuteLoopDetect(subMsg)...)
			case workflow.VerifyReportMsg:
				cmds = append(cmds, m.handleVerifyReport(subMsg)...)
			case workflow.ShipPreflightMsg:
				cmds = append(cmds, m.handleShipPreflight(subMsg)...)
			case workflow.ShipChangelogMsg:
				cmds = append(cmds, m.handleShipChangelog(subMsg)...)
			case workflow.CompactionCompleteMsg:
				cmds = append(cmds, m.handleCompactionComplete(subMsg)...)
			case workflow.TaskDiffSummaryMsg:
				cmds = append(cmds, m.handleTaskDiffSummary(subMsg)...)
			case workflow.AgentSwitchMsg:
				cmds = append(cmds, m.handleAgentSwitch(subMsg)...)
			case NarrativeBubbleMsg:
				cmds = append(cmds, m.handleNarrativeBubble(subMsg)...)
			default:
				slog.Debug("unhandled message in DrainBatchMsg", "type", fmt.Sprintf("%T", sub))
			}
		}
		// Continue draining if there are more messages queued.
		if m.emitterLoad() > 0 {
			cmds = append(cmds, m.drainMultipleCmd())
		} else {
			cmds = append(cmds, m.drainEmitterCmd())
		}

	// ── Goal submitted ────────────────────────────────────────────────────────
	case GoalSubmittedMsg:
		_, cmd := handleGoalSubmittedMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Phase model picked (from dual-model picker) ────────────────────────────
	case PhaseModelPickedMsg:
		cmds = append(cmds, m.handlePhaseModelPicked(msg)...)

	// ── Toast ─────────────────────────────────────────────────────────────────
	case ToastMsg:
		cmds = append(cmds, m.handleToast(msg)...)
	case ToastExpiryMsg:
		_, cmd := handleToastExpiryMsg(m, msg)
		cmds = append(cmds, cmd)
	case DismissToastMsg:
		_, cmd := handleDismissToastMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── First-run wizard complete ─────────────────────────────────────────────
	case FirstRunCompleteMsg:
		_, cmd := handleFirstRunCompleteMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Settings saved ────────────────────────────────────────────────────────
	case SettingsSavedMsg:
		_, cmd := handleSettingsSavedMsg(m, msg)
		cmds = append(cmds, cmd)
	case ResetCompleteMsg:
		_, cmd := handleResetCompleteMsg(m, msg)
		cmds = append(cmds, cmd)
	case ConfigSavedMsg:
		_, cmd := handleConfigSavedMsg(m, msg)
		cmds = append(cmds, cmd)
	case config.ConfigReloadMsg:
		_, cmd := handleConfigReloadMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Fallback event ────────────────────────────────────────────────────────
	case FallbackEventMsg:
		cmds = append(cmds, m.handleFallbackEvent(msg)...)

	// ── Bisect start with commit range ─────────────────────────────────────
	case BisectStartMsg:
		cmds = append(cmds, m.handleBisectStart(msg)...)

	// ── Session detail request ─────────────────────────────────────────────
	case SessionDetailRequestMsg:
		_, cmd := handleSessionDetailRequestMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Chat history: continue from message ────────────────────────────────
	case ChatHistoryContinueMsg:
		cmds = append(cmds, m.handleChatHistoryContinue(msg)...)

	// ── Ghost write request ───────────────────────────────────────────────
	case GhostWriteRequestMsg:
		_, cmd := handleGhostWriteRequestMsg(m, msg)
		cmds = append(cmds, cmd)
	case GhostWriteResultMsg:
		_, cmd := handleGhostWriteResultMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Arbitrage optimization results ───────────────────────────────────────
	case OptimizedMsg:
		_, cmd := handleOptimizedMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Model selected ────────────────────────────────────────────────────────
	case ModelSelectedMsg:
		cmds = append(cmds, m.handleModelSelected(msg)...)

	// ── Sidebar tick (periodic or file-watcher triggered) ──────────────────────
	case SidebarRefreshTickMsg:
		_, cmd := handleSidebarRefreshTickMsg(m, msg)
		cmds = append(cmds, cmd)
	case SidebarRefreshMsg:
		_, cmd := handleSidebarRefreshMsg(m, msg)
		cmds = append(cmds, cmd)
	case SidebarRevertMsg:
		_, cmd := handleSidebarRevertMsg(m, msg)
		cmds = append(cmds, cmd)
	case SidebarTodoUpdateMsg:
		_, cmd := handleSidebarTodoUpdateMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Subagent events ───────────────────────────────────────────────────────
	case SubagentEventMsg:
		cmds = append(cmds, m.handleSubagentEvent(msg)...)

	// ── Diff screen ───────────────────────────────────────────────────────────
	case DiffScreenMsg:
		_, cmd := handleDiffScreenMsg(m, msg)
		cmds = append(cmds, cmd)
	case DiffCloseMsg:
		_, cmd := handleDiffCloseMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Session restore ──────────────────────────────────────────────────────
	case sessionRestoredMsg:
		cmds = append(cmds, m.applySessionRestored(msg))
	case resumeScreenReadyMsg:
		m.sessionList = nil
		if m.resumeModel == nil {
			rm := NewResumeModel(msg.sessions, m.themeManager.Current())
			rm.SetTotalCount(msg.total)
			m.resumeModel = rm
		} else {
			m.resumeModel.Refresh(msg.sessions)
			m.resumeModel.SetTotalCount(msg.total)
		}
		m.screen = ScreenResume

	// ── Metrics loaded (async) ──────────────────────────────────────────────
	case metricsLoadedMsg:
		if m.metricsModel != nil {
			m.metricsModel.ApplyStats(msg.stats)
		}

	// ── Session rename ───────────────────────────────────────────────────────
	case SessionRenameMsg:
		_, cmd := handleSessionRenameMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Session export ───────────────────────────────────────────────────────
	case SessionExportMsg:
		_, cmd := handleSessionExportMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Error ─────────────────────────────────────────────────────────────────
	case ErrorMsg:
		_, cmd := handleErrorMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── ProviderModelsFetched ─────────────────────────────────────────────────
	case ProviderModelsFetchedMsg:
		cmds = append(cmds, m.handleProviderModelsFetched(msg)...)

	// ── ThinkingBlockToggle ───────────────────────────────────────────────────
	case ThinkingBlockToggleMsg:
		_, cmd := handleThinkingBlockToggleMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Tool card click (mouse) ───────────────────────────────────────────────
	case ToolClickMsg:
		_, cmd := handleToolClickMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── M8: Collapse all tool cards (Escape key) ─────────────────────────────
	case ToolCollapseAllMsg:
		_, cmd := handleToolCollapseAllMsg(m, msg)
		cmds = append(cmds, cmd)

	// ── Model/command palette sub-model forwarding ────────────────────────────
	default:
		cmds = append(cmds, m.forwardMsgToScreen(msg))
	}

	return m, tea.Batch(cmds...)
}

// handleAppMsg handles AppMsg screen transitions.
func (m *AppState) handleAppMsg(msg AppMsg) tea.Cmd {
	if msg.ModelSelected != nil {
		m.activeModel = &msg.ModelSelected.Model
		m.activeProvider = msg.ModelSelected.Provider
		if m.registry != nil {
			_ = m.registry.SetActive(msg.ModelSelected.Provider)
		}

		// Persist the selected model and provider to config so they survive restarts.
		if m.config != nil {
			m.config.Model.Default = msg.ModelSelected.Model.ID
			m.config.Provider.Default = msg.ModelSelected.Provider
			if m.configPath != "" {
				if err := m.config.SaveWithKeychain(m.configPath, m.keychain); err != nil {
					slog.Warn("failed to save model selection to config", "error", err)
				}
			}
		}

		var providerCmd tea.Cmd
		if m.replModel != nil {
			m.replModel.activeModel = &msg.ModelSelected.Model
			m.replModel.activeProvider = msg.ModelSelected.Provider
			providerCmd = m.replModel.SetProvider(m.shutdownCtx, m.registry, msg.ModelSelected.Provider, &msg.ModelSelected.Model, m.sessionID, m.config)
		}
		// Return to the previous screen instead of hardcoding REPL
		if len(m.screenStack) > 0 {
			prev := m.screenStack[len(m.screenStack)-1]
			m.screenStack = m.screenStack[:len(m.screenStack)-1]
			m.screen = prev
		} else {
			m.screen = ScreenREPL
		}
		return providerCmd
	}

	// Check SessionID first — resume screen sends Screen=REPL + SessionID,
	// and the session restore must take priority over screen routing.
	if msg.SessionID != "" {
		return m.loadAndRestoreSession(msg.SessionID, true)
	}

	if msg.Screen != 0 || msg.Action != "" {
		return m.routeAppMsgAction(msg)
	}

	return nil
}

// routeAppMsgAction handles action-based screen routing.
func (m *AppState) routeAppMsgAction(msg AppMsg) tea.Cmd {
	switch msg.Action {
	case "new_session":
		return m.startNewSession()
	case "session_list":
		return m.openResumeScreen()
	case "open_settings":
		return m.openSettingsScreen()
	case "open_palette":
		return m.navigateToScreen(ScreenCommandPalette)
	case "toggle_sidebar":
		if m.sidebarModel != nil {
			m.sidebarModel.Toggle()
			if m.replModel != nil {
				// REPL width is already content-area; pass 0 to avoid double-subtracting.
				m.replModel.SetSidebarWidth(0)
			}
		}
		return nil
	case "cancel_stream":
		if m.streamCancelFn != nil {
			m.streamCancelFn()
			m.streamCancelFn = nil
		}
		return nil
	}

	// Screen-based routing
	return m.navigateToScreen(msg.Screen)
}
