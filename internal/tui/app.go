package tui

import (
	"context"
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

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
	apiKey           string
	configPath       string
}

func NewApp(version string, registry *provider.Registry, apiKey string, configPath string) *AppState {
	tm := theme.NewManager(theme.ModeDark)

	app := &AppState{
		version:      version,
		registry:     registry,
		apiKey:       apiKey,
		configPath:   configPath,
		themeManager: tm,
		healthStatus: types.HealthStatus{Status: "unknown"},
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
	if m.screen == ScreenREPL && m.registry != nil && m.activeProvider != "" {
		return HealthCheckTicker(context.Background(), m.registry, m.activeProvider, types.HealthCheckInterval)
	}
	return nil
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
		if msg.String() == "ctrl+c" {
			return m, tea.Quit
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

	case AppMsg:
		if msg.Screen == ScreenREPL && m.replModel == nil {
			rp := NewReplModel(m.themeManager.Current())
			m.replModel = &rp
			m.initialized = true
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

	case ErrorMsg:
		m.currentOperation = fmt.Sprintf("Error: %v", msg.Err)
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
				rp := NewReplModel(m.themeManager.Current())
				m.replModel = &rp
				m.initialized = true
				healthCmd := HealthCheckTicker(
					context.Background(), m.registry, m.activeProvider,
					types.HealthCheckInterval,
				)
				cmds = append(cmds, healthCmd)
			}
			if appMsg.Health != nil {
				m.healthStatus = types.HealthStatus{
					Status: appMsg.Health.Status,
				}
			}
		}
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
