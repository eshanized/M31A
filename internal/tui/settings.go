package tui

import (
	"fmt"
	"os"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/keychain"
)

// Tab labels for the settings screen.
var tabLabels = []string{
	"General",
	"Provider",
	"Permissions",
	"Features",
}

// SettingsModel provides a tabbed configuration editor.
// Initialized in NewApp and active when screen == ScreenSettings.
type SettingsModel struct {
	config    *config.Config
	theme     theme.Theme
	activeTab int // 0=General, 1=Provider, 2=Permissions, 3=Features
	width     int
	height    int
	keychain  keychain.Keychain
	storeInKC bool   // toggle for "store keys in keychain"
	dirty     bool   // true if changes unsaved
	statusMsg string // confirmation/error message
	errMsg    string // error message
}

// NewSettingsModel creates a SettingsModel with the given config, theme, and optional keychain.
func NewSettingsModel(cfg *config.Config, t theme.Theme, kc keychain.Keychain) *SettingsModel {
	return &SettingsModel{
		config:   cfg,
		theme:    t,
		keychain: kc,
	}
}

// Update handles messages for the settings screen.
func (m *SettingsModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height

	case tea.KeyMsg:
		switch msg.String() {
		case "tab":
			m.activeTab = (m.activeTab + 1) % 4
			m.dirty = true
			m.statusMsg = ""
		case "shift+tab":
			m.activeTab = (m.activeTab + 3) % 4
			m.dirty = true
			m.statusMsg = ""
		case "enter":
			// Save config
			if err := m.Save(); err != nil {
				m.errMsg = fmt.Sprintf("Save failed: %v", err)
				m.statusMsg = ""
				return nil, nil
			}
			m.statusMsg = "Config saved"
			m.errMsg = ""
			return nil, &AppMsg{Screen: ScreenREPL}
		case "esc":
			// Discard changes
			return nil, &AppMsg{Screen: ScreenREPL}
		}
	}
	return nil, nil
}

// View renders the settings screen.
func (m *SettingsModel) View() string {
	if m.width == 0 || m.height == 0 {
		return "Loading..."
	}

	tabBar := m.renderTabBar()
	content := m.renderTabContent()

	var parts []string
	parts = append(parts, tabBar)
	parts = append(parts, "")
	parts = append(parts, content)
	parts = append(parts, "")
	parts = append(parts, m.renderFooter())

	return lipgloss.JoinVertical(lipgloss.Top, parts...)
}

// renderTabBar renders the tab headers row.
func (m *SettingsModel) renderTabBar() string {
	var tabs []string
	for i, label := range tabLabels {
		if i == m.activeTab {
			style := lipgloss.NewStyle().
				Background(lipgloss.Color(m.theme.Brand)).
				Foreground(lipgloss.Color("#FFFFFF")).
				Padding(0, 2).
				Bold(true)
			tabs = append(tabs, style.Render(label))
		} else {
			style := lipgloss.NewStyle().
				Background(lipgloss.Color(m.theme.Surface)).
				Foreground(lipgloss.Color(m.theme.TextSecondary)).
				Padding(0, 2)
			tabs = append(tabs, style.Render(label))
		}
	}
	return lipgloss.JoinHorizontal(lipgloss.Top, tabs...)
}

// renderTabContent renders the content of the active tab.
func (m *SettingsModel) renderTabContent() string {
	switch m.activeTab {
	case 0:
		return m.renderGeneralTab()
	case 1:
		return m.renderProviderTab()
	case 2:
		return m.renderPermissionsTab()
	case 3:
		return m.renderFeaturesTab()
	default:
		return ""
	}
}

// renderGeneralTab renders the General tab content.
func (m *SettingsModel) renderGeneralTab() string {
	var lines []string
	lines = append(lines, tabSectionStyle(m.theme, "General Settings"))

	lines = append(lines, fmt.Sprintf("\n  Theme:        %s", m.config.UI.Theme))
	lines = append(lines, fmt.Sprintf("  Compact Mode: %s", boolStr(m.theme, m.config.UI.CompactMode)))
	lines = append(lines, fmt.Sprintf("  Token Usage:  %s", boolStr(m.theme, m.config.UI.ShowTokenUsage)))
	lines = append(lines, fmt.Sprintf("  Cost Estimate:%s", boolStr(m.theme, m.config.UI.ShowCostEstimate)))
	lines = append(lines, fmt.Sprintf("  Max Iterations: %d", m.config.UI.MaxIterations))

	if m.config.UI.Theme == "" {
		lines = append(lines, "\n  "+lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.TextSecondary)).Render("Theme: \"auto\" (auto-detect), \"dark\", or \"light\""))
	}

	return strings.Join(lines, "\n")
}

// renderProviderTab renders the Provider tab content.
func (m *SettingsModel) renderProviderTab() string {
	var lines []string
	lines = append(lines, tabSectionStyle(m.theme, "Provider Settings"))

	lines = append(lines, fmt.Sprintf("\n  Default Provider:  %s", m.config.Provider.Default))
	lines = append(lines, fmt.Sprintf("  Auto-Fallback:     %s", boolStr(m.theme, m.config.Provider.AutoFallback)))

	// API keys — masked
	orKey := m.config.Provider.OpenRouter.APIKey
	zenKey := m.config.Provider.Zen.APIKey
	lines = append(lines, fmt.Sprintf("\n  OpenRouter API Key: %s", maskAPIKey(orKey)))
	lines = append(lines, fmt.Sprintf("  Zen API Key:        %s", maskAPIKey(zenKey)))

	// Store in keychain toggle
	lines = append(lines, fmt.Sprintf("\n  Store in Keychain:  %s", boolStr(m.theme, m.storeInKC)))

	return strings.Join(lines, "\n")
}

// renderPermissionsTab renders the Permissions tab content.
func (m *SettingsModel) renderPermissionsTab() string {
	var lines []string
	lines = append(lines, tabSectionStyle(m.theme, "Permissions Settings"))

	lines = append(lines, fmt.Sprintf("\n  Default Mode:    %s", m.config.Permissions.DefaultMode))
	lines = append(lines, fmt.Sprintf("  Timeout Seconds: %d", m.config.Permissions.TimeoutSeconds))

	if len(m.config.Permissions.Rules) > 0 {
		lines = append(lines, "")
		ruleStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.TextSecondary))
		lines = append(lines, ruleStyle.Render(fmt.Sprintf("  Permission Rules: %d defined", len(m.config.Permissions.Rules))))
	}

	return strings.Join(lines, "\n")
}

// renderFeaturesTab renders the Features tab content.
func (m *SettingsModel) renderFeaturesTab() string {
	var lines []string
	lines = append(lines, tabSectionStyle(m.theme, "Features Settings"))

	lines = append(lines, fmt.Sprintf("\n  AutoDream:       %s", boolStr(m.theme, m.config.Features.AutodreamEnabled)))
	lines = append(lines, fmt.Sprintf("  Subagent Enable: %s", boolStr(m.theme, m.config.Features.SubagentEnabled)))
	lines = append(lines, fmt.Sprintf("  Auto Backup:     %s", boolStr(m.theme, m.config.Features.AutoBackup)))
	lines = append(lines, fmt.Sprintf("  Resume on Start: %s", boolStr(m.theme, m.config.Features.ResumeOnStartup)))

	return strings.Join(lines, "\n")
}

// renderFooter renders the footer with key hints and status messages.
func (m *SettingsModel) renderFooter() string {
	footerStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.TextSecondary))

	footer := footerStyle.Render("Tab/Shift+Tab: cycle tabs  |  Enter: save  |  Esc: discard")

	if m.statusMsg != "" {
		statusStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Success))
		footer = statusStyle.Render(m.statusMsg) + "\n" + footer
	}
	if m.errMsg != "" {
		errStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Error))
		footer = errStyle.Render(m.errMsg) + "\n" + footer
	}

	return footer
}

// Save saves the config to disk and optionally stores keys to keychain.
func (m *SettingsModel) Save() error {
	// Determine config path (same env var logic as config.Load)
	configPath := os.Getenv("M31A_CONFIG")
	if configPath == "" {
		home, err := os.UserHomeDir()
		if err != nil {
			return fmt.Errorf("cannot determine home directory: %w", err)
		}
		configPath = home + "/.m31a/config.toml"
	}

	// Store keys to keychain if enabled and available
	if m.storeInKC && m.keychain != nil {
		if key := m.config.Provider.OpenRouter.APIKey; key != "" {
			if err := m.keychain.Set("openrouter", key); err != nil {
				return fmt.Errorf("keychain set openrouter: %w", err)
			}
		}
		if key := m.config.Provider.Zen.APIKey; key != "" {
			if err := m.keychain.Set("zen", key); err != nil {
				return fmt.Errorf("keychain set zen: %w", err)
			}
		}
	}

	// Save config to file
	if err := m.config.Save(configPath); err != nil {
		return fmt.Errorf("save config: %w", err)
	}

	return nil
}

// maskAPIKey returns a masked version of the API key for display.
// Non-empty keys show as "••••••••", empty keys show as "(empty)".
func maskAPIKey(key string) string {
	if key == "" {
		return lipgloss.NewStyle().
			Foreground(lipgloss.Color("#9AA0A6")).
			Render("(empty)")
	}
	return lipgloss.NewStyle().
		Foreground(lipgloss.Color("#9AA0A6")).
		Render("••••••••")
}

// boolStr renders a boolean value as [x] or [ ] with theme styling.
func boolStr(t theme.Theme, val bool) string {
	var s string
	if val {
		s = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Success)).Render("[x]")
	} else {
		s = lipgloss.NewStyle().Foreground(lipgloss.Color(t.TextSecondary)).Render("[ ]")
	}
	return s
}

// tabSectionStyle renders a section header with theme styling.
func tabSectionStyle(t theme.Theme, label string) string {
	return lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Brand)).
		Bold(true).
		Render(label)
}

// SetConfig updates the config pointer (for theme changes from outside).
func (m *SettingsModel) SetConfig(cfg *config.Config) {
	m.config = cfg
}

// SetTheme updates the theme reference (for in-app theme changes).
func (m *SettingsModel) SetTheme(t theme.Theme) {
	m.theme = t
}
