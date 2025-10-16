package tui

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

// routeToScreen routes messages to the active screen model.
func (m *AppState) routeToScreen(msg tea.Msg) (tea.Model, tea.Cmd) {
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

				// Also save API key to config file so it persists across restarts
				if m.firstRunModel.APIKey() != "" {
					apiKey := m.firstRunModel.APIKey()
					for _, p := range m.firstRunModel.SelectedProviders() {
						switch p {
						case "openrouter":
							m.config.Provider.OpenRouter.APIKey = apiKey
						case "zen":
							m.config.Provider.Zen.APIKey = apiKey
						}
					}
					// Set default provider to the first selected one
					if len(m.firstRunModel.SelectedProviders()) > 0 {
						m.config.Provider.Default = m.firstRunModel.SelectedProviders()[0]
					}
					// Persist to disk
					if err := m.config.Save(m.configPath); err != nil {
						slog.Warn("failed to save config after first-run", "error", err)
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
						// BUG-4 fix: store the session ID on AppState so that
						// workflow commands that read m.sessionID (via workflowEngine.SessionID)
						// can find the active session without requiring a restart.
						m.sessionID = s.ID
						m.dispatcher.SetSessionID(s.ID)
					}
				}

				// BUG-3 fix: Re-initialize the workflow engine now that a provider,
				// model, and session are available. initWorkflowEngine() was called
				// once in NewApp() but returned early because no API key was configured.
				// Calling it again here ensures /workflow, /plan, /execute, etc. work
				// immediately after first-run setup without requiring a restart.
				m.initWorkflowEngine()

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
				// Refresh viewport so welcome screen reflects the new provider/model
				m.replModel.RefreshViewport()
				// Don't auto-show sidebar until git status is loaded
				// BUG-9 fix: only start health/cache tickers when a provider is configured.
				// In offline mode (no provider after first-run skip or key-less setup),
				// starting these tickers creates a pointless 60s retry loop.
				if m.activeProvider != "" && m.registry != nil {
					healthCmd := HealthCheckTicker(
						context.Background(),
						types.HealthCheckInterval,
					)
					cmds = append(cmds, healthCmd)
					cmds = append(cmds,
						CacheRefreshTicker(m.activeProvider, provider.DefaultCacheRefreshInterval),
						providerCmd)
				} else {
					cmds = append(cmds, providerCmd)
				}
			}
		}
	cmds = append(cmds, m.listenerCmds()...)
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
	cmds = append(cmds, m.listenerCmds()...)
	return m, tea.Batch(cmds...)

case ScreenSettings:
	if m.settingsModel == nil {
		return m, nil
	}
	var cmd tea.Cmd
	(*m.settingsModel), cmd = m.settingsModel.Update(msg)
	cmds := []tea.Cmd{cmd}
	cmds = append(cmds, m.listenerCmds()...)
	return m, tea.Batch(cmds...)

case ScreenPermission:
	// Permission modal is visible; keep listeners active so subsequent
	// permission requests are picked up after the current one resolves.
	return m, tea.Batch(m.listenerCmds()...)

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
	cmds = append(cmds, m.listenerCmds()...)
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
		cmds = append(cmds, permissionListenerCmd(m.shutdownCtx, m.dispatcher), questionListenerCmd(m.shutdownCtx, m.dispatcher))
		return m, tea.Batch(cmds...)

	case ScreenPlan:
	cmds := m.listenerCmds()
	if m.planModel != nil {
			subCmds, appMsg := m.planModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
			}
			cmds = append(cmds, subCmds...)
		}
		return m, tea.Batch(cmds...)

	case ScreenExecute:
	cmds := m.listenerCmds()
	if m.executeModel != nil {
			subCmds, appMsg := m.executeModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
			}
			cmds = append(cmds, subCmds...)
		}
		return m, tea.Batch(cmds...)

	case ScreenVerify:
	cmds := m.listenerCmds()
	if m.verifyModel != nil {
			subCmds, appMsg := m.verifyModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
			}
			cmds = append(cmds, subCmds...)
		}
		return m, tea.Batch(cmds...)

	case ScreenShip:
	cmds := m.listenerCmds()
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
	cmds := m.listenerCmds()
	if m.diffModel.lines != nil || m.diffModel.diff != "" {
			updated, cmd := m.diffModel.Update(msg)
			m.diffModel = updated.(DiffModel)
			cmds = append(cmds, cmd)
		}
		return m, tea.Batch(cmds...)

	case ScreenMetrics:
	cmds := m.listenerCmds()
	if m.metricsModel != nil {
			subCmds, appMsg := m.metricsModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
			}
			cmds = append(cmds, subCmds...)
		}
		return m, tea.Batch(cmds...)

	case ScreenGoalInput:
	cmds := m.listenerCmds()
	if m.goalInputModel != nil {
			subCmds, appMsg := m.goalInputModel.Update(msg)
			if appMsg != nil {
				if appMsg.Action == "goal_submitted" && m.goalInputModel != nil {
					// User confirmed goal — route through PhaseInitialize before Discuss
					goal := m.goalInputModel.Goal()
					m.screen = ScreenREPL
					if goal != "" && m.workflowEngine != nil {
						m.workflowGoal = goal
						m.setWorkflowPhase(types.PhaseInitialize)
						m.showPhaseBreadcrumb = true
						return m, RunPhaseCmd(m, types.PhaseInitialize, goal)
					}
				} else {
					m.screen = appMsg.Screen
				}
			}
			cmds = append(cmds, subCmds...)
		}
		return m, tea.Batch(cmds...)

	case ScreenLedger:
	cmds := m.listenerCmds()
	if m.ledgerModel != nil {
			subCmds, appMsg := m.ledgerModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
			}
			cmds = append(cmds, subCmds...)
		}
		return m, tea.Batch(cmds...)

	case ScreenRollback:
	cmds := m.listenerCmds()
	if m.rollbackModel != nil {
			subCmds, appMsg := m.rollbackModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
				if appMsg.Action != "" {
					m.toastText = appMsg.Action
					m.toastExpires = time.Now().Add(4 * time.Second)
					m.toastType = "info"
				}
			}
			cmds = append(cmds, subCmds...)
		}
		return m, tea.Batch(cmds...)

	case ScreenDiscuss:
		cmds := m.listenerCmds()
		if m.discussModel != nil {
			subCmds, appMsg := m.discussModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
				if appMsg.Action == "discuss_complete" {
					m.setWorkflowPhase(types.PhasePlan)
					cmds = append(cmds, RunPhaseCmd(m, types.PhasePlan, m.workflowGoal))
				} else if appMsg.Action == "discuss_cancelled" {
					m.setWorkflowPhase(types.PhaseIdle)
					m.resetDiscussQA()
				}
			}
			cmds = append(cmds, subCmds...)
		}
		return m, tea.Batch(cmds...)

	default:
		return m, nil
	}
}