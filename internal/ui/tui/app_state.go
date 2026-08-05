package tui

import (
	"context"
	"fmt"
	"log/slog"
	"path/filepath"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/core/config"
	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/decision"
	"github.com/eshanized/M31A/internal/engine/narrative"
	"github.com/eshanized/M31A/internal/engine/rollback"
	"github.com/eshanized/M31A/internal/engine/session"
	"github.com/eshanized/M31A/internal/engine/workflow"
	"github.com/eshanized/M31A/internal/integrations/arbitrage"
	"github.com/eshanized/M31A/internal/integrations/autodream"
	"github.com/eshanized/M31A/internal/integrations/git"
	"github.com/eshanized/M31A/internal/integrations/history"
	"github.com/eshanized/M31A/internal/integrations/keychain"
	"github.com/eshanized/M31A/internal/integrations/ledger"
	"github.com/eshanized/M31A/internal/integrations/metrics"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/internal/ui/tui/components"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

// workflowEngineInterface is the interface that AppState uses to invoke workflow phases.
// It is defined separately from the concrete workflow.Engine to allow testing.
type workflowEngineInterface = WorkflowEngine

// totalPhases is the number of active workflow phases (Initialize through Ship).
const totalPhases = 7

// phaseToIndex maps a WorkflowPhase to its numeric index (0-6).
func phaseToIndex(phase types.WorkflowPhase) int {
	switch phase {
	case types.PhaseInitialize:
		return 0
	case types.PhaseDiscuss:
		return 1
	case types.PhasePlan:
		return 2
	case types.PhaseExecute:
		return 3
	case types.PhaseVerify:
		return 4
	case types.PhaseRuntime:
		return 5
	case types.PhaseShip:
		return 6
	default:
		return 0
	}
}

// Compile-time interface check
var _ workflowEngineInterface = (*workflow.Engine)(nil)

// Compile-time interface check for channelEmitter
var _ workflow.MsgEmitter = (*channelEmitter)(nil)

// AppState is the top-level Bubble Tea model.
// All state mutations go through Update(). The Bubble Tea runtime guarantees
// single-threaded access to the model, so no mutex is needed.
type AppState struct {
	// Layout
	width  int
	height int

	// Screen routing
	screen      Screen
	prevScreen  Screen
	screenStack []Screen
	screenCap   int // max screen stack size (prevents unbounded growth)

	// Router for new screen interface (pilot: ConfirmQuit)
	router *Router

	// Theme
	themeManager *theme.Manager

	// Config
	config     *config.Config
	configPath string

	// Session
	sessionManager *session.Manager
	sessionID      string

	// Provider / model
	registry       *provider.Registry
	activeProvider string
	activeModel    *types.ModelInfo
	version        string

	// Tools
	dispatcher *tools.Dispatcher

	// Git
	git *git.Git

	// Working directory (for @-mention file resolution)
	cwd string

	// Workflow
	workflowEngine     workflowEngineInterface
	workflowPhase      types.WorkflowPhase
	workflowPhaseIndex int // numeric phase index (0-6 for 7 phases)
	workflowMode       types.WorkflowMode
	workflowGoal       string
	workflowCancel     context.CancelFunc
	shutdownCtx        context.Context
	shutdownCancel     context.CancelFunc
	emitterCh          chan tea.Msg

	// Cached decisions from workflow engine (updated via DecisionsSnapshotMsg)
	cachedDecisions []decision.DecisionReceipt

	// Optional packages
	ledger         *ledger.Ledger
	rollback       *rollback.Rollback
	autoDream      *autodream.Consolidator
	keychain       keychain.Keychain
	frecentHistory *history.FrecentHistory

	// Sub-models
	replModel     *ReplModel
	sidebarModel  *SidebarModel
	cmdPalette    *CommandPaletteModel
	planModel     *PlanModel
	executeModel  *ExecuteModel
	verifyModel   *VerifyModel
	runtimeModel  *RuntimeModel
	shipModel     *ShipModel
	settingsModel *SettingsModel
	resumeModel   *ResumeModel
	msModel       *ModelSelector
	firstRunModel *FirstRunModel
	tourModel     *components.TourModel
	goalInput     *GoalInputModel
	ledgerModel   *LedgerModel
	rollbackModel *RollbackModel
	discussModel  *DiscussModel
	diffModel     *DiffModel
	metricsModel  *MetricsModel
	configModel   *ConfigModel
	helpModel     *HelpModel

	// New screens (Phase 3)
	bisectModel        *BisectModel
	notifModel         *NotificationModel
	decisionScreen     *DecisionScreen
	dashboardModel     *DashboardModel
	sessionDetailModel *SessionDetailModel
	fileExplorerModel  *FileExplorerModel
	toolDetailModel    *ToolDetailModel

	// Missing screens (Ghost & ConfirmQuit)
	ghostPickerModel *GhostPickerModel
	ghostOutputModel *GhostOutputModel
	confirmQuitModel *ConfirmQuitModel

	// Dual-model picker (Planning vs Coding phase selection)
	phaseModelPicker *PhaseModelPickerModel
	planningModelID  string // model ID assigned to Discuss/Plan/Verify phases
	codingModelID    string // model ID assigned to Execute/Ship phases

	// Chat history browser
	chatHistoryModel *ChatHistoryModel

	// Phase transition confirmation screen
	phaseTransitionModel *PhaseTransitionModel

	// Dedicated command palette screen (ctrl+p)
	commandPaletteScreenModel *CommandPaletteScreenModel

	// Home screen (landing with logo, prompt, tips)
	homeModel *HomeModel

	// Command system
	cmdRegistry *CommandRegistry
	keyRegistry *KeyRegistry

	// Permission modal state
	permRequest     *tools.PermissionRequest
	permCountdown   int
	permModalWidth  int
	permModal       *components.PermissionModal // rich stateful modal
	questionRequest *QuestionRequestMsg
	questionModel   *components.QuestionModel // rich interactive question modal

	// Session list (for resume screen)
	sessionList []*session.Session

	// Workflow discuss questions
	discussQuestions []string

	// Toast notifications (up to 3 visible, queue overflow)
	toasts      []Toast
	toastTimers map[int]*time.Timer // index → auto-dismiss timer
	nextToastID int

	// Arbitrage scorer
	arbitrager *arbitrage.Scorer

	// Health tracking
	healthStatus types.HealthStatus
	lastHealth   time.Time

	// Transition overlay
	transition *ScreenTransition

	// Stream cancellation
	streamCancelFn context.CancelFunc

	// Narrative engine (initialized in NewApp to prevent nil dereference)
	narrativeEngine *narrative.Engine
	narrativeBridge *narrative.Bridge
	narrativeState  *NarrativeState

	// Double ctrl+c exit tracking
	lastCtrlCTime time.Time

	// Sidebar auto-hide notification tracking (UX-38)
	sidebarAutoHideNotified bool

	// Context warning tracking (Wave 2A: proactive context warnings)
	ctxWarned70 bool // whether 70% warning has been shown
	ctxWarned85 bool // whether 85% warning has been shown

	// Confirmation dialog state (non-nil when awaiting y/n)
	pendingConfirm *CommandResult
	confirmPrompt  string

	// Resume session ID set at startup (C3)
	resumeSessionID string

	// File watcher for real-time sidebar refresh
	fileWatcher *FileWatcher

	// Config watcher for hot-reload of config.toml
	configWatcherStop chan struct{}

	// Subagents (parallel child agents with full tool access in own worktrees)
	subagentManager  *subagent.Manager
	subagentsModel   *SubagentsModel
	subagentsVisible bool

	// Autonomous agent mode
	agentMode      bool // when true, plain text triggers agent loop (default)
	quickMode      bool // when true, simple tasks auto-skip Discuss phase
	promptRegistry *workflow.PromptRegistry
	agentCh        <-chan tea.Msg // agent loop channel for cmd chain

	// Intent classification state (pending confirmation from user)
	pendingIntent      *types.IntentResult
	pendingIntentInput string // original user input pending classification routing

	// Metrics collector for session observability
	collector *metrics.Collector

	// Screen routing map: eliminates duplicated per-screen switches.
	screenUpdaters map[Screen]screenUpdateFunc
}

// SetResumeSessionID configures the app to auto-resume a session on startup.
func (a *AppState) SetResumeSessionID(id string) {
	a.resumeSessionID = id
}

// SetKeychain configures the OS keychain for secure API key storage.
func (a *AppState) SetKeychain(kc keychain.Keychain) {
	a.keychain = kc
}

// SetSubagentManager wires the parallel-subagent manager into the app.
// Must be called before the first tea.Program.Run; the manager's event
// channel is drained starting from Init().
func (a *AppState) SetSubagentManager(m *subagent.Manager) {
	a.subagentManager = m
	if m != nil {
		a.subagentsModel = NewSubagentsModel(a.themeManager.Current())
	}
}

// SetQuickMode enables or disables quick mode for simple tasks.
func (a *AppState) SetQuickMode(enabled bool) {
	a.quickMode = enabled
}

// IsQuickMode returns whether quick mode is enabled.
func (a *AppState) IsQuickMode() bool {
	return a.quickMode
}

// SetCwd stores the working directory so it can be propagated to the REPL
// model for @-mention file resolution.
func (a *AppState) SetCwd(cwd string) {
	a.cwd = cwd
}

// NewApp creates a new AppState.
func NewApp(
	cfg *config.Config,
	configPath string,
	registry *provider.Registry,
	sessionManager *session.Manager,
	dispatcher *tools.Dispatcher,
	gitClient *git.Git,
	ledgerClient *ledger.Ledger,
	rollbackClient *rollback.Rollback,
	autoDreamClient *autodream.Consolidator,
	version string,
	themeMode theme.Mode,
) *AppState {
	shutdownCtx, shutdownCancel := context.WithCancel(context.Background())

	tm := theme.NewManager(themeMode)
	if cfg != nil && cfg.UI.Theme != "" {
		switch cfg.UI.Theme {
		case "light", "auto":
			// Light/auto themes are not supported; always use dark.
		}
	}

	keyReg := NewKeyRegistry(KeyRegistryOpts{
		LeaderKey:     "ctrl+x",
		LeaderTimeout: 1 * time.Second,
	})
	keyReg.RegisterDefaultBindings()

	cmdReg := DefaultCommands()

	// Frecent history — persisted to ~/.m31a/history.json
	historyPath := filepath.Join(filepath.Dir(configPath), "history.json")
	frecentHist := history.NewFrecentHistory(historyPath)

	a := &AppState{
		config:         cfg,
		configPath:     configPath,
		registry:       registry,
		sessionManager: sessionManager,
		dispatcher:     dispatcher,
		git:            gitClient,
		ledger:         ledgerClient,
		rollback:       rollbackClient,
		autoDream:      autoDreamClient,
		frecentHistory: frecentHist,
		version:        version,
		themeManager:   tm,
		keyRegistry:    keyReg,
		cmdRegistry:    cmdReg,
		workflowPhase:  types.PhaseIdle,
		shutdownCtx:    shutdownCtx,
		shutdownCancel: shutdownCancel,
		permModalWidth: 60,
		toastTimers:    make(map[int]*time.Timer),
		agentMode:      true,
		screenCap:      16,
		narrativeState: NewNarrativeState(),
	}

	if cfg != nil {
		a.activeProvider = cfg.Provider.Default
		if cfg.UI.PermissionModalWidth > 0 {
			a.permModalWidth = cfg.UI.PermissionModalWidth
		}
		// Wire UI config constants from TOML config
		if cfg.UI.ToastMaxVisible > 0 {
			SetMaxVisibleToasts(cfg.UI.ToastMaxVisible)
		}
		if len(cfg.UI.ToastTypeOverrides) > 0 {
			SetToastOverrides(cfg.UI.ToastTypeOverrides)
		}
	}

	// Initialize sidebar
	a.sidebarModel = NewSidebarModel(gitClient, tm.Current())
	a.sidebarModel.SetShutdownContext(a.shutdownCtx)
	if cfg != nil && cfg.UI.SidebarWidth > 0 {
		a.sidebarModel.SetWidth(cfg.UI.SidebarWidth)
	}

	// Initialize router for new screen interface (pilot: ConfirmQuit)
	a.router = NewRouter()

	// Initialize screen routing map (closures capture m, so nil models are safe)
	a.initScreenUpdaters()

	return a
}

// handleFirstRunComplete processes the wizard results: registers providers in
// the registry, optionally saves API keys to the OS keychain, persists the
// config, and starts a new session.
func (m *AppState) handleFirstRunComplete(msg FirstRunCompleteMsg) tea.Cmd {
	if m.config == nil {
		m.config = config.DefaultConfig()
	}

	// Copy wizard-collected API keys into the config struct so that
	// SaveWithKeychain can persist them (to keychain or config file).
	// Without this, m.config still has empty keys and the save is a no-op.
	for _, entry := range msg.Providers {
		switch entry.ID {
		case types.ProviderOpenRouter:
			m.config.Provider.OpenRouter.APIKey = entry.APIKey
		case types.ProviderZen:
			m.config.Provider.Zen.APIKey = entry.APIKey
		case types.ProviderNvidia:
			m.config.Provider.Nvidia.APIKey = entry.APIKey
		}
	}

	// Register each provider with its collected API key
	for _, entry := range msg.Providers {
		if err := RegisterProvider(m.registry, m.config, entry.ID, entry.APIKey, m.version); err != nil {
			slog.Warn("failed to register provider from wizard", "provider", entry.ID, "error", err)
			m.addToast(fmt.Sprintf("Provider %s: %s", entry.ID, m31errors.UserMessage(err)), "warning")
		}
	}

	// Set the default provider
	if msg.DefaultProvider != "" {
		if err := m.registry.SetActive(msg.DefaultProvider); err != nil {
			slog.Warn("failed to set default provider", "provider", msg.DefaultProvider, "error", err)
		}
		m.activeProvider = msg.DefaultProvider
		m.config.Provider.Default = msg.DefaultProvider
	}

	// Set the model
	if msg.ModelID != "" {
		m.config.Model.Default = msg.ModelID
		p := m.registry.ActiveProvider()
		if p != nil {
			if info, err := p.GetModel(msg.ModelID); err == nil && info != nil {
				m.activeModel = info
			} else {
				m.activeModel = &types.ModelInfo{ID: msg.ModelID}
			}
		} else {
			m.activeModel = &types.ModelInfo{ID: msg.ModelID}
		}
	}

	// Persist config (SaveWithKeychain handles keychain + file fallback).
	// If the user unchecked "save to keychain", pass nil so keys go to file only.
	if m.configPath != "" {
		kc := m.keychain
		if !msg.SaveKeychain {
			kc = nil
		}
		if err := m.config.SaveWithKeychain(m.configPath, kc); err != nil {
			slog.Warn("failed to save config after wizard", "error", err)
			m.addToast("Failed to save configuration", "warning")
		}
	}

	// Transition to the feature tour instead of directly to REPL
	m.tourModel = components.NewTourModel(m.themeManager.Current(), m.width, m.height)
	m.screen = ScreenTour
	return nil
}

// switchScreen updates the current screen and notifies the router (if using new interface).
func (m *AppState) switchScreen(s Screen) {
	m.screen = s
	if m.router != nil && m.router.ActiveID() != s {
		m.router.SwitchTo(s)
	}
}
