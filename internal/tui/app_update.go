package tui

import (
	"errors"
	"fmt"
	"log/slog"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
)

func (m *AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	// CR-04 fix: sync repl context from AppState in Update(), not View().
	// View() must be a pure render function with no state mutations.
	if m.replModel != nil {
		m.replModel.SetKeyRegistry(m.keyRegistry)
		m.replModel.SetLastActivity(m.lastActivity)
	}

	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		m.initialized = true
		// H-1 fix: collect returned tea.Cmd from sub-models and batch them.
		// Previously, all returned cmds were silently discarded, causing
		// streaming freezes on terminal resize.
		var cmds []tea.Cmd
		if m.replModel != nil {
			newCmds, _ := m.replModel.Update(msg)
			cmds = append(cmds, newCmds...)
		}
		if m.firstRunModel != nil {
			newCmds, _ := m.firstRunModel.Update(msg)
			cmds = append(cmds, newCmds...)
		}
		if m.sidebarModel != nil {
			_, cmd := m.sidebarModel.Update(msg)
			if cmd != nil {
				cmds = append(cmds, cmd)
			}
		}
		if m.settingsModel != nil {
			_, cmd := m.settingsModel.Update(msg)
			if cmd != nil {
				cmds = append(cmds, cmd)
			}
		}
		// modelSelector is a value type; only update if initialized
		if m.modelSelector.registry != nil {
			_, cmd := m.modelSelector.Update(msg)
			if cmd != nil {
				cmds = append(cmds, cmd)
			}
		}
		if len(cmds) > 0 {
			return m, tea.Batch(cmds...)
		}
		return m, nil

	case LeaderTimeoutMsg:
		if m.keyRegistry != nil {
			m.keyRegistry.DeactivateLeader()
		}
		return m, nil

	case KeyActionMsg:
		return m.handleKeyAction(msg)

	case tea.KeyMsg:
		// Command palette: when open, route all keys to it
		if m.cmdPaletteOpen && m.cmdPalette != nil {
			switch msg.String() {
			case "esc", "ctrl+c":
				m.cmdPalette.Close()
				m.cmdPaletteOpen = false
				return m, nil
			case "enter":
				if cmd := m.cmdPalette.SelectedCommand(); cmd != nil && cmd.Execute != nil {
					execCmd := cmd.Execute()
					m.cmdPalette.Close()
					m.cmdPaletteOpen = false
					return m, execCmd
				}
				// No command selected, just close
				m.cmdPalette.Close()
				m.cmdPaletteOpen = false
				return m, nil
			}
			m.cmdPalette.Update(msg)
			return m, nil
		}

		// Leader key chord dispatch via KeyRegistry
		if m.keyRegistry != nil && m.screen == ScreenREPL {
			ctx := currentKeyContext(m.screen)
			if handled, cmd := m.keyRegistry.Handle(msg.String(), ctx); handled {
				if cmd != nil {
					return m, cmd
				}
				// Key was consumed (e.g. leader activation), fall through
			}
		}

		// Open command palette with ctrl+p
		if msg.String() == "ctrl+p" {
			if m.cmdPalette != nil && m.screen == ScreenREPL {
				cmds := m.cmdRegistry.AllCommandsWithExecute()
				// Add sidebar toggle command
				cmds = append(cmds, CommandInfo{
					Name:        "Toggle sidebar",
					Description: "Show/hide the file status sidebar",
					Slash:       "",
					Execute: func() tea.Cmd {
						if m.sidebarModel != nil {
							m.sidebarModel.Toggle()
						}
						return nil
					},
				})
				m.cmdPalette.SetCommands(cmds)
				m.cmdPalette.Open()
				m.cmdPaletteOpen = true
				return m, nil
			}
		}

		// Toggle sidebar with ctrl+b
		if msg.String() == "ctrl+b" && m.screen == ScreenREPL {
			if m.sidebarModel != nil {
				m.sidebarModel.Toggle()
				// Refresh git status and adjust REPL layout when sidebar state changes
				if m.sidebarModel.IsVisible() {
					if m.replModel != nil {
						m.replModel.SetSidebarWidth(defaultSidebarWidth)
					}
					return m, m.sidebarModel.refreshCmd()
				} else {
					if m.replModel != nil {
						m.replModel.SetSidebarWidth(0)
					}
				}
			}
			return m, nil
		}

		if m.fallbackNotification != nil && !m.fallbackNotification.Dismissed && msg.String() == "x" {
			m.fallbackNotification.Dismissed = true
			return m, nil
		}
		if m.screen == ScreenPermission && m.permissionModal != nil {
			var resp tools.PermissionResponse
			switch msg.String() {
			case "y", "Y":
				resp = m.permissionModal.Allow()
			case "a", "A":
				resp = m.permissionModal.AllowAlways()
			case "n", "N":
				resp = m.permissionModal.Deny()
			case "ctrl+c":
				resp = m.permissionModal.Deny()
				m.permissionModalActive = false
				m.permissionModal = nil
				m.screen = m.prevScreen
				return m, func() tea.Msg {
					return PermissionResponseMsg{Response: resp}
				}
			case "e", "E":
				return m, tea.Quit
			default:
				return m, nil
			}
			return m, func() tea.Msg {
				return PermissionResponseMsg{Response: resp}
			}
		}
		if msg.String() == "ctrl+c" {
			// If workflow is running, cancel it
			if m.workflowRunning && m.workflowCancel != nil {
				m.workflowCancel()
				m.currentOperation = "Workflow cancelled"
				m.setWorkflowPhase(types.PhaseIdle)
				return m, nil
			}
			// If streaming, cancel stream and stay in app
			if m.screen == ScreenREPL && m.replModel != nil && m.replModel.streaming {
				if m.replModel.streamCancel != nil {
					m.replModel.streamCancel()
				}
				m.replModel.streaming = false
				m.replModel.thinking = false
				m.currentOperation = "Streaming cancelled. Press Ctrl+C again to exit."
				return m, nil
			}
			// Graceful shutdown: save session state before quitting
			if m.sessionManager != nil && m.workflowEngine != nil {
				sessionID := m.workflowEngine.SessionID()
				if sess, err := m.sessionManager.LoadSession(sessionID); err == nil && sess != nil {
					if err := m.sessionManager.SaveSession(sess); err != nil {
						m.currentOperation = "Saving session failed: " + err.Error()
					}
				}
			}
			return m, tea.Quit
		}
	case SlashCommandMsg:
		return m.handleSlashCommand(msg.Command)

	case HealthCheckTickMsg:
		if m.healthCheckInFlight {
			return m, NextHealthTick(types.HealthCheckRetryDelay)
		}
		if m.registry == nil || m.activeProvider == "" {
			return m, NextHealthTick(types.HealthCheckInterval)
		}

		// Skip health check if we're already in a rate-limited state
		// to avoid making things worse
		if m.healthStatus.Status == "offline" ||
			strings.Contains(strings.ToLower(m.healthStatus.Error), "rate limit") ||
			strings.Contains(m.healthStatus.Error, "429") {
			return m, NextHealthTick(120 * time.Second)
		}

		m.healthCheckInFlight = true
		p := m.registry.ActiveProvider()
		if p == nil {
			m.healthCheckInFlight = false
			return m, NextHealthTick(types.HealthCheckInterval)
		}

		// C-1 fix: run health check in a goroutine to avoid blocking Update()
		timeout := time.Duration(m.config.Features.HealthCheckTimeoutSecs) * time.Second
		return m, HealthCheckCmd(p, timeout)

	case HealthCheckResultMsg:
		// C-1 fix: receive async health check result
		m.healthCheckInFlight = false
		m.healthStatus = msg.Result
		m.healthStatusAtomic.Store(msg.Result) // atomic store for safe reads
		m.headerCacheValid = false // H-10: invalidate header cache when health status changes
		m.lastActivity = time.Now()
		return m, NextHealthTick(calculateNextInterval(msg.Result))

	case RefreshCacheMsg:
		if m.registry == nil || m.activeProvider == "" {
			return m, NextCacheRefreshTick(provider.DefaultCacheRefreshInterval)
		}

		providerName := msg.ProviderName
		if providerName == "" {
			providerName = m.activeProvider
		}

		// C-2 fix: run cache refresh in a goroutine to avoid blocking Update()
		return m, CacheRefreshCmd(m.registry, providerName)

	case CacheRefreshResultMsg:
		// C-2 fix: receive async cache refresh result
		if msg.ErrMsg != "" {
			m.currentOperation = msg.ErrMsg
		} else {
			m.currentOperation = ""
			m.lastActivity = time.Now()
		}
		if msg.NextCmd != nil {
			return m, msg.NextCmd
		}
		return m, nil

	case AppMsg:
		return m.handleAppMsg(msg)

	case FallbackEventMsg:
		return m.handleFallbackEvent(msg)

	case ProviderModelsFetchedMsg:
		if m.replModel != nil {
			m.replModel.handleProviderModelsFetched(msg)
		}
		return m, nil

	case StreamChunkMsg:
		if m.workflowRunning && m.currentPhase != types.PhaseDiscuss {
			m.pendingStreamChunks = append(m.pendingStreamChunks, msg.Chunk)
			return m, nil
		}
		if m.replModel != nil {
			m.replModel.AppendStreamChunk(msg.Chunk)
		}
		return m, nil

	case ErrorMsg:
		m.currentOperation = m31errors.UserMessage(msg.Err)
		return m, nil

	case StreamErrorMsg:
		// Check if this is a rate-limit or unavailable error that should trigger fallback
		if m.registry != nil && m.activeProvider != "" && m.config != nil && m.config.Provider.AutoFallback {
			reason := ""
			if errors.Is(msg.Err, m31errors.ErrRateLimited) {
				reason = "rate_limited"
			} else if errors.Is(msg.Err, m31errors.ErrProviderUnreachable) {
				reason = "unavailable"
			} else {
				// Fallback string matching for unwrapped provider errors
				errStr := msg.Err.Error()
				if strings.Contains(errStr, "429") || strings.Contains(strings.ToLower(errStr), "rate limit") {
					reason = "rate_limited"
				} else if strings.Contains(errStr, "503") || strings.Contains(strings.ToLower(errStr), "unavailable") {
					reason = "unavailable"
				}
			}
			if reason != "" {
				_, event, err := provider.FindFallbackProvider(m.registry, m.activeProvider)
				if err == nil && event != nil {
					m.activeProvider = event.To
					wasPaused := m.workflowPaused
					if m.workflowPaused {
						m.workflowPaused = false
					}
					if m.replModel != nil {
						fallbackCmd := m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, m.replModel.sessionID, m.config)
						m.replModel.SetDispatcher(m.dispatcher)
						m.replModel.Update(msg)
						var cmds []tea.Cmd
						cmds = append(cmds, func() tea.Msg {
							return FallbackEventMsg{From: event.From, To: event.To, Reason: reason}
						}, fallbackCmd)
						// Resume paused workflow after provider recovery
						if wasPaused && m.workflowRunning && m.currentPhase != types.PhaseIdle {
							cmds = append(cmds, RunPhaseCmd(m, m.currentPhase, m.workflowGoal))
						}
						return m, tea.Batch(cmds...)
					}
				}
			}
		}
		// Context exceeded stops workflow entirely
		if errors.Is(msg.Err, m31errors.ErrContextExceeded) && m.workflowRunning {
			m.setWorkflowPhase(types.PhaseIdle)
			m.resetDiscussQA()
			m.flushPendingStreamChunks()
			if m.replModel != nil {
				m.replModel.Update(msg)
			}
			return m, nil
		}
		// Pause workflow on stream error if running
		if m.workflowRunning {
			m.workflowPaused = true
		}
		// Pass the error through to the REPL model for display
		if m.replModel != nil {
			m.replModel.Update(msg)
		}
		return m, nil

	case PermissionRequestMsg:
		return m.handlePermissionRequest(msg)

	case PermissionResponseMsg:
		return m.handlePermissionResponse(msg)

	case PermissionTickMsg:
		return m.handlePermissionTick()

	case QuestionRequestMsg:
		return m.handleQuestionRequest(msg)

	case QuestionResponseMsg:
		return m.handleQuestionResponse(msg)

	case DiscussAnswerTimeoutMsg:
		return m.handleDiscussAnswerTimeout()

	case PlanReadyMsg:
		return m.handlePlanReady(msg)

	case workflow.TaskStartMsg:
		return m.handleTaskStart(msg)

	case workflow.TaskUpdateMsg:
		return m.handleTaskUpdate(msg)

	case PhaseResultMsg:
		return m.handlePhaseResult(msg)

	case SettingsSavedMsg:
		return m.handleSettingsSaved()

	case OptimizedMsg:
		// BUG-05 fix: handle arbitrage optimization results
		if len(msg.Recommendations) > 0 && m.planModel != nil {
			m.planModel.ApplyArbitrage(msg.Recommendations)
		}
		m.currentOperation = fmt.Sprintf("Optimized %d tasks", len(msg.Recommendations))
		return m, nil

	case ExecutePauseMsg:
		if m.workflowEngine != nil {
			// Pause/resume is tracked locally on the ExecuteModel.
			// The engine doesn't have Pause/Resume methods in V1,
			// but the UI state is preserved for future use.
			if msg.Paused {
				m.currentOperation = "Execution paused"
			} else {
				m.currentOperation = "Execution resumed"
			}
		}
		return m, nil

	case HealResultMsg:
		if m.verifyModel != nil {
			// Update task status based on heal result
			for i := range m.verifyModel.tasks {
				if m.verifyModel.tasks[i].ID == msg.TaskID {
					if msg.Success {
						m.verifyModel.tasks[i].Status = types.StatusDone
					} else {
						m.verifyModel.tasks[i].Status = types.StatusFailed
					}
					break
				}
			}
		}
		m.currentOperation = fmt.Sprintf("Heal result for task %d: success=%v", msg.TaskID, msg.Success)
		return m, nil

	case SidebarRefreshMsg:
		if m.sidebarModel != nil {
			m.sidebarModel.Update(msg)
			threshold := 120
			if m.config != nil && m.config.UI.SidebarWidthThreshold > 0 {
				threshold = m.config.UI.SidebarWidthThreshold
			}
			if m.width > threshold && m.replModel != nil && !m.sidebarManuallyHidden {
				m.sidebarModel.SetVisible(true)
				m.replModel.SetSidebarWidth(defaultSidebarWidth)
			}
		}
		return m, nil

	case ToastMsg:
		m.toastText = msg.Text
		m.toastExpires = time.Now().Add(msg.Duration)
		m.toastType = msg.Type
		// H-2 fix: schedule toast expiry via tea.Tick so View() stays pure
		return m, tea.Tick(msg.Duration, func(t time.Time) tea.Msg {
			return ToastExpiryMsg{}
		})

	case ToastExpiryMsg:
		// H-2 fix: clear expired toast in Update(), not View()
		if m.toastText != "" && time.Now().After(m.toastExpires) {
			m.toastText = ""
			m.toastType = ""
		}
		return m, nil

	case ThemeChangedMsg:
		return m.handleThemeChanged(msg)
	}

	// Handle diff screen messages at the app level
	switch msg := msg.(type) {
	case DiffScreenMsg:
		m.diffModel = NewDiffModel(m.themeManager.Current(), m.width, m.height)
		m.screen = ScreenDiff
		_, cmd := m.diffModel.Update(msg)
		return m, cmd
	case DiffCloseMsg:
		m.screen = m.prevScreen
		if m.screen == ScreenPermission {
			m.screen = ScreenREPL
		}
		return m, nil
	case config.ConfigReloadMsg:
		if msg.Error != nil {
			slog.Warn("config reload failed", "error", msg.Error)
			return m, nil
		}
		if msg.Config != nil {
			// Detect theme change before updating config
			oldTheme := ""
			if m.config != nil {
				oldTheme = m.config.UI.Theme
			}
			// BUG-12 fix: sync ALL config sections, not just UI/Permissions/Features/Ledger
			m.config.UI = msg.Config.UI
			m.config.Permissions = msg.Config.Permissions
			m.config.Features = msg.Config.Features
			m.config.Ledger = msg.Config.Ledger
			m.config.Provider = msg.Config.Provider
			m.config.Model = msg.Config.Model
			m.config.Agents = msg.Config.Agents
			// Apply theme change if different
			if m.config.UI.Theme != oldTheme && m.config.UI.Theme != "" {
				themeMsg := ThemeChangedMsg{Theme: m.config.UI.Theme}
				return m.handleThemeChanged(themeMsg)
			}
			// Update dispatcher permissions
			if m.dispatcher != nil {
				m.dispatcher.UpdatePermissions(&msg.Config.Permissions)
			}
		}
		return m, nil
	}

	return m.routeToScreen(msg)
}
