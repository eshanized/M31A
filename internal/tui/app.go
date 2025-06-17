package tui

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
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
	"github.com/eshanized/M31A/pkg/autodream"
	"github.com/eshanized/M31A/pkg/keychain"
	"github.com/eshanized/M31A/pkg/ledger"
	"github.com/eshanized/M31A/pkg/rollback"
	"github.com/eshanized/M31A/pkg/session"
)

type FallbackNotification struct {
	Event     FallbackEventMsg
	Dismissed bool
	ShownAt   time.Time
}

// workflowEngineInterface captures the subset of *workflow.Engine methods
// that the TUI uses. Defining it as an interface lets tests inject a mock
// engine (see mockWorkflowEngine in app_test.go) without touching the
// production code path. The real *workflow.Engine satisfies this interface.
type workflowEngineInterface interface {
	SessionID() string
	SetSessionID(id string)
	SetMsgEmitter(em workflow.MsgEmitter)
	RunPhase(ctx context.Context, phase types.WorkflowPhase, goal string) (*workflow.PhaseResult, error)
	DiscussState() workflow.DiscussState
	SubmitDiscussAnswer(index int, answer string) error
	SkipDiscuss() error
	FinalizeDiscuss() error
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
	workflowEngine     workflowEngineInterface
	workflowGoal       string
	workflowRunning    bool
	currentPhase       types.WorkflowPhase
	discussQuestions   []string
	sessionID          string // session ID (set after initWorkflowEngine) — used for workflow state persistence
	// Discuss Q&A flow (D-01 fix)
	pendingDiscussAnswers map[int]string // index -> answer; nil when not in discuss Q&A
	currentDiscussIndex   int            // next question to ask (0-based)
	discussQuestionCount  int            // total questions in this discuss round
	discussAnswerTimeout  *time.Timer    // 5-minute per-question timer
	autoDream          *autodream.Consolidator
	// Workflow message bus (D-04 fix: per-phase lifecycle).
	// msgChan  : current phase's message channel (workflow → TUI)
	// msgDone  : closed by the runner goroutine when the phase completes
	// phaseGen : incremented on every RunPhaseCmd; drainer captures it
	//            at spawn time and stops if it changes (a new phase started)
	msgChan            chan tea.Msg
	msgDone            chan struct{}
	phaseGen           int
	workflowCtx        context.Context
	workflowCancel     context.CancelFunc
	git                *git.Git
	rollback           *rollback.Rollback
	healthCheckInFlight bool
	sidebarModel       *SidebarModel
	cmdPalette         *CommandPaletteModel
	cmdPaletteOpen     bool
	keyRegistry        *KeyRegistry
	toastText          string
	toastExpires       time.Time
	toastType          string
	sidebarManuallyHidden bool
	diffModel          DiffModel
}

func NewApp(version string, registry *provider.Registry, apiKey string, configPath string) *AppState {
	tm := theme.NewManager(theme.ModeDark)

	cwd, err := os.Getwd()
	if err != nil {
		cwd = "."
	}
	backupDir := filepath.Join(filepath.Dir(configPath), "backups")

	// Initialize config
	cfg, err := config.Load(configPath)
	if err != nil {
		// Config may not exist yet — use defaults
		cfg = config.DefaultConfig()
	}
	if cfg == nil {
		cfg = config.DefaultConfig()
	}

	// Initialize keychain (may be nil — keychain may be unavailable)
	kc, err := keychain.New()
	if err != nil {
		// Keychain unavailable — continue without it
		kc = nil
	}

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
	sessionIDBytes := 4 // 8 hex chars default
	if cfg.Features.SessionIDLength > 0 {
		sessionIDBytes = cfg.Features.SessionIDLength / 2
		if sessionIDBytes < 2 {
			sessionIDBytes = 2
		}
	}
	sessionMgr := session.NewManager(sessionBaseDir, session.ManagerOpts{
		SessionIDBytes:  sessionIDBytes,
		MaxRecentModels: cfg.Features.MaxRecentModels,
	})

	// Initialize git operations and rollback
	g := git.New(cwd)
	rb := rollback.New(g)

	dispatcher, err := tools.DefaultDispatcher(cwd, backupDir, sessionBaseDir, &cfg.Permissions)
	if err != nil {
		slog.Error("failed to initialize tool dispatcher", "error", err)
		os.Exit(1)
	}

	app := &AppState{
		version:        version,
		registry:       registry,
		apiKey:         apiKey,
		configPath:     configPath,
		themeManager:   tm,
		healthStatus:   types.HealthStatus{Status: "unknown"},
		dispatcher:     dispatcher,
		config:         cfg,
		keychain:       kc,
		sessionManager: sessionMgr,
		git:            g,
		rollback:       rb,
	}

	// Initialize ledger for settings stats display
	ledgerPath := filepath.Join(filepath.Dir(configPath), "LEDGER.md")
	ledgerInstance := ledger.New(ledgerPath)
	app.ledger = ledgerInstance

	// Initialize settings model (6 tabs, inline editing)
	sm := NewSettingsModel(cfg, configPath, tm.Current(), ledgerInstance, kc)
	app.settingsModel = &sm

	// Initialize resume model
	rm := NewResumeModel(tm.Current(), sessionMgr)
	app.resumeModel = rm

	// Initialize model selector (uses registry)
	if registry != nil {
		app.modelSelector = NewModelSelector(registry, sessionMgr, tm.Current())
	}

	// Initialize command registry
	app.cmdRegistry = DefaultCommands()

	// Initialize sidebar model
	app.sidebarModel = NewSidebarModel(g, tm.Current())

	// Initialize command palette
	app.cmdPalette = NewCommandPaletteModel()

	// Initialize key registry
	leaderTimeout := 1 * time.Second
	if cfg.UI.LeaderTimeoutMs > 0 {
		leaderTimeout = time.Duration(cfg.UI.LeaderTimeoutMs) * time.Millisecond
	}
	app.keyRegistry = NewKeyRegistry(KeyRegistryOpts{
		LeaderKey:     cfg.UI.LeaderKey,
		LeaderTimeout: leaderTimeout,
	})
	app.keyRegistry.RegisterDefaultBindings()

	if registry != nil {
		app.activeProvider = registry.Active()
	}

	// Auto-populate activeModel from provider cache using config default
	if app.activeProvider != "" && registry != nil && app.config != nil && app.config.Model.Default != "" {
		if p := registry.ActiveProvider(); p != nil {
			if model, _ := p.GetModel(app.config.Model.Default); model != nil {
				app.activeModel = model
			}
		}
	}

	app.initWorkflowEngine()

	// D-06 fix: check for a persisted workflow state from a previous
	// session. If the current session has a non-idle phase recorded,
	// pre-populate the AppState so /workflow resume can continue from
	// the saved point, and show a toast so the user is aware.
	app.checkResumedWorkflowState()

	// Initialize AutoDream consolidator with empty messages; will be
	// synced whenever a user message is submitted.
	app.autoDream = autodream.New(nil)

	if apiKey == "" {
		fr := NewFirstRunModel(tm.Current(), configPath)
		app.screen = ScreenFirstRun
		app.firstRunModel = &fr
	} else if registry == nil || registry.ActiveProvider() == nil {
		// No providers available — offline mode
		rp := NewReplModel(tm.Current())
		app.screen = ScreenREPL
		app.replModel = &rp
		app.replModel.SetProvider(registry, app.activeProvider, app.activeModel, "", app.config)
		app.replModel.SetDispatcher(app.dispatcher)
		app.healthStatus = types.HealthStatus{
			Status: "offline",
			Error:  "No providers available — offline mode. History is readable but no new messages.",
		}
		app.currentOperation = "No providers available — offline mode. History is readable but no new messages."
	} else {
		rp := NewReplModel(tm.Current())
		app.screen = ScreenREPL
		app.replModel = &rp
		app.replModel.SetProvider(registry, app.activeProvider, app.activeModel, "", app.config)
		app.replModel.SetDispatcher(app.dispatcher)
		app.healthStatus = types.HealthStatus{Status: "live"}
	}

	return app
}

// currentPhaseGen returns the current phaseGen value for snapshot use by
// the workflow drainer. The drainer captures this at spawn time and
// returns nil if the value changes (a new phase started). Returns 0
// when AppState is uninitialized.
func (m *AppState) currentPhaseGen() int {
	return m.phaseGen
}

func (m *AppState) initWorkflowEngine() {
	if m.registry == nil {
		m.currentOperation = "Workflow engine init failed: no provider registry"
		return
	}
	if m.activeProvider == "" {
		m.currentOperation = "Workflow engine init failed: no active provider"
		return
	}
	if m.sessionManager == nil {
		m.currentOperation = "Workflow engine init failed: no session manager"
		return
	}
	p := m.registry.ActiveProvider()
	if p == nil {
		m.currentOperation = "Workflow engine init failed: active provider is nil"
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
		m.currentOperation = "Workflow engine init failed: no model selected"
		return
	}

	// Create a session for the workflow
	s, err := m.sessionManager.NewSession(modelID, m.activeProvider)
	if err != nil {
		m.currentOperation = fmt.Sprintf("Workflow engine init failed: %v", err)
		return
	}

	// Wire session ID to tool dispatcher (e.g. TodoWrite)
	m.dispatcher.SetSessionID(s.ID)
	m.sessionID = s.ID

	cwd, err := os.Getwd()
	if err != nil {
		cwd = os.TempDir()
	}
	backupDir := filepath.Join(filepath.Dir(m.configPath), "backups")
	sessionBaseDir := filepath.Join(filepath.Dir(m.configPath), "sessions")
	planningDir := filepath.Join(sessionBaseDir, s.ID, "planning")

	g := git.New(cwd)

	est := tokens.NewEstimatorWithOpts(modelID, tokens.EstimatorOpts{
		EMAAlpha: m.config.Model.TokenEMAAlpha,
	})

	eng, err := workflow.NewEngine(s.ID, cwd, backupDir, planningDir,
		p, modelID, m.dispatcher, est, m.sessionManager)
	if err != nil {
		m.currentOperation = fmt.Sprintf("Workflow engine init failed: %v", err)
		return
	}
	eng.SetGit(g)
	m.workflowEngine = eng
}

// persistWorkflowState writes the current workflow state (goal, phase,
// pending discuss questions) to session.json. Failures are logged but
// not returned — the workflow continues even if persistence fails (the
// user can still finish the workflow in this session).
//
// Called on every phase transition so closing the app mid-workflow
// preserves progress. After the Ship phase, the persisted state is
// reset to idle so a future /workflow starts fresh.
func (m *AppState) persistWorkflowState() {
	if m.sessionManager == nil || m.sessionID == "" {
		return
	}
	if err := m.sessionManager.UpdateWorkflowState(
		m.sessionID, m.workflowGoal, m.currentPhase, m.discussQuestions,
	); err != nil {
		slog.Warn("persistWorkflowState failed", "err", err)
	}
}

// checkResumedWorkflowState reads the persisted workflow state for the
// current session. If a workflow was in progress (phase != idle, phase
// != ship), pre-populates the AppState and shows a toast so the user
// can run /workflow resume to continue. Called once during NewApp
// after initWorkflowEngine has set m.sessionID.
//
// The toast uses the AppState's existing toast fields (toastText,
// toastType, toastExpires) — not a dedicated ToastMsg queue — to match
// the convention used elsewhere in the TUI.
func (m *AppState) checkResumedWorkflowState() {
	if m.sessionManager == nil || m.sessionID == "" {
		return
	}
	goal, phase, questions, err := m.sessionManager.LoadWorkflowState(m.sessionID)
	if err != nil {
		// Persistence read failures are non-fatal; the workflow can
		// still start fresh. Log at debug level.
		slog.Debug("LoadWorkflowState failed", "err", err)
		return
	}
	if phase == types.PhaseIdle || phase == types.PhaseShip {
		// No workflow in progress — nothing to resume.
		return
	}
	// Workflow was in progress — pre-populate state for /workflow resume
	m.workflowGoal = goal
	m.currentPhase = phase
	m.discussQuestions = questions
	m.toastText = fmt.Sprintf("Resumable workflow at %s. Use /workflow resume to continue.", phase)
	m.toastType = "info"
	m.toastExpires = time.Now().Add(10 * time.Second)
}

// RunPhaseCmd returns a tea.Cmd that executes the given workflow phase in a
// goroutine and emits a PhaseResultMsg on completion. It also sets up a
// MsgEmitter on the engine so that TaskStartMsg and TaskUpdateMsg are emitted
// during execution and PlanReadyMsg when the plan phase completes.
//
// D-04 fix: per-phase lifecycle synchronization. Each call:
//   1. Cancels the previous phase's context
//   2. Increments app.phaseGen so the old drainer sees the change and stops
//   3. Closes the OLD app.msgDone (via safeClose) to signal the old drainer
//   4. Creates a fresh msgCh + doneCh pair
//   5. Captures the current phaseGen for the new drainer
//   6. Sets the engine's MsgEmitter to use the new channel
//
// The runner goroutine uses `defer close(doneCh)` to signal the drainer
// when the phase completes. The drainer selects on msgCh, done, and a
// 100ms poll timer, and returns nil if app.phaseGen has changed.
func RunPhaseCmd(app *AppState, phase types.WorkflowPhase, goal string) tea.Cmd {
	// 1. Cancel any previous phase's context to stop lingering goroutines
	if app.workflowCancel != nil {
		app.workflowCancel()
	}

	// 2. Increment phase generation FIRST so any drainer from the
	//    previous phase sees the change and stops on its next check.
	app.phaseGen++

	// 3. Close the OLD app.msgDone (defensive against double-close) to
	//    signal the old drainer to stop. The old channel reference is
	//    left in place until the drainer returns; we don't nil it out
	//    because the drainer might still be reading from it.
	if app.msgDone != nil {
		safeClose(app.msgDone)
	}

	// 4. Create new per-phase channels
	msgCh := make(chan tea.Msg, 256)
	doneCh := make(chan struct{})
	app.msgChan = msgCh
	app.msgDone = doneCh

	// Create a cancellable context for this phase
	ctx, cancel := context.WithCancel(context.Background())
	app.workflowCtx = ctx
	app.workflowCancel = cancel

	// 5. Capture the current phaseGen for the drainer. The drainer
	//    checks this on every invocation and returns nil if it changes.
	currentGen := app.phaseGen

	eng := app.workflowEngine
	eng.SetMsgEmitter(&channelEmitter{ch: msgCh})

	// Phase runner: executes the phase, emits PlanReadyMsg if applicable,
	// then closes the done channel to signal the drainer.
	runner := func() tea.Msg {
		defer close(doneCh) // <-- signal drainer when phase completes

		result, err := eng.RunPhase(ctx, phase, goal)
		cancel() // Ensure cleanup

		if err != nil {
			return PhaseResultMsg{Phase: phase, Error: err.Error()}
		}
		if result == nil {
			return PhaseResultMsg{Phase: phase, Error: "nil result"}
		}

		// Emit PlanReadyMsg when the plan phase completes successfully.
		if phase == types.PhasePlan && result.Success && len(result.Tasks) > 0 {
			select {
			case msgCh <- PlanReadyMsg{
				Tasks:        result.Tasks,
				CostEstimate: fmt.Sprintf("%d tasks", len(result.Tasks)),
				TimeEstimate: "",
			}:
			case <-time.After(500 * time.Millisecond):
				slog.Warn("dropped PlanReadyMsg: channel full")
			}
		}

		return PhaseResultMsg{
			Phase:               phase,
			Tasks:               result.Tasks,
			Messages:            result.Messages,
			Success:             result.Success,
			Error:               result.Error,
			NeedsAnswers:        result.NeedsAnswers,
			RequiresManualInput: result.RequiresManualInput,
			DurationMs:          result.DurationMs,
		}
	}

	// Return a batch: the runner executes the phase, the drainer reads
	// emitted messages until done is closed or a new phase starts.
	return tea.Batch(runner, workflowMsgDrainer(app, currentGen, doneCh))
}

// workflowMsgDrainer returns a tea.Cmd that reads one message from the
// workflow message channel. Captures the phaseGen at spawn time; if
// the gen changes (a new phase started), the drainer returns nil and
// stops. Selects on msgCh, done, and a 100ms poll timer to ensure
// forward progress even when no messages are pending.
//
// The Update handler should re-schedule the drainer with the CURRENT
// app.phaseGen and app.msgDone to continue reading.
func workflowMsgDrainer(app *AppState, gen int, done chan struct{}) tea.Cmd {
	return func() tea.Msg {
		// If the phase has been superseded, stop draining immediately.
		if app.phaseGen != gen {
			return nil
		}
		// If the done channel is closed, the phase is finished.
		select {
		case <-done:
			return nil
		default:
		}
		// Try to receive a message, but don't block forever.
		select {
		case msg, ok := <-app.msgChan:
			if !ok {
				return nil
			}
			return msg
		case <-done:
			return nil
		case <-time.After(100 * time.Millisecond):
			// Re-check gen; if changed, stop. Otherwise re-schedule.
			if app.phaseGen != gen {
				return nil
			}
			return workflowMsgDrainer(app, gen, done)()
		}
	}
}

// safeClose closes ch if it's non-nil and not already closed. Returns
// true if it actually performed the close, false otherwise. Used for
// defensive double-close protection on the per-phase msgDone channel.
func safeClose(ch chan struct{}) bool {
	if ch == nil {
		return false
	}
	select {
	case <-ch:
		// Already closed
		return false
	default:
		close(ch)
		return true
	}
}

// channelEmitter implements workflow.MsgEmitter by sending messages into a channel.
type channelEmitter struct {
	ch chan tea.Msg
}

func (ce *channelEmitter) Emit(msg tea.Msg) {
	select {
	case ce.ch <- msg:
	case <-time.After(500 * time.Millisecond):
		// Channel full after timeout — drop to avoid blocking the engine.
		slog.Warn("workflow message dropped: channel full", "msg_type", fmt.Sprintf("%T", msg))
	}
}

// resetDiscussQA clears the discuss Q&A state and stops the active timer.
// Called when leaving the discuss phase (finalize, skip, error) so subsequent
// Q&A rounds start from a clean slate.
func (m *AppState) resetDiscussQA() {
	if m.discussAnswerTimeout != nil {
		m.discussAnswerTimeout.Stop()
		m.discussAnswerTimeout = nil
	}
	m.pendingDiscussAnswers = nil
	m.currentDiscussIndex = 0
	m.discussQuestionCount = 0
}

// askNextDiscussQuestion emits a QuestionRequestMsg for the current
// question and starts a 5-minute timeout. Returns a tea.Cmd that
// produces both the question and the timeout (use tea.Batch).
func (m *AppState) askNextDiscussQuestion() tea.Cmd {
	if m.currentDiscussIndex >= len(m.discussQuestions) {
		// All questions answered — finalize and advance
		return m.finalizeDiscussAndAdvance()
	}
	q := m.discussQuestions[m.currentDiscussIndex]
	header := fmt.Sprintf("Discuss Q%d/%d", m.currentDiscussIndex+1, m.discussQuestionCount)

	// Stop any existing timeout
	if m.discussAnswerTimeout != nil {
		m.discussAnswerTimeout.Stop()
	}
	// Start 5-minute timeout
	m.discussAnswerTimeout = time.NewTimer(5 * time.Minute)

	return tea.Batch(
		func() tea.Msg {
			return QuestionRequestMsg{
				Question:    q,
				Header:      header,
				Options:     []string{},
				AllowCustom: true,
				ResponseCh:  m.dispatcher.QuestionResponseCh(),
			}
		},
		func() tea.Msg {
			<-m.discussAnswerTimeout.C
			return DiscussAnswerTimeoutMsg{QuestionIndex: m.currentDiscussIndex}
		},
	)
}

// finalizeDiscussAndAdvance calls engine.FinalizeDiscuss, then advances to Plan.
func (m *AppState) finalizeDiscussAndAdvance() tea.Cmd {
	if m.workflowEngine != nil {
		if err := m.workflowEngine.FinalizeDiscuss(); err != nil {
			slog.Warn("FinalizeDiscuss failed", "err", err)
		}
	}
	m.resetDiscussQA()
	m.currentPhase = types.PhasePlan
	return RunPhaseCmd(m, types.PhasePlan, m.workflowGoal)
}

// skipDiscussAndAdvance calls engine.SkipDiscuss, then advances to Plan.
func (m *AppState) skipDiscussAndAdvance() tea.Cmd {
	if m.workflowEngine != nil {
		if err := m.workflowEngine.SkipDiscuss(); err != nil {
			slog.Warn("SkipDiscuss failed", "err", err)
		}
	}
	return m.finalizeDiscussAndAdvance()
}

func (m *AppState) Init() tea.Cmd {
	cmds := []tea.Cmd{permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher)}
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

// questionListenerCmd returns a tea.Cmd that watches the dispatcher's
// question request channel and feeds requests into the Bubble Tea event loop.
func questionListenerCmd(dispatcher *tools.Dispatcher) tea.Cmd {
	return func() tea.Msg {
		req := <-dispatcher.QuestionRequestCh()
		return QuestionRequestMsg{
			Question:    req.Question,
			Header:      req.Header,
			Options:     req.Options,
			AllowCustom: req.AllowCustom,
			ResponseCh:  dispatcher.QuestionResponseCh(),
		}
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
		if m.sidebarModel != nil {
			m.sidebarModel.Update(msg)
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
				cmds := m.cmdRegistry.AllCommands()
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
				m.workflowRunning = false
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
				m.workflowRunning = true
				m.currentPhase = phase
				return m, RunPhaseCmd(m, phase, goal)
			}
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
				m.workflowRunning = true
				m.currentPhase = types.PhaseInitialize
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
				m.currentPhase = result.ResumePhase
				m.discussQuestions = result.ResumeQuestions
				m.workflowRunning = true
				return m, RunPhaseCmd(m, result.ResumePhase, result.ResumeGoal)
			}

			// Session switching: /fork, /prev, /next set SessionID to transition
			if result.SessionID != nil && *result.SessionID != sessionID {
				if sess, err := m.sessionManager.LoadSession(*result.SessionID); err == nil && sess != nil {
					if m.replModel == nil {
						rp := NewReplModel(m.themeManager.Current())
						m.replModel = &rp
					}
					m.replModel.SetProvider(m.registry, sess.Provider, m.activeModel, sess.ID, m.config)
					m.replModel.SetDispatcher(m.dispatcher)
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
				}
				if result.Cmd != nil {
					return m, tea.Batch(result.Cmd)
				}
				return m, nil
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
			return m, NextHealthTick(5 * time.Second)
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
		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		p := m.registry.ActiveProvider()
		if p == nil {
			cancel()
			m.healthCheckInFlight = false
			return m, NextHealthTick(types.HealthCheckInterval)
		}

		result := p.HealthCheck(ctx)
		cancel()
		m.healthCheckInFlight = false
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
		// Workflow phase routing: sub-models (Plan, Execute, Verify, Ship)
		// emit AppMsg{Screen: ScreenExecute|Verify|Ship} to drive the next
		// workflow phase. We dispatch to RunPhaseCmd instead of just
		// changing m.screen so the engine actually runs the phase
		// (D-02/D-05 fix).
		switch msg.Screen {
		case ScreenExecute:
			if m.workflowEngine == nil {
				return m, nil
			}
			m.currentPhase = types.PhaseExecute
			return m, RunPhaseCmd(m, types.PhaseExecute, m.workflowGoal)
		case ScreenVerify:
			if m.workflowEngine == nil {
				return m, nil
			}
			m.currentPhase = types.PhaseVerify
			return m, RunPhaseCmd(m, types.PhaseVerify, m.workflowGoal)
		case ScreenShip:
			if m.workflowEngine == nil {
				return m, nil
			}
			m.currentPhase = types.PhaseShip
			return m, RunPhaseCmd(m, types.PhaseShip, m.workflowGoal)
		}

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
			m.initialized = true
			// Init sidebar
			if m.sidebarModel == nil {
				m.sidebarModel = NewSidebarModel(m.git, m.themeManager.Current())
			}
			// Create a session so that /status, /save, etc. work
			sessionID := ""
			if m.sessionManager != nil && m.activeModel != nil && m.activeProvider != "" {
				s, err := m.sessionManager.NewSession(m.activeModel.ID, m.activeProvider)
				if err == nil {
					sessionID = s.ID
					m.dispatcher.SetSessionID(s.ID)
				}
			}
			m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, sessionID, m.config)
			m.replModel.SetDispatcher(m.dispatcher)
			// Size the REPL immediately with current window dimensions
			if m.width > 0 && m.height > 0 {
				m.replModel.Update(tea.WindowSizeMsg{Width: m.width, Height: m.height})
			}
			// Don't auto-show sidebar until git status is loaded
		}
		if msg.Screen == ScreenModelSelector {
			m.prevScreen = m.screen
			m.modelSelector = NewModelSelector(m.registry, m.sessionManager, m.themeManager.Current())
			m.screen = ScreenModelSelector
			return m, m.modelSelector.Init()
		}
		m.screen = msg.Screen
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
						m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, m.replModel.sessionID, m.config)
						m.replModel.SetDispatcher(m.dispatcher)
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
		return m, tea.Batch(
			permissionListenerCmd(m.dispatcher),
			questionListenerCmd(m.dispatcher),
			tea.Every(100*time.Millisecond, func(t time.Time) tea.Msg {
				return PermissionTickMsg{}
			}),
		)

	case PermissionResponseMsg:
		m.dispatcher.ApprovePermission(msg.Response.Allowed, msg.Response.Remember)
		m.screen = m.prevScreen
		m.permissionModal = nil
		return m, tea.Batch(permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher))

	case PermissionTickMsg:
		if m.screen == ScreenPermission && m.permissionModal != nil {
			m.permissionModal.Tick()
			if m.permissionModal.Remaining() <= 0 {
				// Auto-deny on timeout
				resp := m.permissionModal.Deny()
				m.dispatcher.ApprovePermission(resp.Allowed, resp.Remember)
				m.screen = m.prevScreen
				m.permissionModal = nil
				return m, tea.Batch(permissionListenerCmd(m.dispatcher), questionListenerCmd(m.dispatcher))
			}
		}
		return m, tea.Every(100*time.Millisecond, func(t time.Time) tea.Msg {
			return PermissionTickMsg{}
		})

	case QuestionRequestMsg:
		// Display question in REPL and wait for user answer
		if m.replModel != nil {
			m.replModel.ShowQuestion(msg)
		}
		return m, questionListenerCmd(m.dispatcher)

	case QuestionResponseMsg:
		// If we're in the discuss Q&A flow, route the answer to the engine.
		if m.pendingDiscussAnswers != nil && m.workflowEngine != nil {
			idx := m.currentDiscussIndex
			if err := m.workflowEngine.SubmitDiscussAnswer(idx, msg.Answer); err != nil {
				slog.Warn("SubmitDiscussAnswer failed", "idx", idx, "err", err)
			} else {
				m.pendingDiscussAnswers[idx] = msg.Answer
			}
			m.currentDiscussIndex++
			if m.currentDiscussIndex >= m.discussQuestionCount {
				return m, m.finalizeDiscussAndAdvance()
			}
			return m, m.askNextDiscussQuestion()
		}
		// Otherwise, forward to the question tool via dispatcher
		// (existing behavior for AskUserQuestion tool)
		dresp := tools.QuestionResponse{Answer: msg.Answer}
		select {
		case m.dispatcher.QuestionResponseCh() <- dresp:
		default:
		}
		return m, questionListenerCmd(m.dispatcher)

	case DiscussAnswerTimeoutMsg:
		// 5-minute timeout — skip remaining questions and advance
		if m.pendingDiscussAnswers == nil {
			return m, nil // Not in discuss Q&A; ignore
		}
		m.currentOperation = fmt.Sprintf("Discuss timeout on Q%d", m.currentDiscussIndex+1)
		return m, m.skipDiscussAndAdvance()

	case PlanReadyMsg:
		// Plan phase completed with valid tasks — update the plan screen.
		if len(msg.Tasks) > 0 && m.planModel == nil {
			t := m.themeManager.Current()
			modelID := ""
			if m.activeModel != nil {
				modelID = m.activeModel.ID
			}
			pm := NewPlanModel(msg.Tasks, t, modelID, m.activeProvider, 0, msg.CostEstimate, m.width, m.height)
			m.planModel = pm
		}
		m.currentOperation = fmt.Sprintf("Plan ready: %d tasks", len(msg.Tasks))
		return m, workflowMsgDrainer(m, m.phaseGen, m.msgDone)

	case workflow.TaskStartMsg:
		// A task has begun execution — update the execute model.
		m.currentOperation = fmt.Sprintf("Running task %d: %s", msg.Task.ID, msg.Task.Action)
		if m.executeModel != nil {
			m.executeModel.UpdateTaskStatus(msg.Task.ID, types.StatusRunning)
		}
		return m, workflowMsgDrainer(m, m.phaseGen, m.msgDone)

	case workflow.TaskUpdateMsg:
		// A task status changed — update the execute model.
		m.currentOperation = fmt.Sprintf("Task %d: %s", msg.Task.ID, msg.Status)
		if m.executeModel != nil {
			switch msg.Status {
			case "done":
				m.executeModel.UpdateTaskStatus(msg.Task.ID, types.StatusDone)
			case "failed":
				m.executeModel.UpdateTaskStatus(msg.Task.ID, types.StatusFailed)
			}
		}
		return m, workflowMsgDrainer(m, m.phaseGen, m.msgDone)

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
			m.persistWorkflowState()
			return m, RunPhaseCmd(m, types.PhaseDiscuss, m.workflowGoal)

		case types.PhaseDiscuss:
			// D-01 fix: wire the Discuss Q&A flow.
			//
			// Extract the parsed questions from the engine's discuss state
			// (the engine already populated e.discussState.Questions in runDiscuss).
			if m.workflowEngine == nil {
				m.currentOperation = "Discuss phase: no engine"
				m.workflowRunning = false
				return m, nil
			}
			engineState := m.workflowEngine.DiscussState()
			questions := engineState.Questions

			// For backward compat, also fall back to scanning Messages for
			// assistant content (matches the old code path).
			if len(questions) == 0 {
				for _, msg2 := range msg.Messages {
					if msg2.Role == "assistant" {
						questions = append(questions, msg2.Content)
					}
				}
			}

			m.discussQuestions = questions
			m.discussQuestionCount = len(questions)

			if !msg.NeedsAnswers || len(questions) == 0 {
				// No questions — auto-advance to Plan (unchanged behavior)
				m.currentPhase = types.PhasePlan
				m.persistWorkflowState()
				return m, RunPhaseCmd(m, types.PhasePlan, m.workflowGoal)
			}

			// Initialize Q&A state and ask the first question
			m.pendingDiscussAnswers = make(map[int]string)
			m.currentDiscussIndex = 0
			m.currentPhase = types.PhaseDiscuss
			m.screen = ScreenREPL
			m.persistWorkflowState()

			return m, m.askNextDiscussQuestion()

		case types.PhasePlan:
			if len(msg.Tasks) > 0 && m.planModel == nil {
				t := m.themeManager.Current()
				modelID := ""
				if m.activeModel != nil {
					modelID = m.activeModel.ID
				}
				providerName := m.activeProvider
				pm := NewPlanModel(msg.Tasks, t, modelID, providerName, 0, "", m.width, m.height)
				m.planModel = pm
			}
			if m.planModel != nil {
				// Ensure latest window dimensions are reflected in the model
				m.planModel.width = m.width
				m.planModel.height = m.height
				m.screen = ScreenPlan
				m.currentPhase = types.PhasePlan
				m.persistWorkflowState()
				return m, nil // Stop auto-advance — wait for user 'A' press
			}
			// No tasks — skip ahead to Execute
			m.currentPhase = types.PhaseExecute
			m.persistWorkflowState()
			return m, RunPhaseCmd(m, types.PhaseExecute, m.workflowGoal)

		case types.PhaseExecute:
			t := m.themeManager.Current()
			m.executeModel = NewExecuteModel(msg.Tasks, t, m.width, m.height)
			m.executeModel.width = m.width
			m.executeModel.height = m.height
			m.screen = ScreenExecute
			m.currentPhase = types.PhaseExecute
			m.persistWorkflowState()
			return m, nil // Stop auto-advance to Verify — wait for AppMsg from execute

		case types.PhaseVerify:
			t := m.themeManager.Current()
			results := make(map[int]workflow.VerificationResult)
			m.verifyModel = NewVerifyModel(msg.Tasks, results, t, m.width, m.height)
			m.verifyModel.width = m.width
			m.verifyModel.height = m.height
			m.screen = ScreenVerify
			m.currentPhase = types.PhaseVerify
			m.persistWorkflowState()
			return m, nil // Stop auto-advance to Ship — wait for AppMsg from verify

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
			m.shipModel = NewShipModel(summary, t, m.width, m.height)
			m.screen = ScreenShip
			m.workflowRunning = false
			m.persistWorkflowState()
			// After successful ship, reset the persisted workflow state to
			// idle so a future /workflow starts fresh (and so the next
			// startup doesn't show a stale resume toast).
			if m.sessionManager != nil && m.sessionID != "" {
				if err := m.sessionManager.UpdateWorkflowState(
					m.sessionID, "", types.PhaseIdle, nil,
				); err != nil {
					slog.Warn("failed to reset workflow state after ship", "err", err)
				}
			}
			m.workflowGoal = ""
			m.currentPhase = types.PhaseIdle
			return m, nil

		default:
			return m, nil
		}

	case SettingsSavedMsg:
		m.currentOperation = "Settings saved"
		// Reload config into active components
		if m.config != nil && m.settingsModel != nil {
			m.settingsModel.SetConfig(m.config)
		}

		// If the default model or provider changed, update active provider/model
		if m.config != nil && m.registry != nil {
			cfgProvider := m.config.Provider.Default
			if cfgProvider != "" && m.activeProvider != cfgProvider {
				if err := m.registry.SetActive(cfgProvider); err == nil {
					m.activeProvider = cfgProvider
					m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, "", m.config)
					m.replModel.SetDispatcher(m.dispatcher)
				}
			}

			cfgModel := m.config.Model.Default
			if cfgModel != "" && (m.activeModel == nil || m.activeModel.ID != cfgModel) {
				if p := m.registry.ActiveProvider(); p != nil {
					ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
					if models, err := p.FetchModels(ctx); err == nil {
						for _, mi := range models {
							if mi.ID == cfgModel {
								m.activeModel = &mi
								m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, "", m.config)
								m.replModel.SetDispatcher(m.dispatcher)
								break
							}
						}
					}
					cancel()
				}
			}
		}

		if m.screen == ScreenREPL && m.registry != nil && m.activeProvider != "" {
			m.lastActivity = time.Now()
		}
		return m, tea.Batch(
			NextHealthTick(types.HealthCheckInterval),
			NextCacheRefreshTick(provider.DefaultCacheRefreshInterval),
		)

	case SidebarRefreshMsg:
		if m.sidebarModel != nil {
			m.sidebarModel.Update(msg)
			// Auto-show sidebar on wide terminals now that status is loaded
			if m.width > 120 && m.replModel != nil && !m.sidebarManuallyHidden {
				m.sidebarModel.SetVisible(true)
				m.replModel.SetSidebarWidth(sidebarWidth)
			}
		}
		return m, nil

	case ToastMsg:
		m.toastText = msg.Text
		m.toastExpires = time.Now().Add(msg.Duration)
		m.toastType = msg.Type
		return m, nil

	case ThemeChangedMsg:
		switch msg.Theme {
		case "dark":
			m.themeManager = theme.NewManager(theme.ModeDark)
		case "light":
			m.themeManager = theme.NewManager(theme.ModeLight)
		}
		t := m.themeManager.Current()
		if m.replModel != nil {
			m.replModel.SetTheme(t)
		}
		if m.modelSelector.registry != nil {
			m.modelSelector.theme = t
		}
		if m.sidebarModel != nil {
			m.sidebarModel.SetTheme(t)
		}
		if m.settingsModel != nil {
			m.settingsModel.SetTheme(t)
		}
		return m, nil
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

				// Create a session so that /status, /save, etc. work
				sessionID := ""
				if m.sessionManager != nil && m.activeModel != nil && m.activeProvider != "" {
					s, err := m.sessionManager.NewSession(m.activeModel.ID, m.activeProvider)
					if err == nil {
						sessionID = s.ID
						m.dispatcher.SetSessionID(s.ID)
					}
				}

				m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, sessionID, m.config)
				m.replModel.SetDispatcher(m.dispatcher)
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
					CacheRefreshTicker(m.activeProvider, provider.DefaultCacheRefreshInterval))
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
		// Permission modal only handles key events; non-key messages are ignored
		// but the modal remains visible
		return m, nil

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
						rp := NewReplModel(m.themeManager.Current())
						m.replModel = &rp
					}
					m.replModel.SetProvider(m.registry, sess.Provider, m.activeModel, sess.ID, m.config)
					m.replModel.SetDispatcher(m.dispatcher)
					for _, msg := range sess.Messages {
						m.replModel.AddMessage(msg)
					}
					m.currentOperation = fmt.Sprintf("Session %s loaded", appMsg.SessionID)
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
				if appMsg.Screen == ScreenFirstRun {
					m.replModel = nil
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

func (m *AppState) View() string {
	if !m.initialized || m.width == 0 {
		return "M31A — starting..."
	}

	if m.width < 40 || m.height < 10 {
		return fmt.Sprintf("Terminal too small: %dx%d (minimum 40x10)", m.width, m.height)
	}

	// Check if toast has expired
	if m.toastText != "" && time.Now().After(m.toastExpires) {
		m.toastText = ""
		m.toastType = ""
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

		// Update REPL model with current app state for View() rendering
		m.replModel.SetKeyRegistry(m.keyRegistry)
		m.replModel.SetLastActivity(m.lastActivity)

		var mainContent string
		if m.sidebarModel != nil && m.sidebarModel.IsVisible() {
			sidebar := m.sidebarModel.View()
			replView := m.replModel.View()
			mainContent = lipgloss.JoinHorizontal(lipgloss.Top, replView, sidebar)
		} else {
			mainContent = m.replModel.View()
		}

		return m.renderWithPalette(mainContent)

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

	case ScreenDiff:
		if m.diffModel.lines != nil || m.diffModel.diff != "" {
			return m.renderToast(m.diffModel.View())
		}
		return m.renderToast(lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
			"Loading diff..."))

	default:
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
			"Unknown screen")
	}
}

func (m *AppState) renderToast(content string) string {
	if m.toastText == "" {
		return content
	}
	var color lipgloss.Color
	switch m.toastType {
	case "success":
		color = m.themeManager.Current().Success
	case "warning":
		color = m.themeManager.Current().Warning
	case "error":
		color = m.themeManager.Current().Error
	default:
		color = m.themeManager.Current().Thinking
	}
	toastStyle := lipgloss.NewStyle().
		Foreground(color).
		Background(m.themeManager.Current().Surface).
		Padding(0, 2).
		Bold(true)
	toast := toastStyle.Render(m.toastText)
	return lipgloss.JoinVertical(lipgloss.Center,
		lipgloss.PlaceHorizontal(m.width, lipgloss.Center, toast),
		content,
	)
}

func (m *AppState) renderWithPalette(base string) string {
	if m.cmdPaletteOpen && m.cmdPalette != nil {
		palette := m.cmdPalette.View()
		if palette != "" {
			return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center, palette)
		}
	}
	return base
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

func currentKeyContext(screen Screen) KeyContext {
	switch screen {
	case ScreenREPL:
		return CtxREPL
	case ScreenSettings:
		return CtxSettings
	case ScreenModelSelector:
		return CtxModelSel
	case ScreenResume:
		return CtxResume
	case ScreenFirstRun:
		return CtxFirstRun
	default:
		return CtxGlobal
	}
}

// cycleRecentModel cycles the active model through the recent models list.
// Direction +1 = forward (newer), -1 = backward (older).
func (m *AppState) cycleRecentModel(direction int) {
	if m.sessionManager == nil || m.activeProvider == "" {
		return
	}

	data, err := m.sessionManager.LoadRecentModels()
	if err != nil || len(data.Recent) == 0 {
		return
	}

	// Find current model index in recent list
	currentIdx := -1
	currentID := ""
	if m.activeModel != nil {
		currentID = m.activeModel.ID
		for i, id := range data.Recent {
			if id == currentID {
				currentIdx = i
				break
			}
		}
	}

	// Compute target index with wrap-around
	targetIdx := 0
	if currentIdx >= 0 {
		targetIdx = currentIdx + direction
		if targetIdx < 0 {
			targetIdx = len(data.Recent) - 1
		} else if targetIdx >= len(data.Recent) {
			targetIdx = 0
		}
	}

	// Look up model from active provider
	provider, err := m.registry.Get(m.activeProvider)
	if err != nil {
		return
	}
	model, err := provider.GetModel(data.Recent[targetIdx])
	if err != nil {
		return
	}

	m.activeModel = model
	if m.replModel != nil {
			m.replModel.SetProvider(m.registry, m.activeProvider, m.activeModel, m.replModel.sessionID, m.config)
			m.replModel.SetDispatcher(m.dispatcher)
		}

	// Mark the model as recently used
	m.sessionManager.AddRecentModel(model.ID) //nolint:errcheck

	// Emit toast feedback
	m.toastText = fmt.Sprintf("Model: %s", model.Name)
	m.toastExpires = time.Now().Add(2 * time.Second)
	m.toastType = "info"
}

func (m *AppState) handleKeyAction(msg KeyActionMsg) (*AppState, tea.Cmd) {
	switch msg.Action {
	case "toggle_sidebar":
		if m.sidebarModel != nil {
			m.sidebarModel.Toggle()
			m.sidebarManuallyHidden = !m.sidebarModel.IsVisible()
			if m.sidebarModel.IsVisible() {
				if m.replModel != nil {
					m.replModel.SetSidebarWidth(sidebarWidth)
				}
				return m, m.sidebarModel.refreshCmd()
			}
			if m.replModel != nil {
				m.replModel.SetSidebarWidth(0)
			}
		}
	case "open_settings":
		m.screen = ScreenSettings
	case "new_session":
		m.screen = ScreenFirstRun
		m.replModel = nil
	case "session_list":
		if m.resumeModel != nil {
			m.resumeModel.Refresh()
		}
		m.screen = ScreenResume
	case "cycle_model":
		if m.registry != nil {
			m.prevScreen = m.screen
			m.modelSelector = NewModelSelector(m.registry, m.sessionManager, m.themeManager.Current())
			m.screen = ScreenModelSelector
			return m, m.modelSelector.Init()
		}
	case "cycle_model_forward":
		if m.sessionManager != nil {
			m.cycleRecentModel(+1)
			return m, nil
		}
	case "cycle_model_backward":
		if m.sessionManager != nil {
			m.cycleRecentModel(-1)
			return m, nil
		}
	case "toggle_theme":
		if m.themeManager != nil {
			m.themeManager.Cycle()
			t := m.themeManager.Current()
			if m.replModel != nil {
				m.replModel.SetTheme(t)
			}
			if m.sidebarModel != nil {
				m.sidebarModel.SetTheme(t)
			}
			if m.settingsModel != nil {
				m.settingsModel.SetTheme(t)
			}
		}
	}
	return m, nil
}
