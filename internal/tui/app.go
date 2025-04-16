package tui

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tokens"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/pkg/keychain"
	"github.com/eshanized/M31A/pkg/ledger"
	"github.com/eshanized/M31A/pkg/session"
)

type FallbackNotification struct {
	Event     FallbackEventMsg
	Dismissed bool
	ShownAt   time.Time
}

type AppState struct {
	screen           Screen
	version          string
	initialized      bool
	registry         *provider.Registry
	activeProvider   string
	activeModel      *types.ModelInfo
	contextUsed      int64
	contextTotal     int64
	width            int
	height           int
	focused          bool
	lastActivity     time.Time
	currentOperation string
	healthStatus     types.HealthStatus
	themeManager     *theme.Manager
	firstRunModel    *FirstRunModel
	replModel        *ReplModel
	settingsModel    *SettingsModel
	resumeModel      *ResumeModel
	sessionManager   *session.Manager
	keychain         keychain.Keychain
	config           *config.Config
	apiKey           string
	configPath       string
	ledger           *ledger.Ledger
	prevScreen       Screen
	permissionModal  *components.PermissionModal
	dispatcher       *tools.Dispatcher
	modelSelector      ModelSelector
	fallbackNotification *FallbackNotification
	cmdRegistry        *CommandRegistry
	planModel          *PlanModel
	executeModel       *ExecuteModel
	verifyModel        *VerifyModel
	shipModel          *ShipModel
	workflowEngine     *workflow.Engine
	workflowGoal       string
	workflowRunning    bool
	currentPhase       types.WorkflowPhase
	discussQuestions   []string
}

func NewApp(version string, registry *provider.Registry, apiKey string, configPath string) *AppState {
	tm := theme.NewManager(theme.ModeDark)

	cwd, _ := os.Getwd()
	backupDir := filepath.Join(filepath.Dir(configPath), "backups")

	// Initialize config
	cfg, _ := config.Load(configPath)
	if cfg == nil {
		cfg = config.DefaultConfig()
	}

	// Initialize keychain (may be nil — keychain may be unavailable)
	kc, _ := keychain.New()

	// Resolve API keys via env var → keychain → config file
	if apiKey == "" {
		// If no explicit apiKey, try resolving from config resolution
		if kc != nil {
			cfg.ResolveAPIKeys(kc)
		}
		apiKey = cfg.Provider.OpenRouter.APIKey
		if apiKey == "" {
			apiKey = cfg.Provider.Zen.APIKey
		}
	}

	// Initialize session manager
	sessionBaseDir := filepath.Join(filepath.Dir(configPath), "sessions")
	sessionMgr := session.NewManager(sessionBaseDir)

	app := &AppState{
		version:        version,
		registry:       registry,
		apiKey:         apiKey,
		configPath:     configPath,
		themeManager:   tm,
		healthStatus:   types.HealthStatus{Status: "unknown"},
		dispatcher:     tools.DefaultDispatcher(cwd, backupDir),
		config:         cfg,
		keychain:       kc,
		sessionManager: sessionMgr,
	}

	// Initialize ledger for settings stats display
	ledgerPath := filepath.Join(filepath.Dir(configPath), "LEDGER.md")
	ledgerInstance := ledger.New(ledgerPath)
	app.ledger = ledgerInstance

	// Initialize settings model (6 tabs, inline editing)
	sm := NewSettingsModel(cfg, configPath, tm.Current(), ledgerInstance)
	app.settingsModel = &sm

	// Initialize resume model
	rm := NewResumeModel(tm.Current(), sessionMgr)
	app.resumeModel = rm

	// Initialize model selector (uses registry)
	if registry != nil {
		app.modelSelector = NewModelSelector(registry)
	}

	// Initialize command registry
	app.cmdRegistry = DefaultCommands()

	if registry != nil {
		app.activeProvider = registry.Active()
	}

	app.initWorkflowEngine()

	if apiKey == "" {
		fr := NewFirstRunModel(tm.Current(), configPath)
		app.screen = ScreenFirstRun
		app.firstRunModel = &fr
	} else if registry == nil || registry.ActiveProvider() == nil {
		// No providers available — offline mode
		rp := NewReplModel(tm.Current())
		app.screen = ScreenREPL
		app.replModel = &rp
		app.replModel.SetProvider(registry, app.activeProvider, app.activeModel, "")
		app.healthStatus = types.HealthStatus{
			Status: "offline",
			Error:  "No providers available — offline mode. History is readable but no new messages.",
		}
		app.currentOperation = "No providers available — offline mode. History is readable but no new messages."
	} else {
		rp := NewReplModel(tm.Current())
		app.screen = ScreenREPL
		app.replModel = &rp
		app.replModel.SetProvider(registry, app.activeProvider, app.activeModel, "")
		app.healthStatus = types.HealthStatus{Status: "live"}
	}

	return app
}

func (m *AppState) initWorkflowEngine() {
	if m.registry == nil || m.activeProvider == "" || m.sessionManager == nil {
		return
	}
	p := m.registry.ActiveProvider()
	if p == nil {
		return
	}

	modelID := ""
	if m.activeModel != nil {
		modelID = m.activeModel.ID
	}
	if modelID == "" && m.config != nil {
		modelID = m.config.Model.Default
	}
	if modelID == "" {
		return
	}

	// Create a session for the workflow
	s, err := m.sessionManager.NewSession(modelID, m.activeProvider)
	if err != nil {
		m.currentOperation = fmt.Sprintf("Workflow engine init failed: %v", err)
		return
	}

	cwd, err := os.Getwd()
	if err != nil {
		cwd = os.TempDir()
	}
	backupDir := filepath.Join(filepath.Dir(m.configPath), "backups")
	sessionBaseDir := filepath.Join(filepath.Dir(m.configPath), "sessions")
	planningDir := filepath.Join(sessionBaseDir, s.ID, "planning")

	g := git.New(cwd)

	est := tokens.NewEstimator(modelID)

	eng := workflow.NewEngine(s.ID, cwd, backupDir, planningDir,
		p, modelID, m.dispatcher, est, m.sessionManager)
	eng.SetGit(g)
	m.workflowEngine = eng
}

// RunPhaseCmd returns a tea.Cmd that executes the given workflow phase in a
// goroutine and emits a PhaseResultMsg on completion.
func RunPhaseCmd(eng *workflow.Engine, phase types.WorkflowPhase, goal string) tea.Cmd {
	return func() tea.Msg {
		ctx := context.Background()
		result, err := eng.RunPhase(ctx, phase, goal)
		if err != nil {
			return PhaseResultMsg{
				Phase: phase,
				Error: err.Error(),
			}
		}
		if result == nil {
			return PhaseResultMsg{
				Phase: phase,
				Error: "nil result",
			}
		}
		return PhaseResultMsg{
			Phase:    phase,
			Tasks:    result.Tasks,
			Messages: result.Messages,
			Success:  result.Success,
			Error:    result.Error,
		}
	}
}

func (m *AppState) Init() tea.Cmd {
	cmds := []tea.Cmd{permissionListenerCmd(m.dispatcher)}
	if m.screen == ScreenREPL && m.registry != nil && m.activeProvider != "" {
		cmds = append(cmds, HealthCheckTicker(context.Background(), m.registry, m.activeProvider, types.HealthCheckInterval))
		cmds = append(cmds, CacheRefreshTicker(m.activeProvider, provider.DefaultCacheRefreshInterval))
	}
	return tea.Batch(cmds...)
}

// permissionListenerCmd returns a tea.Cmd that watches the dispatcher's
// permission request channel and feeds requests into the Bubble Tea event loop.
func permissionListenerCmd(dispatcher *tools.Dispatcher) tea.Cmd {
	return func() tea.Msg {
		req := <-dispatcher.RequestCh()
		return PermissionRequestMsg{Request: req}
	}
}

func (m *AppState) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		m.initialized = true
		if m.replModel != nil {
			m.replModel.Update(msg)
		}
		if m.firstRunModel != nil {
			m.firstRunModel.Update(msg)
		}
		return m, nil

	case tea.KeyMsg:
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
		if m.screen == ScreenREPL {
			// Handle TUI-specific commands first
			switch msg.String() {
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
					m.modelSelector = NewModelSelector(m.registry)
					m.screen = ScreenModelSelector
					return m, m.modelSelector.Init()
				}
			}

			// Intercept /phase to start workflow engine phases
			if strings.HasPrefix(msg.String(), "/phase ") && m.workflowEngine != nil {
				parts := strings.Fields(msg.String())
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
						return m, nil
					}
					m.workflowGoal = goal
					m.workflowRunning = true
					m.currentPhase = phase
					return m, RunPhaseCmd(m.workflowEngine, phase, goal)
				}
			}

			// Intercept /workflow to start the full workflow chain
			if strings.HasPrefix(msg.String(), "/workflow ") && m.workflowEngine != nil {
				goal := strings.TrimPrefix(msg.String(), "/workflow ")
				if goal == "" {
					m.currentOperation = "Usage: /workflow <your goal>"
					return m, nil
				}
				m.workflowGoal = goal
				m.workflowRunning = true
				m.currentPhase = types.PhaseInitialize
				m.currentOperation = fmt.Sprintf("Starting workflow: %s", goal)
				return m, RunPhaseCmd(m.workflowEngine, types.PhaseInitialize, goal)
			}

			// Try command registry for all other slash commands
			if strings.HasPrefix(msg.String(), "/") {
				sessionID := ""
				if m.workflowEngine != nil {
					sessionID = m.workflowEngine.SessionID()
				}
				ctx := CommandContext{
					Registry:        m.registry,
					SessionManager:  m.sessionManager,
					Config:          m.config,
					Dispatcher:      m.dispatcher,
					Ledger:          m.ledger,
					WorkflowEngine:  m.workflowEngine,
					SessionID:       sessionID,
				}
				result, handled := m.cmdRegistry.Execute(msg.String(), ctx)
				if handled {
					m.currentOperation = result.Message
					if result.Screen != nil {
						m.screen = *result.Screen
						if *result.Screen == ScreenFirstRun {
							m.replModel = nil
						}
						return m, nil
					}
					if strings.HasPrefix(result.Message, "Goodbye") {
						return m, tea.Quit
					}
					return m, nil
				}
			}
		}

	case HealthCheckTickMsg:
		if m.registry == nil || m.activeProvider == "" {
			return m, NextHealthTick(types.HealthCheckInterval)
		}

		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		p := m.registry.ActiveProvider()
		if p == nil {
			cancel()
			return m, NextHealthTick(types.HealthCheckInterval)
		}

		result := p.HealthCheck(ctx)
		cancel()
		m.healthStatus = result
		m.lastActivity = time.Now()
		return m, NextHealthTick(calculateNextInterval(result))

	case RefreshCacheMsg:
		if m.registry == nil || m.activeProvider == "" {
			return m, NextCacheRefreshTick(provider.DefaultCacheRefreshInterval)
		}

		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)

		providerName := msg.ProviderName
		if providerName == "" {
			providerName = m.activeProvider
		}

		errMsg, nextCmd := handleCacheRefresh(ctx, m.registry, providerName)
		cancel()
		if errMsg != "" {
			m.currentOperation = errMsg
		} else {
			m.currentOperation = ""
			m.lastActivity = time.Now()
		}
		return m, nextCmd

	case AppMsg:
		// Handle ModelSelected first — it may be set without a Screen field
		if msg.ModelSelected != nil {
			if m.registry != nil {
				_ = m.registry.SetActive(msg.ModelSelected.Provider)
			}
			m.activeProvider = msg.ModelSelected.Provider
			m.activeModel = &msg.ModelSelected.Model
			m.screen = m.prevScreen
			return m, nil
		}

		if msg.Screen == ScreenREPL && m.replModel == nil {
			rp := NewReplModel(m.themeManager.Current())
			m.replModel = &rp
			m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, "")
			m.initialized = true
		}
		if msg.Screen == ScreenModelSelector {
			m.prevScreen = m.screen
			m.modelSelector = NewModelSelector(m.registry)
			m.screen = ScreenModelSelector
			return m, m.modelSelector.Init()
		}
		m.screen = msg.Screen
		if msg.InitError != nil {
			m.currentOperation = fmt.Sprintf("Error: %v", msg.InitError)
		}
		return m, nil

	case FallbackEventMsg:
		m.activeProvider = msg.To
		m.fallbackNotification = &FallbackNotification{
			Event:     msg,
			Dismissed: false,
			ShownAt:   time.Now(),
		}
		return m, nil

	case ErrorMsg:
		m.currentOperation = fmt.Sprintf("Error: %v", msg.Err)
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
					if m.replModel != nil {
						m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, m.replModel.sessionID)
						m.replModel.Update(msg)
					}
					return m, tea.Batch(
						func() tea.Msg {
							return FallbackEventMsg{From: event.From, To: event.To, Reason: reason}
						},
					)
				}
			}
		}
		// Pass the error through to the REPL model for display
		if m.replModel != nil {
			m.replModel.Update(msg)
		}
		return m, nil

	case PermissionRequestMsg:
		m.prevScreen = m.screen
		m.screen = ScreenPermission
		t := m.themeManager.Current()
		pm := components.NewPermissionModal(msg.Request, t, 5*time.Minute)
		m.permissionModal = pm
		return m, nil

	case PermissionResponseMsg:
		m.dispatcher.ApprovePermission(msg.Response.Allowed, msg.Response.Remember)
		m.screen = m.prevScreen
		m.permissionModal = nil
		return m, permissionListenerCmd(m.dispatcher)

	case PhaseResultMsg:
		if msg.Error != "" {
			m.currentOperation = fmt.Sprintf("Phase %s failed: %s", msg.Phase, msg.Error)
			m.workflowRunning = false
			return m, nil
		}
		if !msg.Success {
			m.currentOperation = fmt.Sprintf("Phase %s completed unsuccessfully", msg.Phase)
			m.workflowRunning = false
			return m, nil
		}
		m.currentOperation = fmt.Sprintf("Phase %s completed", msg.Phase)

		switch msg.Phase {
		case types.PhaseInitialize:
			// Auto-advance to Discuss
			m.currentPhase = types.PhaseDiscuss
			return m, RunPhaseCmd(m.workflowEngine, types.PhaseDiscuss, m.workflowGoal)

		case types.PhaseDiscuss:
			// Extract questions and transition to Discuss screen for Q&A
			m.currentPhase = types.PhaseDiscuss
			m.discussQuestions = make([]string, 0, len(msg.Messages))
			for _, msg2 := range msg.Messages {
				if msg2.Role == "assistant" {
					m.discussQuestions = append(m.discussQuestions, msg2.Content)
				}
			}
			// For V1, we auto-advance to Plan with saved answers
			// The discuss answers are already saved to PROJECT.md by the engine
			m.currentPhase = types.PhasePlan
			return m, RunPhaseCmd(m.workflowEngine, types.PhasePlan, m.workflowGoal)

		case types.PhasePlan:
			if len(msg.Tasks) > 0 && m.planModel == nil {
				t := m.themeManager.Current()
				modelID := ""
				if m.activeModel != nil {
					modelID = m.activeModel.ID
				}
				providerName := m.activeProvider
				pm := NewPlanModel(msg.Tasks, t, modelID, providerName, 0, "")
				m.planModel = pm
			}
			// Auto-advance to Execute
			m.currentPhase = types.PhaseExecute
			return m, RunPhaseCmd(m.workflowEngine, types.PhaseExecute, m.workflowGoal)

		case types.PhaseExecute:
			t := m.themeManager.Current()
			m.executeModel = NewExecuteModel(msg.Tasks, t)
			m.screen = ScreenExecute
			// Run the actual execute phase via engine (tool dispatch, git commits)
			m.currentPhase = types.PhaseVerify
			return m, RunPhaseCmd(m.workflowEngine, types.PhaseExecute, m.workflowGoal)

		case types.PhaseVerify:
			t := m.themeManager.Current()
			results := make(map[int]workflow.VerificationResult)
			m.verifyModel = NewVerifyModel(msg.Tasks, results, t)
			m.screen = ScreenVerify
			// Run the actual verify phase via engine
			m.currentPhase = types.PhaseShip
			return m, RunPhaseCmd(m.workflowEngine, types.PhaseVerify, m.workflowGoal)

		case types.PhaseShip:
			t := m.themeManager.Current()
			summary := ShipSummary{
				SessionID: m.workflowEngine.SessionID(),
			}
			if m.executeModel != nil {
				done, total, failed, skipped := 0, len(msg.Tasks), 0, 0
				for _, task := range msg.Tasks {
					switch task.Status {
					case types.StatusDone:
						done++
					case types.StatusFailed:
						failed++
					case types.StatusSkipped:
						skipped++
					}
				}
				summary.TaskDone = done
				summary.TaskTotal = total
				summary.TaskFailed = failed
				summary.TaskSkipped = skipped
			}
			m.shipModel = NewShipModel(summary, t)
			m.screen = ScreenShip
			m.workflowRunning = false
			return m, nil

		default:
			return m, nil
		}

	case SettingsSavedMsg:
		// Restart health check and cache refresh tickers after settings change
		m.currentOperation = "Settings saved"
		if m.screen == ScreenREPL && m.registry != nil && m.activeProvider != "" {
			m.lastActivity = time.Now()
		}
		return m, tea.Batch(
			NextHealthTick(types.HealthCheckInterval),
			NextCacheRefreshTick(provider.DefaultCacheRefreshInterval),
		)
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
				rp := NewReplModel(m.themeManager.Current())
				m.replModel = &rp
				m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, "")
				m.initialized = true
				healthCmd := HealthCheckTicker(
					context.Background(), m.registry, m.activeProvider,
					types.HealthCheckInterval,
				)
				cmds = append(cmds, healthCmd)
				cmds = append(cmds,
					CacheRefreshTicker(m.activeProvider, provider.DefaultCacheRefreshInterval))
			}
		}
		cmds = append(cmds, permissionListenerCmd(m.dispatcher))
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
		cmds = append(cmds, permissionListenerCmd(m.dispatcher))
		return m, tea.Batch(cmds...)

	case ScreenSettings:
		if m.settingsModel == nil {
			return m, nil
		}
		var cmd tea.Cmd
		(*m.settingsModel), cmd = m.settingsModel.Update(msg)
		cmds := []tea.Cmd{cmd}
		cmds = append(cmds, permissionListenerCmd(m.dispatcher))
		return m, tea.Batch(cmds...)

	case ScreenResume:
		if m.resumeModel == nil {
			return m, nil
		}
		cmds, appMsg := m.resumeModel.Update(msg)
		if appMsg != nil {
			m.screen = appMsg.Screen
			if appMsg.SessionID != "" {
				// SessionID will be used by the load handler
				m.currentOperation = fmt.Sprintf("Loading session %s...", appMsg.SessionID)
			}
		}
		cmds = append(cmds, permissionListenerCmd(m.dispatcher))
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
		cmds = append(cmds, permissionListenerCmd(m.dispatcher))
		return m, tea.Batch(cmds...)

	case ScreenPlan:
		if m.planModel != nil {
			cmds, appMsg := m.planModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
			}
			_ = cmds
		}
		cmds := []tea.Cmd{permissionListenerCmd(m.dispatcher)}
		return m, tea.Batch(cmds...)

	case ScreenExecute:
		if m.executeModel != nil {
			cmds, appMsg := m.executeModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
			}
			_ = cmds
		}
		cmds := []tea.Cmd{permissionListenerCmd(m.dispatcher)}
		return m, tea.Batch(cmds...)

	case ScreenVerify:
		if m.verifyModel != nil {
			cmds, appMsg := m.verifyModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
			}
			_ = cmds
		}
		cmds := []tea.Cmd{permissionListenerCmd(m.dispatcher)}
		return m, tea.Batch(cmds...)

	case ScreenShip:
		if m.shipModel != nil {
			cmds, appMsg := m.shipModel.Update(msg)
			if appMsg != nil {
				m.screen = appMsg.Screen
				if appMsg.Screen == ScreenFirstRun {
					m.replModel = nil
				}
			}
			_ = cmds
		}
		cmds := []tea.Cmd{permissionListenerCmd(m.dispatcher)}
		return m, tea.Batch(cmds...)

	default:
		return m, nil
	}
}

func (m *AppState) View() string {
	if !m.initialized || m.width == 0 {
		return "M31A — starting..."
	}

	if m.width < 40 || m.height < 10 {
		return fmt.Sprintf("Terminal too small: %dx%d (minimum 40x10)", m.width, m.height)
	}

	switch m.screen {
	case ScreenFirstRun:
		if m.firstRunModel != nil {
			return m.firstRunModel.View()
		}
		return "Loading..."

	case ScreenPermission:
		if m.permissionModal != nil {
			return m.permissionModal.Render(m.width, m.height)
		}
		return "Permission screen error"

	case ScreenREPL:
		if m.replModel == nil {
			return "Loading..."
		}

		t := m.themeManager.Current()

		header := RenderHeader(
			t,
			m.activeProvider,
			m.activeModel,
			m.healthStatus,
			m.contextUsed,
			m.contextTotal,
			m.width,
		)

		body := m.replModel.View()

		operation := m.currentOperation
		if m.replModel.streaming || m.replModel.thinking {
			operation = m.replModel.GetStatusText()
		}
		status := RenderStatusBar(t, operation, m.lastActivity, m.width)

		return lipgloss.JoinVertical(
			lipgloss.Top,
			header,
			body,
			status,
		)

	case ScreenSettings:
		if m.settingsModel != nil {
			return m.settingsModel.View()
		}
		return "Loading..."

	case ScreenResume:
		if m.resumeModel != nil {
			return m.resumeModel.View()
		}
		return "Loading..."

	case ScreenModelSelector:
		return m.modelSelector.View()

	case ScreenPlan:
		if m.planModel != nil {
			return m.planModel.View()
		}
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
			"Plan screen — driven by workflow engine")

	case ScreenExecute:
		if m.executeModel != nil {
			return m.executeModel.View()
		}
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
			"Execute screen — driven by workflow engine")

	case ScreenVerify:
		if m.verifyModel != nil {
			return m.verifyModel.View()
		}
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
			"Verify screen — driven by workflow engine")

	case ScreenShip:
		if m.shipModel != nil {
			return m.shipModel.View()
		}
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
			"Ship screen — driven by workflow engine")

	default:
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
			"Unknown screen")
	}
}

func calculateNextInterval(status types.HealthStatus) time.Duration {
	if status.Error != "" &&
		(strings.Contains(strings.ToLower(status.Error), "rate limit") ||
			strings.Contains(strings.ToLower(status.Error), "429")) {
		return 120 * time.Second
	}
	if status.Status == "offline" {
		return 120 * time.Second
	}
	return types.HealthCheckInterval
}
