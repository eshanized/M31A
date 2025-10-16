package tui

import (
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
)

// handleSlashCommand processes slash commands from the REPL.
// It handles TUI-specific commands, phase command interception, and delegation to command registry.
func (m *AppState) handleSlashCommand(cmd string) (tea.Model, tea.Cmd) {
	// Process slash commands from the REPL
	if m.screen != ScreenREPL {
		return m, nil
	}

	// Handle TUI-specific commands first
	switch cmd {
	case "/settings":
		m.screen = ScreenSettings
		if m.settingsModel != nil {
			m.settingsModel.width = m.width
			m.settingsModel.height = m.height
		}
		return m, nil
	case "/resume":
		if m.resumeModel != nil {
			m.resumeModel.Refresh()
		}
		m.screen = ScreenResume
		return m, nil
	case "/models":
		// BUG-7 fix: add explicit error instead of silent fallthrough when registry is nil.
		if m.registry == nil {
			if m.replModel != nil {
				m.replModel.AddMessage(makeAssistantMsg("No providers configured. Use /settings to add an API key."))
			}
			return m, nil
		}
		m.prevScreen = m.screen
		m.modelSelector = NewModelSelector(m.registry, m.sessionManager, m.themeManager.Current())
		m.screen = ScreenModelSelector
		return m, m.modelSelector.Init()
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
			m.replModel.AddMessage(makeAssistantMsg(fmt.Sprintf("Usage: /%s <goal> — e.g., /%s build a REST API", parts[0], parts[0])))
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
			// Lazy-init ScreenMetrics model and load stats on transition
			if *result.Screen == ScreenMetrics {
				if m.metricsModel == nil {
					m.metricsModel = NewMetricsModel(m.themeManager.Current())
				}
				m.metricsModel.width = m.width
				m.metricsModel.height = m.height
				m.metricsModel.LoadStats(m.sessionManager)
			}
			// Lazy-init ScreenGoalInput model on transition
			if *result.Screen == ScreenGoalInput {
				if m.goalInputModel == nil {
					var recent []string
					if m.sessionManager != nil {
						sessions, _ := m.sessionManager.ListSessions()
						for _, s := range sessions {
							if len(recent) >= 5 {
								break
							}
							if sess, err := m.sessionManager.LoadSession(s.ID); err == nil && sess != nil && sess.Project != nil && sess.Project.Goal != "" {
								recent = append(recent, sess.Project.Goal)
							}
						}
					}
					m.goalInputModel = NewGoalInputModel(m.themeManager.Current(), recent)
				}
				m.goalInputModel.width = m.width
				m.goalInputModel.height = m.height
			}
			// Lazy-init ScreenLedger model on transition
			if *result.Screen == ScreenLedger {
				if m.ledgerModel == nil {
					m.ledgerModel = NewLedgerModel(m.themeManager.Current(), m.ledger)
				}
				m.ledgerModel.width = m.width
				m.ledgerModel.height = m.height
				m.ledgerModel.LoadEntries()
			}
			// Lazy-init ScreenRollback model on transition
			if *result.Screen == ScreenRollback {
				if m.rollbackModel == nil {
					m.rollbackModel = NewRollbackModel(m.themeManager.Current(), m.git, m.rollback, m.width, m.height)
				}
				m.rollbackModel.width = m.width
				m.rollbackModel.height = m.height
				m.rollbackModel.LoadCommits()
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
		m.replModel.AddMessage(makeAssistantMsg(fmt.Sprintf("Unknown command: %s. Type /help for available commands.", cmd)))
	}
	return m, nil
}