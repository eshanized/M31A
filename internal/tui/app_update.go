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
	"github.com/eshanized/M31A/internal/tools/subagent"
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
		// Route sidebar clicks before passing to active screen
		if m.sidebarModel != nil && m.sidebarModel.IsVisible() && m.screen == ScreenREPL {
			sidebarW := m.sidebarModel.GetWidth()
			if sidebarW > 0 && msg.X < sidebarW {
				if c := m.sidebarModel.HandleMouse(msg, 0); c != nil {
					cmds = append(cmds, c)
					return m, tea.Batch(cmds...)
				}
			}
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
		if m.keyRegistry != nil {
			m.keyRegistry.DeactivateLeader()
		}

	// ── Slash command ─────────────────────────────────────────────────────────
	case SlashCommandMsg:
		cmds = append(cmds, m.handleSlashCommand(msg.Command, msg.AttachedFiles))

	// ── Streaming ─────────────────────────────────────────────────────────────
	case StreamMsg:
		if m.replModel != nil {
			cs := m.replModel.handleStreamMsg(msg)
			cmds = append(cmds, cs...)
		}
	case StreamDoneMsg:
		if m.replModel != nil {
			m.replModel.handleStreamDoneMsg(msg)
			m.checkAutoDream()
			// Update sidebar with token usage
			m.updateSidebarUsage()
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
	case AgentStreamMsg:
		if m.replModel != nil && msg.Chunk != nil {
			sm := StreamMsg{
				Chunk:     msg.Chunk,
				ModelID:   m.activeModel.ID,
				SessionID: m.sessionID,
			}
			cs := m.replModel.handleStreamMsg(sm)
			cmds = append(cmds, cs...)
		}
		cmds = append(cmds, m.readAgentCh())
	case AgentThinkingMsg:
		if m.replModel != nil {
			thinkMsg := types.Message{
				Role: "assistant",
				Segments: []types.MessageSegment{{
					Type:      "thinking",
					Content:   fmt.Sprintf("Agent thinking (iteration %d)…", msg.Iteration),
					Visible:   true,
					StartedAt: time.Now(),
				}},
				CreatedAt: time.Now(),
			}
			m.replModel.AddMessage(thinkMsg)
		}
		cmds = append(cmds, m.readAgentCh())
	case AgentToolStartMsg:
		if m.replModel != nil {
			tcJSON, _ := json.Marshal(types.ToolCall{
				ID:    msg.ToolCall.ID,
				Name:  msg.ToolCall.Name,
				Input: msg.ToolCall.Input,
			})
			toolMsg := types.Message{
				Role: "assistant",
				Segments: []types.MessageSegment{{
					Type:    "tool_use",
					Content: string(tcJSON),
					Visible: true,
				}},
				CreatedAt: time.Now(),
			}
			m.replModel.AddMessage(toolMsg)
			m.replModel.TrackLiveTool(msg.ToolCall.Name, len(m.replModel.Messages())-1)
		}
		cmds = append(cmds, m.readAgentCh())
	case AgentToolDoneMsg:
		if m.replModel != nil {
			m.replModel.UpdateLiveTool(msg.ToolCall.Name, msg.Err, msg.DurationMs)
		}
		cmds = append(cmds, m.readAgentCh())
	case AgentIterationDoneMsg:
		if m.replModel != nil {
			m.replModel.streaming = false
			iterContent := fmt.Sprintf("**Iteration %d complete** — executing %d tool(s)…", msg.Iteration, msg.ToolCount)
			iterMsg := StreamDoneMsg{
				Message: types.Message{
					Role:    "assistant",
					Content: iterContent,
					Segments: []types.MessageSegment{{
						Type:    "content",
						Content: iterContent,
						Visible: true,
					}},
				},
				ModelID:   m.activeModel.ID,
				SessionID: m.sessionID,
			}
			m.replModel.handleStreamDoneMsg(iterMsg)
		}
		cmds = append(cmds, m.readAgentCh())
	case AgentIterationMsg:
		if m.replModel != nil {
			var toolNames []string
			for _, tc := range msg.ToolCalls {
				toolNames = append(toolNames, tc.Name)
			}
			iterContent := fmt.Sprintf("**Agent iteration %d** — tools: %s", msg.Iteration, strings.Join(toolNames, ", "))
			iterMsg := types.Message{
				Role: "assistant",
				Segments: []types.MessageSegment{{
					Type:    "content",
					Content: iterContent,
					Visible: true,
				}},
				CreatedAt: time.Now(),
			}
			m.replModel.AddMessage(iterMsg)
		}
		cmds = append(cmds, m.readAgentCh())
	case AgentDoneMsg:
		if m.replModel != nil {
			m.replModel.streaming = false
			doneMsg := StreamDoneMsg{
				Message:   msg.Message,
				Usage:     msg.Usage,
				ModelID:   m.activeModel.ID,
				SessionID: m.sessionID,
			}
			m.replModel.handleStreamDoneMsg(doneMsg)
			m.checkAutoDream()
		}
		m.streamCancelFn = nil
		m.agentCh = nil
		// Persist agent conversation to session
		m.saveAgentSession()
	case AgentErrorMsg:
		if m.replModel != nil {
			m.replModel.streaming = false
			errMsg := StreamErrorMsg{Err: msg.Err, ModelID: m.activeModel.ID, ProviderName: m.activeProvider}
			m.replModel.handleStreamErrorMsg(errMsg)
		}
		m.streamCancelFn = nil
		m.agentCh = nil

	case TickMsg:
		if m.replModel != nil && (m.replModel.streaming || m.replModel.thinking) {
			replM, cmd := m.replModel.Update(msg)
			if r, ok := replM.(*ReplModel); ok {
				m.replModel = r
			}
			cmds = append(cmds, cmd)
		}
		// Forward to execute screen for animated progress bar
		if m.screen == ScreenExecute && m.executeModel != nil {
			execM, cmd := m.executeModel.Update(msg)
			m.executeModel = execM
			cmds = append(cmds, cmd)
		}
		// Forward to phase model picker for spinner animation
		if m.screen == ScreenPhaseModelPicker && m.phaseModelPicker != nil {
			newPMP, cmd := m.phaseModelPicker.Update(msg)
			m.phaseModelPicker = newPMP
			cmds = append(cmds, cmd)
		}
		// Forward to model selector screen for spinner animation
		if m.screen == ScreenModelSelector && m.msModel != nil {
			newMS, cmd := m.msModel.Update(msg)
			if r, ok := newMS.(*ModelSelector); ok {
				m.msModel = r
			}
			cmds = append(cmds, cmd)
		}
		// Forward to verify screen for heal spinner animation
		if m.screen == ScreenVerify && m.verifyModel != nil {
			m.verifyModel.TickSpinner()
		}
		// Forward to plan screen for animations
		if m.screen == ScreenPlan && m.planModel != nil {
			newPlan, cmd := m.planModel.Update(msg)
			m.planModel = newPlan
			cmds = append(cmds, cmd)
		}
		// Forward to ship screen for animations
		if m.screen == ScreenShip && m.shipModel != nil {
			newShip, cmd := m.shipModel.Update(msg)
			m.shipModel = newShip
			cmds = append(cmds, cmd)
		}
		// Forward to theme picker for animations
		if m.screen == ScreenThemePicker && m.themePickerModel != nil {
			newTP, cmd := m.themePickerModel.Update(msg)
			if nt, ok := newTP.(*ThemePickerModel); ok {
				m.themePickerModel = nt
			}
			cmds = append(cmds, cmd)
		}
		// Forward to dashboard for animations
		if m.screen == ScreenDashboard && m.dashboardModel != nil {
			newDash, cmd := m.dashboardModel.Update(msg)
			if nd, ok := newDash.(*DashboardModel); ok {
				m.dashboardModel = nd
			}
			cmds = append(cmds, cmd)
		}
		// Screen transition tick
		if m.transition != nil && m.transition.Active {
			if m.transition.TransitionTick() {
				// Transition complete — switch to target screen
				// Sub-model was already created in navigateToScreen → ensureSubModel
				m.screen = m.transition.ToScreen
				m.transition = nil
			} else {
				// Keep ticking
				cmds = append(cmds, StreamTickCmd())
			}
		}

	// ── Health ────────────────────────────────────────────────────────────────
	case HealthCheckTickMsg:
		if m.activeProvider != "" && m.registry != nil {
			if p, err := m.registry.Get(m.activeProvider); err == nil {
				cmds = append(cmds, HealthCheckCmd(m.shutdownCtx, p, 10*time.Second))
			}
		}
		cmds = append(cmds, NextHealthTick(m.shutdownCtx, types.HealthCheckInterval))

	case HealthCheckResultMsg:
		m.healthStatus = msg.Result
		m.lastHealth = time.Now()

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
		if m.executeModel != nil {
			found := false
			for i, t := range m.executeModel.tasks {
				if t.ID == msg.Task.ID {
					m.executeModel.SetCurrentTask(i)
					m.executeModel.UpdateTaskStatus(msg.Task.ID, types.StatusRunning)
					found = true
					break
				}
			}
			if !found {
				// Task was auto-generated (e.g. Fast/Direct mode without Plan).
				// Append it to the model's task list so progress tracking works.
				m.executeModel.tasks = append(m.executeModel.tasks, msg.Task)
				m.executeModel.SetCurrentTask(len(m.executeModel.tasks) - 1)
				m.executeModel.UpdateTaskStatus(msg.Task.ID, types.StatusRunning)
			}
		}
		cmds = append(cmds, m.drainEmitterCmd())
	case workflow.TaskUpdateMsg:
		if m.executeModel != nil {
			var status types.TaskStatus
			switch msg.Status {
			case "done":
				status = types.StatusDone
			case "failed":
				status = types.StatusFailed
			default:
				status = types.StatusRunning
			}
			m.executeModel.UpdateTaskStatus(msg.Task.ID, status)
		}
		cmds = append(cmds, m.drainEmitterCmd())
	case workflow.ToolStartMsg:
		if m.executeModel != nil {
			detail := msg.Description
			if detail == "" {
				detail = msg.ToolName
			}
			m.executeModel.AppendLiveOutput([]string{
				fmt.Sprintf("→ %s: %s", msg.ToolName, detail),
			})
		}
		cmds = append(cmds, m.drainEmitterCmd())
	case workflow.ToolCompleteMsg:
		if m.executeModel != nil {
			status := "ok"
			if !msg.Success {
				status = "failed"
			}
			m.executeModel.AppendLiveOutput([]string{
				fmt.Sprintf("  %s %s (%dms)", status, msg.ToolName, msg.DurationMs),
			})
		}
		cmds = append(cmds, m.drainEmitterCmd())
	case workflow.SelfHealStartMsg:
		if m.executeModel != nil {
			m.executeModel.AppendLiveOutput([]string{
				fmt.Sprintf("  [warn] Self-heal attempt %d/%d for task %d", msg.Attempt, msg.Max, msg.TaskID),
			})
		}
		if m.verifyModel != nil {
			m.verifyModel.StartHealing(msg.TaskID, msg.Attempt)
		}
		cmds = append(cmds, m.drainEmitterCmd())
	case workflow.SelfHealCompleteMsg:
		if m.executeModel != nil {
			status := "ok"
			if !msg.Success {
				status = "failed"
			}
			m.executeModel.AppendLiveOutput([]string{
				fmt.Sprintf("  Self-heal %s (attempt %d/%d)", status, msg.Attempt, msg.Max),
			})
		}
		if m.verifyModel != nil {
			m.verifyModel.StopHealing()
		}
		cmds = append(cmds, m.drainEmitterCmd())
	case workflow.PhaseTransitionStartMsg, workflow.PhaseTransitionCompleteMsg,
		workflow.IntermediateProgressMsg,
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
		m.planningModelID = msg.PlanningModelID
		m.codingModelID = msg.CodingModelID
		// Inject per-phase model overrides into the workflow engine.
		if m.workflowEngine != nil && msg.PlanningModelID != "" {
			m.workflowEngine.SetPhaseModel(types.PhaseDiscuss, msg.PlanningModelID)
			m.workflowEngine.SetPhaseModel(types.PhasePlan, msg.PlanningModelID)
			m.workflowEngine.SetPhaseModel(types.PhaseVerify, msg.PlanningModelID)
		}
		if m.workflowEngine != nil && msg.CodingModelID != "" {
			m.workflowEngine.SetPhaseModel(types.PhaseExecute, msg.CodingModelID)
			m.workflowEngine.SetPhaseModel(types.PhaseShip, msg.CodingModelID)
		}
		// Resume the workflow from the REPL screen.
		m.screen = ScreenREPL
		cmds = append(cmds, m.runWorkflowFromGoal(m.workflowGoal))

	// ── Toast ─────────────────────────────────────────────────────────────────
	case ToastMsg:
		id := m.addToast(msg.Text, msg.Type)
		duration := msg.Duration
		if duration <= 0 {
			duration = 3 * time.Second
		}
		cmds = append(cmds, tea.Tick(duration, func(time.Time) tea.Msg {
			return ToastExpiryMsg{ToastID: id}
		}))

	case ToastExpiryMsg:
		m.removeToastByID(msg.ToastID)

	case DismissToastMsg:
		m.removeToastByID(msg.ToastID)

	// ── First-run wizard complete ─────────────────────────────────────────────
	case FirstRunCompleteMsg:
		cmds = append(cmds, m.handleFirstRunComplete(msg))

	// ── Settings saved ────────────────────────────────────────────────────────
	case SettingsSavedMsg:
		if m.configModel != nil {
			m.configModel.buildContent()
		} else {
			cw, ch := m.contentDimensions()
			m.configModel = NewConfigModel(m.themeManager.Current(), m.config, m.configPath, cw, ch, m.keychain)
		}
		// Re-register providers with updated API keys from settings
		m.reRegisterProvidersFromConfig()
		cmds = append(cmds, m.popScreen())

	// ── Config editor saved (stays on ScreenConfig) ────────────────────────
	case ConfigSavedMsg:
		// Re-register providers so new API keys take effect immediately
		m.reRegisterProvidersFromConfig()
		if m.configModel != nil {
			m.configModel.buildContent()
		}
		if m.dispatcher != nil && m.config != nil {
			m.dispatcher.UpdatePermissions(&m.config.Permissions)
		}
		cmds = append(cmds, m.addToastCmd("Config saved to disk", "success", 3*time.Second))

	// ── Config hot-reload (file changed on disk) ────────────────────────────
	case config.ConfigReloadMsg:
		if msg.Error != nil {
			slog.Warn("config reload failed", "error", msg.Error)
			cmds = append(cmds, m.addToastCmd("Config reload failed: "+m31errors.UserMessage(msg.Error), "error", 5*time.Second))
			break
		}
		if msg.Config != nil {
			m.config = msg.Config
			m.reRegisterProvidersFromConfig()
			if m.dispatcher != nil {
				m.dispatcher.UpdatePermissions(&m.config.Permissions)
			}
			if m.configModel != nil {
				m.configModel.cfg = m.config
				m.configModel.buildContent()
			}
			cmds = append(cmds, m.addToastCmd("Config reloaded from disk", "info", 3*time.Second))
		}

	// ── Theme changed ─────────────────────────────────────────────────────────
	case ThemeChangedMsg:
		m.applyTheme(msg.Theme)

	// ── Settings theme change (from settings screen) ────────────────────────
	case settingsThemeChanged:
		m.applyTheme(msg.ThemeName)

	// ── Fallback event ────────────────────────────────────────────────────────
	case FallbackEventMsg:
		m.activeProvider = msg.To
		slog.Info("provider fallback", "from", msg.From, "to", msg.To, "reason", msg.Reason)
		m.addToast(fmt.Sprintf("Switched from %s to %s (provider unavailable)", msg.From, msg.To), "warning")
		// Re-validate the active model against the new provider
		if m.registry != nil && m.activeModel != nil {
			if p, err := m.registry.Get(msg.To); err == nil && p != nil {
				if info, err := p.GetModel(m.activeModel.ID); err == nil && info != nil {
					m.activeModel = info
				}
			}
		}

	// ── Bisect start with commit range ─────────────────────────────────────
	case BisectStartMsg:
		if m.bisectModel != nil && m.git != nil {
			commits, err := m.git.Log(0) // all commits
			if err == nil && len(commits) > 0 {
				var bisectCommits []bisectCommit
				goodIdx := -1
				badIdx := -1
				for i, c := range commits {
					hash := c.Hash
					if len(hash) > 7 {
						hash = hash[:7]
					}
					status := "pending"
					if msg.GoodCommit != "" && (c.Hash == msg.GoodCommit || c.ShortHash == msg.GoodCommit || hash == msg.GoodCommit) {
						status = "good"
						goodIdx = i
					}
					if msg.BadCommit != "" && (c.Hash == msg.BadCommit || c.ShortHash == msg.BadCommit || hash == msg.BadCommit) {
						status = "bad"
						badIdx = i
					}
					bisectCommits = append(bisectCommits, bisectCommit{
						Hash:    c.Hash,
						Message: c.Message,
						Status:  status,
					})
				}
				// If we found good and bad, keep only the range between them
				if goodIdx >= 0 && badIdx >= 0 && goodIdx != badIdx {
					start, end := goodIdx, badIdx
					if start > end {
						start, end = end, start
					}
					bisectCommits = bisectCommits[start : end+1]
				}
				if len(bisectCommits) > 0 {
					m.bisectModel.SetCommits(bisectCommits)
				}
			}
		}

	// ── Session detail request ─────────────────────────────────────────────
	case SessionDetailRequestMsg:
		if m.sessionDetailModel != nil && msg.Session != nil {
			m.sessionDetailModel.SetSession(msg.Session)
		}

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
		m.activeModel = &msg.Model
		m.activeProvider = msg.Provider
		if m.replModel != nil {
			// Immediately sync the model to replModel to avoid split-brain
			// between AppState.activeModel and replModel.activeModel.
			m.replModel.activeModel = &msg.Model
			m.replModel.activeProvider = msg.Provider
			providerCmd := m.replModel.SetProvider(m.shutdownCtx, m.registry, msg.Provider, &msg.Model, m.sessionID, m.config)
			cmds = append(cmds, providerCmd)
		}
		cmds = append(cmds, m.popScreen())

	// ── Sidebar tick (periodic or file-watcher triggered) ──────────────────────
	case SidebarRefreshTickMsg:
		if m.sidebarModel != nil {
			newSidebar, cmd := m.sidebarModel.Update(msg)
			m.sidebarModel = newSidebar
			cmds = append(cmds, cmd)
		}
		// Re-register file watcher listener for next event
		if m.fileWatcher != nil {
			cmds = append(cmds, m.drainFileWatcherCmd())
		}

	// ── Sidebar refresh ───────────────────────────────────────────────────────
	case SidebarRefreshMsg:
		if m.sidebarModel != nil {
			newSidebar, cmd := m.sidebarModel.Update(msg)
			m.sidebarModel = newSidebar
			cmds = append(cmds, cmd)
		}
		// Propagate git branch to REPL for status bar display
		if m.replModel != nil && msg.Branch != "" {
			m.replModel.sidebarBranch = msg.Branch
		}
		// Propagate changed-file count so the welcome screen project card is accurate
		if m.replModel != nil {
			m.replModel.SetChangedFiles(len(msg.Files))
		}

	// ── Subagent events ───────────────────────────────────────────────────────
	case SubagentEventMsg:
		if m.subagentsModel != nil {
			m.subagentsModel.ApplyEvent(msg.Event)
			// Auto-show the panel on first activity so the user sees progress.
			if msg.Event.Type == subagent.EventSpawned {
				m.subagentsVisible = true
			}
		}
		// Surface terminal events in the REPL so the parent conversation
		// has access to subagent summaries without opening the panel.
		if m.replModel != nil {
			switch msg.Event.Type {
			case subagent.EventDone:
				if msg.Event.Summary != "" {
					label := msg.Event.Name
					if label == "" {
						label = msg.Event.AgentID
					}
					m.replModel.AddMessage(makeAssistantMsg(
						fmt.Sprintf("**Subagent %s done** (%d tools, %d+%d tokens)\n\n%s",
							label, msg.Event.ToolCalls, msg.Event.InputToks, msg.Event.OutputToks,
							msg.Event.Summary),
					))
				}
			case subagent.EventError:
				label := msg.Event.Name
				if label == "" {
					label = msg.Event.AgentID
				}
				m.replModel.AddMessage(makeAssistantMsg(
					fmt.Sprintf("**Subagent %s errored:** %s", label, msg.Event.Error),
				))
			}
		}
		// Re-register the listener so the next event is delivered.
		if m.subagentManager != nil {
			cmds = append(cmds, subagentListenerCmd(m.shutdownCtx, m.subagentManager.Events()))
		}

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
			m.resumeModel = rm
		} else {
			m.resumeModel.Refresh(msg.sessions)
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
		if m.replModel != nil {
			m.replModel.handleProviderModelsFetched(msg)
			// Sync enriched model info back to AppState so sendChatMessage
			// uses the fully-populated ModelInfo (pricing, context, capabilities).
			if m.replModel.activeModel != nil {
				m.activeModel = m.replModel.activeModel
			}
		}

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
		switch m.screen {
		case ScreenModelSelector:
			if m.msModel != nil {
				newMs, cmd := m.msModel.Update(msg)
				if nm, ok := newMs.(*ModelSelector); ok {
					m.msModel = nm
				}
				cmds = append(cmds, cmd)
			}
		case ScreenSettings:
			if m.settingsModel != nil {
				newSettings, cmd := m.settingsModel.Update(msg)
				m.settingsModel = newSettings
				cmds = append(cmds, cmd)
			}
		case ScreenResume:
			if m.resumeModel != nil {
				newResume, cmd := m.resumeModel.Update(msg)
				if nr, ok := newResume.(*ResumeModel); ok {
					m.resumeModel = nr
				}
				cmds = append(cmds, cmd)
			}
		case ScreenDiscuss:
			if m.discussModel != nil {
				newDiscuss, cmd := m.discussModel.Update(msg)
				if nd, ok := newDiscuss.(*DiscussModel); ok {
					m.discussModel = nd
				}
				cmds = append(cmds, cmd)
			}
		case ScreenMetrics:
			if m.metricsModel != nil {
				newMetrics, cmd := m.metricsModel.Update(msg)
				if nm, ok := newMetrics.(*MetricsModel); ok {
					m.metricsModel = nm
				}
				cmds = append(cmds, cmd)
			}
		case ScreenGoalInput:
			if m.goalInput != nil {
				newGoal, cmd := m.goalInput.Update(msg)
				if ng, ok := newGoal.(*GoalInputModel); ok {
					m.goalInput = ng
				}
				cmds = append(cmds, cmd)
			}
		case ScreenLedger:
			if m.ledgerModel != nil {
				newLedger, cmd := m.ledgerModel.Update(msg)
				if nl, ok := newLedger.(*LedgerModel); ok {
					m.ledgerModel = nl
				}
				cmds = append(cmds, cmd)
			}
		case ScreenRollback:
			if m.rollbackModel != nil {
				newRB, cmd := m.rollbackModel.Update(msg)
				if nr, ok := newRB.(*RollbackModel); ok {
					m.rollbackModel = nr
				}
				cmds = append(cmds, cmd)
			}
		case ScreenConfig:
			if m.configModel != nil {
				newCfg, cmd := m.configModel.Update(msg)
				m.configModel = newCfg
				cmds = append(cmds, cmd)
			}
		case ScreenDiff:
			if m.diffModel != nil {
				newDiff, cmd := m.diffModel.Update(msg)
				if nd, ok := newDiff.(*DiffModel); ok {
					m.diffModel = nd
				}
				cmds = append(cmds, cmd)
			}
		case ScreenShip:
			if m.shipModel != nil {
				newShip, cmd := m.shipModel.Update(msg)
				m.shipModel = newShip
				cmds = append(cmds, cmd)
			}
		case ScreenPlan:
			if m.planModel != nil {
				newPlan, cmd := m.planModel.Update(msg)
				m.planModel = newPlan
				cmds = append(cmds, cmd)
			}
		case ScreenExecute:
			if m.executeModel != nil {
				newExec, cmd := m.executeModel.Update(msg)
				m.executeModel = newExec
				cmds = append(cmds, cmd)
			}
		case ScreenVerify:
			if m.verifyModel != nil {
				newVerify, cmd := m.verifyModel.Update(msg)
				m.verifyModel = newVerify
				cmds = append(cmds, cmd)
			}
		case ScreenHelp:
			if m.helpModel != nil {
				newHelp, cmd := m.helpModel.Update(msg)
				if nh, ok := newHelp.(*HelpModel); ok {
					m.helpModel = nh
				}
				cmds = append(cmds, cmd)
			}
		case ScreenBisect:
			if m.bisectModel != nil {
				newBisect, cmd := m.bisectModel.Update(msg)
				if nb, ok := newBisect.(*BisectModel); ok {
					m.bisectModel = nb
				}
				cmds = append(cmds, cmd)
			}
		case ScreenThemePicker:
			if m.themePickerModel != nil {
				newTP, cmd := m.themePickerModel.Update(msg)
				if nt, ok := newTP.(*ThemePickerModel); ok {
					m.themePickerModel = nt
				}
				cmds = append(cmds, cmd)
			}
		case ScreenNotifications:
			if m.notifModel != nil {
				newNotif, cmd := m.notifModel.Update(msg)
				if nn, ok := newNotif.(*NotificationModel); ok {
					m.notifModel = nn
				}
				cmds = append(cmds, cmd)
			}
		case ScreenDashboard:
			if m.dashboardModel != nil {
				newDash, cmd := m.dashboardModel.Update(msg)
				if nd, ok := newDash.(*DashboardModel); ok {
					m.dashboardModel = nd
				}
				cmds = append(cmds, cmd)
			}
		case ScreenSessionDetail:
			if m.sessionDetailModel != nil {
				newSD, cmd := m.sessionDetailModel.Update(msg)
				if ns, ok := newSD.(*SessionDetailModel); ok {
					m.sessionDetailModel = ns
				}
				cmds = append(cmds, cmd)
			}
		case ScreenFileExplorer:
			if m.fileExplorerModel != nil {
				newFE, cmd := m.fileExplorerModel.Update(msg)
				if nf, ok := newFE.(*FileExplorerModel); ok {
					m.fileExplorerModel = nf
				}
				cmds = append(cmds, cmd)
			}
		case ScreenPhaseModelPicker:
			if m.phaseModelPicker != nil {
				newPMP, cmd := m.phaseModelPicker.Update(msg)
				m.phaseModelPicker = newPMP
				cmds = append(cmds, cmd)
			}
		case ScreenToolDetail:
			if m.toolDetailModel != nil {
				newTD, cmd := m.toolDetailModel.Update(msg)
				if nt, ok := newTD.(*ToolDetailModel); ok {
					m.toolDetailModel = nt
				}
				cmds = append(cmds, cmd)
			}
		case ScreenFirstRun:
			if m.firstRunModel != nil {
				newFR, cmd := m.firstRunModel.Update(msg)
				if nfr, ok := newFR.(*FirstRunModel); ok {
					m.firstRunModel = nfr
				}
				cmds = append(cmds, cmd)
			}
		}
	}

	return m, tea.Batch(cmds...)
}

// ─── Routing helpers ──────────────────────────────────────────────────────────

// routeToScreen returns the initialization cmd for the current screen.
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
	case ScreenThemePicker:
		if m.themePickerModel == nil {
			cw, ch := m.contentDimensions()
			m.themePickerModel = NewThemePickerModel(m.themeManager.Current(), cw, ch)
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
		m.diffModel.width = contentW
		m.diffModel.height = contentH
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
	if m.themePickerModel != nil {
		m.themePickerModel.SetDimensions(contentW, contentH)
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

	switch m.screen {
	case ScreenREPL:
		if m.replModel != nil {
			newRepl, cmd := m.replModel.Update(msg)
			if r, ok := newRepl.(*ReplModel); ok {
				m.replModel = r
			}
			return cmd
		}
	case ScreenPermission:
		return m.handlePermissionKey(msg)
	case ScreenModelSelector:
		if m.msModel != nil {
			newMs, cmd := m.msModel.Update(msg)
			if nm, ok := newMs.(*ModelSelector); ok {
				m.msModel = nm
			}
			return cmd
		}
	case ScreenSettings:
		if m.settingsModel != nil {
			newSettings, cmd := m.settingsModel.Update(msg)
			m.settingsModel = newSettings
			return cmd
		}
	case ScreenResume:
		if m.resumeModel != nil {
			newResume, cmd := m.resumeModel.Update(msg)
			if nr, ok := newResume.(*ResumeModel); ok {
				m.resumeModel = nr
			}
			return cmd
		}
	case ScreenPlan:
		if m.planModel != nil {
			newPlan, cmd := m.planModel.Update(msg)
			m.planModel = newPlan
			return cmd
		}
	case ScreenExecute:
		if m.executeModel != nil {
			newExec, cmd := m.executeModel.Update(msg)
			m.executeModel = newExec
			return cmd
		}
	case ScreenVerify:
		if m.verifyModel != nil {
			newVerify, cmd := m.verifyModel.Update(msg)
			m.verifyModel = newVerify
			return cmd
		}
	case ScreenShip:
		if m.shipModel != nil {
			newShip, cmd := m.shipModel.Update(msg)
			m.shipModel = newShip
			return cmd
		}
	case ScreenDiscuss:
		if m.discussModel != nil {
			newDiscuss, cmd := m.discussModel.Update(msg)
			if nd, ok := newDiscuss.(*DiscussModel); ok {
				m.discussModel = nd
			}
			return cmd
		}
	case ScreenDiff:
		if m.diffModel != nil {
			newDiff, cmd := m.diffModel.Update(msg)
			if nd, ok := newDiff.(*DiffModel); ok {
				m.diffModel = nd
			}
			return cmd
		}
	case ScreenGoalInput:
		if m.goalInput != nil {
			newGoal, cmd := m.goalInput.Update(msg)
			if ng, ok := newGoal.(*GoalInputModel); ok {
				m.goalInput = ng
			}
			return cmd
		}
	case ScreenFirstRun:
		if m.firstRunModel != nil {
			newFR, cmd := m.firstRunModel.Update(msg)
			if nfr, ok := newFR.(*FirstRunModel); ok {
				m.firstRunModel = nfr
			}
			return cmd
		}
	case ScreenLedger:
		if m.ledgerModel != nil {
			newLedger, cmd := m.ledgerModel.Update(msg)
			if nl, ok := newLedger.(*LedgerModel); ok {
				m.ledgerModel = nl
			}
			return cmd
		}
	case ScreenRollback:
		if m.rollbackModel != nil {
			newRB, cmd := m.rollbackModel.Update(msg)
			if nr, ok := newRB.(*RollbackModel); ok {
				m.rollbackModel = nr
			}
			return cmd
		}
	case ScreenConfig:
		if m.configModel != nil {
			newCfg, cmd := m.configModel.Update(msg)
			m.configModel = newCfg
			return cmd
		}
	case ScreenMetrics:
		if m.metricsModel != nil {
			newMetrics, cmd := m.metricsModel.Update(msg)
			if nm, ok := newMetrics.(*MetricsModel); ok {
				m.metricsModel = nm
			}
			return cmd
		}
	case ScreenHelp:
		if m.helpModel != nil {
			newHelp, cmd := m.helpModel.Update(msg)
			if nh, ok := newHelp.(*HelpModel); ok {
				m.helpModel = nh
			}
			return cmd
		}
	case ScreenBisect:
		if m.bisectModel != nil {
			newBisect, cmd := m.bisectModel.Update(msg)
			if nb, ok := newBisect.(*BisectModel); ok {
				m.bisectModel = nb
			}
			return cmd
		}
	case ScreenThemePicker:
		if m.themePickerModel != nil {
			newTP, cmd := m.themePickerModel.Update(msg)
			if nt, ok := newTP.(*ThemePickerModel); ok {
				m.themePickerModel = nt
			}
			return cmd
		}
	case ScreenNotifications:
		if m.notifModel != nil {
			newNotif, cmd := m.notifModel.Update(msg)
			if nn, ok := newNotif.(*NotificationModel); ok {
				m.notifModel = nn
			}
			return cmd
		}
	case ScreenDashboard:
		if m.dashboardModel != nil {
			newDash, cmd := m.dashboardModel.Update(msg)
			if nd, ok := newDash.(*DashboardModel); ok {
				m.dashboardModel = nd
			}
			return cmd
		}
	case ScreenSessionDetail:
		if m.sessionDetailModel != nil {
			newSD, cmd := m.sessionDetailModel.Update(msg)
			if ns, ok := newSD.(*SessionDetailModel); ok {
				m.sessionDetailModel = ns
			}
			return cmd
		}
	case ScreenFileExplorer:
		if m.fileExplorerModel != nil {
			newFE, cmd := m.fileExplorerModel.Update(msg)
			if nf, ok := newFE.(*FileExplorerModel); ok {
				m.fileExplorerModel = nf
			}
			return cmd
		}
	case ScreenToolDetail:
		if m.toolDetailModel != nil {
			newTD, cmd := m.toolDetailModel.Update(msg)
			if nt, ok := newTD.(*ToolDetailModel); ok {
				m.toolDetailModel = nt
			}
			return cmd
		}
	case ScreenGhostPicker:
		if m.ghostPickerModel != nil {
			newGP, cmd := m.ghostPickerModel.Update(msg)
			if ngp, ok := newGP.(*GhostPickerModel); ok {
				m.ghostPickerModel = ngp
			}
			return cmd
		}
	case ScreenGhostOutput:
		if m.ghostOutputModel != nil {
			newGO, cmd := m.ghostOutputModel.Update(msg)
			if ngo, ok := newGO.(*GhostOutputModel); ok {
				m.ghostOutputModel = ngo
			}
			return cmd
		}
	case ScreenConfirmQuit:
		if m.confirmQuitModel != nil {
			newCQ, cmd := m.confirmQuitModel.Update(msg)
			if ncq, ok := newCQ.(*ConfirmQuitModel); ok {
				m.confirmQuitModel = ncq
			}
			return cmd
		}
	case ScreenPhaseModelPicker:
		if m.phaseModelPicker != nil {
			newPM, cmd := m.phaseModelPicker.Update(msg)
			m.phaseModelPicker = newPM
			return cmd
		}
	}
	return nil
}

// handleAppMsg handles AppMsg screen transitions.
func (m *AppState) handleAppMsg(msg AppMsg) tea.Cmd {
	if msg.ModelSelected != nil {
		m.activeModel = &msg.ModelSelected.Model
		m.activeProvider = msg.ModelSelected.Provider
		// Return to the previous screen instead of hardcoding REPL
		if len(m.screenStack) > 0 {
			prev := m.screenStack[len(m.screenStack)-1]
			m.screenStack = m.screenStack[:len(m.screenStack)-1]
			m.screen = prev
		} else {
			m.screen = ScreenREPL
		}
		return nil
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
		if m.cmdPalette == nil {
			m.cmdPalette = NewCommandPalette(m.cmdRegistry, m.themeManager.Current())
			cw, ch := m.contentDimensions()
			m.cmdPalette.SetDimensions(cw, ch)
		}
		m.cmdPalette.Open()
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
	case "toggle_theme":
		newMode := m.themeManager.Cycle()
		themeName := "dark"
		switch newMode {
		case theme.ModeLight:
			themeName = "light"
		case theme.ModeAuto:
			themeName = "auto"
		}
		m.applyTheme(themeName)
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

	// Push current screen to back-stack for esc-to-go-back navigation
	if m.screen != screen && m.screen != ScreenPermission && screen != ScreenPermission {
		m.screenStack = append(m.screenStack, m.screen)
		// Cap the stack to prevent unbounded growth on repeated push/pop cycles
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
	case ScreenThemePicker:
		if m.themePickerModel == nil {
			m.themePickerModel = NewThemePickerModel(m.themeManager.Current(), cw, ch)
		} else {
			m.themePickerModel.SetDimensions(cw, ch)
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
	default:
		return nil
	}
}

// handleKeyAction processes KeyActionMsg strings.
func (m *AppState) handleKeyAction(action string) tea.Cmd {
	switch action {
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
	case "open_themes":
		return m.navigateToScreen(ScreenThemePicker)
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
		if m.cmdPalette == nil {
			m.cmdPalette = NewCommandPalette(m.cmdRegistry, m.themeManager.Current())
			cw, ch := m.contentDimensions()
			m.cmdPalette.SetDimensions(cw, ch)
		}
		m.cmdPalette.Open()
		return nil
	case "cycle_model", "cycle_model_forward":
		return m.navigateToScreen(ScreenModelSelector)
	case "cycle_model_backward":
		return m.navigateToScreen(ScreenModelSelector)
	case "toggle_theme":
		newMode := m.themeManager.Cycle()
		themeName := "dark"
		switch newMode {
		case theme.ModeLight:
			themeName = "light"
		case theme.ModeAuto:
			themeName = "auto"
		}
		m.applyTheme(themeName)
		return nil
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
	if m.themePickerModel != nil {
		m.themePickerModel.SetTheme(t)
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
	m.screen = ScreenREPL

	return m.syncReplProvider(sess.ID)
}

// openResumeScreen loads session list and navigates to the resume screen.
func (m *AppState) openResumeScreen() tea.Cmd {
	return func() tea.Msg {
		var sessions []session.SessionInfo
		if m.sessionManager != nil {
			infos, err := m.sessionManager.ListSessions()
			if err == nil {
				for i, info := range infos {
					if i >= 20 {
						break
					}
					sessions = append(sessions, info)
				}
			}
		}
		return resumeScreenReadyMsg{sessions: sessions}
	}
}

// resumeScreenReadyMsg carries loaded sessions for the resume screen.
type resumeScreenReadyMsg struct {
	sessions []session.SessionInfo
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
	cmds = append(cmds, m.RunPhaseCmd(startPhase))
	return tea.Batch(cmds...)
}

// resolveWorkflowMode determines the appropriate workflow mode.
// If the user has set an explicit mode via config, that takes precedence.
// Otherwise, the goal is classified automatically.
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
			m.executeModel = NewExecuteModel(tasks, m.themeManager.Current(), m.width, m.height)
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

	result := provider.FindFallbackWithRetryAfter(m.registry, m.activeProvider, "")
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
