package tui

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/charmbracelet/bubbles/spinner"
	tea "github.com/charmbracelet/bubbletea"
)

// Update handles messages for the settings screen.
func (m SettingsModel) Update(msg tea.Msg) (SettingsModel, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return m, nil

	case spinner.TickMsg:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		return m, cmd

	case SettingsSavedMsg:
		m.dirty = false
		m.statusMsg = "Configuration saved successfully."
		return m, nil

	case ErrorMsg:
		m.err = fmt.Sprintf("Save error: %v", msg.Err)
		return m, nil

	case tea.KeyMsg:
		return m.handleKeyMsg(msg)
	}

	return m, nil
}

// handleKeyMsg processes keyboard input for the settings screen.
func (m SettingsModel) handleKeyMsg(msg tea.KeyMsg) (SettingsModel, tea.Cmd) {
	switch msg.String() {
	case "ctrl+c":
		return m, tea.Quit

	case "esc":
		if m.showUnsavedWarning {
			m.showUnsavedWarning = false
			return m, nil
		}
		if m.isEditing() {
			return m.cancelEdit(), nil
		}
		// Not editing — check for unsaved changes
		if m.dirty {
			m.showUnsavedWarning = true
			return m, nil
		}
		// No changes — return to REPL
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
		return m.save()

	case "y", "Y":
		// Save and exit from unsaved warning
		if m.showUnsavedWarning {
			return m.saveAndExit()
		}

	case "n", "N":
		// Discard and exit from unsaved warning
		if m.showUnsavedWarning {
			m.dirty = false
			m.showUnsavedWarning = false
			return m, func() tea.Msg {
				return AppMsg{Screen: ScreenREPL}
			}
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

	return m, nil
}

// save applies field values and persists to config file and keychain.
func (m SettingsModel) save() (SettingsModel, tea.Cmd) {
	oldTheme := m.config.UI.Theme
	m.applyFieldValues()
	m.dirty = false
	if err := m.config.Save(m.configPath); err != nil {
		m.err = fmt.Sprintf("Save failed: %v", err)
		return m, nil
	}
	m.saveAPIKeysToKeychain()
	m.statusMsg = "Configuration saved successfully."
	// H-5 fix: emit ThemeChangedMsg if theme actually changed
	var cmds []tea.Cmd
	cmds = append(cmds, func() tea.Msg {
		return SettingsSavedMsg{}
	})
	if m.config.UI.Theme != oldTheme {
		cmds = append(cmds, func() tea.Msg {
			return ThemeChangedMsg{Theme: m.config.UI.Theme}
		})
	}
	return m, tea.Batch(cmds...)
}

// saveAndExit saves changes and returns to the REPL screen.
func (m SettingsModel) saveAndExit() (SettingsModel, tea.Cmd) {
	oldTheme := m.config.UI.Theme
	m.applyFieldValues()
	m.dirty = false
	m.showUnsavedWarning = false
	if err := m.config.Save(m.configPath); err != nil {
		m.err = fmt.Sprintf("Save failed: %v", err)
		return m, nil
	}
	m.saveAPIKeysToKeychain()
	var cmds []tea.Cmd
	cmds = append(cmds, func() tea.Msg {
		return AppMsg{Screen: ScreenREPL}
	})
	if m.config.UI.Theme != oldTheme {
		cmds = append(cmds, func() tea.Msg {
			return ThemeChangedMsg{Theme: m.config.UI.Theme}
		})
	}
	return m, tea.Batch(cmds...)
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
	if !f.masked && (f.key == "provider.openrouter.api_key" || f.key == "provider.zen.api_key") {
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
	if !f.masked && (f.key == "provider.openrouter.api_key" || f.key == "provider.zen.api_key") {
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
		// Allow digits and a single leading minus
		if ch >= "0" && ch <= "9" {
			// OK
		} else if ch == "-" && f.value == "" {
			// OK: leading minus
		} else {
			return m
		}
	}
	if f.fieldType == "float" {
		if ch >= "0" && ch <= "9" {
			// OK
		} else if ch == "." && !strings.Contains(f.value, ".") {
			// OK: single decimal point
		} else if ch == "-" && f.value == "" {
			// OK: leading minus
		} else {
			return m
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
