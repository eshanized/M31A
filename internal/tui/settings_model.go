package tui

import (
	"fmt"

	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/keychain"
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

var tabIcons = map[settingsTab]string{
	tabGeneral:     "⚙",
	tabProvider:    "🔑",
	tabModel:       "🤖",
	tabPermissions: "🛡",
	tabFeatures:    "📋",
	tabLedger:      "📊",
}

var fieldDescriptions = map[string]string{
	"ui.theme":                    "Controls the color scheme of M31A. Options: \"dark\", \"light\", \"auto\"",
	"ui.compact_mode":             "Reduces spacing for more content on screen",
	"ui.show_token_usage":         "Displays token count in the header bar",
	"ui.show_cost_estimate":       "Shows estimated cost per request",
	"ui.max_iterations":           "Maximum tool call iterations per response",
	"provider.default":            "Which provider to use by default (openrouter or zen)",
	"provider.auto_fallback":      "Automatically switch provider on rate limit or error",
	"provider.openrouter.api_key": "API key for OpenRouter gateway. Stored in OS keychain on save.",
	"provider.zen.api_key":        "API key for OpenCode Zen gateway. Stored in OS keychain on save.",
	"model.default":               "Default model ID for chat completions",
	"model.context_warning_threshold": "Context usage % that triggers a warning banner (0.0-1.0)",
	"model.show_thinking_by_default":  "Show thinking/reasoning blocks by default",
	"model.auto_collapse_tools":       "Automatically collapse tool call output in chat",
	"model.auto_arbitrage":            "Automatically suggest cheaper models for simple tasks",
	"model.arbitrage_threshold":       "Complexity threshold for auto-arbitrage suggestions (0.0-1.0)",
	"permissions.default_mode":    "Permission mode: ask, auto, or deny",
	"permissions.timeout_seconds": "Seconds to wait for permission response before auto-deny",
	"features.auto_backup":        "Create backup before file writes",
	"features.resume_on_startup":  "Offer to resume last session on startup",
	"ledger.enabled":              "Enable cross-session learning ledger",
	"ledger.max_entries":          "Maximum entries in the learning ledger",
}

// editableField represents a single config field that can be edited inline.
type editableField struct {
	label     string // display label
	value     string // current displayed value
	original  string // original value for cancel
	editing   bool   // currently being edited inline
	masked    bool   // whether to mask display (for API keys)
	maskChar  string // masking character ("•")
	fieldType string // "string", "int", "float", "bool"
	key       string // config key path for save mapping
}

// SettingsModel provides a tabbed configuration editor with inline editing.
// Follows Bubble Tea model pattern (value receiver, returns new model).
type SettingsModel struct {
	config            *config.Config
	theme             theme.Theme
	activeTab         settingsTab
	fields            map[settingsTab][]*editableField
	focusedField      int
	width             int
	height            int
	dirty             bool
	err               string
	statusMsg         string
	ledger            *ledger.Ledger
	configPath        string
	keychain          keychain.Keychain
	spinner           spinner.Model
	showUnsavedWarning bool
}

// NewSettingsModel creates a SettingsModel with the given config, theme, and optional ledger.
func NewSettingsModel(cfg *config.Config, configPath string, t theme.Theme, l *ledger.Ledger, kc keychain.Keychain) SettingsModel {
	sp := spinner.New()
	sp.Spinner = spinner.Dot
	sp.Style = lipgloss.NewStyle().Foreground(t.Brand)
	m := SettingsModel{
		config:     cfg,
		configPath: configPath,
		theme:      t,
		activeTab:  tabGeneral,
		fields:     make(map[settingsTab][]*editableField),
		ledger:     l,
		keychain:   kc,
		spinner:    sp,
	}
	m.buildFields()
	return m
}

func (m SettingsModel) Init() tea.Cmd {
	return m.spinner.Tick
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

// SetConfig updates the config pointer (for theme changes from outside).
func (m *SettingsModel) SetConfig(cfg *config.Config) {
	m.config = cfg
	m.buildFields()
	// Re-mask API key fields after rebuild
	for _, fields := range m.fields {
		for _, f := range fields {
			if f.key == "provider.openrouter.api_key" || f.key == "provider.zen.api_key" {
				f.masked = true
			}
		}
	}
}

// SetTheme updates the theme reference (for in-app theme changes).
func (m *SettingsModel) SetTheme(t theme.Theme) {
	m.theme = t
}

// saveAPIKeysToKeychain persists API keys from the config to the OS keychain.
// This ensures keys saved via settings override any stale keychain values on next load.
func (m *SettingsModel) saveAPIKeysToKeychain() {
	if m.keychain == nil {
		return
	}
	if key := m.config.Provider.OpenRouter.APIKey; key != "" {
		m.keychain.Set("openrouter", key)
	}
	if key := m.config.Provider.Zen.APIKey; key != "" {
		m.keychain.Set("zen", key)
	}
}
