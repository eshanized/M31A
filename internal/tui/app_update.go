package tui

import (
	"context"
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
						m.replModel.SetSidebarWidth(sidebarWidth)
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
		// Process slash commands from the REPL
		if m.screen != ScreenREPL {
			return m, nil
		}

		cmd := msg.Command

		// Handle TUI-specific commands first
		switch cmd {
		case "/settings":
			m.screen = ScreenSettings
			return m, nil
		case "/resume":
			if m.resumeModel != nil {
				m.resumeModel.Refresh()
			}
			m.screen = ScreenResume
			return m, nil
		case "/models":
			if m.registry != nil {
				m.prevScreen = m.screen
				m.modelSelector = NewModelSelector(m.registry, m.sessionManager, m.themeManager.Current())
				m.screen = ScreenModelSelector
				return m, m.modelSelector.Init()
			}
		}

		// Intercept /phase and phase aliases (/plan, /execute, /verify, /ship)
		phaseCmd := ""
		if strings.HasPrefix(cmd, "/phase ") {
			phaseCmd = strings.TrimPrefix(cmd, "/phase ")
		} else if cmd == "/plan" || strings.HasPrefix(cmd, "/plan ") {
			phaseCmd = "plan" + strings.TrimPrefix(cmd, "/plan")
		} else if cmd == "/execute" || strings.HasPrefix(cmd, "/execute ") {
			phaseCmd = "execute" + strings.TrimPrefix(cmd, "/execute")
		} else if cmd == "/verify" || strings.HasPrefix(cmd, "/verify ") {
			phaseCmd = "verify" + strings.TrimPrefix(cmd, "/verify")
		} else if cmd == "/ship" || strings.HasPrefix(cmd, "/ship ") {
			phaseCmd = "ship" + strings.TrimPrefix(cmd, "/ship")
		}
		if phaseCmd != "" && m.workflowEngine != nil {
			parts := strings.Fields(phaseCmd)
			if len(parts) >= 2 {
				phaseName := parts[1]
				goal := ""
				if len(parts) > 2 {
					goal = strings.Join(parts[2:], " ")
				}
				var phase types.WorkflowPhase
				switch phaseName {
				case "initialize":
					phase = types.PhaseInitialize
				case "discuss":
					phase = types.PhaseDiscuss
				case "plan":
					phase = types.PhasePlan
				case "execute":
					phase = types.PhaseExecute
				case "verify":
					phase = types.PhaseVerify
				case "ship":
					phase = types.PhaseShip
				default:
					m.currentOperation = fmt.Sprintf("Unknown phase: %q", phaseName)
					return m, nil
				}
				m.workflowGoal = goal
				m.setWorkflowPhase(phase)
				m.workflowStartTime = time.Now()
				return m, RunPhaseCmd(m, phase, goal)
			}
			// Bare alias like /plan — show usage hint instead of silently doing nothing
			if m.replModel != nil {
				errMsg := types.Message{
					Role:    "assistant",
					Content: fmt.Sprintf("Usage: /%s <goal> — e.g., /%s build a REST API", parts[0], parts[0]),
					Segments: []types.MessageSegment{{
						Type:    "content",
						Content: fmt.Sprintf("Usage: /%s <goal> — e.g., /%s build a REST API", parts[0], parts[0]),
						Visible: true,
					}},
					CreatedAt: time.Now(),
				}
				m.replModel.AddMessage(errMsg)
			}
			return m, nil
		}

		// Intercept /workflow to start the full workflow chain.
		// The "resume" subcommand is a special case: it's handled by
		// the command registry's handleWorkflow (which sets
		// WorkflowResume: true) so we skip the prefix match below and
		// let it fall through to the registry.
		if strings.HasPrefix(cmd, "/workflow ") && m.workflowEngine != nil {
			goal := strings.TrimPrefix(cmd, "/workflow ")
			if goal == "resume" || strings.HasPrefix(goal, "resume ") {
				// Fall through to the command registry for /workflow resume
			} else if goal == "" {
				m.currentOperation = "Usage: /workflow <your goal>"
				return m, nil
			} else {
				m.workflowGoal = goal
				m.setWorkflowPhase(types.PhaseInitialize)
				m.workflowStartTime = time.Now()
				m.currentOperation = fmt.Sprintf("Starting workflow: %s", goal)
				return m, RunPhaseCmd(m, types.PhaseInitialize, goal)
			}
		}

		// Try command registry for all other slash commands
		if m.autoDream != nil && m.replModel != nil {
			m.autoDream.SetMessages(m.replModel.Messages())
		}

		sessionID := ""
		if m.workflowEngine != nil {
			sessionID = m.workflowEngine.SessionID()
		}
		ctx := CommandContext{
			Registry:       m.registry,
			SessionManager: m.sessionManager,
			Config:         m.config,
			ConfigPath:     m.configPath,
			Dispatcher:     m.dispatcher,
			Ledger:         m.ledger,
			WorkflowEngine: m.workflowEngine,
			SessionID:      sessionID,
			AutoDream:      m.autoDream,
			Git:            m.git,
			Rollback:       m.rollback,
			CmdRegistry:    m.cmdRegistry,
			ClearMessages: func() {
				if m.replModel != nil {
					m.replModel.ClearMessages()
				}
			},
		}
		result, handled := m.cmdRegistry.Execute(cmd, ctx)
		if handled {
			m.currentOperation = result.Message

			// /workflow resume — re-run the persisted phase (D-06).
			// Populate the AppState from the loaded state and call
			// RunPhaseCmd. The persistWorkflowState call from the
			// PhaseResultMsg handler will then update session.json
			// with the new transitions.
			if result.WorkflowResume {
				if m.workflowEngine == nil {
					m.currentOperation = "Cannot resume workflow: no engine."
					return m, nil
				}
				m.workflowGoal = result.ResumeGoal
				m.setWorkflowPhase(result.ResumePhase)
				m.discussQuestions = result.ResumeQuestions
				return m, RunPhaseCmd(m, result.ResumePhase, result.ResumeGoal)
			}

			// Session switching: /fork, /prev, /next set SessionID to transition
			if result.SessionID != nil && *result.SessionID != sessionID {
				var providerCmd tea.Cmd
				if sess, err := m.sessionManager.LoadSession(*result.SessionID); err == nil && sess != nil {
					if m.replModel == nil {
						rp := NewReplModel(m.themeManager.Current(), m.version)
						m.replModel = &rp
					}
					providerCmd = m.replModel.SetProvider(m.registry, sess.Provider, m.activeModel, sess.ID, m.config)
					m.replModel.SetDispatcher(m.dispatcher)
					m.replModel.SetCommandRegistry(m.cmdRegistry)
					// Replace messages with the loaded session's messages
					m.replModel.ClearMessages()
					for _, msg := range sess.Messages {
						m.replModel.AddMessage(msg)
					}
					// Update workflow engine session ID so subsequent operations
					// write to the correct session directory.
					if m.workflowEngine != nil {
						m.workflowEngine.SetSessionID(*result.SessionID)
					}
					m.dispatcher.SetSessionID(*result.SessionID)
					m.currentOperation = fmt.Sprintf("Session %s loaded", *result.SessionID)
					if m.planModel != nil {
						m.planModel.sessionID = *result.SessionID
					}
					if m.executeModel != nil {
						m.executeModel.sessionID = *result.SessionID
					}
					if m.verifyModel != nil {
						m.verifyModel.sessionID = *result.SessionID
					}
					if m.shipModel != nil {
						m.shipModel.sessionID = *result.SessionID
					}
				}
				if result.Cmd != nil {
					return m, tea.Batch(result.Cmd, providerCmd)
				}
				return m, providerCmd
			}

			if result.Screen != nil {
				m.screen = *result.Screen
				if *result.Screen == ScreenFirstRun {
					m.replModel = nil
				}
				if result.Cmd != nil {
					return m, tea.Batch(result.Cmd)
				}
				return m, nil
			}
			if strings.HasPrefix(result.Message, "Goodbye") {
				return m, tea.Quit
			}
			if result.Cmd != nil {
				return m, tea.Batch(result.Cmd)
			}
			return m, nil
		}

		// Unknown command — show as error in REPL
		if m.replModel != nil {
			errMsg := types.Message{
				Role:    "assistant",
				Content: fmt.Sprintf("Unknown command: %s. Type /help for available commands.", cmd),
				Segments: []types.MessageSegment{{
					Type:    "content",
					Content: fmt.Sprintf("Unknown command: %s. Type /help for available commands.", cmd),
					Visible: true,
				}},
				CreatedAt: time.Now(),
			}
			m.replModel.AddMessage(errMsg)
		}
		return m, nil

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

	case SidebarRefreshMsg:
		if m.sidebarModel != nil {
			m.sidebarModel.Update(msg)
			threshold := 120
			if m.config != nil && m.config.UI.SidebarWidthThreshold > 0 {
				threshold = m.config.UI.SidebarWidthThreshold
			}
			if m.width > threshold && m.replModel != nil && !m.sidebarManuallyHidden {
				m.sidebarModel.SetVisible(true)
				m.replModel.SetSidebarWidth(sidebarWidth)
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

	switch m.screen {
	case ScreenFirstRun:
		if m.firstRunModel == nil {
			return m, nil
		}
		cmds, appMsg := m.firstRunModel.Update(msg)
		if appMsg != nil {
			m.screen = appMsg.Screen
			if appMsg.Screen == ScreenREPL && m.replModel == nil {
				// Save API key to keychain if requested
				if appMsg.SaveKeychain && m.keychain != nil && m.firstRunModel.APIKey() != "" {
					for _, provider := range m.firstRunModel.SelectedProviders() {
						key := m.firstRunModel.APIKey()
						service := fmt.Sprintf("m31a/%s", provider)
						m.keychain.Set(service, key)
					}
				}

				// Update active provider/model from first-run selections
				selectedProviders := m.firstRunModel.SelectedProviders()
				if len(selectedProviders) > 0 {
					m.activeProvider = selectedProviders[0]
					if m.registry != nil {
						// Try to set the active provider in the registry
						if err := m.registry.SetActive(m.activeProvider); err == nil {
							// Fetch models to populate the cache and find the default model
							p := m.registry.ActiveProvider()
							if p != nil {
								ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
								models, err := p.FetchModels(ctx)
								cancel()
								if err == nil && len(models) > 0 {
									// Use config default model if set, otherwise use first model
									if m.config != nil && m.config.Model.Default != "" {
										for _, model := range models {
											if model.ID == m.config.Model.Default {
												m.activeModel = &model
												break
											}
										}
									}
									if m.activeModel == nil {
										m.activeModel = &models[0]
									}
								}
							}
						}
					}
				}

						rp := NewReplModel(m.themeManager.Current(), m.version)
						m.replModel = &rp

				// Create a session so that /status, /save, etc. work
				sessionID := ""
				if m.sessionManager != nil && m.activeModel != nil && m.activeProvider != "" {
					s, err := m.sessionManager.NewSession(m.activeModel.ID, m.activeProvider)
					if err == nil {
						sessionID = s.ID
						m.dispatcher.SetSessionID(s.ID)
					}
				}

				providerCmd := m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, sessionID, m.config)
				m.replModel.SetDispatcher(m.dispatcher)
				m.replModel.SetCommandRegistry(m.cmdRegistry)
				m.initialized = true
				// Init sidebar
				if m.sidebarModel == nil {
					m.sidebarModel = NewSidebarModel(m.git, m.themeManager.Current())
				}
				// Size the REPL immediately with current window dimensions
				if m.width > 0 && m.height > 0 {
					m.replModel.Update(tea.WindowSizeMsg{Width: m.width, Height: m.height})
				}
				// Don't auto-show sidebar until git status is loaded
				healthCmd := HealthCheckTicker(
					context.Background(), m.registry, m.activeProvider,
					types.HealthCheckInterval,
				)
				cmds = append(cmds, healthCmd)
				cmds = append(cmds,
					CacheRefreshTicker(m.activeProvider, provider.DefaultCacheRefreshInterval),
					providerCmd)
			}
		}
		cmds = append(cmds, permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher))
		return m, tea.Batch(cmds...)

	case ScreenREPL:
		if m.replModel == nil {
			return m, nil
		}
		cmds, sent := m.replModel.Update(msg)
		if sent {
			m.lastActivity = time.Now()
			m.currentOperation = "Ready"
		}
		cmds = append(cmds, permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher))
		return m, tea.Batch(cmds...)

	case ScreenSettings:
		if m.settingsModel == nil {
			return m, nil
		}
		var cmd tea.Cmd
		(*m.settingsModel), cmd = m.settingsModel.Update(msg)
		cmds := []tea.Cmd{cmd}
		cmds = append(cmds, permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher))
		return m, tea.Batch(cmds...)

	case ScreenPermission:
		// Permission modal is visible; keep listeners active so subsequent
		// permission requests are picked up after the current one resolves.
		return m, tea.Batch(permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher))

	case ScreenResume:
		if m.resumeModel == nil {
			return m, nil
		}
		cmds, appMsg := m.resumeModel.Update(msg)
		if appMsg != nil {
			m.screen = appMsg.Screen
			if appMsg.SessionID != "" {
				// Load session data into the REPL model
				if sess, err := m.sessionManager.LoadSession(appMsg.SessionID); err == nil && sess != nil {
					if m.replModel == nil {
						rp := NewReplModel(m.themeManager.Current(), m.version)
						m.replModel = &rp
					}
					providerCmd := m.replModel.SetProvider(m.registry, sess.Provider, m.activeModel, sess.ID, m.config)
					m.replModel.SetDispatcher(m.dispatcher)
					m.replModel.SetSessionID(sess.ID)
					for _, msg := range sess.Messages {
						m.replModel.AddMessage(msg)
					}
					if m.workflowEngine != nil {
						m.workflowEngine.SetSessionID(sess.ID)
					}
					m.dispatcher.SetSessionID(sess.ID)
					if m.planModel != nil {
						m.planModel.sessionID = sess.ID
					}
					if m.executeModel != nil {
						m.executeModel.sessionID = sess.ID
					}
					if m.verifyModel != nil {
						m.verifyModel.sessionID = sess.ID
					}
					if m.shipModel != nil {
						m.shipModel.sessionID = sess.ID
					}
					m.currentOperation = fmt.Sprintf("Session %s loaded", appMsg.SessionID)
					cmds = append(cmds, providerCmd)
				} else {
					m.currentOperation = fmt.Sprintf("Failed to load session %s", appMsg.SessionID)
				}
			}
		}
		cmds = append(cmds, permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher))
		return m, tea.Batch(cmds...)

	case ScreenModelSelector:
		// Intercept Esc to navigate back (two-step: first Esc blurs search, second Esc exits)
		if keyMsg, ok := msg.(tea.KeyMsg); ok && !m.modelSelector.searchFocused && keyMsg.String() == "esc" {
			m.screen = m.prevScreen
			return m, nil
		}
		updated, cmd := m.modelSelector.Update(msg)
		m.modelSelector = updated.(ModelSelector)
		cmds := []tea.Cmd{cmd}
		cmds = append(cmds, permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher))
		return m, tea.Batch(cmds...)

	case ScreenPlan:
		cmds := []tea.Cmd{permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher)}
		if m.planModel != nil {
			subCmds, appMsg := m.planModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
			}
			cmds = append(cmds, subCmds...)
		}
		return m, tea.Batch(cmds...)

	case ScreenExecute:
		cmds := []tea.Cmd{permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher)}
		if m.executeModel != nil {
			subCmds, appMsg := m.executeModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
			}
			cmds = append(cmds, subCmds...)
		}
		return m, tea.Batch(cmds...)

	case ScreenVerify:
		cmds := []tea.Cmd{permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher)}
		if m.verifyModel != nil {
			subCmds, appMsg := m.verifyModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
			}
			cmds = append(cmds, subCmds...)
		}
		return m, tea.Batch(cmds...)

	case ScreenShip:
		cmds := []tea.Cmd{permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher)}
		if m.shipModel != nil {
			subCmds, appMsg := m.shipModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
				if appMsg.Action == "new_session" {
					// Create a fresh session and reset workflow state
					if m.sessionManager != nil {
						modelID := ""
						providerName := ""
						if m.activeModel != nil {
							modelID = m.activeModel.ID
						}
						providerName = m.activeProvider
						sess, err := m.sessionManager.NewSession(modelID, providerName)
						if err == nil && m.workflowEngine != nil {
							m.workflowEngine.SetSessionID(sess.ID)
						}
					}
					m.workflowGoal = ""
					m.setWorkflowPhase(types.PhaseIdle)
					if m.replModel != nil {
						m.replModel.ClearMessages()
					}
				}
			}
			cmds = append(cmds, subCmds...)
		}
		return m, tea.Batch(cmds...)

	case ScreenDiff:
		cmds := []tea.Cmd{permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher)}
		if m.diffModel.lines != nil || m.diffModel.diff != "" {
			updated, cmd := m.diffModel.Update(msg)
			m.diffModel = updated.(DiffModel)
			cmds = append(cmds, cmd)
		}
		return m, tea.Batch(cmds...)

	default:
		return m, nil
	}
}
