package tui

import (
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tui/theme"
)

func newTestSettingsModel() SettingsModel {
	m := NewSettingsModel(config.DefaultConfig(), "/tmp/.m31a/config.toml", theme.Dark(), nil)
	m.width = 80
	m.height = 24
	return m
}

func TestSettings_TabCount(t *testing.T) {
	m := newTestSettingsModel()
	if len(m.fields) != int(tabCount) {
		t.Errorf("Expected %d tabs, got %d", tabCount, len(m.fields))
	}
	// Verify all 6 tabs exist
	for tab := tabGeneral; tab < tabCount; tab++ {
		name, ok := tabNames[tab]
		if !ok || name == "" {
			t.Errorf("Tab %d missing from tabNames", tab)
		}
		fields, ok := m.fields[tab]
		if !ok || len(fields) == 0 {
			t.Errorf("Tab %d (%s) has no fields", tab, name)
		}
	}
}

func TestSettings_TabCycling(t *testing.T) {
	m := newTestSettingsModel()

	// Verify initial state is tabGeneral (0)
	if m.activeTab != tabGeneral {
		t.Fatalf("Expected initial activeTab=tabGeneral, got %d", m.activeTab)
	}

	// Cycle forward through all 6 tabs
	expectedTabs := []settingsTab{tabProvider, tabModel, tabPermissions, tabFeatures, tabLedger, tabGeneral}
	for _, expected := range expectedTabs {
		m, _ = m.Update(tea.KeyMsg{Type: tea.KeyTab})
		if m.activeTab != expected {
			t.Fatalf("After Tab, expected activeTab=%d (%s), got %d", expected, tabNames[expected], m.activeTab)
		}
	}

	// Test shift+tab reverses: tabGeneral → tabLedger
	m, _ = m.Update(tea.KeyMsg{Type: tea.KeyShiftTab})
	if m.activeTab != tabLedger {
		t.Errorf("After shift+tab, expected tabLedger=%d, got %d", tabLedger, m.activeTab)
	}

	// shift+tab wraps: tabGeneral → tabLedger → tabFeatures
	m.activeTab = tabGeneral
	m, _ = m.Update(tea.KeyMsg{Type: tea.KeyShiftTab})
	if m.activeTab != tabLedger {
		t.Errorf("After shift+tab from tabGeneral, expected tabLedger=%d, got %d", tabLedger, m.activeTab)
	}
}

func TestSettings_GeneralTabFields(t *testing.T) {
	m := newTestSettingsModel()
	fields := m.fields[tabGeneral]

	expectedLabels := []string{
		"Theme", "Compact Mode", "Show Token Usage",
		"Show Cost Estimate", "Max Iterations",
	}
	if len(fields) != len(expectedLabels) {
		t.Fatalf("General tab: expected %d fields, got %d", len(expectedLabels), len(fields))
	}
	for i, exp := range expectedLabels {
		if fields[i].label != exp {
			t.Errorf("General tab field %d: expected label %q, got %q", i, exp, fields[i].label)
		}
	}
}

func TestSettings_ModelTabFields(t *testing.T) {
	m := newTestSettingsModel()
	fields := m.fields[tabModel]

	expectedLabels := []string{
		"Default Model",
		"Context Warning Threshold",
		"Show Thinking By Default",
		"Auto Collapse Tools",
		"Auto Arbitrage",
		"Arbitrage Threshold",
	}
	if len(fields) != len(expectedLabels) {
		t.Fatalf("Model tab: expected %d fields, got %d", len(expectedLabels), len(fields))
	}
	for i, exp := range expectedLabels {
		if fields[i].label != exp {
			t.Errorf("Model tab field %d: expected label %q, got %q", i, exp, fields[i].label)
		}
	}
}

func TestSettings_LedgerTabFields(t *testing.T) {
	m := newTestSettingsModel()
	fields := m.fields[tabLedger]

	expectedLabels := []string{
		"Enabled",
		"Max Entries",
	}
	if len(fields) != len(expectedLabels) {
		t.Fatalf("Ledger tab: expected %d fields, got %d", len(expectedLabels), len(fields))
	}
	for i, exp := range expectedLabels {
		if fields[i].label != exp {
			t.Errorf("Ledger tab field %d: expected label %q, got %q", i, exp, fields[i].label)
		}
	}
}

func TestSettings_APIKeyMasked(t *testing.T) {
	m := newTestSettingsModel()
	m.activeTab = tabProvider

	// Set API key and verify it's masked
	m.config.Provider.OpenRouter.APIKey = "sk-or-v1-secret-key-value"
	// Rebuild fields to pick up the config change
	m.buildFields()

	v := m.View()

	if strings.Contains(v, "sk-or-v1-secret-key-value") {
		t.Error("API key should not appear in plaintext in View()")
	}
	if !strings.Contains(v, "••••••••") {
		t.Error("API key should be masked as •••••••• in View()")
	}
}

func TestSettings_APIKeyReveal(t *testing.T) {
	m := newTestSettingsModel()
	m.activeTab = tabProvider

	// Focus the OpenRouter API key field
	m.config.Provider.OpenRouter.APIKey = "sk-or-v1-test"
	m.buildFields()
	m.focusedField = 2 // OpenRouter API Key is the 3rd field (index 2)

	f := m.fields[tabProvider][2]
	if !f.masked {
		t.Error("API key field should start masked")
	}

	// Enter edit mode - should unmask
	m = m.startEdit()
	if m.fields[tabProvider][2].masked {
		t.Error("API key field should be unmasked during edit")
	}

	// Confirm edit - should re-mask
	m = m.confirmEdit()
	if !m.fields[tabProvider][2].masked {
		t.Error("API key field should be re-masked after edit confirmation")
	}

	// Test cancel edit
	m = m.startEdit()
	if m.fields[tabProvider][2].masked {
		t.Error("API key should be unmasked during edit (cancel test)")
	}
	m = m.cancelEdit()
	if !m.fields[tabProvider][2].masked {
		t.Error("API key field should be re-masked after cancel")
	}
}

func TestSettings_InlineEditString(t *testing.T) {
	m := newTestSettingsModel()
	m.activeTab = tabGeneral
	m.focusedField = 0 // Theme field

	f := m.fields[tabGeneral][0]
	orig := f.value

	// Start editing
	m = m.startEdit()
	if !m.fields[tabGeneral][0].editing {
		t.Error("Field should be in editing mode")
	}

	// Insert characters
	m = m.insertChar("n")
	m = m.insertChar("e")
	m = m.insertChar("w")

	if m.fields[tabGeneral][0].value != orig+"new" {
		t.Errorf("After insert, expected %q, got %q", orig+"new", m.fields[tabGeneral][0].value)
	}

	// Confirm edit
	m = m.confirmEdit()
	if m.fields[tabGeneral][0].editing {
		t.Error("Field should not be in editing mode after confirm")
	}
	if !m.dirty {
		t.Error("Model should be dirty after editing a field")
	}
}

func TestSettings_InlineEditBool(t *testing.T) {
	m := newTestSettingsModel()
	m.activeTab = tabFeatures
	m.focusedField = 0 // AutoDream Enabled

	orig := m.fields[tabFeatures][0].value

	// Toggle bool field with Enter
	m = m.startEdit()

	expected := "false"
	if orig == "false" {
		expected = "true"
	}
	if m.fields[tabFeatures][0].value != expected {
		t.Errorf("Bool toggle: expected %q, got %q", expected, m.fields[tabFeatures][0].value)
	}
	if !m.dirty {
		t.Error("Model should be dirty after toggling bool")
	}

	// Toggle back
	m.dirty = false
	m = m.startEdit()
	if m.fields[tabFeatures][0].value != orig {
		t.Errorf("Bool toggle back: expected %q, got %q", orig, m.fields[tabFeatures][0].value)
	}
}

func TestSettings_InlineEditInt(t *testing.T) {
	m := newTestSettingsModel()
	m.activeTab = tabGeneral
	m.focusedField = 4 // Max Iterations

	// Start editing
	m = m.startEdit()

	// Insert digits
	m = m.insertChar("4")
	m = m.insertChar("2")

	if m.fields[tabGeneral][4].value != "042" {
		t.Errorf("After inserting 42 into '0', expected '042', got %q", m.fields[tabGeneral][4].value)
	}

	// Confirm edit
	m = m.confirmEdit()
	if m.fields[tabGeneral][4].editing {
		t.Error("Field should not be in editing mode after confirm")
	}
}

func TestSettings_InlineEditInt_Invalid(t *testing.T) {
	m := newTestSettingsModel()
	m.activeTab = tabGeneral
	m.focusedField = 4 // Max Iterations

	// Start editing
	m = m.startEdit()
	orig := m.fields[tabGeneral][4].value

	// Try to insert non-numeric characters - should be rejected
	m = m.insertChar("a")
	m = m.insertChar("b")
	m = m.insertChar("c")

	if m.fields[tabGeneral][4].value != orig {
		t.Errorf("Non-numeric chars should be rejected for int field, got %q", m.fields[tabGeneral][4].value)
	}

	// Valid digits should work (appended to existing "0")
	m = m.insertChar("5")
	expected := orig + "5"
	if m.fields[tabGeneral][4].value != expected {
		t.Errorf("Numeric char should be accepted for int field, expected %q, got %q", expected, m.fields[tabGeneral][4].value)
	}
}

func TestSettings_Save(t *testing.T) {
	m := newTestSettingsModel()

	// Ctrl+S should save and return SettingsSavedMsg
	m, cmd := m.Update(tea.KeyMsg{Type: tea.KeyCtrlS})
	if cmd == nil {
		t.Fatal("Ctrl+S should return a cmd")
	}
	msg := cmd()
	if _, ok := msg.(SettingsSavedMsg); !ok {
		t.Fatalf("Expected SettingsSavedMsg, got %T", msg)
	}
}

func TestSettings_DirtyFlag(t *testing.T) {
	m := newTestSettingsModel()

	// Initially not dirty
	if m.dirty {
		t.Error("Model should not be dirty initially")
	}

	// Edit a field
	m.activeTab = tabGeneral
	m.focusedField = 0
	m = m.startEdit()
	m = m.insertChar("x")
	m = m.confirmEdit()

	if !m.dirty {
		t.Error("Model should be dirty after edit")
	}

	// Save should reset dirty
	m, cmd := m.Update(tea.KeyMsg{Type: tea.KeyCtrlS})
	if cmd == nil {
		t.Fatal("Ctrl+S should return a cmd")
	}
	cmd() // execute the save

	if m.dirty {
		t.Error("Model should not be dirty after save")
	}
}

func TestSettings_TabContent(t *testing.T) {
	m := newTestSettingsModel()

	tests := []struct {
		tab      settingsTab
		keywords []string
	}{
		{tabGeneral, []string{"General", "Theme", "Compact", "Token", "Iterations"}},
		{tabProvider, []string{"Provider", "Default", "Fallback", "API Key"}},
		{tabModel, []string{"Model", "Default Model", "Threshold", "Thinking", "Arbitrage"}},
		{tabPermissions, []string{"Permissions", "Default Mode", "Timeout"}},
		{tabFeatures, []string{"Features", "AutoDream", "Subagent", "Backup", "Resume"}},
		{tabLedger, []string{"Ledger", "Enabled", "Max Entries", "Statistics"}},
	}

	for _, tt := range tests {
		m.activeTab = tt.tab
		v := m.View()
		if v == "" {
			t.Errorf("Tab %d View() should not be empty", tt.tab)
		}
		for _, kw := range tt.keywords {
			if !strings.Contains(v, kw) {
				t.Errorf("Tab %d should contain keyword %q", tt.tab, kw)
			}
		}
	}
}

func TestSettings_WindowSize(t *testing.T) {
	m := NewSettingsModel(config.DefaultConfig(), "/tmp/.m31a/config.toml", theme.Dark(), nil)
	if m.width != 0 || m.height != 0 {
		t.Error("Initial width/height should be 0")
	}

	m, _ = m.Update(tea.WindowSizeMsg{Width: 120, Height: 40})
	if m.width != 120 {
		t.Errorf("Expected width=120, got %d", m.width)
	}
	if m.height != 40 {
		t.Errorf("Expected height=40, got %d", m.height)
	}
}

func TestSettings_EscNavigate(t *testing.T) {
	m := newTestSettingsModel()

	// Esc when not editing should return AppMsg{Screen: ScreenREPL}
	m, cmd := m.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if cmd == nil {
		t.Fatal("Esc should return a cmd")
	}
	msg := cmd()
	appMsg, ok := msg.(AppMsg)
	if !ok {
		t.Fatalf("Expected AppMsg, got %T", msg)
	}
	if appMsg.Screen != ScreenREPL {
		t.Errorf("Expected ScreenREPL, got %d", appMsg.Screen)
	}
}

func TestSettings_EscCancelEdit(t *testing.T) {
	m := newTestSettingsModel()
	m.activeTab = tabGeneral
	m.focusedField = 0

	// Start editing and insert text
	m = m.startEdit()
	m = m.insertChar("x")

	orig := m.fields[tabGeneral][0].original

	// Esc should cancel edit
	m, _ = m.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if m.fields[tabGeneral][0].editing {
		t.Error("Field should not be in editing mode after Esc")
	}
	if m.fields[tabGeneral][0].value != orig {
		t.Errorf("After cancel, value should be %q, got %q", orig, m.fields[tabGeneral][0].value)
	}
}

func TestSettings_DeleteChar(t *testing.T) {
	m := newTestSettingsModel()
	m.activeTab = tabGeneral
	m.focusedField = 0

	// Start editing
	m = m.startEdit()

	// Insert and delete
	m = m.insertChar("a")
	m = m.insertChar("b")
	m = m.deleteChar()

	expected := "a"
	if m.fields[tabGeneral][0].value != expected {
		t.Errorf("After insert 'ab' then delete, expected %q, got %q", expected, m.fields[tabGeneral][0].value)
	}

	// Delete again
	m = m.deleteChar()
	if m.fields[tabGeneral][0].value != "" {
		t.Errorf("After delete all, expected empty, got %q", m.fields[tabGeneral][0].value)
	}

	// Delete on empty should not crash
	m = m.deleteChar() // no-op
}
