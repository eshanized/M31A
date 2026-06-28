package tui

import (
	"encoding/json"
	stderrors "errors"
	"fmt"
	"log/slog"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/layout"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/pkg/session"
)

// Update implements tea.Model. It is the single dispatch point for all messages.
// CRITICAL: Never mutate AppState from a goroutine. All mutations go here.
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
				return m, tea.Quit
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
		if m.keyRegistry != nil && m.keyRegistry.IsLeaderActive() {
			m.keyRegistry.DeactivateLeader()
			cmds = append(cmds, m.addToastCmd("Leader key timed out", "info", 2*time.Second))
		}

	// ── Slash command ─────────────────────────────────────────────────────────
	case SlashCommandMsg:
		cmds = append(cmds, m.handleSlashCommand(msg.Command, msg.AttachedFiles))

	// ── Home screen submit ───────────────────────────────────────────────────
	case HomeSubmitMsg:
		m.ensureReplModel()
		if m.replModel != nil {
			m.replModel.textarea.SetValue(msg.Text)
			m.replModel.textarea.Focus()
			m.replModel.updateAutoExpandHeight()
		}
		cmds = append(cmds, m.navigateToScreen(ScreenREPL))

	// ── Intent classification result ──────────────────────────────────────────
	case IntentClassifiedMsg:
		cmds = append(cmds, m.handleIntentClassified(msg))

	// ── Streaming ─────────────────────────────────────────────────────────────
	case StreamMsg:
		if m.replModel != nil {
			// Start token burn tracking on the first chunk of a new response.
			if !m.replModel.streaming && m.sidebarModel != nil {
				m.sidebarModel.StartTokenBurn()
			}
			cs := m.replModel.handleStreamMsg(msg)
			cmds = append(cmds, cs...)
		}
	case StreamDoneMsg:
		if m.replModel != nil {
			collapsed := m.replModel.handleStreamDoneMsg(msg)
			m.checkAutoDream()
			// Update sidebar with token usage
			m.updateSidebarUsage()
			if collapsed > 0 {
				cmds = append(cmds, m.addToastCmd(
					fmt.Sprintf("↓ %d tool output(s) collapsed — press Enter to expand", collapsed),
					"info", 3*time.Second))
			}
		}
		m.streamCancelFn = nil
	case StreamErrorMsg:
		if m.replModel != nil {
			m.replModel.handleStreamErrorMsg(msg)
		}
		m.streamCancelFn = nil
		// Auto-fallback on rate limit or provider unreachable
		if m.config != nil && m.config.Provider.AutoFallback && m.registry != nil {
			if stderrors.Is(msg.Err, m31errors.ErrRateLimited) || stderrors.Is(msg.Err, m31errors.ErrProviderUnreachable) {
				cmds = append(cmds, m.attemptAutoFallback(msg.Err))
			}
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
		if m.activeProvider != "" && m.registry != nil {
			if p, err := m.registry.Get(m.activeProvider); err == nil {
				cmds = append(cmds, HealthCheckCmd(m.shutdownCtx, p, 10*time.Second))
			}
		}
		cmds = append(cmds, NextHealthTick(m.shutdownCtx, types.HealthCheckInterval))

	case HealthCheckResultMsg:
		cmds = append(cmds, m.handleHealthCheckResult(msg)...)

	// ── Cache refresh ─────────────────────────────────────────────────────────
	case RefreshCacheMsg:
		if m.registry != nil {
			provider := msg.ProviderName
			if provider == "" {
				provider = m.activeProvider
			}
			cmds = append(cmds, CacheRefreshCmd(m.shutdownCtx, m.registry, provider))
		}
	case CacheRefreshResultMsg:
		if msg.NextCmd != nil {
			cmds = append(cmds, msg.NextCmd)
		}

	// ── Permission modal ──────────────────────────────────────────────────────
	case PermissionRequestMsg:
		m.permRequest = &msg.Request
		m.permCountdown = msg.Request.TimeoutSecs
		timeout := components.DefaultPermissionTimeout
		if msg.Request.TimeoutSecs > 0 {
			timeout = time.Duration(msg.Request.TimeoutSecs) * time.Second
		}
		m.permModal = components.NewPermissionModal(msg.Request, m.themeManager.Current(), timeout)
		m.screen = ScreenPermission

	case PermissionResponseMsg:
		cmds = append(cmds, m.handlePermissionResponse(msg))

	case PermissionTickMsg:
		cmds = append(cmds, m.handlePermissionTick())

	// ── Question modal ────────────────────────────────────────────────────────
	case QuestionRequestMsg:
		m.questionRequest = &msg
		qModel := components.NewQuestionModel(tools.QuestionRequest{
			Question:    msg.Question,
			Header:      msg.Header,
			Options:     msg.Options,
			AllowCustom: true,
		}, m.themeManager.Current(), m.permModalWidth)
		m.questionModel = &qModel
		m.screen = ScreenPermission // reuse permission overlay

	case QuestionResponseMsg:
		cmds = append(cmds, m.handleQuestionResponse(msg))

	case tools.QuestionResponse:
		cmds = append(cmds, m.handleQuestionResponse(QuestionResponseMsg{Answer: msg.Answer}))

	// ── Discuss Q&A ────────────────────────────────────────────────────────
	case DiscussAnswerMsg:
		cmds = append(cmds, m.handleDiscussAnswer(msg))

	case DiscussCompleteMsg:
		cmds = append(cmds, m.handleDiscussComplete())

	// ── Workflow phase result ─────────────────────────────────────────────────
	case PhaseResultMsg:
		cmds = append(cmds, m.handlePhaseResult(msg))

	case PlanReadyMsg:
		cmds = append(cmds, m.handlePlanReady(msg))

	case PlanApproveMsg:
		cmds = append(cmds, m.handlePlanApprove())

	case PlanRefineMsg:
		cmds = append(cmds, m.handlePlanRefine(msg))

	case workflow.DemonstrationReadyMsg:
		if m.shipModel != nil {
			m.shipModel.SetDemonstration(msg.Content)
		}

	case ExecutePauseMsg:
		if m.executeModel != nil {
			m.executeModel.paused = msg.Paused
		}

	case HealResultMsg:
		if m.verifyModel != nil {
			newVerify, cmd := m.verifyModel.Update(msg)
			m.verifyModel = newVerify
			cmds = append(cmds, cmd)
		}

	case workflow.TaskStartMsg:
		cmds = append(cmds, m.handleWorkflowTaskStart(msg)...)
	case workflow.TaskUpdateMsg:
		cmds = append(cmds, m.handleWorkflowTaskUpdate(msg)...)
	case workflow.ToolStartMsg:
		cmds = append(cmds, m.handleWorkflowToolStart(msg)...)
	case workflow.ToolCompleteMsg:
		cmds = append(cmds, m.handleWorkflowToolComplete(msg)...)
	case workflow.SelfHealStartMsg:
		cmds = append(cmds, m.handleWorkflowSelfHealStart(msg)...)
	case workflow.SelfHealCompleteMsg:
		cmds = append(cmds, m.handleWorkflowSelfHealComplete(msg)...)
	case workflow.RuntimeCheckCompleteMsg:
		if m.runtimeModel != nil {
			m.runtimeModel.SetSummary(msg.Summary)
		}
		cmds = append(cmds, m.drainEmitterCmd())
	case workflow.PhaseTransitionStartMsg:
		cmds = append(cmds, m.handlePhaseTransitionStart(msg)...)
	case workflow.PhaseTransitionCompleteMsg:
		cmds = append(cmds, m.handlePhaseTransitionComplete(msg)...)
	case workflow.IntermediateProgressMsg,
		workflow.ThinkingStartMsg, workflow.ThinkingCompleteMsg:
		cmds = append(cmds, m.drainEmitterCmd())

	// ── Goal submitted ────────────────────────────────────────────────────────
	case GoalSubmittedMsg:
		m.workflowGoal = msg.Goal
		cw, ch := m.contentDimensions()
		picker := NewPhaseModelPickerModel(m.shutdownCtx, m.registry, m.themeManager.Current(), cw, ch)
		m.phaseModelPicker = picker
		m.screen = ScreenPhaseModelPicker
		cmds = append(cmds, picker.Init())

	// ── Phase model picked (from dual-model picker) ────────────────────────────
	case PhaseModelPickedMsg:
		cmds = append(cmds, m.handlePhaseModelPicked(msg)...)

	// ── Toast ─────────────────────────────────────────────────────────────────
	case ToastMsg:
		cmds = append(cmds, m.handleToast(msg)...)

	case ToastExpiryMsg:
		m.removeToastByID(msg.ToastID)

	case DismissToastMsg:
		m.removeToastByID(msg.ToastID)

	// ── First-run wizard complete ─────────────────────────────────────────────
	case FirstRunCompleteMsg:
		cmds = append(cmds, m.handleFirstRunComplete(msg))

	// ── Settings saved ────────────────────────────────────────────────────────
	case SettingsSavedMsg:
		cmds = append(cmds, m.handleSettingsSaved()...)
	case ResetCompleteMsg:
		cmds = append(cmds, m.handleResetComplete()...)
	case ConfigSavedMsg:
		cmds = append(cmds, m.handleConfigSaved()...)
	case config.ConfigReloadMsg:
		cmds = append(cmds, m.handleConfigReload(msg)...)

	// ── Fallback event ────────────────────────────────────────────────────────
	case FallbackEventMsg:
		cmds = append(cmds, m.handleFallbackEvent(msg)...)

	// ── Bisect start with commit range ─────────────────────────────────────
	case BisectStartMsg:
		cmds = append(cmds, m.handleBisectStart(msg)...)

	// ── Session detail request ─────────────────────────────────────────────
	case SessionDetailRequestMsg:
		if m.sessionDetailModel != nil && msg.Session != nil {
			m.sessionDetailModel.SetSession(msg.Session)
		}

	// ── Chat history: continue from message ────────────────────────────────
	case ChatHistoryContinueMsg:
		cmds = append(cmds, m.handleChatHistoryContinue(msg)...)

	// ── Ghost write request ───────────────────────────────────────────────
	case GhostWriteRequestMsg:
		if len(msg.Files) > 0 {
			cmds = append(cmds, m.addToastCmd(
				fmt.Sprintf("Ghost write started for %d file(s)...", len(msg.Files)),
				"info", 3*time.Second))
			// Navigate to ghost output screen
			cmds = append(cmds, m.navigateToScreen(ScreenGhostOutput))
		}

	// ── Ghost write result ────────────────────────────────────────────────
	case GhostWriteResultMsg:
		if m.ghostOutputModel != nil && msg.Result != nil {
			m.ghostOutputModel.SetResult(msg.Result)
		}

	// ── Arbitrage optimization results ───────────────────────────────────────
	case OptimizedMsg:
		if len(msg.Recommendations) > 0 {
			rec := msg.Recommendations[0]
			cmds = append(cmds, m.addToastCmd(
				fmt.Sprintf("Optimization: recommended %s (saving $%.4f)",
					rec.RecommendedModel.ModelID, rec.Savings),
				"info", 5*time.Second))
		}

	// ── Model selected ────────────────────────────────────────────────────────
	case ModelSelectedMsg:
		cmds = append(cmds, m.handleModelSelected(msg)...)

	// ── Sidebar tick (periodic or file-watcher triggered) ──────────────────────
	case SidebarRefreshTickMsg:
		cmds = append(cmds, m.handleSidebarRefreshTick(msg)...)
	case SidebarRefreshMsg:
		cmds = append(cmds, m.handleSidebarRefresh(msg)...)

	case SidebarRevertMsg:
		if m.sidebarModel != nil {
			m.sidebarModel.RevertToFiles()
		}

	case SidebarTodoUpdateMsg:
		cmds = append(cmds, m.handleSidebarTodoUpdate(msg)...)

	// ── Subagent events ───────────────────────────────────────────────────────
	case SubagentEventMsg:
		cmds = append(cmds, m.handleSubagentEvent(msg)...)

	// ── Diff screen ───────────────────────────────────────────────────────────
	case DiffScreenMsg:
		if m.diffModel == nil {
			m.diffModel = NewDiffModel(m.themeManager.Current())
		}
		cw, ch := m.contentDimensions()
		m.diffModel.SetDimensions(cw, ch)
		m.diffModel.SetDiff(msg.Diff)
		m.diffModel.SetTitle(msg.Title)
		if m.sidebarModel != nil {
			m.sidebarModel.Blur()
		}
		m.screen = ScreenDiff

	case DiffCloseMsg:
		cmds = append(cmds, m.popScreen())
		// Refresh sidebar git status after closing diff
		if m.sidebarModel != nil {
			cmds = append(cmds, m.sidebarModel.refreshCmd())
		}

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
		if m.sessionManager != nil && msg.SessionID != "" {
			label := time.Now().Format("2006-01-02_150405")
			if err := m.sessionManager.RenameSession(msg.SessionID, label); err != nil {
				cmds = append(cmds, m.addToastCmd("Rename failed: "+m31errors.UserMessage(err), "error", 5*time.Second))
			} else {
				cmds = append(cmds, m.addToastCmd("Session renamed", "success", 3*time.Second))
				// Refresh the resume screen session list
				cmds = append(cmds, m.openResumeScreen())
			}
		}

	// ── Session export ───────────────────────────────────────────────────────
	case SessionExportMsg:
		if m.sessionManager != nil && msg.SessionID != "" {
			exportPath := fmt.Sprintf("session_%s.md", msg.SessionID)
			if err := m.sessionManager.ExportSessionMarkdown(msg.SessionID, exportPath); err != nil {
				cmds = append(cmds, m.addToastCmd("Export failed: "+m31errors.UserMessage(err), "error", 5*time.Second))
			} else {
				cmds = append(cmds, m.addToastCmd(fmt.Sprintf("Exported to %s", exportPath), "success", 5*time.Second))
			}
		}

	// ── Error ─────────────────────────────────────────────────────────────────
	case ErrorMsg:
		if m.replModel != nil {
			m.replModel.AddMessage(makeErrorBannerMsg(plainErrorBanner(msg.Err, m.activeProvider), m.activeProvider))
		}

	// ── ProviderModelsFetched ─────────────────────────────────────────────────
	case ProviderModelsFetchedMsg:
		cmds = append(cmds, m.handleProviderModelsFetched(msg)...)

	// ── ThinkingBlockToggle ───────────────────────────────────────────────────
	case ThinkingBlockToggleMsg:
		if m.replModel != nil {
			m.replModel.handleThinkingToggle(msg)
		}

	// ── Tool card click (mouse) ───────────────────────────────────────────────
	case ToolClickMsg:
		if m.replModel != nil && msg.MessageIndex >= 0 && msg.MessageIndex < len(m.replModel.messages) {
			m.ensureToolDetailModel()
			title, body := m.extractToolDetail(msg.MessageIndex, msg.ToolName)
			if title != "" {
				m.toolDetailModel.SetContent(title, body)
				cmds = append(cmds, m.navigateToScreen(ScreenToolDetail))
			}
		}

	// ── Model/command palette sub-model forwarding ────────────────────────────
	default:
		cmds = append(cmds, m.forwardMsgToScreen(msg))
	}

	return m, tea.Batch(cmds...)
}

// ─── Routing helpers ──────────────────────────────────────────────────────────

// routeToScreen returns the initialization cmd for the current screen.
// This switch is intentionally kept separate from ensureSubModel():
// routeToScreen() is called on FIRST navigation to a screen (from navigateToScreen)
// and creates the sub-model + calls Init(). ensureSubModel() is called on
// TRANSITIONS and popScreen() and handles resize + conditional re-init.
// They share structure but differ in lifecycle semantics.
func (m *AppState) routeToScreen() tea.Cmd {
	switch m.screen {
	case ScreenFirstRun:
		if m.firstRunModel == nil {
			fm := NewFirstRunModel(m.themeManager.Current(), m.registry, m.config, m.version, m.shutdownCtx)
			cw, ch := m.contentDimensions()
			fm.SetDimensions(cw, ch)
			m.firstRunModel = fm
		}
		return m.firstRunModel.Init()
	case ScreenREPL:
		m.ensureReplModel()
		if m.replModel.width == 0 {
			cw, ch := m.contentDimensions()
			m.replModel.width = cw
			m.replModel.height = ch
		}
		return m.replModel.Init()
	case ScreenModelSelector:
		if m.msModel == nil {
			m.msModel = NewModelSelector(m.shutdownCtx, m.registry, m.sessionManager, m.themeManager.Current())
		}
		cw, ch := m.contentDimensions()
		m.msModel.SetDimensions(cw, ch)
		return m.msModel.Init()
	case ScreenSettings:
		if m.settingsModel == nil {
			m.settingsModel = NewSettingsModel(m.config, m.registry, m.themeManager.Current(), m.configPath, m.version, m.keychain, m.shutdownCtx)
		}
		cw, ch := m.contentDimensions()
		m.settingsModel.width = cw
		m.settingsModel.height = ch
		return m.settingsModel.Init()
	case ScreenResume:
		return m.openResumeScreen()
	case ScreenPlan:
		if m.planModel != nil {
			cw, ch := m.contentDimensions()
			m.planModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenExecute:
		if m.executeModel != nil {
			cw, ch := m.contentDimensions()
			m.executeModel.width = cw
			m.executeModel.height = ch
		}
		return nil
	case ScreenVerify:
		if m.verifyModel != nil {
			cw, ch := m.contentDimensions()
			m.verifyModel.width = cw
			m.verifyModel.height = ch
		}
		return nil
	case ScreenRuntimeCheck:
		if m.runtimeModel != nil {
			cw, ch := m.contentDimensions()
			m.runtimeModel.width = cw
			m.runtimeModel.height = ch
			return func() tea.Msg { return RuntimeTickMsg{} }
		}
		return nil
	case ScreenShip:
		if m.shipModel != nil {
			cw, ch := m.contentDimensions()
			m.shipModel.width = cw
			m.shipModel.height = ch
		}
		return nil
	case ScreenGoalInput:
		if m.goalInput == nil {
			m.goalInput = NewGoalInputModel(m.themeManager.Current(), nil)
		}
		cw, ch := m.contentDimensions()
		m.goalInput.SetDimensions(cw, ch)
		return m.goalInput.Init()
	case ScreenLedger:
		if m.ledgerModel == nil {
			m.ledgerModel = NewLedgerModel(m.themeManager.Current(), m.ledger)
			cw, ch := m.contentDimensions()
			m.ledgerModel.SetDimensions(cw, ch)
			m.ledgerModel.LoadEntries()
		}
		return nil
	case ScreenRollback:
		if m.rollbackModel == nil {
			cw, ch := m.contentDimensions()
			m.rollbackModel = NewRollbackModel(m.themeManager.Current(), m.git, m.rollback, cw, ch)
			m.rollbackModel.LoadCommits()
		}
		return nil
	case ScreenMetrics:
		if m.metricsModel == nil {
			m.metricsModel = NewMetricsModel(m.themeManager.Current())
		}
		cw, ch := m.contentDimensions()
		m.metricsModel.SetDimensions(cw, ch)
		if m.sessionManager != nil {
			return m.metricsModel.LoadStatsCmd(m.sessionManager)
		}
		return nil
	case ScreenDiscuss:
		if m.discussModel == nil {
			cw, ch := m.contentDimensions()
			m.discussModel = NewDiscussModel(m.themeManager.Current(), m.discussQuestions, cw, ch)
			if m.config != nil && m.config.UI.DiscussTimeout > 0 {
				m.discussModel.SetTimeout(m.config.UI.DiscussTimeout)
			}
		} else {
			cw, ch := m.contentDimensions()
			m.discussModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenConfig:
		if m.configModel == nil {
			cw, ch := m.contentDimensions()
			m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, cw, ch, m.keychain)
		}
		return nil
	case ScreenHelp:
		if m.helpModel == nil {
			m.helpModel = NewHelpModel(m.themeManager.Current())
		}
		cw, ch := m.contentDimensions()
		m.helpModel.SetDimensions(cw, ch)
		return m.helpModel.Init()
	case ScreenBisect:
		if m.bisectModel == nil {
			cw, ch := m.contentDimensions()
			m.bisectModel = NewBisectModel(m.themeManager.Current(), cw, ch)
		}
		return nil
	case ScreenNotifications:
		if m.notifModel == nil {
			cw, ch := m.contentDimensions()
			m.notifModel = NewNotificationModel(m.themeManager.Current(), cw, ch)
		}
		return nil
	case ScreenDashboard:
		if m.dashboardModel == nil {
			cw, ch := m.contentDimensions()
			m.dashboardModel = NewDashboardModel(m.themeManager.Current(), cw, ch)
		}
		if m.workflowEngine != nil {
			m.dashboardModel.SetWorkflowState(m.workflowPhase, m.workflowGoal, "", m.activeProvider)
		}
		return nil
	case ScreenSessionDetail:
		if m.sessionDetailModel == nil {
			cw, ch := m.contentDimensions()
			m.sessionDetailModel = NewSessionDetailModel(m.themeManager.Current(), cw, ch)
		}
		return nil
	case ScreenFileExplorer:
		if m.fileExplorerModel == nil {
			cw, ch := m.contentDimensions()
			m.fileExplorerModel = NewFileExplorerModel(m.themeManager.Current(), cw, ch)
			if m.cwd != "" {
				root := buildFileTree(m.cwd, 0, 3)
				if root != nil {
					m.fileExplorerModel.SetRoot(root)
				}
			}
		}
		return nil
	case ScreenToolDetail:
		if m.toolDetailModel == nil {
			cw, ch := m.contentDimensions()
			m.toolDetailModel = NewToolDetailModel(m.themeManager.Current(), cw, ch)
		}
		return nil
	case ScreenPhaseModelPicker:
		if m.phaseModelPicker == nil {
			cw, ch := m.contentDimensions()
			m.phaseModelPicker = NewPhaseModelPickerModel(m.shutdownCtx, m.registry, m.themeManager.Current(), cw, ch)
		}
		return nil
	case ScreenGhostPicker:
		if m.ghostPickerModel == nil {
			cw, ch := m.contentDimensions()
			m.ghostPickerModel = NewGhostPickerModel(m.themeManager.Current(), cw, ch)
		}
		return nil
	case ScreenGhostOutput:
		if m.ghostOutputModel == nil {
			cw, ch := m.contentDimensions()
			m.ghostOutputModel = NewGhostOutputModel(m.themeManager.Current(), cw, ch)
		}
		return nil
	case ScreenConfirmQuit:
		if m.confirmQuitModel == nil {
			cw, ch := m.contentDimensions()
			m.confirmQuitModel = NewConfirmQuitModel(m.themeManager.Current(), cw, ch)
		}
		return nil
	case ScreenDiff:
		if m.diffModel == nil {
			m.diffModel = NewDiffModel(m.themeManager.Current())
		}
		cw, ch := m.contentDimensions()
		m.diffModel.SetDimensions(cw, ch)
		return nil
	case ScreenCommandPalette:
		if m.commandPaletteScreenModel == nil {
			cw, ch := m.contentDimensions()
			m.commandPaletteScreenModel = NewCommandPaletteScreenModel(m.cmdRegistry, m.themeManager.Current(), cw, ch)
		}
		return m.commandPaletteScreenModel.Init()
	case ScreenHome:
		if m.homeModel == nil {
			cw, ch := m.contentDimensions()
			m.homeModel = NewHomeModel(m.themeManager.Current(), cw, ch, m.version)
			m.homeModel.SetCommandRegistry(m.cmdRegistry)
		}
		return m.homeModel.Init()
	default:
		return nil
	}
}

// handleWindowResize resizes all sub-models using content dimensions
// (terminal minus unified chrome and sidebar).
func (m *AppState) handleWindowResize(msg tea.WindowSizeMsg) tea.Cmd {
	// Compute content dimensions: terminal minus unified chrome (header+footer)
	contentW := msg.Width
	contentH := msg.Height - layout.ChromeHeight
	if contentH < 1 {
		contentH = 1
	}

	// Subtract sidebar width when visible at full breakpoint
	sw := 0
	if m.sidebarModel != nil && m.sidebarModel.IsVisible() && layout.ShowSidebar(msg.Width) {
		sw = m.sidebarModel.GetWidth()
		contentW -= sw
	}
	if contentW < 1 {
		contentW = 1
	}

	// Build a content-sized WindowSizeMsg for sub-models
	contentMsg := tea.WindowSizeMsg{Width: contentW, Height: contentH}

	var cmd tea.Cmd
	if m.replModel != nil {
		m.replModel.width = contentW
		m.replModel.height = contentH
		// contentW already accounts for the sidebar; pass 0 to avoid double-subtracting.
		m.replModel.SetSidebarWidth(0)
		replM, replCmd := m.replModel.Update(contentMsg)
		if r, ok := replM.(*ReplModel); ok {
			m.replModel = r
		}
		cmd = replCmd
	}

	if m.planModel != nil {
		m.planModel.SetDimensions(contentW, contentH)
	}
	if m.executeModel != nil {
		m.executeModel.width = contentW
		m.executeModel.height = contentH
	}
	if m.verifyModel != nil {
		m.verifyModel.width = contentW
		m.verifyModel.height = contentH
	}
	if m.metricsModel != nil {
		m.metricsModel.width = contentW
		m.metricsModel.height = contentH
	}
	if m.settingsModel != nil {
		m.settingsModel.width = contentW
		m.settingsModel.height = contentH
	}
	if m.cmdPalette != nil {
		m.cmdPalette.SetDimensions(contentW, contentH)
	}
	if m.msModel != nil {
		m.msModel.SetDimensions(contentW, contentH)
	}
	if m.resumeModel != nil {
		m.resumeModel.SetDimensions(contentW, contentH)
	}
	if m.diffModel != nil {
		m.diffModel.SetDimensions(contentW, contentH)
	}
	if m.goalInput != nil {
		m.goalInput.SetDimensions(contentW, contentH)
	}
	if m.ledgerModel != nil {
		m.ledgerModel.SetDimensions(contentW, contentH)
	}
	if m.rollbackModel != nil {
		m.rollbackModel.SetDimensions(contentW, contentH)
	}
	if m.shipModel != nil {
		m.shipModel.width = contentW
		m.shipModel.height = contentH
	}
	if m.firstRunModel != nil {
		m.firstRunModel.SetDimensions(contentW, contentH)
	}
	if m.configModel != nil {
		m.configModel.width = contentW
		m.configModel.height = contentH
	}
	if m.discussModel != nil {
		m.discussModel.SetDimensions(contentW, contentH)
	}
	if m.helpModel != nil {
		m.helpModel.SetDimensions(contentW, contentH)
	}
	if m.bisectModel != nil {
		m.bisectModel.SetDimensions(contentW, contentH)
	}
	if m.notifModel != nil {
		m.notifModel.SetDimensions(contentW, contentH)
	}
	if m.dashboardModel != nil {
		m.dashboardModel.SetDimensions(contentW, contentH)
	}
	if m.sessionDetailModel != nil {
		m.sessionDetailModel.SetDimensions(contentW, contentH)
	}
	if m.fileExplorerModel != nil {
		m.fileExplorerModel.SetDimensions(contentW, contentH)
	}
	if m.toolDetailModel != nil {
		m.toolDetailModel.SetDimensions(contentW, contentH)
	}
	if m.phaseModelPicker != nil {
		m.phaseModelPicker.SetDimensions(contentW, contentH)
	}
	if m.ghostPickerModel != nil {
		m.ghostPickerModel.SetDimensions(contentW, contentH)
	}
	if m.ghostOutputModel != nil {
		m.ghostOutputModel.SetDimensions(contentW, contentH)
	}
	if m.confirmQuitModel != nil {
		m.confirmQuitModel.SetDimensions(contentW, contentH)
	}
	if m.commandPaletteScreenModel != nil {
		m.commandPaletteScreenModel.SetDimensions(contentW, contentH)
	}
	if m.homeModel != nil {
		m.homeModel.SetDimensions(contentW, contentH)
	}

	// UX-38: Notify when sidebar auto-hides due to narrow terminal
	if m.sidebarModel != nil && m.sidebarModel.IsVisible() && msg.Width < WidthFull {
		if !m.sidebarAutoHideNotified {
			m.addToast("Sidebar hidden — resize wider or press ctrl+b to toggle", "info")
			m.sidebarAutoHideNotified = true
		}
	} else if msg.Width >= WidthFull {
		m.sidebarAutoHideNotified = false
	}

	return cmd
}

// routeKeyMsg routes key events to the active screen.
func (m *AppState) routeKeyMsg(msg tea.KeyMsg) tea.Cmd {
	// Command palette has priority
	if m.cmdPalette != nil && m.cmdPalette.IsOpen() {
		newPalette, cmd := m.cmdPalette.Update(msg)
		m.cmdPalette = newPalette
		return cmd
	}

	// Confirmation dialog active — intercept y/n/esc
	if m.pendingConfirm != nil {
		switch msg.String() {
		case "y", "Y":
			pending := *m.pendingConfirm
			m.pendingConfirm = nil
			m.confirmPrompt = ""
			pending.ConfirmRequired = false
			return m.processCommandResult(pending)
		case "n", "N", "esc":
			m.pendingConfirm = nil
			m.confirmPrompt = ""
			if m.replModel != nil {
				m.replModel.AddMessage(makeAssistantMsg("Cancelled."))
			}
			return nil
		}
		return nil
	}

	// Intent classification confirmation active — intercept y/n
	if m.pendingIntent != nil {
		switch msg.String() {
		case "y", "Y":
			intent := m.pendingIntent
			input := m.pendingIntentInput
			m.pendingIntent = nil
			m.pendingIntentInput = ""
			mode := types.WorkflowModeForIntent(*intent)
			m.workflowMode = mode
			if m.workflowEngine != nil {
				m.workflowEngine.SetWorkflowMode(mode)
			}
			return m.runWorkflowFromGoal(input)
		case "n", "N", "esc":
			input := m.pendingIntentInput
			m.pendingIntent = nil
			m.pendingIntentInput = ""
			if m.replModel != nil {
				m.replModel.AddMessage(makeAssistantMsg("Proceeding in chat mode."))
			}
			p := m.registry.ActiveProvider()
			if p != nil && m.agentMode {
				return m.startAgentLoop(p, input)
			}
			if p != nil {
				return m.sendPlainTextChat(p, input)
			}
			return nil
		}
		return nil
	}

	// Sidebar focus toggle — works from any screen
	if msg.String() == "ctrl+g" && m.sidebarModel != nil && m.sidebarModel.IsVisible() {
		m.sidebarModel.ToggleFocus()
		return nil
	}

	// Leader key must never be swallowed by sidebar or screen handlers.
	// Activate leader mode or complete a chord before routing to sidebar.
	if m.keyRegistry != nil {
		if !m.keyRegistry.IsLeaderActive() {
			// First press: check if this is the leader key itself
			if msg.String() == m.keyRegistry.LeaderKey() {
				handled, cmd := m.keyRegistry.Handle(msg.String(), CtxREPL)
				if handled {
					return cmd
				}
			}
		} else {
			// Leader active: complete chord regardless of focused screen.
			// Use CtxREPL so Handle searches CtxREPL then CtxGlobal,
			// catching all registered chords.
			handled, cmd := m.keyRegistry.Handle(msg.String(), CtxREPL)
			if handled {
				return cmd
			}
		}
	}

	// When sidebar is focused, route keys to sidebar regardless of active screen.
	// ctrl+g can toggle sidebar focus from any screen, so keys must reach it.
	if m.sidebarModel != nil && m.sidebarModel.IsFocused() {
		return m.sidebarModel.HandleKey(msg)
	}

	return m.routeKeyToScreen(msg)
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

// navigateToScreen transitions to the given screen.
func (m *AppState) navigateToScreen(screen Screen) tea.Cmd {
	m.prevScreen = m.screen

	// Push current screen to back-stack for esc-to-go-back navigation.
	// Skip: same screen, permission overlay, diff overlay, and REPL (always fallback).
	if m.screen != screen && m.screen != ScreenPermission && screen != ScreenPermission &&
		m.screen != ScreenREPL {
		// Avoid duplicate consecutive entries
		if len(m.screenStack) == 0 || m.screenStack[len(m.screenStack)-1] != m.screen {
			m.screenStack = append(m.screenStack, m.screen)
		}
		if m.screenCap > 0 && len(m.screenStack) > m.screenCap {
			m.screenStack = m.screenStack[len(m.screenStack)-m.screenCap:]
		}
	}

	// Start a brief transition overlay if this is a real screen change.
	// Skip for overlays and first-run.
	skipTransition := screen == ScreenPermission || screen == ScreenDiff ||
		screen == ScreenFirstRun || screen == ScreenModelSelector
	if m.screen != screen && !skipTransition {
		// Eagerly ensure sub-model exists so it's ready when transition completes.
		initCmd := m.ensureSubModel(screen)
		m.StartTransition(screen, "")
		return tea.Batch(StreamTickCmd(), initCmd)
	}

	m.screen = screen
	return m.ensureSubModel(screen)
}

// popScreen navigates back to the previous screen in the back-stack, or to REPL if empty.
func (m *AppState) popScreen() tea.Cmd {
	if len(m.screenStack) > 0 {
		prev := m.screenStack[len(m.screenStack)-1]
		m.screenStack = m.screenStack[:len(m.screenStack)-1]
		m.screen = prev
		return m.ensureSubModel(prev)
	}
	m.screen = ScreenREPL
	return nil
}

// contentDimensions returns the content-area width and height, accounting for
// the unified chrome (header+footer) and the sidebar when visible.
func (m *AppState) contentDimensions() (w, h int) {
	w = m.width
	h = m.height - layout.ChromeHeight
	if h < 1 {
		h = 1
	}
	if m.sidebarModel != nil && m.sidebarModel.IsVisible() && layout.ShowSidebar(m.width) {
		w -= m.sidebarModel.GetWidth()
	}
	if w < 1 {
		w = 1
	}
	return w, h
}

// ensureSubModel creates or resizes the sub-model for the given screen.
func (m *AppState) ensureSubModel(screen Screen) tea.Cmd {
	cw, ch := m.contentDimensions()
	switch screen {
	case ScreenREPL:
		m.ensureReplModel()
		return nil
	case ScreenModelSelector:
		if m.msModel == nil {
			m.msModel = NewModelSelector(m.shutdownCtx, m.registry, m.sessionManager, m.themeManager.Current())
		}
		m.msModel.SetDimensions(cw, ch)
		return m.msModel.Init()
	case ScreenSettings:
		if m.settingsModel == nil {
			m.settingsModel = NewSettingsModel(m.config, m.registry, m.themeManager.Current(), m.configPath, m.version, m.keychain, m.shutdownCtx)
		}
		m.settingsModel.width = cw
		m.settingsModel.height = ch
		return m.settingsModel.Init()
	case ScreenResume:
		// Resume screen loads sessions async; use the existing command.
		return m.openResumeScreen()
	case ScreenGoalInput:
		if m.goalInput == nil {
			m.goalInput = NewGoalInputModel(m.themeManager.Current(), nil)
		}
		m.goalInput.SetDimensions(cw, ch)
		return m.goalInput.Init()
	case ScreenPlan:
		if m.planModel != nil {
			m.planModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenExecute:
		if m.executeModel != nil {
			m.executeModel.width = cw
			m.executeModel.height = ch
		}
		return nil
	case ScreenVerify:
		if m.verifyModel != nil {
			m.verifyModel.width = cw
			m.verifyModel.height = ch
		}
		return nil
	case ScreenRuntimeCheck:
		if m.runtimeModel != nil {
			m.runtimeModel.width = cw
			m.runtimeModel.height = ch
		}
		return nil
	case ScreenShip:
		if m.shipModel != nil {
			m.shipModel.width = cw
			m.shipModel.height = ch
		}
		return nil
	case ScreenLedger:
		if m.ledgerModel == nil {
			m.ledgerModel = NewLedgerModel(m.themeManager.Current(), m.ledger)
			m.ledgerModel.SetDimensions(cw, ch)
			m.ledgerModel.LoadEntries()
		}
		return nil
	case ScreenRollback:
		if m.rollbackModel == nil {
			m.rollbackModel = NewRollbackModel(m.themeManager.Current(), m.git, m.rollback, cw, ch)
			m.rollbackModel.LoadCommits()
		}
		return nil
	case ScreenMetrics:
		if m.metricsModel == nil {
			m.metricsModel = NewMetricsModel(m.themeManager.Current())
		}
		m.metricsModel.SetDimensions(cw, ch)
		if m.sessionManager != nil {
			return m.metricsModel.LoadStatsCmd(m.sessionManager)
		}
		return nil
	case ScreenConfig:
		if m.configModel == nil {
			m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, cw, ch, m.keychain)
		} else {
			// Sync live config pointer so edits made in settings are visible
			m.configModel.cfg = m.config
			m.configModel.cfgPath = m.configPath
			m.configModel.width = cw
			m.configModel.height = ch
			m.configModel.theme = m.themeManager.Current()
			m.configModel.buildSections()
		}
		return nil
	case ScreenHelp:
		if m.helpModel == nil {
			m.helpModel = NewHelpModel(m.themeManager.Current())
		}
		m.helpModel.SetDimensions(cw, ch)
		return m.helpModel.Init()
	case ScreenDiscuss:
		if m.discussModel == nil {
			m.discussModel = NewDiscussModel(m.themeManager.Current(), m.discussQuestions, cw, ch)
			// Activate discuss timeout from config
			if m.config != nil && m.config.UI.DiscussTimeout > 0 {
				m.discussModel.SetTimeout(m.config.UI.DiscussTimeout)
			}
		} else {
			m.discussModel.SetDimensions(cw, ch)
		}
		return m.discussModel.Init()
	case ScreenBisect:
		if m.bisectModel == nil {
			m.bisectModel = NewBisectModel(m.themeManager.Current(), cw, ch)
		} else {
			m.bisectModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenNotifications:
		if m.notifModel == nil {
			m.notifModel = NewNotificationModel(m.themeManager.Current(), cw, ch)
		} else {
			m.notifModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenDashboard:
		if m.dashboardModel == nil {
			m.dashboardModel = NewDashboardModel(m.themeManager.Current(), cw, ch)
		} else {
			m.dashboardModel.SetDimensions(cw, ch)
		}
		if m.workflowEngine != nil {
			m.dashboardModel.SetWorkflowState(m.workflowPhase, m.workflowGoal, "", m.activeProvider)
		}
		return nil
	case ScreenSessionDetail:
		if m.sessionDetailModel == nil {
			m.sessionDetailModel = NewSessionDetailModel(m.themeManager.Current(), cw, ch)
		} else {
			m.sessionDetailModel.SetDimensions(cw, ch)
		}
		// Load session data if we have a session ID from AppMsg
		if m.sessionDetailModel.sess == nil && m.sessionManager != nil {
			// Try to find the session from the resume model's current selection
			if m.resumeModel != nil && len(m.resumeModel.sessions) > 0 {
				idx := m.resumeModel.cursor
				if idx >= 0 && idx < len(m.resumeModel.sessions) {
					sid := m.resumeModel.sessions[idx].ID
					if sess, err := m.sessionManager.LoadSession(sid); err == nil && sess != nil {
						m.sessionDetailModel.SetSession(sess)
					}
				}
			}
		}
		return nil
	case ScreenFileExplorer:
		if m.fileExplorerModel == nil {
			m.fileExplorerModel = NewFileExplorerModel(m.themeManager.Current(), cw, ch)
			// Build real file tree from working directory
			if m.cwd != "" {
				root := buildFileTree(m.cwd, 0, 3)
				if root != nil {
					m.fileExplorerModel.SetRoot(root)
				}
			}
		} else {
			m.fileExplorerModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenToolDetail:
		if m.toolDetailModel == nil {
			m.toolDetailModel = NewToolDetailModel(m.themeManager.Current(), cw, ch)
		} else {
			m.toolDetailModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenFirstRun:
		fm := NewFirstRunModel(m.themeManager.Current(), m.registry, m.config, m.version, m.shutdownCtx)
		fm.SetDimensions(cw, ch)
		m.firstRunModel = fm
		return fm.Init()
	case ScreenGhostPicker:
		if m.ghostPickerModel == nil {
			m.ghostPickerModel = NewGhostPickerModel(m.themeManager.Current(), cw, ch)
		} else {
			m.ghostPickerModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenGhostOutput:
		if m.ghostOutputModel == nil {
			m.ghostOutputModel = NewGhostOutputModel(m.themeManager.Current(), cw, ch)
		} else {
			m.ghostOutputModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenConfirmQuit:
		if m.confirmQuitModel == nil {
			m.confirmQuitModel = NewConfirmQuitModel(m.themeManager.Current(), cw, ch)
		} else {
			m.confirmQuitModel.SetDimensions(cw, ch)
		}
		return nil
	case ScreenPhaseModelPicker:
		if m.phaseModelPicker == nil {
			m.phaseModelPicker = NewPhaseModelPickerModel(m.shutdownCtx, m.registry, m.themeManager.Current(), cw, ch)
		} else {
			m.phaseModelPicker.SetDimensions(cw, ch)
		}
		return nil
	case ScreenChatHistory:
		if m.chatHistoryModel == nil {
			m.chatHistoryModel = NewChatHistoryModel(m.themeManager.Current(), cw, ch)
		} else {
			m.chatHistoryModel.SetDimensions(cw, ch)
		}
		// Load messages from current REPL session
		if m.replModel != nil {
			m.chatHistoryModel.SetMessages(m.replModel.Messages())
		}
		return nil
	case ScreenCommandPalette:
		if m.commandPaletteScreenModel == nil {
			m.commandPaletteScreenModel = NewCommandPaletteScreenModel(m.cmdRegistry, m.themeManager.Current(), cw, ch)
		} else {
			m.commandPaletteScreenModel.SetDimensions(cw, ch)
		}
		return m.commandPaletteScreenModel.Init()
	case ScreenHome:
		if m.homeModel == nil {
			m.homeModel = NewHomeModel(m.themeManager.Current(), cw, ch, m.version)
			m.homeModel.SetCommandRegistry(m.cmdRegistry)
		} else {
			m.homeModel.SetDimensions(cw, ch)
		}
		return m.homeModel.Init()
	default:
		return nil
	}
}

// handleKeyAction processes KeyActionMsg strings.
func (m *AppState) handleKeyAction(action string) tea.Cmd {
	switch action {
	case "open_home":
		return m.navigateToScreen(ScreenHome)
	case "open_settings":
		return m.navigateToScreen(ScreenSettings)
	case "open_help":
		return m.navigateToScreen(ScreenHelp)
	case "open_ledger":
		return m.navigateToScreen(ScreenLedger)
	case "open_rollback":
		return m.navigateToScreen(ScreenRollback)
	case "open_dashboard":
		return m.navigateToScreen(ScreenDashboard)
	case "open_notifications":
		return m.navigateToScreen(ScreenNotifications)
	case "open_files":
		return m.navigateToScreen(ScreenFileExplorer)
	case "open_config":
		return m.navigateToScreen(ScreenConfig)
	case "open_session_detail":
		return m.navigateToScreen(ScreenSessionDetail)
	case "open_tool_detail":
		return m.navigateToScreen(ScreenToolDetail)
	case "toggle_subagents":
		if m.subagentsModel != nil && !m.subagentsModel.IsEmpty() {
			m.subagentsVisible = !m.subagentsVisible
		}
		return nil
	case "toggle_sidebar":
		if m.sidebarModel != nil {
			m.sidebarModel.Toggle()
			if m.replModel != nil {
				// REPL width is already content-area; pass 0 to avoid double-subtracting.
				m.replModel.SetSidebarWidth(0)
			}
		}
		return nil
	case "sidebar_wider":
		if m.sidebarModel != nil && m.sidebarModel.IsVisible() {
			m.sidebarModel.IncreaseWidth()
			if m.replModel != nil {
				m.replModel.SetSidebarWidth(0)
			}
		}
		return nil
	case "sidebar_narrower":
		if m.sidebarModel != nil && m.sidebarModel.IsVisible() {
			m.sidebarModel.DecreaseWidth()
			if m.replModel != nil {
				m.replModel.SetSidebarWidth(0)
			}
		}
		return nil
	case "new_session":
		return m.startNewSession()
	case "session_list":
		return m.openResumeScreen()
	case "open_palette":
		return m.navigateToScreen(ScreenCommandPalette)
	case "cycle_model", "cycle_model_forward":
		return m.navigateToScreen(ScreenModelSelector)
	case "cycle_model_backward":
		return m.navigateToScreen(ScreenModelSelector)
	case "cancel_stream":
		if m.streamCancelFn != nil {
			m.streamCancelFn()
			m.streamCancelFn = nil
		}
		return nil
	case "view_ship_diff":
		if m.git != nil {
			if diff, err := m.git.Diff("", ""); err == nil && diff != "" {
				return func() tea.Msg {
					return DiffScreenMsg{Diff: diff, Title: "Ship Diff"}
				}
			}
		}
		return nil
	case "runtime_continue":
		m.setWorkflowPhase(types.PhaseShip)
		m.screen = ScreenShip
		if m.workflowEngine != nil {
			if err := m.workflowEngine.Transition(m.shutdownCtx, types.PhaseRuntime, types.PhaseShip); err != nil {
				slog.Error("phase transition failed", "from", types.PhaseRuntime, "to", types.PhaseShip, "error", err)
			}
		}
		tasks, _ := m.sessionManager.LoadTasks(m.sessionID)
		modelName := ""
		if m.activeModel != nil {
			modelName = m.activeModel.Name
		}
		summary := ShipSummary{
			SessionID: m.sessionID,
			TaskDone:  countDone(tasks),
			TaskTotal: len(tasks),
			Model:     modelName,
			Provider:  m.activeProvider,
		}
		cw, ch := m.contentDimensions()
		m.shipModel = NewShipModel(summary, m.themeManager.Current(), cw, ch)
		m.persistWorkflowState()
		return m.RunPhaseCmd(types.PhaseShip)
	}
	return nil
}

// ─── Theme helpers ────────────────────────────────────────────────────────────

// applyTheme switches the theme and propagates it to all sub-models.
func (m *AppState) applyTheme(themeName string) {
	switch themeName {
	case "dark":
		m.themeManager = theme.NewManager(theme.ModeDark)
	case "light":
		m.themeManager = theme.NewManager(theme.ModeLight)
	case "auto":
		m.themeManager = theme.NewManager(theme.ModeAuto)
	}

	// Persist theme selection to config so it survives restarts.
	if m.config != nil {
		m.config.UI.Theme = themeName
		if m.configPath != "" {
			if err := m.config.SaveWithKeychain(m.configPath, m.keychain); err != nil {
				slog.Warn("failed to save theme to config", "error", err)
			}
		}
	}

	t := m.themeManager.Current()
	if m.replModel != nil {
		m.replModel.SetTheme(t)
	}
	if m.sidebarModel != nil {
		m.sidebarModel.SetTheme(t)
	}
	if m.cmdPalette != nil {
		m.cmdPalette.SetTheme(t)
	}
	if m.settingsModel != nil {
		m.settingsModel.SetTheme(t)
	}
	if m.planModel != nil {
		m.planModel.theme = t
	}
	if m.executeModel != nil {
		m.executeModel.theme = t
	}
	if m.verifyModel != nil {
		m.verifyModel.theme = t
	}
	if m.shipModel != nil {
		m.shipModel.theme = t
	}
	if m.discussModel != nil {
		m.discussModel.SetTheme(t)
	}
	if m.metricsModel != nil {
		m.metricsModel.SetTheme(t)
	}
	if m.resumeModel != nil {
		m.resumeModel.SetTheme(t)
	}
	if m.diffModel != nil {
		m.diffModel.SetTheme(t)
	}
	if m.goalInput != nil {
		m.goalInput.SetTheme(t)
	}
	if m.ledgerModel != nil {
		m.ledgerModel.SetTheme(t)
	}
	if m.rollbackModel != nil {
		m.rollbackModel.SetTheme(t)
	}
	if m.firstRunModel != nil {
		m.firstRunModel.SetTheme(t)
	}
	if m.msModel != nil {
		m.msModel.SetTheme(t)
	}
	if m.helpModel != nil {
		m.helpModel.SetTheme(t)
	}
	if m.configModel != nil {
		m.configModel.theme = t
	}
	if m.bisectModel != nil {
		m.bisectModel.SetTheme(t)
	}
	if m.notifModel != nil {
		m.notifModel.SetTheme(t)
	}
	if m.dashboardModel != nil {
		m.dashboardModel.SetTheme(t)
	}
	if m.sessionDetailModel != nil {
		m.sessionDetailModel.SetTheme(t)
	}
	if m.fileExplorerModel != nil {
		m.fileExplorerModel.SetTheme(t)
	}
	if m.toolDetailModel != nil {
		m.toolDetailModel.SetTheme(t)
	}
	if m.confirmQuitModel != nil {
		m.confirmQuitModel.SetTheme(t)
	}
	if m.ghostPickerModel != nil {
		m.ghostPickerModel.SetTheme(t)
	}
	if m.ghostOutputModel != nil {
		m.ghostOutputModel.SetTheme(t)
	}
	if m.phaseModelPicker != nil {
		m.phaseModelPicker.SetTheme(t)
	}
	if m.commandPaletteScreenModel != nil {
		m.commandPaletteScreenModel.SetTheme(t)
	}
}

// ─── Session helpers ──────────────────────────────────────────────────────────

// startNewSession creates a new session and switches to the REPL.
func (m *AppState) startNewSession() tea.Cmd {
	if m.sessionManager == nil {
		return nil
	}

	modelID := ""
	if m.activeModel != nil {
		modelID = m.activeModel.ID
	}

	sess, err := m.sessionManager.NewSession(modelID, m.activeProvider)
	if err != nil {
		slog.Error("new session failed", "err", err)
		m.addToast("Failed to create session: "+m31errors.UserMessage(err), "error")
		return nil
	}

	m.sessionID = sess.ID
	m.propagateSessionID(sess.ID)
	m.ensureReplModel()
	m.replModel.ClearMessages()
	m.replModel.SetSessionID(sess.ID)
	m.workflowPhase = types.PhaseIdle
	m.workflowGoal = ""
	// Only switch to REPL if not already on a dedicated landing/startup screen.
	if m.screen != ScreenHome && m.screen != ScreenFirstRun {
		m.screen = ScreenREPL
	}

	return m.syncReplProvider(sess.ID)
}

// openResumeScreen loads session list and navigates to the resume screen.
func (m *AppState) openResumeScreen() tea.Cmd {
	return func() tea.Msg {
		var sessions []session.SessionInfo
		total := 0
		if m.sessionManager != nil {
			infos, err := m.sessionManager.ListSessions()
			if err == nil {
				total = len(infos)
				for i, info := range infos {
					if i >= 20 {
						break
					}
					sessions = append(sessions, info)
				}
			}
		}
		return resumeScreenReadyMsg{sessions: sessions, total: total}
	}
}

// resumeScreenReadyMsg carries loaded sessions for the resume screen.
type resumeScreenReadyMsg struct {
	sessions []session.SessionInfo
	total    int
}

// ensureToolDetailModel lazily creates the tool-detail model with the current
// theme and REPL dimensions so the detail screen is ready to display.
func (m *AppState) ensureToolDetailModel() {
	if m.toolDetailModel != nil {
		return
	}
	t := m.themeManager.Current()
	w := m.width
	h := m.height
	if w < 40 {
		w = 40
	}
	if h < 10 {
		h = 10
	}
	m.toolDetailModel = NewToolDetailModel(t, w, h)
}

// extractToolDetail returns (title, body) for the first tool_use segment in
// messages[messageIndex] whose Name matches toolName. Falls back to the first
// tool_use segment when toolName is empty or unmatched. Returns ("", "") when
// no tool_use segment exists in the message.
func (m *AppState) extractToolDetail(messageIndex int, toolName string) (string, string) {
	if m.replModel == nil {
		return "", ""
	}
	msgs := m.replModel.messages
	if messageIndex < 0 || messageIndex >= len(msgs) {
		return "", ""
	}
	msg := msgs[messageIndex]

	type candidate struct {
		name  string
		input string
		body  string
	}
	var fallback *candidate

	for _, seg := range msg.Segments {
		if seg.Type != "tool_use" {
			continue
		}
		name := extractToolName(seg.Content)
		body := seg.Content
		c := &candidate{name: name, input: seg.Content, body: body}
		if toolName != "" && name == toolName {
			return name, c.body
		}
		if fallback == nil {
			fallback = c
		}
	}

	if fallback != nil {
		return fallback.name, fallback.body
	}
	return "", ""
}

// openSettingsScreen transitions to the settings screen.
func (m *AppState) openSettingsScreen() tea.Cmd {
	return m.navigateToScreen(ScreenSettings)
}

// runWorkflowFromGoal starts the workflow with adaptive phase routing.
// If m.workflowPhase is already set (e.g., from /resume-task), it resumes from that phase.
func (m *AppState) runWorkflowFromGoal(goal string) tea.Cmd {
	m.workflowGoal = goal
	cmds := []tea.Cmd{m.initWorkflowEngine()}

	// Switch sidebar to todo mode immediately so the user sees phase/task
	// progress instead of the file tree from the moment the workflow starts.
	if m.sidebarModel != nil {
		m.sidebarModel.SetMode(SidebarModeTodo)
	}

	// Classify the goal and set the workflow mode on the engine
	if m.workflowEngine != nil {
		mode := m.resolveWorkflowMode(goal)
		m.workflowEngine.SetWorkflowMode(mode)
		m.workflowMode = mode
		if mode != types.ModeFull {
			m.addToast(fmt.Sprintf("Workflow mode: %s (adaptive — skipping unnecessary phases)", mode), "info")
		}
	}

	// Resume from existing phase if set, otherwise start from Initialize
	startPhase := m.workflowPhase
	if startPhase == types.PhaseIdle || startPhase == "" {
		startPhase = types.PhaseInitialize
	}
	m.workflowPhase = startPhase

	// Seed the sidebar phase pipeline so the phase bar is visible right away.
	if m.sidebarModel != nil {
		m.sidebarModel.SetCurrentPhase(string(startPhase))
	}

	cmds = append(cmds, m.RunPhaseCmd(startPhase))
	return tea.Batch(cmds...)
}

// resolveWorkflowMode determines the appropriate workflow mode.
// If the user has set an explicit mode via config, that takes precedence.
// If a prior intent classification result is available, it uses that.
// Otherwise, the goal is classified with keyword heuristics.
func (m *AppState) resolveWorkflowMode(goal string) types.WorkflowMode {
	// Config override takes highest precedence
	if m.config != nil {
		switch m.config.Features.WorkflowMode {
		case string(types.ModeFull):
			return types.ModeFull
		case string(types.ModeFast):
			return types.ModeFast
		case string(types.ModeDirect):
			return types.ModeDirect
		}
	}

	// Use intent result from the workflow engine if available (LLM-classified)
	if m.workflowEngine != nil {
		if eng, ok := m.workflowEngine.(*workflow.Engine); ok {
			if ir := eng.IntentResult(); ir != nil {
				return types.WorkflowModeForIntent(*ir)
			}
		}
	}

	// Use pending intent if available (from REPL classification)
	if m.pendingIntent != nil {
		return types.WorkflowModeForIntent(*m.pendingIntent)
	}

	// Classify based on goal content when mode is auto or unset
	workDir := "."
	if m.git != nil {
		workDir = m.git.WorkDir()
	}
	complexity := workflow.ClassifyPrompt(goal, workDir)
	return workflow.WorkflowModeForComplexity(complexity)
}

// ─── Permission helpers ───────────────────────────────────────────────────────

// handlePermissionResponse processes the user's permission decision.
func (m *AppState) handlePermissionResponse(msg PermissionResponseMsg) tea.Cmd {
	if m.dispatcher == nil {
		return nil
	}
	if m.permRequest == nil {
		return nil
	}
	reqID := m.permRequest.ID
	allowed := msg.Response.Allowed
	remember := msg.Response.Remember
	m.permRequest = nil
	// Return to the previous screen (before permission overlay)
	if len(m.screenStack) > 0 {
		prev := m.screenStack[len(m.screenStack)-1]
		m.screenStack = m.screenStack[:len(m.screenStack)-1]
		m.screen = prev
	} else {
		m.screen = ScreenREPL
	}
	m.dispatcher.ApprovePermission(reqID, allowed, remember)
	return permListenerCmd(m.shutdownCtx, m.dispatcher)
}

// handlePermissionTick decrements the permission countdown.
func (m *AppState) handlePermissionTick() tea.Cmd {
	if m.permRequest == nil {
		return nil
	}
	if m.permCountdown > 0 {
		m.permCountdown--
		if m.permModal != nil {
			m.permModal.Tick()
		}
		if m.permCountdown == 0 {
			// Auto-deny on timeout — guard against permRequest already cleared
			if m.permRequest == nil {
				return nil
			}
			return m.handlePermissionResponse(PermissionResponseMsg{
				Response: tools.PermissionResponse{
					RequestID: m.permRequest.ID,
					Allowed:   false,
				},
			})
		}
	}
	return tea.Tick(time.Second, func(time.Time) tea.Msg {
		return PermissionTickMsg{}
	})
}

// handlePermissionKey processes keys in the permission modal.
func (m *AppState) handlePermissionKey(msg tea.KeyMsg) tea.Cmd {
	if m.questionRequest != nil {
		return m.handleQuestionKey(msg)
	}
	if m.permRequest == nil {
		// Return to previous screen instead of hardcoding REPL
		if len(m.screenStack) > 0 {
			prev := m.screenStack[len(m.screenStack)-1]
			m.screenStack = m.screenStack[:len(m.screenStack)-1]
			m.screen = prev
		} else {
			m.screen = ScreenREPL
		}
		return nil
	}
	switch msg.String() {
	case "y":
		return m.handlePermissionResponse(PermissionResponseMsg{
			Response: tools.PermissionResponse{
				RequestID: m.permRequest.ID,
				Allowed:   true,
			},
		})
	case "a", "A":
		return m.handlePermissionResponse(PermissionResponseMsg{
			Response: tools.PermissionResponse{
				RequestID: m.permRequest.ID,
				Allowed:   true,
				Remember:  true,
			},
		})
	case "n", "enter", "esc":
		return m.handlePermissionResponse(PermissionResponseMsg{
			Response: tools.PermissionResponse{
				RequestID: m.permRequest.ID,
				Allowed:   false,
			},
		})
	}
	return nil
}

// handleQuestionKey routes key events to the question model.
func (m *AppState) handleQuestionKey(msg tea.KeyMsg) tea.Cmd {
	if m.questionModel == nil || m.questionRequest == nil {
		// Return to previous screen instead of hardcoding REPL
		if len(m.screenStack) > 0 {
			prev := m.screenStack[len(m.screenStack)-1]
			m.screenStack = m.screenStack[:len(m.screenStack)-1]
			m.screen = prev
		} else {
			m.screen = ScreenREPL
		}
		return nil
	}
	_, cmd := m.questionModel.Update(msg)
	// QuestionModel emits tools.QuestionResponse via cmd
	return cmd
}

// handleQuestionResponse processes the user's question answer.
func (m *AppState) handleQuestionResponse(msg QuestionResponseMsg) tea.Cmd {
	if m.dispatcher == nil || m.questionRequest == nil {
		return nil
	}
	reqID := m.questionRequest.ID
	m.questionRequest = nil
	// Return to the previous screen (before question overlay)
	if len(m.screenStack) > 0 {
		prev := m.screenStack[len(m.screenStack)-1]
		m.screenStack = m.screenStack[:len(m.screenStack)-1]
		m.screen = prev
	} else {
		m.screen = ScreenREPL
	}

	m.dispatcher.RespondQuestion(reqID, msg.Answer)
	return questionListenerCmd(m.shutdownCtx, m.dispatcher)
}

// handleDiscussAnswer submits a single discuss answer to the workflow engine.
func (m *AppState) handleDiscussAnswer(msg DiscussAnswerMsg) tea.Cmd {
	if m.workflowEngine == nil {
		return nil
	}
	if err := m.workflowEngine.SubmitDiscussAnswer(msg.Index, msg.Answer); err != nil {
		slog.Warn("failed to submit discuss answer", "index", msg.Index, "error", err)
	}
	return nil
}

// handleDiscussComplete finalizes the discuss phase and transitions to planning.
func (m *AppState) handleDiscussComplete() tea.Cmd {
	if m.workflowEngine == nil {
		return nil
	}
	if err := m.workflowEngine.FinalizeDiscuss(); err != nil {
		slog.Error("failed to finalize discuss", "error", err)
		m.addToast("Failed to save discuss answers", "error")
		return nil
	}

	next, ok := nextPhaseForMode(types.PhaseDiscuss, m.workflowMode)
	if !ok || next == types.PhaseIdle {
		m.setWorkflowPhase(types.PhaseIdle)
		m.screen = ScreenREPL
		return nil
	}

	m.setWorkflowPhase(next)
	if err := m.workflowEngine.Transition(m.shutdownCtx, types.PhaseDiscuss, next); err != nil {
		slog.Error("phase transition failed", "from", types.PhaseDiscuss, "to", next, "error", err)
		m.addToast("Phase transition failed — session may not resume correctly", "error")
		return nil
	}
	m.persistWorkflowState()

	if next == types.PhaseExecute {
		m.screen = ScreenExecute
		var tasks []types.Task
		if m.sessionManager != nil {
			var loadErr error
			tasks, loadErr = m.sessionManager.LoadTasks(m.sessionID)
			if loadErr != nil {
				slog.Warn("failed to load tasks for execute screen", "error", loadErr)
			}
		}
		if m.executeModel == nil {
			cw, ch := m.contentDimensions()
			m.executeModel = NewExecuteModel(tasks, m.themeManager.Current(), cw, ch)
		} else {
			m.executeModel.tasks = tasks
		}
		return m.RunPhaseCmd(types.PhaseExecute)
	}

	m.screen = ScreenPlan
	return m.RunPhaseCmd(next)
}

// attemptAutoFallback tries to switch to a fallback provider when the active one fails.
func (m *AppState) attemptAutoFallback(origErr error) tea.Cmd {
	if origErr != nil {
		slog.Warn("attempting auto-fallback due to error", "error", origErr)
	}
	if m.registry == nil || m.activeProvider == "" {
		return nil
	}

	// Extract Retry-After header from rate-limit errors (embedded by provider clients).
	retryAfter := ""
	if origErr != nil {
		errMsg := origErr.Error()
		const prefix = "(retry-after: "
		if idx := strings.Index(errMsg, prefix); idx != -1 {
			start := idx + len(prefix)
			if end := strings.Index(errMsg[start:], ")"); end != -1 {
				retryAfter = errMsg[start : start+end]
			}
		}
	}

	result := provider.FindFallbackWithRetryAfter(m.registry, m.activeProvider, retryAfter)
	if result.Err != nil {
		slog.Warn("auto-fallback failed: no healthy fallback provider", "error", result.Err)
		return m.addToastCmd("Auto-fallback failed: no healthy provider available", "error", 5*time.Second)
	}
	if result.Event == nil {
		return nil
	}

	oldProvider := m.activeProvider
	m.activeProvider = result.Event.To
	m.addToast(fmt.Sprintf("Switched to %s (was %s: %s)", result.Event.To, oldProvider, result.Event.Reason), "warning")

	// Sync the new provider to replModel so the next manual chat uses the
	// fallback provider instead of the old (failed) one.
	if m.replModel != nil {
		m.replModel.activeProvider = result.Event.To
	}

	// Update workflow engine if active
	if m.workflowEngine != nil {
		p := m.registry.ActiveProvider()
		if p != nil && m.activeModel != nil {
			m.workflowEngine.SetModel(m.activeModel.ID, p)
		}
	}

	// Emit FallbackEventMsg so the notification system tracks auto-fallback events
	return func() tea.Msg {
		return FallbackEventMsg{
			From:   oldProvider,
			To:     result.Event.To,
			Reason: result.Event.Reason,
		}
	}
}

// reRegisterProvidersFromConfig re-registers OpenRouter and Zen with the current
// API keys from m.config. Called after both SettingsSavedMsg and ConfigSavedMsg.
func (m *AppState) reRegisterProvidersFromConfig() {
	if m.registry == nil || m.config == nil {
		return
	}
	if m.config.Provider.OpenRouter.APIKey != "" {
		if err := RegisterProvider(m.registry, m.config, "openrouter", m.config.Provider.OpenRouter.APIKey, m.version); err != nil {
			slog.Warn("failed to re-register OpenRouter after config save", "error", err)
		}
	}
	if m.config.Provider.Zen.APIKey != "" {
		if err := RegisterProvider(m.registry, m.config, "zen", m.config.Provider.Zen.APIKey, m.version); err != nil {
			slog.Warn("failed to re-register Zen after config save", "error", err)
		}
	}
	if m.config.Provider.Nvidia.APIKey != "" {
		if err := RegisterProvider(m.registry, m.config, "nvidia", m.config.Provider.Nvidia.APIKey, m.version); err != nil {
			slog.Warn("failed to re-register NVIDIA after config save", "error", err)
		}
	}
}

// readAgentCh returns a tea.Cmd that reads the next message from the agent
// loop channel. Used to continue the Bubble Tea cmd chain for agent loop events.
func (m *AppState) readAgentCh() tea.Cmd {
	if m.agentCh == nil {
		return nil
	}
	return func() tea.Msg {
		msg, ok := <-m.agentCh
		if !ok {
			return nil
		}
		return msg
	}
}

// saveAgentSession persists the current REPL messages to the session file
// after an agent loop completes. This ensures the agent's conversation
// (including tool calls and results) survives session resume.
func (m *AppState) saveAgentSession() {
	if m.sessionManager == nil || m.sessionID == "" || m.replModel == nil {
		return
	}
	sess, err := m.sessionManager.LoadSession(m.sessionID)
	if err != nil || sess == nil {
		slog.Warn("saveAgentSession: failed to load session", "error", err)
		return
	}
	sess.Messages = m.replModel.Messages()
	sess.MessageCount = len(sess.Messages)
	if err := m.sessionManager.SaveSession(sess); err != nil {
		slog.Warn("saveAgentSession: failed to save session", "error", err)
	}
}

// extractToolInputSnippet returns a short human-readable description of a tool call's input.
// Extracts the most relevant parameter (path, command, pattern) and truncates to 40 chars.
func extractToolInputSnippet(tc types.ToolCall) string {
	var params map[string]any
	if err := json.Unmarshal(tc.Input, &params); err != nil {
		return ""
	}
	// Priority order for display: path > command > pattern > query > description
	for _, key := range []string{"path", "command", "pattern", "query", "url"} {
		if v, ok := params[key]; ok {
			if s, ok := v.(string); ok && s != "" {
				snippet := strings.ReplaceAll(s, "\n", " ")
				if len(snippet) > 40 {
					snippet = snippet[:37] + "..."
				}
				return snippet
			}
		}
	}
	return ""
}
