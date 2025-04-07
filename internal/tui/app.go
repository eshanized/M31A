package tui

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
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

	if apiKey == "" {
		fr := NewFirstRunModel(tm.Current(), configPath)
		app.screen = ScreenFirstRun
		app.firstRunModel = &fr
	} else {
		rp := NewReplModel(tm.Current())
		app.screen = ScreenREPL
		app.replModel = &rp
		app.healthStatus = types.HealthStatus{Status: "live"}
	}

	if registry != nil {
		app.activeProvider = registry.Active()
	}

	return app
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
			return m, tea.Quit
		}
		if m.screen == ScreenREPL {
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
		}

	case HealthCheckTickMsg:
		if m.registry == nil || m.activeProvider == "" {
			return m, NextHealthTick(types.HealthCheckInterval)
		}

		ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()

		p := m.registry.ActiveProvider()
		if p == nil {
			return m, NextHealthTick(types.HealthCheckInterval)
		}

		result := p.HealthCheck(ctx)
		m.healthStatus = result
		m.lastActivity = time.Now()
		return m, NextHealthTick(calculateNextInterval(result))

	case RefreshCacheMsg:
		if m.registry == nil || m.activeProvider == "" {
			return m, NextCacheRefreshTick(provider.DefaultCacheRefreshInterval)
		}

		ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer cancel()

		providerName := msg.ProviderName
		if providerName == "" {
			providerName = m.activeProvider
		}

		errMsg, nextCmd := handleCacheRefresh(ctx, m.registry, providerName)
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
			m.initialized = true
		}
		if msg.Screen == ScreenModelSelector {
			m.prevScreen = m.screen
			m.modelSelector = NewModelSelector(m.registry)
			m.screen = ScreenModelSelector
			return m, m.modelSelector.Init()
		}
		m.screen = msg.Screen
		if msg.Health != nil {
			m.healthStatus = types.HealthStatus{
				Status: msg.Health.Status,
			}
		}
		if msg.Provider != nil {
			m.activeProvider = msg.Provider.Provider
		}
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
				rp := NewReplModel(m.themeManager.Current())
				m.replModel = &rp
				m.initialized = true
				healthCmd := HealthCheckTicker(
					context.Background(), m.registry, m.activeProvider,
					types.HealthCheckInterval,
				)
				cmds = append(cmds, healthCmd)
				cmds = append(cmds,
					CacheRefreshTicker(m.activeProvider, provider.DefaultCacheRefreshInterval))
			}
			if appMsg.Health != nil {
				m.healthStatus = types.HealthStatus{
					Status: appMsg.Health.Status,
				}
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
