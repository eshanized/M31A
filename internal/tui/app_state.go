package tui

import (
	"context"
	"fmt"
	"log/slog"
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
	"github.com/eshanized/M31A/pkg/arbitrage"
	"github.com/eshanized/M31A/pkg/autodream"
	"github.com/eshanized/M31A/pkg/keychain"
	"github.com/eshanized/M31A/pkg/ledger"
	"github.com/eshanized/M31A/pkg/rollback"
	"github.com/eshanized/M31A/pkg/session"
)

// workflowEngineInterface is the interface that AppState uses to invoke workflow phases.
// It is defined separately from the concrete workflow.Engine to allow testing.
type workflowEngineInterface interface {
	RunPhase(ctx context.Context, phase types.WorkflowPhase, goal string) (*workflow.PhaseResult, error)
	Transition(ctx context.Context, from, to types.WorkflowPhase) error
	SetModel(modelID string, p provider.LLMProvider)
	SetMsgEmitter(em workflow.MsgEmitter)
	SetSessionID(id string)
	SetGit(g *git.Git)
	SessionID() string
	HealTask(ctx context.Context, taskID int) bool
}

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

	// Workflow
	workflowEngine workflowEngineInterface
	workflowPhase  types.WorkflowPhase
	workflowGoal   string
	workflowCancel context.CancelFunc
	shutdownCtx    context.Context
	shutdownCancel context.CancelFunc
	emitterCh      chan tea.Msg

	// Optional packages
	ledger    *ledger.Ledger
	rollback  *rollback.Rollback
	autoDream *autodream.Consolidator
	keychain  keychain.Keychain

	// Sub-models
	replModel     *ReplModel
	sidebarModel  *SidebarModel
	cmdPalette    *CommandPaletteModel
	planModel     *PlanModel
	executeModel  *ExecuteModel
	verifyModel   *VerifyModel
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

	// Workflow discuss questions + answers
	discussQuestions []string
	discussAnswers   []string
	discussIndex     int

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

	// Status
	lastActivity    time.Time
	streamErrorTime time.Time

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
}

// SetResumeSessionID configures the app to auto-resume a session on startup.
func (a *AppState) SetResumeSessionID(id string) {
	a.resumeSessionID = id
}

// SetKeychain configures the OS keychain for secure API key storage.
func (a *AppState) SetKeychain(kc keychain.Keychain) {
	a.keychain = kc
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
		version:        version,
		themeManager:   tm,
		keyRegistry:    keyReg,
		cmdRegistry:    cmdReg,
		workflowPhase:  types.PhaseIdle,
		shutdownCtx:    shutdownCtx,
		shutdownCancel: shutdownCancel,
		permModalWidth: 60,
		toastTimers:    make(map[int]*time.Timer),
	}

	if cfg != nil {
		a.activeProvider = cfg.Provider.Default
		if cfg.UI.PermissionModalWidth > 0 {
			a.permModalWidth = cfg.UI.PermissionModalWidth
		}
	}

	// Initialize sidebar
	a.sidebarModel = NewSidebarModel(gitClient, tm.Current())

	return a
}

// handleFirstRunComplete processes the wizard results: registers providers in
// the registry, optionally saves API keys to the OS keychain, persists the
// config, and starts a new session.
func (m *AppState) handleFirstRunComplete(msg FirstRunCompleteMsg) tea.Cmd {
	if m.config == nil {
		m.config = config.DefaultConfig()
	}

	// Register each provider with its collected API key
	for _, entry := range msg.Providers {
		if err := RegisterProvider(m.registry, m.config, entry.ID, entry.APIKey); err != nil {
			slog.Warn("failed to register provider from wizard", "provider", entry.ID, "error", err)
			m.addToast(fmt.Sprintf("Failed to register provider %s", entry.ID), "warning")
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

	// Save API keys to keychain if requested
	if msg.SaveKeychain && m.keychain != nil {
		for _, entry := range msg.Providers {
			if err := m.keychain.Set(entry.ID, entry.APIKey); err != nil {
				slog.Warn("failed to save API key to keychain", "provider", entry.ID, "error", err)
			}
		}
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

	// Persist config
	if m.configPath != "" {
		if err := m.config.Save(m.configPath); err != nil {
			slog.Warn("failed to save config after wizard", "error", err)
			m.addToast("Failed to save configuration", "warning")
		}
	}

	// Start a new session
	m.screen = ScreenREPL
	return m.startNewSession()
}
