package tui

import (
	"context"
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/bubbles/textinput"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// SettingsTab identifies which settings tab is active.
type SettingsTab int

const (
	TabProvider SettingsTab = iota
	TabModel
	TabUI
	TabKeys
	TabWorkflow
	TabAbout
)

var settingsTabNames = []string{
	"Provider", "Model", "UI", "Keys", "Workflow", "About",
}

// SettingsModel manages the settings editor (6-tab layout).
type SettingsModel struct {
	theme     theme.Theme
	config    *config.Config
	registry  *provider.Registry
	activeTab SettingsTab
	width     int
	height    int

	// Editing state
	editing   bool
	editField string
	editValue textinput.Model

	// Config path
	configPath string

	// Status message
	statusMsg  string
	statusTime time.Time
}

// NewSettingsModel creates a SettingsModel.
func NewSettingsModel(cfg *config.Config, registry *provider.Registry, t theme.Theme, configPath string) *SettingsModel {
	ti := textinput.New()
	ti.CharLimit = 512
	ti.Width = 40

	return &SettingsModel{
		theme:      t,
		config:     cfg,
		registry:   registry,
		editValue:  ti,
		configPath: configPath,
	}
}

// SetConfig updates the config reference.
func (s *SettingsModel) SetConfig(cfg *config.Config) {
	s.config = cfg
}

// SetTheme updates the settings theme.
func (s *SettingsModel) SetTheme(t theme.Theme) {
	s.theme = t
}

// Update handles settings screen key events.
func (s *SettingsModel) Update(msg tea.Msg) (*SettingsModel, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.KeyMsg:
		if s.editing {
			return s.updateEditing(msg)
		}

		switch msg.String() {
		case "esc", "q":
			return s, func() tea.Msg {
				return AppMsg{Screen: ScreenREPL}
			}
		case "tab", "right":
			s.activeTab = SettingsTab((int(s.activeTab) + 1) % len(settingsTabNames))
		case "shift+tab", "left":
			s.activeTab = SettingsTab((int(s.activeTab) - 1 + len(settingsTabNames)) % len(settingsTabNames))
		case "1":
			s.activeTab = TabProvider
		case "2":
			s.activeTab = TabModel
		case "3":
			s.activeTab = TabUI
		case "4":
			s.activeTab = TabKeys
		case "5":
			s.activeTab = TabWorkflow
		case "6":
			s.activeTab = TabAbout
		case "e", "enter":
			return s.startEditing()
		case "s":
			return s.saveConfig()
		}
	}
	return s, nil
}

func (s *SettingsModel) updateEditing(msg tea.KeyMsg) (*SettingsModel, tea.Cmd) {
	switch msg.String() {
	case "esc":
		s.editing = false
		s.editField = ""
		return s, nil
	case "enter":
		return s.commitEdit()
	}
	var cmd tea.Cmd
	s.editValue, cmd = s.editValue.Update(msg)
	return s, cmd
}

func (s *SettingsModel) startEditing() (*SettingsModel, tea.Cmd) {
	s.editing = true
	s.editValue.SetValue("")
	s.editValue.Focus()
	switch s.activeTab {
	case TabProvider:
		s.editField = "provider"
		if s.config != nil {
			s.editValue.SetValue(s.config.Provider.Default)
		}
		s.editValue.Placeholder = "openrouter or zen"
	case TabModel:
		s.editField = "model"
		if s.config != nil {
			s.editValue.SetValue(s.config.Model.Default)
		}
		s.editValue.Placeholder = "model ID"
	case TabKeys:
		s.editField = "apikey"
		s.editValue.Placeholder = "API key (for current provider)"
		s.editValue.EchoMode = textinput.EchoPassword
	default:
		s.editing = false
		s.editField = ""
	}
	return s, textinput.Blink
}

func (s *SettingsModel) commitEdit() (*SettingsModel, tea.Cmd) {
	val := strings.TrimSpace(s.editValue.Value())
	s.editing = false
	s.editValue.EchoMode = textinput.EchoNormal
	if val == "" {
		return s, nil
	}
	if s.config == nil {
		s.config = config.DefaultConfig()
	}
	switch s.editField {
	case "provider":
		s.config.Provider.Default = val
	case "model":
		s.config.Model.Default = val
	case "apikey":
		provName := s.config.Provider.Default
		switch provName {
		case "zen":
			s.config.Provider.Zen.APIKey = val
		default:
			s.config.Provider.OpenRouter.APIKey = val
		}
	}
	s.statusMsg = "Value updated. Press 's' to save."
	s.statusTime = time.Now()
	return s, nil
}

func (s *SettingsModel) saveConfig() (*SettingsModel, tea.Cmd) {
	if s.config != nil && s.configPath != "" {
		s.statusMsg = "Config updated in memory (restart to reload)"
		s.statusTime = time.Now()
	}
	return s, func() tea.Msg { return SettingsSavedMsg{} }
}

// View renders the settings screen with a left sidebar and ThinBorder content card.
func (s *SettingsModel) View() string {
	t := s.theme
	w := s.width

	if w < 50 {
		return "Terminal too narrow for settings"
	}

	// Left navigation sidebar
	navWidth := 20
	if w < 70 {
		navWidth = 16
	}
	leftNav := s.renderLeftNav()

	// Content inside ThinBorder card
	content := s.renderTabContent()
	contentCard := lipgloss.NewStyle().
		Border(theme.ThinBorder).
		BorderForeground(t.Border).
		Padding(0, 1).
		Width(w-navWidth-6).
		Render(content)

	// Main layout
	mainArea := lipgloss.JoinHorizontal(lipgloss.Top,
		lipgloss.NewStyle().Width(navWidth).Padding(0, 1).Render(leftNav),
		contentCard,
	)

	// Status bar
	status := ""
	if s.statusMsg != "" && time.Since(s.statusTime) < 5*time.Second {
		status = lipgloss.NewStyle().Foreground(t.Success).PaddingLeft(2).Render(s.statusMsg)
	}

	// Edit overlay (shows below the main area)
	if s.editing {
		editBox := s.renderEditBox()
		return lipgloss.JoinVertical(lipgloss.Left,
			mainArea,
			"",
			editBox,
			"",
			status,
		)
	}

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("tab next  1-6 jump  e edit  esc back")

	return lipgloss.JoinVertical(lipgloss.Left,
		mainArea,
		status,
		footer,
	)
}



func (s *SettingsModel) renderTabContent() string {
	switch s.activeTab {
	case TabProvider:
		return s.renderProviderTab()
	case TabModel:
		return s.renderModelTab()
	case TabUI:
		return s.renderUITab()
	case TabKeys:
		return s.renderKeysTab()
	case TabWorkflow:
		return s.renderWorkflowTab()
	case TabAbout:
		return s.renderAboutTab()
	}
	return ""
}

func (s *SettingsModel) renderProviderTab() string {
	t := s.theme
	var lines []string
	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(2).Render("Provider Settings"))
	lines = append(lines, "")

	defaultProvider := "(not set)"
	if s.config != nil && s.config.Provider.Default != "" {
		defaultProvider = s.config.Provider.Default
	}
	lines = append(lines, settingRow("Default provider", defaultProvider, t))
	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).
		Render("Available: openrouter, zen"))

	// Provider health
	if s.registry != nil {
		lines = append(lines, "")
		lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render("Provider Status:"))
		for _, name := range s.registry.List() {
			p, err := s.registry.Get(name)
			if err != nil {
				continue
			}
			ctx, cancel := context.WithTimeout(context.Background(), 3*time.Second)
			status := p.HealthCheck(ctx)
			cancel()
			icon := "✓"
			color := t.Success
			if status.Status != "ok" {
				icon = "✗"
				color = t.Error
			}
			lines = append(lines, lipgloss.NewStyle().Foreground(color).PaddingLeft(4).
				Render(fmt.Sprintf("%s %s", icon, name)))
		}
	}

	return strings.Join(lines, "\n")
}

func (s *SettingsModel) renderModelTab() string {
	t := s.theme
	var lines []string
	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(2).Render("Model Settings"))
	lines = append(lines, "")

	defaultModel := "(not set)"
	if s.config != nil && s.config.Model.Default != "" {
		defaultModel = s.config.Model.Default
	}
	lines = append(lines, settingRow("Default model", defaultModel, t))
	if s.config != nil {
		lines = append(lines, settingRow("Auto-collapse tools", boolStr(s.config.Model.AutoCollapseTools), t))
		lines = append(lines, settingRow("Show thinking by default", boolStr(s.config.Model.ShowThinkingByDefault), t))
		lines = append(lines, settingRow("Auto-arbitrage", boolStr(s.config.Model.AutoArbitrage), t))
	}
	return strings.Join(lines, "\n")
}

func (s *SettingsModel) renderUITab() string {
	t := s.theme
	var lines []string
	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(2).Render("UI Settings"))
	lines = append(lines, "")
	if s.config != nil {
		lines = append(lines, settingRow("Theme", s.config.UI.Theme, t))
		lines = append(lines, settingRow("Show cost estimate", boolStr(s.config.UI.ShowCostEstimate), t))
		lines = append(lines, settingRow("Show token usage", boolStr(s.config.UI.ShowTokenUsage), t))
		lines = append(lines, settingRow("Discuss timeout", fmt.Sprintf("%ds", s.config.UI.DiscussTimeout), t))
		lines = append(lines, settingRow("Sidebar width", fmt.Sprintf("%d cols", s.config.UI.SidebarWidth), t))
	}
	return strings.Join(lines, "\n")
}

func (s *SettingsModel) renderKeysTab() string {
	t := s.theme
	var lines []string
	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(2).Render("API Keys"))
	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).
		Render("Keys are stored in OS keychain or config file."))
	lines = append(lines, "")
	if s.config != nil {
		for name, key := range map[string]string{
			"openrouter": s.config.Provider.OpenRouter.APIKey,
			"zen":        s.config.Provider.Zen.APIKey,
		} {
			keyDisplay := "(not set)"
			if key != "" {
				tail := key
				if len(tail) > 4 {
					tail = key[len(key)-4:]
				}
				keyDisplay = "●●●●●●●" + tail
			}
			lines = append(lines, settingRow(name+" API key", keyDisplay, t))
		}
	}
	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).
		Render("Press 'e' to edit the active provider's API key."))
	return strings.Join(lines, "\n")
}

func (s *SettingsModel) renderWorkflowTab() string {
	t := s.theme
	var lines []string
	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(2).Render("Workflow Settings"))
	lines = append(lines, "")
	if s.config != nil {
		lines = append(lines, settingRow("Permission timeout", fmt.Sprintf("%ds", s.config.Permissions.TimeoutSeconds), t))
		lines = append(lines, settingRow("Max iterations", fmt.Sprintf("%d", s.config.UI.MaxIterations), t))
		lines = append(lines, settingRow("Auto fallback", boolStr(s.config.Provider.AutoFallback), t))
		lines = append(lines, settingRow("Auto backup", boolStr(s.config.Features.AutoBackup), t))
	}
	return strings.Join(lines, "\n")
}

func (s *SettingsModel) renderAboutTab() string {
	t := s.theme
	return lipgloss.JoinVertical(lipgloss.Left,
		"",
		lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(2).Render("About M31A"),
		"",
		lipgloss.NewStyle().Foreground(t.Text).PaddingLeft(4).Render("M31A — Terminal AI Coding Agent"),
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).Render("Module: github.com/eshanized/M31A"),
		"",
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).Render("Built with:"),
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(6).Render("• charmbracelet/bubbletea"),
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(6).Render("• charmbracelet/lipgloss"),
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(6).Render("• charmbracelet/bubbles"),
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(6).Render("• charmbracelet/glamour"),
		"",
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).Render("Config: ~/.m31a/config.toml"),
		lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(4).Render("Sessions: ~/.m31a/sessions/"),
	)
}

func (s *SettingsModel) renderEditBox() string {
	t := s.theme
	label := "Editing: " + s.editField
	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(0, 2).
		Width(50).
		PaddingLeft(4).
		Render(lipgloss.JoinVertical(lipgloss.Left,
			lipgloss.NewStyle().Foreground(t.TextMuted).Render(label),
			s.editValue.View(),
			lipgloss.NewStyle().Foreground(t.TextMuted).Render("↵ save  esc cancel"),
		))
}

func settingRow(label, value string, t theme.Theme) string {
	labelS := lipgloss.NewStyle().Foreground(t.TextMuted).Width(28).PaddingLeft(4).Render(label)
	valueS := lipgloss.NewStyle().Foreground(t.Text).Render(value)
	return labelS + valueS
}

func boolStr(b bool) string {
	if b {
		return "yes"
	}
	return "no"
}
