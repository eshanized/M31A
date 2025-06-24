package tui

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
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
	currentOperation string
	width            int
	height           int
	lastActivity     time.Time
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
		app.replModel.SetCommandRegistry(app.cmdRegistry)
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
		app.replModel.SetCommandRegistry(app.cmdRegistry)
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
			// Wire execution metrics
			Usage:     result.Usage,
			Cost:      result.Cost,
			ToolCalls: result.ToolCalls,
			Commits:   result.Commits,
			DiffStats: result.DiffStats,
		}
	}

	// Return a batch: the runner executes the phase, the drainer reads
	// emitted messages until done is closed or a new phase starts.
	return tea.Batch(runner, workflowMsgDrainer(app, currentGen, doneCh))
}

// workflowMsgDrainer returns a tea.Cmd that reads one message from the
// workflow message channel. Captures the phaseGen at spawn time; if
// the gen changes (a new phase started), the drainer returns nil and
// stops. Uses a blocking select on msgCh and done — no polling.
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
		// Block until a message arrives, the phase completes, or a new phase starts.
		select {
		case msg, ok := <-app.msgChan:
			if !ok {
				return nil
			}
			return msg
		case <-done:
			return nil
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

// formatDurationMs converts milliseconds to a human-readable duration string.
func formatDurationMs(ms int64) string {
	d := time.Duration(ms) * time.Millisecond
	if d < time.Minute {
		return fmt.Sprintf("%ds", int(d.Seconds()))
	}
	if d < time.Hour {
		return fmt.Sprintf("%dm %ds", int(d.Minutes()), int(d.Seconds())%60)
	}
	return fmt.Sprintf("%dh %dm %ds", int(d.Hours()), int(d.Minutes())%60, int(d.Seconds())%60)
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
