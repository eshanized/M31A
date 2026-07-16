package handlers

import (
	"fmt"
	"log/slog"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/internal/types"
)

// ─── navigation.go — navigation and screen-switching event handling ───────────────

// NavigateToScreen switches to the given screen, managing the screen stack
// and updating the sidebar mode as appropriate.
func NavigateToScreen(m *AppState, target Screen) tea.Cmd {
	// Handle special cases before pushing
	switch target {
	case ScreenREPL:
		// If we're already on REPL, just ensure it's focused
		if m.screen == ScreenREPL {
			if m.replModel != nil {
				m.replModel.Focus()
			}
			return nil
		}
	case ScreenConfig:
		if m.screen == ScreenConfig {
			return nil
		}
	case ScreenSessions:
		if m.screen == ScreenSessions {
			return nil
		}
	}

	// Push current screen to stack before navigating
	if m.screen != target && m.screen != ScreenFirstRun {
		m.screenStack = append(m.screenStack, m.screen)
	}

	m.screen = target
	UpdateSidebarModeForScreen(m, target)

	// Return a tick to allow models to initialize
	return tea.Tick(0, func(time.Time) tea.Msg {
		return ScreenEnterMsg{Screen: target}
	})
}

// UpdateSidebarModeForScreen sets the sidebar mode appropriate for the active screen.
func UpdateSidebarModeForScreen(m *AppState, s Screen) {
	if m.SidebarModel == nil {
		return
	}
	switch s {
	case ScreenREPL:
		m.SidebarModel.SetMode(SidebarModeTodo)
	case ScreenPlan, ScreenDiscuss, ScreenExecute, ScreenVerify, ScreenRuntimeCheck, ScreenShip:
		m.SidebarModel.SetMode(SidebarModeTodo)
	case ScreenConfig:
		m.SidebarModel.SetMode(SidebarModeFiles)
	case ScreenSessions:
		m.SidebarModel.SetMode(SidebarModeFiles)
	case ScreenDashboard:
		m.SidebarModel.SetMode(SidebarModeTodo)
	default:
		m.SidebarModel.SetMode(SidebarModeTodo)
	}
}

// HandleScreenEnter processes ScreenEnterMsg: initializes screen-specific models.
func HandleScreenEnter(m *AppState, msg ScreenEnterMsg) []tea.Cmd {
	var cmds []tea.Cmd

	switch msg.Screen {
	case ScreenREPL:
		if m.replModel != nil {
			m.replModel.Focus()
		}
	case ScreenConfig:
		if m.configModel == nil && m.config != nil && m.configPath != "" {
			cw, ch := m.contentDimensions()
			m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, cw, ch, m.keychain)
		}
	case ScreenSessions:
		if m.sessionsModel != nil {
			m.sessionsModel.LoadSessions()
		}
	case ScreenExecute:
		if m.executeModel != nil && m.workflowEngine != nil {
			cmds = append(cmds, m.runWorkflowFromGoal(m.workflowGoal))
		}
	case ScreenPlan:
		if m.planModel != nil && m.workflowEngine != nil {
			cmds = append(cmds, m.runWorkflowFromGoal(m.workflowGoal))
		}
	case ScreenDiscuss:
		if m.discussModel != nil && m.workflowEngine != nil {
			cmds = append(cmds, m.runWorkflowFromGoal(m.workflowGoal))
		}
	case ScreenVerify:
		if m.verifyModel != nil && m.workflowEngine != nil {
			cmds = append(cmds, m.runWorkflowFromGoal(m.workflowGoal))
		}
	case ScreenRuntimeCheck:
		if m.runtimeModel != nil && m.workflowEngine != nil {
			cmds = append(cmds, m.runWorkflowFromGoal(m.workflowGoal))
		}
	case ScreenShip:
		if m.shipModel != nil && m.workflowEngine != nil {
			cmds = append(cmds, m.runWorkflowFromGoal(m.workflowGoal))
		}
	case ScreenFirstRun:
		if m.firstRunModel == nil && m.configPath != "" {
			m.firstRunModel = NewFirstRunModel(m.themeManager.Current(), m.configPath)
		}
	case ScreenSessions:
		if m.sessionsModel != nil {
			m.sessionsModel.LoadSessions()
		}
	}

	return tea.Batch(cmds...)
}

// PopScreen returns to the previous screen.
func PopScreen(m *AppState) tea.Cmd {
	if len(m.screenStack) > 0 {
		prev := m.screenStack[len(m.screenStack)-1]
		m.screenStack = m.screenStack[:len(m.screenStack)-1]
		return NavigateToScreen(m, prev)
	}
	return nil
}

// HandleScreenTransition processes ScreenTransitionMsg: animates transition
// between screens with optional direction.
func HandleScreenTransition(m *AppState, msg ScreenTransitionMsg) tea.Cmd {
	m.transition = &ScreenTransition{
		FromScreen: m.screen,
		ToScreen:   msg.To,
		Direction:  msg.Direction,
		Active:     true,
		Progress:   0.0,
	}
	m.screen = msg.To
	UpdateSidebarModeForScreen(m, msg.To)

	return tea.Tick(transitionTickInterval, func(t time.Time) tea.Msg {
		return TickMsg{Time: t}
	})
}

// ─── Misc handlers (moved from app_handlers_misc.go) ───────────────────────────

// HandleSubagentEvent processes SubagentEventMsg: updates the subagent model,
// surfaces events in the REPL, and cleans up worktrees.
func HandleSubagentEvent(m *AppState, msg SubagentEventMsg) []tea.Cmd {
	var cmds []tea.Cmd

	if m.subagentsModel != nil {
		m.subagentsModel.ApplyEvent(msg.Event)
		if msg.Event.Type == subagent.EventSpawned {
			m.subagentsVisible = true
		}
		if m.SidebarModel != nil {
			total, active := m.subagentsModel.GetStatus()
			m.SidebarModel.UpdateSubAgentStatus(total, active)
		}
	}

	if m.replModel != nil {
		switch msg.Event.Type {
		case subagent.EventSpawned:
			label := msg.Event.Name
			if label == "" {
				label = msg.Event.AgentID
			}
			m.replModel.AddMessage(makeAssistantMsg(
				fmt.Sprintf("**Subagent %s** spawned", label),
			))
		case subagent.EventDone:
			label := msg.Event.Name
			if label == "" {
				label = msg.Event.AgentID
			}
			body := fmt.Sprintf("**Subagent %s done** (%d tools, %d+%d tokens)",
				label, msg.Event.ToolCalls, msg.Event.InputToks, msg.Event.OutputToks)
			if msg.Event.Summary != "" {
				body += "\n\n" + msg.Event.Summary
			}
			m.replModel.AddMessage(makeAssistantMsg(body))
			if m.subagentManager != nil {
				agentID := msg.Event.AgentID
				cmds = append(cmds, func() tea.Msg {
					if err := m.subagentManager.Cleanup(m.shutdownCtx, agentID); err != nil {
						slog.Warn("subagent worktree cleanup failed", "id", agentID, "error", err)
					}
					return nil
				})
			}
		case subagent.EventError:
			label := msg.Event.Name
			if label == "" {
				label = msg.Event.AgentID
			}
			m.replModel.AddMessage(makeAssistantMsg(
				fmt.Sprintf("**Subagent %s errored:** %s", label, msg.Event.Error),
			))
			if m.subagentManager != nil {
				agentID := msg.Event.AgentID
				cmds = append(cmds, func() tea.Msg {
					if err := m.subagentManager.Cleanup(m.shutdownCtx, agentID); err != nil {
						slog.Warn("subagent worktree cleanup failed", "id", agentID, "error", err)
					}
					return nil
				})
			}
		}
	}

	if m.subagentManager != nil {
		cmds = append(cmds, subagentListenerCmd(m.shutdownCtx, m.subagentManager.Events()))
	}

	return cmds
}

// HandleChatHistoryContinue processes ChatHistoryContinueMsg: truncates messages
// to the selected point and persists the truncated session.
func HandleChatHistoryContinue(m *AppState, msg ChatHistoryContinueMsg) []tea.Cmd {
	var cmds []tea.Cmd

	if m.replModel != nil && msg.MessageIndex >= 0 && msg.MessageIndex < len(m.replModel.Messages()) {
		truncated := make([]types.Message, msg.MessageIndex+1)
		copy(truncated, m.replModel.Messages()[:msg.MessageIndex+1])
		m.replModel.SetMessages(truncated)
		if m.sessionManager != nil && m.sessionID != "" {
			sess, err := m.sessionManager.LoadSession(m.sessionID)
			if err == nil {
				sess.Messages = truncated
				sess.MessageCount = len(truncated)
				_ = m.sessionManager.SaveSession(sess)
			}
		}
		cmds = append(cmds, AddToastCmd(m,
			fmt.Sprintf("Continued from message %d — %d messages remaining", msg.MessageIndex+1, len(truncated)),
			"success", 3*time.Second))
		cmds = append(cmds, NavigateToScreen(m, ScreenREPL))
	}

	return cmds
}

// HandlePhaseModelPicked processes PhaseModelPickedMsg: sets planning/coding
// model IDs and resumes the workflow.
func HandlePhaseModelPicked(m *AppState, msg PhaseModelPickedMsg) []tea.Cmd {
	m.planningModelID = msg.PlanningModelID
	m.codingModelID = msg.CodingModelID
	if m.workflowEngine != nil && msg.PlanningModelID != "" {
		m.workflowEngine.SetPhaseModel(types.PhaseDiscuss, msg.PlanningModelID)
		m.workflowEngine.SetPhaseModel(types.PhasePlan, msg.PlanningModelID)
		m.workflowEngine.SetPhaseModel(types.PhaseVerify, msg.PlanningModelID)
	}
	if m.workflowEngine != nil && msg.CodingModelID != "" {
		m.workflowEngine.SetPhaseModel(types.PhaseExecute, msg.CodingModelID)
		m.workflowEngine.SetPhaseModel(types.PhaseShip, msg.CodingModelID)
	}
	m.screen = ScreenREPL
	return []tea.Cmd{m.runWorkflowFromGoal(m.workflowGoal)}
}

// HandleToast processes ToastMsg: adds a toast and schedules its expiry.
func HandleToast(m *AppState, msg ToastMsg) []tea.Cmd {
	id := m.addToast(msg.Text, msg.Type)
	duration := msg.Duration
	if duration <= 0 {
		duration = 3 * time.Second
	}
	for i := len(m.toasts) - 1; i >= 0; i-- {
		if m.toasts[i].ID == id {
			m.toasts[i].Duration = duration
			break
		}
	}
	return []tea.Cmd{tea.Tick(duration, func(time.Time) tea.Msg {
		return ToastExpiryMsg{ToastID: id}
	})}
}

// HandleSidebarRefreshTick processes SidebarRefreshTickMsg and SidebarRefreshMsg.
func HandleSidebarRefreshTick(m *AppState, msg SidebarRefreshTickMsg) []tea.Cmd {
	var cmds []tea.Cmd
	if m.SidebarModel != nil {
		newSidebar, cmd := m.SidebarModel.Update(msg)
		m.SidebarModel = newSidebar
		cmds = append(cmds, cmd)
	}
	if m.fileWatcher != nil {
		cmds = append(cmds, DrainFileWatcherCmd(m))
	}
	return cmds
}

func HandleSidebarRefresh(m *AppState, msg SidebarRefreshMsg) []tea.Cmd {
	var cmds []tea.Cmd
	if m.SidebarModel != nil {
		newSidebar, cmd := m.SidebarModel.Update(msg)
		m.SidebarModel = newSidebar
		cmds = append(cmds, cmd)
	}
	if m.replModel != nil && msg.Branch != "" {
		m.replModel.sidebarBranch = msg.Branch
	}
	if m.replModel != nil {
		m.replModel.SetChangedFiles(len(msg.Files))
	}
	return cmds
}

func HandleSidebarTodoUpdate(m *AppState, msg SidebarTodoUpdateMsg) []tea.Cmd {
	if m.SidebarModel != nil {
		if m.SidebarModel.GetMode() == SidebarModeFiles {
			m.SidebarModel.SetMode(SidebarModeTodo)
		}
		for _, item := range msg.Items {
			m.SidebarModel.AddTodoItem(SidebarTodoItem{
				Content:  item.Content,
				Status:   item.Status,
				Priority: item.Priority,
				Source:   "llm",
			})
		}
	}
	return nil
}

// CheckContextWarnings checks context usage and emits toast warnings at 70% and 85%.
// Called after updateSidebarUsage() when token data is fresh. Returns cmds for toasts.
func CheckContextWarnings(m *AppState) []tea.Cmd {
	if m.SidebarModel == nil {
		return nil
	}
	total := m.SidebarModel.totalTokens
	ctxLen := m.SidebarModel.contextLen
	if ctxLen <= 0 || total <= 0 {
		return nil
	}
	pct := float64(total) / float64(ctxLen)

	var cmds []tea.Cmd

	if pct >= 0.85 && !m.ctxWarned85 {
		m.ctxWarned85 = true
		cmds = append(cmds, AddToastCmd(m,
			fmt.Sprintf("Context at %d%%. Compaction recommended — /compress to free space.", int(pct*100)),
			"warning", 8*time.Second))
	} else if pct >= 0.70 && !m.ctxWarned70 {
		m.ctxWarned70 = true
		cmds = append(cmds, AddToastCmd(m,
			fmt.Sprintf("Context at %d%%. Consider /compress to prevent overflow.", int(pct*100)),
			"info", 6*time.Second))
	}
	return cmds
}

// HandleDecisionsSnapshot updates the cached decisions from the workflow engine.
func HandleDecisionsSnapshot(m *AppState, msg workflow.DecisionsSnapshotMsg) tea.Cmd {
	m.cachedDecisions = msg.Decisions
	return nil
}

// HandleEmitterDropLogTick logs the emitter drop counter if any drops have occurred.
// Re-schedules itself for the next interval.
func HandleEmitterDropLogTick(m *AppState, msg EmitterDropLogTickMsg) tea.Cmd {
	if dropped := DroppedMessages(); dropped > 0 {
		slog.Warn("workflow->TUI channel drops observed", "total_dropped", dropped)
	}
	// Re-schedule the next tick
	return EmitterDropLogTick(m.shutdownCtx)
}

// ─── Provider handlers ────────────────────────────────────────────────────────

// HandleHealthCheckResult processes HealthCheckResultMsg: updates health state
// and displays the result in the REPL.
func HandleHealthCheckResult(m *AppState, msg HealthCheckResultMsg) []tea.Cmd {
	m.healthStatus = msg.Result
	m.lastHealth = time.Now()

	if m.replModel != nil {
		result := msg.Result
		var emoji, status string
		switch result.Status {
		case types.HealthStatusLive:
			emoji = "✓"
			status = "healthy"
		case types.HealthStatusSlow:
			emoji = "⚠"
			status = "slow"
		case types.HealthStatusOffline:
			emoji = "✗"
			status = "offline"
		case types.HealthStatusDegraded:
			emoji = "⚠"
			status = "degraded"
		default:
			emoji = "?"
			status = result.Status
		}
		var text string
		if result.Error != "" {
			text = fmt.Sprintf("%s Health check: %s (%s) — %s", emoji, status, fmt.Sprintf("%dms", result.LatencyMs), result.Error)
		} else {
			text = fmt.Sprintf("%s Health check: %s (%s)", emoji, status, fmt.Sprintf("%dms", result.LatencyMs))
		}
		m.replModel.AddMessage(makeAssistantMsg(text))
	}

	return nil
}

// HandleFallbackEvent processes FallbackEventMsg: switches the active provider
// and persists the change to config.
func HandleFallbackEvent(m *AppState, msg FallbackEventMsg) []tea.Cmd {
	m.activeProvider = msg.To
	slog.Info("provider fallback", "from", msg.From, "to", msg.To, "reason", msg.Reason)
	AddToast(m, fmt.Sprintf("Switched from %s to %s (provider unavailable)", msg.From, msg.To), "warning")

	if m.config != nil {
		m.config.Provider.Default = msg.To
		if m.configPath != "" {
			if err := m.config.SaveWithKeychain(m.configPath, m.keychain); err != nil {
				slog.Warn("failed to save provider to config", "error", err)
			}
		}
	}

	if m.registry != nil && m.activeModel != nil {
		if p, err := m.registry.Get(msg.To); err == nil && p != nil {
			if info, err := p.GetModel(m.activeModel.ID); err == nil && info != nil {
				m.activeModel = info
			}
		}
	}

	return nil
}

// HandleProviderModelsFetched processes ProviderModelsFetchedMsg.
func HandleProviderModelsFetched(m *AppState, msg ProviderModelsFetchedMsg) []tea.Cmd {
	var cmds []tea.Cmd
	if msg.Err != nil {
		slog.Warn("failed to fetch provider models", "error", msg.Err)
		cmds = append(cmds, AddToastCmd(m, "Could not load model catalog: "+fmt.Sprintf("%v", msg.Err), "warning", 5*time.Second))
	}
	if m.replModel != nil {
		m.replModel.handleProviderModelsFetched(msg)
		if m.replModel.activeModel != nil {
			m.activeModel = m.replModel.activeModel
		}
	}
	return cmds
}

// HandleModelSelected processes ModelSelectedMsg: switches the active provider/model
// and syncs with the REPL.
func HandleModelSelected(m *AppState, msg ModelSelectedMsg) []tea.Cmd {
	var cmds []tea.Cmd

	m.activeModel = &msg.Model
	m.activeProvider = msg.Provider
	if m.registry != nil {
		if err := m.registry.SetActive(msg.Provider); err != nil {
			slog.Warn("failed to set active provider", "provider", msg.Provider, "error", err)
			cmds = append(cmds, AddToastCmd(m, "Could not switch to "+msg.Provider+": "+fmt.Sprintf("%v", err), "warning", 5*time.Second))
		}
	}
	if m.replModel != nil {
		m.replModel.activeModel = &msg.Model
		m.replModel.activeProvider = msg.Provider
		providerCmd := m.replModel.SetProvider(m.shutdownCtx, m.registry, msg.Provider, &msg.Model, m.sessionID, m.config)
		cmds = append(cmds, providerCmd)
	}
	cmds = append(cmds, PopScreen(m))

	return cmds
}