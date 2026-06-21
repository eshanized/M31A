package tui

import (
	"context"
	"fmt"
	"log/slog"
	"path/filepath"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/pkg/arbitrage"
	"github.com/eshanized/M31A/pkg/autodream"
	"github.com/eshanized/M31A/pkg/history"
	"github.com/eshanized/M31A/pkg/keychain"
	"github.com/eshanized/M31A/pkg/ledger"
	"github.com/eshanized/M31A/pkg/rollback"
	"github.com/eshanized/M31A/pkg/session"
)

// workflowEngineInterface is the interface that AppState uses to invoke workflow phases.
// It is defined separately from the concrete workflow.Engine to allow testing.
type workflowEngineInterface = WorkflowEngine

// Compile-time interface check
var _ workflowEngineInterface = (*workflow.Engine)(nil)

// Compile-time interface check for channelEmitter
var _ workflow.MsgEmitter = (*channelEmitter)(nil)

// AppState is the top-level Bubble Tea model.
// All state mutations go through Update(). No goroutine may mutate AppState directly.
type AppState struct {
	// Layout
	width  int
	height int

	// Screen routing
	screen      Screen
	prevScreen  Screen
	screenStack []Screen
	screenCap   int // max screen stack size (prevents unbounded growth)

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
	workflowEngine workflowEngineInterface
	workflowPhase  types.WorkflowPhase
	workflowMode   types.WorkflowMode
	workflowGoal   string
	workflowCancel context.CancelFunc
	shutdownCtx    context.Context
	shutdownCancel context.CancelFunc
	emitterCh      chan tea.Msg

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
	themePickerModel   *ThemePickerModel
	notifModel         *NotificationModel
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

	// Double ctrl+c exit tracking
	lastCtrlCTime time.Time

	// Sidebar auto-hide notification tracking (UX-38)
	sidebarAutoHideNotified bool

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
	promptRegistry *workflow.PromptRegistry
	agentCh        <-chan tea.Msg // agent loop channel for cmd chain

	// Intent classification state (pending confirmation from user)
	pendingIntent      *types.IntentResult
	pendingIntentInput string // original user input pending classification routing
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
		case "light":
			tm = theme.NewManager(theme.ModeLight)
		case "auto":
			tm = theme.NewManager(theme.ModeAuto)
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
	}

	if cfg != nil {
		a.activeProvider = cfg.Provider.Default
		if cfg.UI.PermissionModalWidth > 0 {
			a.permModalWidth = cfg.UI.PermissionModalWidth
		}
	}

	// Initialize sidebar
	a.sidebarModel = NewSidebarModel(gitClient, tm.Current())
	a.sidebarModel.SetShutdownContext(a.shutdownCtx)
	if cfg != nil && cfg.UI.SidebarWidth > 0 {
		a.sidebarModel.SetWidth(cfg.UI.SidebarWidth)
	}

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
		case "openrouter":
			m.config.Provider.OpenRouter.APIKey = entry.APIKey
		case "zen":
			m.config.Provider.Zen.APIKey = entry.APIKey
		case "nvidia":
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

	// Sync REPL provider+model so chat works immediately
	m.screen = ScreenREPL
	m.ensureReplModel()
	var cmds []tea.Cmd
	if providerCmd := m.syncReplProvider(m.sessionID); providerCmd != nil {
		cmds = append(cmds, providerCmd)
	}
	cmds = append(cmds, m.startNewSession())
	return tea.Batch(cmds...)
}
