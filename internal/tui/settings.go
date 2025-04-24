package tui

import (
	"fmt"
	"strconv"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/ledger"
)

// settingsTab represents a tab in the settings screen.
type settingsTab int

const (
	tabGeneral settingsTab = iota
	tabProvider
	tabModel
	tabPermissions
	tabFeatures
	tabLedger
	tabCount // = 6
)

var tabNames = map[settingsTab]string{
	tabGeneral:     "General",
	tabProvider:    "Provider",
	tabModel:       "Model",
	tabPermissions: "Permissions",
	tabFeatures:    "Features",
	tabLedger:      "Ledger",
}

// editableField represents a single config field that can be edited inline.
type editableField struct {
	label    string // display label
	value    string // current displayed value
	original string // original value for cancel
	editing  bool   // currently being edited inline
	masked   bool   // whether to mask display (for API keys)
	maskChar string // masking character ("•")
	fieldType string // "string", "int", "float", "bool"
	key      string // config key path for save mapping
}

// SettingsModel provides a tabbed configuration editor with inline editing.
// Follows Bubble Tea model pattern (value receiver, returns new model).
type SettingsModel struct {
	config       *config.Config
	theme        theme.Theme
	activeTab    settingsTab
	fields       map[settingsTab][]*editableField
	focusedField int
	width        int
	height       int
	dirty        bool
	err          string
	statusMsg    string
	ledger       *ledger.Ledger
	configPath   string
}

// NewSettingsModel creates a SettingsModel with the given config, theme, and optional ledger.
func NewSettingsModel(cfg *config.Config, configPath string, t theme.Theme, l *ledger.Ledger) SettingsModel {
	m := SettingsModel{
		config:     cfg,
		configPath: configPath,
		theme:      t,
		activeTab:  tabGeneral,
		fields:     make(map[settingsTab][]*editableField),
		ledger:     l,
	}
	m.buildFields()
	return m
}

// buildFields populates editable fields from the current config.
func (m *SettingsModel) buildFields() {
	m.fields = make(map[settingsTab][]*editableField)

	// General tab (5 fields)
	m.fields[tabGeneral] = []*editableField{
		{label: "Theme", value: m.config.UI.Theme, fieldType: "string", key: "ui.theme"},
		{label: "Compact Mode", value: fmtBool(m.config.UI.CompactMode), fieldType: "bool", key: "ui.compact_mode"},
		{label: "Show Token Usage", value: fmtBool(m.config.UI.ShowTokenUsage), fieldType: "bool", key: "ui.show_token_usage"},
		{label: "Show Cost Estimate", value: fmtBool(m.config.UI.ShowCostEstimate), fieldType: "bool", key: "ui.show_cost_estimate"},
		{label: "Max Iterations", value: fmt.Sprintf("%d", m.config.UI.MaxIterations), fieldType: "int", key: "ui.max_iterations"},
	}
	for _, f := range m.fields[tabGeneral] {
		f.original = f.value
	}

	// Provider tab (4 fields)
	orKey := m.config.Provider.OpenRouter.APIKey
	zenKey := m.config.Provider.Zen.APIKey
	m.fields[tabProvider] = []*editableField{
		{label: "Default Provider", value: m.config.Provider.Default, fieldType: "string", key: "provider.default"},
		{label: "Auto Fallback", value: fmtBool(m.config.Provider.AutoFallback), fieldType: "bool", key: "provider.auto_fallback"},
		{label: "OpenRouter API Key", value: orKey, fieldType: "string", key: "provider.openrouter.api_key", masked: true, maskChar: "•"},
		{label: "Zen API Key", value: zenKey, fieldType: "string", key: "provider.zen.api_key", masked: true, maskChar: "•"},
	}
	for _, f := range m.fields[tabProvider] {
		f.original = f.value
	}

	// Model tab (6 fields)
	m.fields[tabModel] = []*editableField{
		{label: "Default Model", value: m.config.Model.Default, fieldType: "string", key: "model.default"},
		{label: "Context Warning Threshold", value: fmt.Sprintf("%.2f", m.config.Model.ContextWarningThreshold), fieldType: "float", key: "model.context_warning_threshold"},
		{label: "Show Thinking By Default", value: fmtBool(m.config.Model.ShowThinkingByDefault), fieldType: "bool", key: "model.show_thinking_by_default"},
		{label: "Auto Collapse Tools", value: fmtBool(m.config.Model.AutoCollapseTools), fieldType: "bool", key: "model.auto_collapse_tools"},
		{label: "Auto Arbitrage", value: fmtBool(m.config.Model.AutoArbitrage), fieldType: "bool", key: "model.auto_arbitrage"},
		{label: "Arbitrage Threshold", value: fmt.Sprintf("%.2f", m.config.Model.ArbitrageThreshold), fieldType: "float", key: "model.arbitrage_threshold"},
	}
	for _, f := range m.fields[tabModel] {
		f.original = f.value
	}

	// Permissions tab (2 fields + read-only rules count)
	m.fields[tabPermissions] = []*editableField{
		{label: "Default Mode", value: m.config.Permissions.DefaultMode, fieldType: "string", key: "permissions.default_mode"},
		{label: "Timeout Seconds", value: fmt.Sprintf("%d", m.config.Permissions.TimeoutSeconds), fieldType: "int", key: "permissions.timeout_seconds"},
	}
	for _, f := range m.fields[tabPermissions] {
		f.original = f.value
	}

	// Features tab (2 fields)
	m.fields[tabFeatures] = []*editableField{
		{label: "Auto Backup", value: fmtBool(m.config.Features.AutoBackup), fieldType: "bool", key: "features.auto_backup"},
		{label: "Resume On Startup", value: fmtBool(m.config.Features.ResumeOnStartup), fieldType: "bool", key: "features.resume_on_startup"},
	}
	for _, f := range m.fields[tabFeatures] {
		f.original = f.value
	}

	// Ledger tab (2 fields)
	m.fields[tabLedger] = []*editableField{
		{label: "Enabled", value: fmtBool(m.config.Ledger.Enabled), fieldType: "bool", key: "ledger.enabled"},
		{label: "Max Entries", value: fmt.Sprintf("%d", m.config.Ledger.MaxEntries), fieldType: "int", key: "ledger.max_entries"},
	}
	for _, f := range m.fields[tabLedger] {
		f.original = f.value
	}
}

// fmtBool formats a boolean as "true" or "false".
func fmtBool(v bool) string {
	if v {
		return "true"
	}
	return "false"
}

// Tab navigation (wraps at 6 tabs).

func (m SettingsModel) nextTab() SettingsModel {
	m.activeTab = (m.activeTab + 1) % tabCount
	m.focusedField = 0
	return m
}

func (m SettingsModel) prevTab() SettingsModel {
	m.activeTab = (m.activeTab - 1 + tabCount) % tabCount
	m.focusedField = 0
	return m
}

// Inline editing helpers.

func (m SettingsModel) isEditing() bool {
	fields := m.fields[m.activeTab]
	if m.focusedField < 0 || m.focusedField >= len(fields) {
		return false
	}
	return fields[m.focusedField].editing
}

func (m SettingsModel) startEdit() SettingsModel {
	fields := m.fields[m.activeTab]
	if m.focusedField < 0 || m.focusedField >= len(fields) {
		return m
	}
	f := fields[m.focusedField]
	if f.fieldType == "bool" {
		// Bool fields toggle on Enter, no edit mode
		if f.value == "true" {
			f.value = "false"
		} else {
			f.value = "true"
		}
		m.dirty = true
		return m
	}
	// Enter edit mode for non-bool fields
	f.editing = true
	f.original = f.value
	// Unmask API key while editing
	if f.masked {
		f.masked = false
	}
	return m
}

func (m SettingsModel) confirmEdit() SettingsModel {
	fields := m.fields[m.activeTab]
	if m.focusedField < 0 || m.focusedField >= len(fields) {
		return m
	}
	f := fields[m.focusedField]
	if !f.editing {
		return m
	}
	// Validate before confirming
	valid := true
	switch f.fieldType {
	case "int":
		if f.value != "" {
			_, err := strconv.Atoi(f.value)
			if err != nil {
				m.err = "Invalid integer value"
				valid = false
			}
		}
	case "float":
		if f.value != "" {
			_, err := strconv.ParseFloat(f.value, 64)
			if err != nil {
				m.err = "Invalid float value"
				valid = false
			}
		}
	}
	if !valid {
		return m
	}
	f.editing = false
	m.err = ""
	if f.value != f.original {
		m.dirty = true
	}
	// Re-mask API key after edit
	if f.masked == false && (f.key == "provider.openrouter.api_key" || f.key == "provider.zen.api_key") {
		f.masked = true
	}
	return m
}

func (m SettingsModel) cancelEdit() SettingsModel {
	fields := m.fields[m.activeTab]
	if m.focusedField < 0 || m.focusedField >= len(fields) {
		return m
	}
	f := fields[m.focusedField]
	if !f.editing {
		return m
	}
	f.editing = false
	f.value = f.original
	m.err = ""
	// Re-mask API key after cancel
	if f.masked == false && (f.key == "provider.openrouter.api_key" || f.key == "provider.zen.api_key") {
		f.masked = true
	}
	return m
}

func (m SettingsModel) insertChar(ch string) SettingsModel {
	fields := m.fields[m.activeTab]
	if m.focusedField < 0 || m.focusedField >= len(fields) {
		return m
	}
	f := fields[m.focusedField]
	if !f.editing {
		return m
	}
	// Validate input for int/float fields
	if f.fieldType == "int" {
		if ch < "0" || ch > "9" {
			if ch != "-" {
				return m
			}
		}
	}
	if f.fieldType == "float" {
		if ch < "0" || ch > "9" {
			if ch != "." && ch != "-" {
				return m
			}
		}
	}
	f.value += ch
	m.err = ""
	return m
}

func (m SettingsModel) deleteChar() SettingsModel {
	fields := m.fields[m.activeTab]
	if m.focusedField < 0 || m.focusedField >= len(fields) {
		return m
	}
	f := fields[m.focusedField]
	if !f.editing {
		return m
	}
	if len(f.value) > 0 {
		f.value = f.value[:len(f.value)-1]
	}
	m.err = ""
	return m
}

// applyFieldValues pushes field values from all tabs back to the config struct.
func (m *SettingsModel) applyFieldValues() {
	for _, fields := range m.fields {
		for _, f := range fields {
			switch f.key {
			// General
			case "ui.theme":
				m.config.UI.Theme = f.value
			case "ui.compact_mode":
				m.config.UI.CompactMode = f.value == "true"
			case "ui.show_token_usage":
				m.config.UI.ShowTokenUsage = f.value == "true"
			case "ui.show_cost_estimate":
				m.config.UI.ShowCostEstimate = f.value == "true"
			case "ui.max_iterations":
				if v, err := strconv.Atoi(f.value); err == nil {
					m.config.UI.MaxIterations = v
				}
			// Provider
			case "provider.default":
				m.config.Provider.Default = f.value
			case "provider.auto_fallback":
				m.config.Provider.AutoFallback = f.value == "true"
			case "provider.openrouter.api_key":
				m.config.Provider.OpenRouter.APIKey = f.value
			case "provider.zen.api_key":
				m.config.Provider.Zen.APIKey = f.value
			// Model
			case "model.default":
				m.config.Model.Default = f.value
			case "model.context_warning_threshold":
				if v, err := strconv.ParseFloat(f.value, 64); err == nil {
					m.config.Model.ContextWarningThreshold = v
				}
			case "model.show_thinking_by_default":
				m.config.Model.ShowThinkingByDefault = f.value == "true"
			case "model.auto_collapse_tools":
				m.config.Model.AutoCollapseTools = f.value == "true"
			case "model.auto_arbitrage":
				m.config.Model.AutoArbitrage = f.value == "true"
			case "model.arbitrage_threshold":
				if v, err := strconv.ParseFloat(f.value, 64); err == nil {
					m.config.Model.ArbitrageThreshold = v
				}
			// Permissions
			case "permissions.default_mode":
				m.config.Permissions.DefaultMode = f.value
			case "permissions.timeout_seconds":
				if v, err := strconv.Atoi(f.value); err == nil {
					m.config.Permissions.TimeoutSeconds = v
				}
			// Features
			case "features.auto_backup":
				m.config.Features.AutoBackup = f.value == "true"
			case "features.resume_on_startup":
				m.config.Features.ResumeOnStartup = f.value == "true"
			// Ledger
			case "ledger.enabled":
				m.config.Ledger.Enabled = f.value == "true"
			case "ledger.max_entries":
				if v, err := strconv.Atoi(f.value); err == nil {
					m.config.Ledger.MaxEntries = v
				}
			}
		}
	}
}


// applyFieldsToConfig pushes the snapshot field values to the config struct.
func applyFieldsToConfig(cfg *config.Config, snapshots map[settingsTab][]editableField) {
	for _, fields := range snapshots {
		for _, f := range fields {
			switch f.key {
			// General
			case "ui.theme":
				cfg.UI.Theme = f.value
			case "ui.compact_mode":
				cfg.UI.CompactMode = f.value == "true"
			case "ui.show_token_usage":
				cfg.UI.ShowTokenUsage = f.value == "true"
			case "ui.show_cost_estimate":
				cfg.UI.ShowCostEstimate = f.value == "true"
			case "ui.max_iterations":
				if v, err := strconv.Atoi(f.value); err == nil {
					cfg.UI.MaxIterations = v
				}
			// Provider
			case "provider.default":
				cfg.Provider.Default = f.value
			case "provider.auto_fallback":
				cfg.Provider.AutoFallback = f.value == "true"
			case "provider.openrouter.api_key":
				cfg.Provider.OpenRouter.APIKey = f.value
			case "provider.zen.api_key":
				cfg.Provider.Zen.APIKey = f.value
			// Model
			case "model.default":
				cfg.Model.Default = f.value
			case "model.context_warning_threshold":
				if v, err := strconv.ParseFloat(f.value, 64); err == nil {
					cfg.Model.ContextWarningThreshold = v
				}
			case "model.show_thinking_by_default":
				cfg.Model.ShowThinkingByDefault = f.value == "true"
			case "model.auto_collapse_tools":
				cfg.Model.AutoCollapseTools = f.value == "true"
			case "model.auto_arbitrage":
				cfg.Model.AutoArbitrage = f.value == "true"
			case "model.arbitrage_threshold":
				if v, err := strconv.ParseFloat(f.value, 64); err == nil {
					cfg.Model.ArbitrageThreshold = v
				}
			// Permissions
			case "permissions.default_mode":
				cfg.Permissions.DefaultMode = f.value
			case "permissions.timeout_seconds":
				if v, err := strconv.Atoi(f.value); err == nil {
					cfg.Permissions.TimeoutSeconds = v
				}
			// Features
			case "features.auto_backup":
				cfg.Features.AutoBackup = f.value == "true"
			case "features.resume_on_startup":
				cfg.Features.ResumeOnStartup = f.value == "true"
			// Ledger
			case "ledger.enabled":
				cfg.Ledger.Enabled = f.value == "true"
			case "ledger.max_entries":
				if v, err := strconv.Atoi(f.value); err == nil {
					cfg.Ledger.MaxEntries = v
				}
			}
		}
	}
}

// Update handles messages for the settings screen.
func (m SettingsModel) Update(msg tea.Msg) (SettingsModel, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return m, nil

	case SettingsSavedMsg:
		m.dirty = false
		m.statusMsg = "Configuration saved successfully."
		return m, nil

	case ErrorMsg:
		m.err = fmt.Sprintf("Save error: %v", msg.Err)
		return m, nil

	case tea.KeyMsg:
		switch msg.String() {
		case "ctrl+c":
			return m, tea.Quit

		case "esc":
			if m.isEditing() {
				return m.cancelEdit(), nil
			}
			// Not editing — return to REPL
			return m, func() tea.Msg {
				return AppMsg{Screen: ScreenREPL}
			}

		case "tab", "right":
			if m.isEditing() {
				// Try to insert character (only for single-char keys)
				if len(msg.String()) == 1 {
					return m.insertChar(msg.String()), nil
				}
				return m, nil
			}
			return m.nextTab(), nil

		case "shift+tab", "left":
			if !m.isEditing() {
				return m.prevTab(), nil
			}
			// If editing a masked field, allow arrow key insertion
			return m, nil

		case "down":
			if !m.isEditing() {
				fields := m.fields[m.activeTab]
				if m.focusedField < len(fields)-1 {
					m.focusedField++
				}
				return m, nil
			}
			return m, nil

		case "up":
			if !m.isEditing() {
				if m.focusedField > 0 {
					m.focusedField--
				}
				return m, nil
			}
			return m, nil

		case "enter":
			if m.isEditing() {
				return m.confirmEdit(), nil
			}
			return m.startEdit(), nil

		case "ctrl+s":
			// Save synchronously to avoid async closure issues
			m.applyFieldValues()
			m.dirty = false
			if err := m.config.Save(m.configPath); err != nil {
				m.err = fmt.Sprintf("Save failed: %v", err)
				return m, nil
			}
			m.statusMsg = "Configuration saved successfully."
			return m, func() tea.Msg {
				return SettingsSavedMsg{}
			}

		case "backspace":
			if m.isEditing() {
				return m.deleteChar(), nil
			}
			return m, nil

		default:
			if m.isEditing() && len(msg.String()) == 1 {
				return m.insertChar(msg.String()), nil
			}
			return m, nil
		}
	}

	return m, nil
}

// View renders the settings screen.
func (m SettingsModel) View() string {
	if m.width == 0 || m.height == 0 {
		return "Loading..."
	}

	tabBar := m.renderTabBar()
	content := m.renderActiveTab()
	footer := m.renderFooter()

	return lipgloss.JoinVertical(lipgloss.Top, tabBar, "", content, "", footer)
}

// renderTabBar renders the tab headers row.
func (m SettingsModel) renderTabBar() string {
	var tabs []string
	for i := 0; i < int(tabCount); i++ {
		tab := settingsTab(i)
		name := tabNames[tab]

		var style lipgloss.Style
		if tab == m.activeTab {
			style = lipgloss.NewStyle().
				Background(lipgloss.Color(m.theme.Brand)).
				Foreground(lipgloss.Color("#FFFFFF")).
				Padding(0, 2).
				Bold(true)
		} else {
			style = lipgloss.NewStyle().
				Background(lipgloss.Color(m.theme.Surface)).
				Foreground(lipgloss.Color(m.theme.TextSecondary)).
				Padding(0, 2)
		}
		tabs = append(tabs, style.Render(name))
	}
	return lipgloss.JoinHorizontal(lipgloss.Top, tabs...)
}

// renderActiveTab dispatches to the correct tab renderer.
func (m SettingsModel) renderActiveTab() string {
	switch m.activeTab {
	case tabGeneral:
		return m.renderGeneralTab()
	case tabProvider:
		return m.renderProviderTab()
	case tabModel:
		return m.renderModelTab()
	case tabPermissions:
		return m.renderPermissionsTab()
	case tabFeatures:
		return m.renderFeaturesTab()
	case tabLedger:
		return m.renderLedgerTab()
	default:
		return ""
	}
}

// renderField renders a single editable field with label and value.
func (m SettingsModel) renderField(f *editableField, isFocused bool) string {
	labelStyle := lipgloss.NewStyle().Width(30).Align(lipgloss.Right).PaddingRight(1).
		Foreground(lipgloss.Color(m.theme.TextPrimary))

	label := labelStyle.Render(f.label)

	var value string
	if isFocused && f.editing {
		// Editing mode — show current value with cursor marker
		editingStyle := lipgloss.NewStyle().
			Foreground(lipgloss.Color(m.theme.Success)).
			Background(lipgloss.Color(m.theme.Surface))
		val := f.value
		if val == "" {
			val = " "
		}
		value = editingStyle.Render(">" + val + "<")
	} else if f.masked {
		// Masked display
		mutedStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.TextSecondary))
		if f.value == "" {
			value = mutedStyle.Render("(not configured)")
		} else {
			masked := strings.Repeat(f.maskChar, 8)
			value = mutedStyle.Render(masked)
			if isFocused {
				value += " " + mutedStyle.Render("(Enter to reveal)")
			}
		}
	} else if isFocused {
		// Focused but not editing
		focusedStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Brand))
		if f.value == "" {
			value = focusedStyle.Render("[empty]")
		} else {
			value = focusedStyle.Render(f.value)
		}
	} else {
		// Normal display
		valStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.TextPrimary))
		if f.value == "" {
			value = lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.TextSecondary)).Render("(empty)")
		} else {
			value = valStyle.Render(f.value)
		}
	}

	// Apply dirty indicator
	if isFocused && m.dirty {
		dirtyStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Warning)).Bold(true)
		return dirtyStyle.Render("▶ ") + label + "  " + value
	}

	if isFocused {
		return "▶ " + label + "  " + value
	}
	return "  " + label + "  " + value
}

// renderGeneralTab renders the General tab.
func (m SettingsModel) renderGeneralTab() string {
	var lines []string
	lines = append(lines, m.sectionHeader("General Settings"))
	lines = append(lines, "")

	for i, f := range m.fields[tabGeneral] {
		lines = append(lines, m.renderField(f, i == m.focusedField))
	}

	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.TextSecondary)).
		Render("  Theme: \"auto\", \"dark\", or \"light\""),
	)

	return strings.Join(lines, "\n")
}

// renderProviderTab renders the Provider tab.
func (m SettingsModel) renderProviderTab() string {
	var lines []string
	lines = append(lines, m.sectionHeader("Provider Settings"))
	lines = append(lines, "")

	for i, f := range m.fields[tabProvider] {
		lines = append(lines, m.renderField(f, i == m.focusedField))
	}

	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.TextSecondary)).
		Render("  API keys are masked by default. Press Enter to reveal."),
	)
	lines = append(lines, lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.TextSecondary)).
		Render("  Store keys in OS keychain via Settings on save."),
	)

	return strings.Join(lines, "\n")
}

// renderModelTab renders the Model tab.
func (m SettingsModel) renderModelTab() string {
	var lines []string
	lines = append(lines, m.sectionHeader("Model Settings"))
	lines = append(lines, "")

	for i, f := range m.fields[tabModel] {
		lines = append(lines, m.renderField(f, i == m.focusedField))
	}

	return strings.Join(lines, "\n")
}

// renderPermissionsTab renders the Permissions tab.
func (m SettingsModel) renderPermissionsTab() string {
	var lines []string
	lines = append(lines, m.sectionHeader("Permissions Settings"))
	lines = append(lines, "")

	for i, f := range m.fields[tabPermissions] {
		lines = append(lines, m.renderField(f, i == m.focusedField))
	}

	// Read-only rules count
	ruleText := fmt.Sprintf("%d rule(s) configured", len(m.config.Permissions.Rules))
	if len(m.config.Permissions.Rules) == 0 {
		ruleText = "No permission rules configured"
	}
	lines = append(lines, "  "+lipgloss.NewStyle().
		Width(30).Align(lipgloss.Right).PaddingRight(1).
		Foreground(lipgloss.Color(m.theme.TextPrimary)).
		Render("Permission Rules")+"  "+
		lipgloss.NewStyle().
			Foreground(lipgloss.Color(m.theme.TextSecondary)).
			Render(ruleText),
	)

	return strings.Join(lines, "\n")
}

// renderFeaturesTab renders the Features tab.
func (m SettingsModel) renderFeaturesTab() string {
	var lines []string
	lines = append(lines, m.sectionHeader("Features Settings"))
	lines = append(lines, "")

	for i, f := range m.fields[tabFeatures] {
		lines = append(lines, m.renderField(f, i == m.focusedField))
	}

	return strings.Join(lines, "\n")
}

// renderLedgerTab renders the Ledger tab with stats.
func (m SettingsModel) renderLedgerTab() string {
	var lines []string
	lines = append(lines, m.sectionHeader("Ledger Settings"))
	lines = append(lines, "")

	for i, f := range m.fields[tabLedger] {
		lines = append(lines, m.renderField(f, i == m.focusedField))
	}

	// Stats section
	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.Brand)).
		Bold(true).
		Render("  Ledger Statistics"),
	)

	if m.ledger != nil {
		stats := m.ledger.Stats()
		lines = append(lines, fmt.Sprintf("  Total Sessions:    %d", stats.TotalSessions))
		lines = append(lines, fmt.Sprintf("  Avg Tasks/Session: %.1f", stats.AvgTaskCount))
		lines = append(lines, fmt.Sprintf("  Avg Duration:      %.0f min", stats.AvgDurationMinutes))
		lines = append(lines, fmt.Sprintf("  Avg Cost:          $%.4f", stats.AvgCost))
		lines = append(lines, fmt.Sprintf("  Total Failed:      %d", stats.TotalFailedTasks))

		if len(stats.TopFrameworks) > 0 {
			lines = append(lines, fmt.Sprintf("  Top Frameworks:    %s", strings.Join(stats.TopFrameworks, ", ")))
		}
		if len(stats.TopFailures) > 0 {
			lines = append(lines, fmt.Sprintf("  Top Failures:      %s", strings.Join(stats.TopFailures, ", ")))
		}
		if len(stats.ByProjectType) > 0 {
			lines = append(lines, "")
			lines = append(lines, lipgloss.NewStyle().
				Foreground(lipgloss.Color(m.theme.TextSecondary)).
				Render("  By Project Type:"),
			)
			for ptype, count := range stats.ByProjectType {
				if ptype == "" {
					ptype = "(unknown)"
				}
				lines = append(lines, fmt.Sprintf("    %s: %d", ptype, count))
			}
		}
	} else {
		lines = append(lines, "  "+lipgloss.NewStyle().
			Foreground(lipgloss.Color(m.theme.TextSecondary)).
			Render("Ledger not initialized"),
		)

	}

	return strings.Join(lines, "\n")
}

// renderFooter renders the footer with key hints and status.
func (m SettingsModel) renderFooter() string {
	helpStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.TextSecondary)).Faint(true)
	footer := helpStyle.Render("[Tab/←→] Navigate  [↑↓] Select  [Enter] Edit/Toggle  [Esc] Back  [Ctrl+S] Save")

	if m.dirty {
		dirtyStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Warning)).Bold(true)
		footer = dirtyStyle.Render("Unsaved changes • ") + footer
	}
	if m.statusMsg != "" {
		statusStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Success))
		footer = statusStyle.Render(m.statusMsg) + "\n" + footer
	}
	if m.err != "" {
		errStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Error))
		footer = errStyle.Render(m.err) + "\n" + footer
	}

	return footer
}

// sectionHeader renders a section title with theme styling.
func (m SettingsModel) sectionHeader(label string) string {
	return lipgloss.NewStyle().
		Foreground(lipgloss.Color(m.theme.Brand)).
		Bold(true).
		Render(label)
}

// SetConfig updates the config pointer (for theme changes from outside).
func (m *SettingsModel) SetConfig(cfg *config.Config) {
	m.config = cfg
	m.buildFields()
}

// SetTheme updates the theme reference (for in-app theme changes).
func (m *SettingsModel) SetTheme(t theme.Theme) {
	m.theme = t
}
