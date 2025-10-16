package tui

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"sync"
	"sync/atomic"
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

const offlineModeMsg = "No providers available — offline mode. History is readable but no new messages."

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
	SetModel(modelID string, p provider.LLMProvider)
	SetMsgEmitter(em workflow.MsgEmitter)
	RunPhase(ctx context.Context, phase types.WorkflowPhase, goal string) (*workflow.PhaseResult, error)
	Transition(ctx context.Context, from, to types.WorkflowPhase) error
	DiscussState() workflow.DiscussState
	SubmitDiscussAnswer(index int, answer string) error
	SkipDiscuss() error
	FinalizeDiscuss() error
	HealTask(taskID int) bool
}

type AppState struct {
	screen               Screen
	version              string
	initialized          bool
	registry             *provider.Registry
	activeProvider       string
	activeModel          *types.ModelInfo
	currentOperation     string
	width                int
	height               int
	lastActivity         time.Time
	healthStatus         types.HealthStatus
	healthStatusAtomic   atomic.Value // atomic copy for safe reads from View goroutine
	themeManager         *theme.Manager
	firstRunModel        *FirstRunModel
	replModel            *ReplModel
	settingsModel        *SettingsModel
	resumeModel          *ResumeModel
	sessionManager       *session.Manager
	keychain             keychain.Keychain
	config               *config.Config
	configPath           string
	ledger               *ledger.Ledger
	prevScreen           Screen
	permissionModal      *components.PermissionModal
	dispatcher           *tools.Dispatcher
	modelSelector        ModelSelector
	fallbackNotification *FallbackNotification
	cmdRegistry          *CommandRegistry
	planModel            *PlanModel
	executeModel         *ExecuteModel
	verifyModel          *VerifyModel
	shipModel            *ShipModel
	workflowEngine       workflowEngineInterface
	workflowGoal         string
	workflowRunning      bool
	currentPhase         types.WorkflowPhase
	discussQuestions     []string
	sessionID            string // session ID (set after initWorkflowEngine) — used for workflow state persistence
	// Discuss Q&A flow (D-01 fix)
	pendingDiscussAnswers map[int]string // index -> answer; nil when not in discuss Q&A
	currentDiscussIndex   int            // next question to ask (0-based)
	discussQuestionCount  int            // total questions in this discuss round
	discussAnswerTimeout  *time.Timer    // 5-minute per-question timer
	autoDream             *autodream.Consolidator
	// Workflow message bus (D-04 fix: per-phase lifecycle).
	// msgChan  : current phase's message channel (workflow → TUI)
	// msgDone  : closed by the runner goroutine when the phase completes
	// phaseGen : incremented on every RunPhaseCmd; drainer captures it
	//            at spawn time and stops if it changes (a new phase started)
	msgChan               chan tea.Msg
	msgDoneCloser         *channelCloser
	phaseGen              int
	workflowCtx           context.Context
	workflowCancel        context.CancelFunc
	git                   *git.Git
	rollback              *rollback.Rollback
	healthCheckInFlight   bool
	sidebarModel          *SidebarModel
	cmdPalette            *CommandPaletteModel
	cmdPaletteOpen        bool
	keyRegistry           *KeyRegistry
	toastText             string
	toastExpires          time.Time
	toastType             string
	sidebarManuallyHidden bool
	diffModel             DiffModel
	// H-10/M-26: Header render cache — avoids re-rendering the header
	// string via lipgloss on every TickMsg when nothing changed.
	headerCacheKey   uint64 // FNV-1a hash of (provider, modelID, ctxUsed, ctxTotal, healthStatus, logLevel)
	headerCacheValue string
	headerCacheValid bool
	// FEAT-3: Config hot-reload
	configReloadCh    chan config.ConfigReloadMsg
	configWatchCtx    context.Context
	configWatchCancel context.CancelFunc
	configWatcherWg   sync.WaitGroup // CR-06: track config watcher goroutine for clean shutdown
	// CR-07: shutdown context for listener goroutines
	shutdownCtx    context.Context
	shutdownCancel context.CancelFunc
	listenerWg     sync.WaitGroup
	// Phase 21: state synchronization and message flow
	pendingStreamChunks         []*types.StreamChunk // buffered during non-discuss workflow phases
	permissionModalActive       bool                 // true while permission modal is displayed
	pendingPermissionRequests   []PermissionRequestMsg // queued when modal already active
	pendingPermissionRequestID  int64                // RC-2: request ID for correlation with response
	workflowPaused              bool                 // true when workflow paused due to provider error
	workflowStartTime           time.Time            // when the current workflow started
	healthTicker                *time.Ticker         // health check ticker (stopped in Shutdown)

	// New screens from TUI redesign proposal (section 6.2)
	metricsModel        *MetricsModel   // ScreenMetrics — session analytics dashboard
	goalInputModel      *GoalInputModel // ScreenGoalInput — full-screen goal entry
	showPhaseBreadcrumb bool            // show workflow phase breadcrumb row below header

	// Phase 26: new screens
	ledgerModel   *LedgerModel
	rollbackModel *RollbackModel
	discussModel  *DiscussModel
}

func NewApp(version string, registry *provider.Registry, configPath string) (*AppState, error) {
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
	resolvedAPIKey := ""
	if kc != nil {
		cfg.ResolveAPIKeys(kc)
	}
	resolvedAPIKey = cfg.Provider.OpenRouter.APIKey
	if resolvedAPIKey == "" {
		resolvedAPIKey = cfg.Provider.Zen.APIKey
	}

	// L-2: Reconcile default provider with available keys
	if cfg.Provider.Default != "" && resolvedAPIKey != "" {
		defaultHasKey := false
		switch cfg.Provider.Default {
		case "openrouter":
			defaultHasKey = cfg.Provider.OpenRouter.APIKey != ""
		case "zen":
			defaultHasKey = cfg.Provider.Zen.APIKey != ""
		}
		if !defaultHasKey {
			var switchedTo string
			if cfg.Provider.OpenRouter.APIKey != "" {
				switchedTo = "openrouter"
			} else if cfg.Provider.Zen.APIKey != "" {
				switchedTo = "zen"
			}
			if switchedTo != "" {
				slog.Warn("provider config mismatch", "default", cfg.Provider.Default, "switched_to", switchedTo)
				cfg.Provider.Default = switchedTo
			}
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

	// Session auto-cleanup: remove sessions older than configured retention
	retentionDays := cfg.Features.SessionRetentionDays
	if retentionDays <= 0 {
		retentionDays = 30
	}
	if removed, err := sessionMgr.Cleanup(time.Duration(retentionDays) * 24 * time.Hour); err != nil {
		slog.Warn("session cleanup failed", "error", err)
	} else if removed > 0 {
		slog.Info("cleaned up old sessions", "count", removed)
	}

	// Initialize git operations and rollback
	g := git.New(cwd)
	rb := rollback.New(g)

	tools.SetVersion(version)
	dispatcher, err := tools.DefaultDispatcher(cwd, backupDir, sessionBaseDir, &cfg.Permissions)
	if err != nil {
		return nil, fmt.Errorf("failed to initialize tool dispatcher: %w", err)
	}

	app := &AppState{
		version:        version,
		registry:       registry,
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
	// CR-07: initialize shutdown context for listener goroutines
	app.shutdownCtx, app.shutdownCancel = context.WithCancel(context.Background())

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
	if cfg.UI.SidebarWidth > 0 {
		app.sidebarModel.width = cfg.UI.SidebarWidth
	}

	// Initialize command palette
	app.cmdPalette = NewCommandPaletteModel(tm.Current())

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
	// Task 5: check BEFORE initWorkflowEngine creates a new session,
	// and load session history for the resumed session.
	if sessions, err := sessionMgr.ListSessions(); err == nil && len(sessions) > 0 {
		recent := sessions[0]
		if goal, phase, questions, err := sessionMgr.LoadWorkflowState(recent.ID); err == nil {
			if phase != types.PhaseIdle && phase != types.PhaseShip {
				app.sessionID = recent.ID
				app.workflowGoal = goal
				app.currentPhase = phase
				app.discussQuestions = questions
				app.toastText = fmt.Sprintf("Resumable workflow at %s. Use /workflow resume to continue.", phase)
				app.toastType = "info"
				app.toastExpires = time.Now().Add(types.ToastDuration)
				// Load session history for the resumed session
				if sess, err := sessionMgr.LoadSession(recent.ID); err == nil && sess != nil {
					for _, msg := range sess.Messages {
						app.replModel.AddMessage(msg)
					}
				}
			}
		}
	}

	// Initialize AutoDream consolidator with empty messages; will be
	// synced whenever a user message is submitted.
	app.autoDream = autodream.New(nil)

	// Populate session activity sparkline on the REPL's provider card
	// when we have enough history. Best-effort — errors fall back to
	// an empty sparkline.
	if app.replModel != nil {
		app.replModel.SetSessionSparkline(app.sessionHealthSparkline())
	}

	// FEAT-3: Start config file watcher for hot-reload
	app.configReloadCh = make(chan config.ConfigReloadMsg, 1)
	app.configWatchCtx, app.configWatchCancel = context.WithCancel(context.Background())
	app.configWatcherWg.Add(1)
	go func() {
		defer app.configWatcherWg.Done()
		config.WatchConfig(app.configWatchCtx, configPath, app.configReloadCh)
	}()

	if resolvedAPIKey == "" {
		fr := NewFirstRunModel(tm.Current(), configPath, version, FirstRunOpts{
			OpenRouterBaseURL: cfg.Provider.OpenRouterBaseURL,
			ZenBaseURL:        cfg.Provider.ZenBaseURL,
			OpenRouterReferer: cfg.Provider.OpenRouterReferer,
			OpenRouterTitle:   cfg.Provider.OpenRouterTitle,
		})
		app.screen = ScreenFirstRun
		app.firstRunModel = &fr
	} else if cfg.Features.ResumeOnStartup {
		// ResumeOnStartup: prefer the session browser so the user can
		// pick up an existing conversation instead of starting fresh.
		// Fall through to the REPL path if no sessions are available.
		if sessions, err := sessionMgr.ListSessions(); err == nil && len(sessions) > 0 {
			app.screen = ScreenResume
		} else {
			rp := NewReplModel(tm.Current(), version)
			app.screen = ScreenREPL
			app.replModel = &rp
			_ = app.replModel.SetProvider(registry, app.activeProvider, app.activeModel, "", app.config)
			app.replModel.SetDispatcher(app.dispatcher)
			app.replModel.SetCommandRegistry(app.cmdRegistry)
			if registry == nil || registry.ActiveProvider() == nil {
				app.healthStatus = types.HealthStatus{
					Status: "offline",
					Error:  offlineModeMsg,
				}
				app.currentOperation = offlineModeMsg
			} else {
				app.healthStatus = types.HealthStatus{Status: "live"}
			}
		}
	} else if registry == nil || registry.ActiveProvider() == nil {
		// No providers available — offline mode
		rp := NewReplModel(tm.Current(), version)
		app.screen = ScreenREPL
		app.replModel = &rp
		_ = app.replModel.SetProvider(registry, app.activeProvider, app.activeModel, "", app.config)
		app.replModel.SetDispatcher(app.dispatcher)
		app.replModel.SetCommandRegistry(app.cmdRegistry)
		app.healthStatus = types.HealthStatus{
			Status:  "offline",
			Error:   offlineModeMsg,
		}
		app.currentOperation = offlineModeMsg
	} else {
		rp := NewReplModel(tm.Current(), version)
		app.screen = ScreenREPL
		app.replModel = &rp
		_ = app.replModel.SetProvider(registry, app.activeProvider, app.activeModel, "", app.config)
		app.replModel.SetDispatcher(app.dispatcher)
		app.replModel.SetCommandRegistry(app.cmdRegistry)
		app.healthStatus = types.HealthStatus{Status: "live"}
	}

	return app, nil
}