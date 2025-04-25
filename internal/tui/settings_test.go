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
	m.focusedField = 0 // Auto Backup

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
		{tabFeatures, []string{"Features", "Backup", "Resume"}},
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

// ---------------------------------------------------------------------------
// Test applyFieldsToConfig
// ---------------------------------------------------------------------------

func TestApplyFieldsToConfig_GeneralTab(t *testing.T) {
	cfg := config.DefaultConfig()
	fields := map[settingsTab][]editableField{
		tabGeneral: {
			{key: "ui.theme", value: "light", fieldType: "string"},
			{key: "ui.compact_mode", value: "true", fieldType: "bool"},
			{key: "ui.show_token_usage", value: "true", fieldType: "bool"},
			{key: "ui.show_cost_estimate", value: "false", fieldType: "bool"},
			{key: "ui.max_iterations", value: "100", fieldType: "int"},
		},
	}

	applyFieldsToConfig(cfg, fields)

	if cfg.UI.Theme != "light" {
		t.Errorf("expected theme 'light', got %q", cfg.UI.Theme)
	}
	if !cfg.UI.CompactMode {
		t.Error("expected CompactMode=true")
	}
	if !cfg.UI.ShowTokenUsage {
		t.Error("expected ShowTokenUsage=true")
	}
	if cfg.UI.ShowCostEstimate {
		t.Error("expected ShowCostEstimate=false")
	}
	if cfg.UI.MaxIterations != 100 {
		t.Errorf("expected MaxIterations=100, got %d", cfg.UI.MaxIterations)
	}
}

func TestApplyFieldsToConfig_ProviderTab(t *testing.T) {
	cfg := config.DefaultConfig()
	fields := map[settingsTab][]editableField{
		tabProvider: {
			{key: "provider.default", value: "zen", fieldType: "string"},
			{key: "provider.auto_fallback", value: "true", fieldType: "bool"},
			{key: "provider.openrouter.api_key", value: "sk-or-test-key", fieldType: "string"},
			{key: "provider.zen.api_key", value: "sk-zen-test-key", fieldType: "string"},
		},
	}

	applyFieldsToConfig(cfg, fields)

	if cfg.Provider.Default != "zen" {
		t.Errorf("expected provider default 'zen', got %q", cfg.Provider.Default)
	}
	if !cfg.Provider.AutoFallback {
		t.Error("expected AutoFallback=true")
	}
	if cfg.Provider.OpenRouter.APIKey != "sk-or-test-key" {
		t.Errorf("expected OpenRouter API key, got %q", cfg.Provider.OpenRouter.APIKey)
	}
	if cfg.Provider.Zen.APIKey != "sk-zen-test-key" {
		t.Errorf("expected Zen API key, got %q", cfg.Provider.Zen.APIKey)
	}
}

func TestApplyFieldsToConfig_ModelTab(t *testing.T) {
	cfg := config.DefaultConfig()
	fields := map[settingsTab][]editableField{
		tabModel: {
			{key: "model.default", value: "claude-3.5-sonnet", fieldType: "string"},
			{key: "model.context_warning_threshold", value: "0.85", fieldType: "float"},
			{key: "model.show_thinking_by_default", value: "true", fieldType: "bool"},
			{key: "model.auto_collapse_tools", value: "true", fieldType: "bool"},
			{key: "model.auto_arbitrage", value: "false", fieldType: "bool"},
			{key: "model.arbitrage_threshold", value: "1.5", fieldType: "float"},
		},
	}

	applyFieldsToConfig(cfg, fields)

	if cfg.Model.Default != "claude-3.5-sonnet" {
		t.Errorf("expected model default 'claude-3.5-sonnet', got %q", cfg.Model.Default)
	}
	if cfg.Model.ContextWarningThreshold != 0.85 {
		t.Errorf("expected ContextWarningThreshold=0.85, got %f", cfg.Model.ContextWarningThreshold)
	}
	if !cfg.Model.ShowThinkingByDefault {
		t.Error("expected ShowThinkingByDefault=true")
	}
	if !cfg.Model.AutoCollapseTools {
		t.Error("expected AutoCollapseTools=true")
	}
	if cfg.Model.AutoArbitrage {
		t.Error("expected AutoArbitrage=false")
	}
	if cfg.Model.ArbitrageThreshold != 1.5 {
		t.Errorf("expected ArbitrageThreshold=1.5, got %f", cfg.Model.ArbitrageThreshold)
	}
}

func TestApplyFieldsToConfig_PermissionsTab(t *testing.T) {
	cfg := config.DefaultConfig()
	fields := map[settingsTab][]editableField{
		tabPermissions: {
			{key: "permissions.default_mode", value: "whitelist", fieldType: "string"},
			{key: "permissions.timeout_seconds", value: "60", fieldType: "int"},
		},
	}

	applyFieldsToConfig(cfg, fields)

	if cfg.Permissions.DefaultMode != "whitelist" {
		t.Errorf("expected DefaultMode 'whitelist', got %q", cfg.Permissions.DefaultMode)
	}
	if cfg.Permissions.TimeoutSeconds != 60 {
		t.Errorf("expected TimeoutSeconds=60, got %d", cfg.Permissions.TimeoutSeconds)
	}
}

func TestApplyFieldsToConfig_FeaturesTab(t *testing.T) {
	cfg := config.DefaultConfig()
	fields := map[settingsTab][]editableField{
		tabFeatures: {
			{key: "features.auto_backup", value: "true", fieldType: "bool"},
			{key: "features.resume_on_startup", value: "true", fieldType: "bool"},
		},
	}

	applyFieldsToConfig(cfg, fields)

	if !cfg.Features.AutoBackup {
		t.Error("expected AutoBackup=true")
	}
	if !cfg.Features.ResumeOnStartup {
		t.Error("expected ResumeOnStartup=true")
	}
}

func TestApplyFieldsToConfig_LedgerTab(t *testing.T) {
	cfg := config.DefaultConfig()
	fields := map[settingsTab][]editableField{
		tabLedger: {
			{key: "ledger.enabled", value: "false", fieldType: "bool"},
			{key: "ledger.max_entries", value: "200", fieldType: "int"},
		},
	}

	applyFieldsToConfig(cfg, fields)

	if cfg.Ledger.Enabled {
		t.Error("expected Ledger.Enabled=false")
	}
	if cfg.Ledger.MaxEntries != 200 {
		t.Errorf("expected MaxEntries=200, got %d", cfg.Ledger.MaxEntries)
	}
}

func TestApplyFieldsToConfig_InvalidIntFloat(t *testing.T) {
	cfg := config.DefaultConfig()
	origMaxIter := cfg.UI.MaxIterations
	origThreshold := cfg.Model.ArbitrageThreshold

	fields := map[settingsTab][]editableField{
		tabGeneral: {
			{key: "ui.max_iterations", value: "not-a-number", fieldType: "int"},
		},
		tabModel: {
			{key: "model.arbitrage_threshold", value: "not-a-number", fieldType: "float"},
			{key: "model.context_warning_threshold", value: "not-a-number", fieldType: "float"},
		},
	}

	applyFieldsToConfig(cfg, fields)

	// Invalid values should leave originals unchanged
	if cfg.UI.MaxIterations != origMaxIter {
		t.Errorf("invalid int should not change MaxIterations, expected %d, got %d", origMaxIter, cfg.UI.MaxIterations)
	}
	if cfg.Model.ArbitrageThreshold != origThreshold {
		t.Errorf("invalid float should not change ArbitrageThreshold, expected %f, got %f", origThreshold, cfg.Model.ArbitrageThreshold)
	}
}

func TestApplyFieldsToConfig_EmptySnapshots(t *testing.T) {
	cfg := config.DefaultConfig()
	fields := map[settingsTab][]editableField{}

	// Should not panic
	applyFieldsToConfig(cfg, fields)
}

func TestApplyFieldsToConfig_UnknownKey(t *testing.T) {
	cfg := config.DefaultConfig()
	fields := map[settingsTab][]editableField{
		tabGeneral: {
			{key: "unknown.key", value: "value", fieldType: "string"},
		},
	}

	// Should not panic, unknown keys are silently ignored
	applyFieldsToConfig(cfg, fields)
}

// ---------------------------------------------------------------------------
// Test SettingsModel.SetConfig
// ---------------------------------------------------------------------------

func TestSettingsModel_SetConfig(t *testing.T) {
	m := newTestSettingsModel()

	// Create new config with different values
	newCfg := config.DefaultConfig()
	newCfg.UI.Theme = "nord"
	newCfg.Model.Default = "gpt-4o-mini"

	m.SetConfig(newCfg)

	if m.config.UI.Theme != "nord" {
		t.Errorf("expected theme 'nord' after SetConfig, got %q", m.config.UI.Theme)
	}
	if m.config.Model.Default != "gpt-4o-mini" {
		t.Errorf("expected model 'gpt-4o-mini' after SetConfig, got %q", m.config.Model.Default)
	}

	// Verify fields were rebuilt
	if len(m.fields) != int(tabCount) {
		t.Errorf("expected %d tabs after SetConfig, got %d", tabCount, len(m.fields))
	}
}

// ---------------------------------------------------------------------------
// Test SettingsModel.SetTheme
// ---------------------------------------------------------------------------

func TestSettingsModel_SetTheme(t *testing.T) {
	m := newTestSettingsModel()

	// Verify initial theme
	if m.theme.Mode != theme.ModeDark {
		t.Errorf("expected initial theme ModeDark, got %d", m.theme.Mode)
	}

	// Set new theme (light theme)
	newTheme := theme.Light()
	m.SetTheme(newTheme)

	if m.theme.Mode != theme.ModeLight {
		t.Errorf("expected theme ModeLight after SetTheme, got %d", m.theme.Mode)
	}
}

// ---------------------------------------------------------------------------
// Test SettingsModel.Update with SettingsSavedMsg
// ---------------------------------------------------------------------------

func TestSettingsModel_UpdateSettingsSavedMsg(t *testing.T) {
	m := newTestSettingsModel()
	m.dirty = true
	m.statusMsg = "old status"

	m, cmd := m.Update(SettingsSavedMsg{})

	if cmd != nil {
		t.Error("SettingsSavedMsg should not return a cmd")
	}
	if m.dirty {
		t.Error("SettingsSavedMsg should set dirty=false")
	}
	if m.statusMsg != "Configuration saved successfully." {
		t.Errorf("expected status message, got %q", m.statusMsg)
	}
}

// ---------------------------------------------------------------------------
// Test SettingsModel.Update with ErrorMsg
// ---------------------------------------------------------------------------

func TestSettingsModel_UpdateErrorMsg(t *testing.T) {
	m := newTestSettingsModel()

	m, cmd := m.Update(ErrorMsg{Err: &testError{}})

	if cmd != nil {
		t.Error("ErrorMsg should not return a cmd")
	}
	if !strings.Contains(m.err, "Save error") {
		t.Errorf("expected error message, got %q", m.err)
	}
	if !strings.Contains(m.err, "test error") {
		t.Errorf("expected error details in message, got %q", m.err)
	}
}

// ---------------------------------------------------------------------------
// Test SettingsModel.Update with arrow keys
// ---------------------------------------------------------------------------

func TestSettingsModel_UpdateArrowKeys(t *testing.T) {
	m := newTestSettingsModel()
	m.activeTab = tabGeneral
	m.focusedField = 0

	// Down arrow should increase focusedField
	m, _ = m.Update(tea.KeyMsg{Type: tea.KeyDown})
	if m.focusedField != 1 {
		t.Errorf("expected focusedField=1 after down, got %d", m.focusedField)
	}

	// Up arrow should decrease focusedField
	m, _ = m.Update(tea.KeyMsg{Type: tea.KeyUp})
	if m.focusedField != 0 {
		t.Errorf("expected focusedField=0 after up, got %d", m.focusedField)
	}

	// Up at 0 should stay at 0
	m, _ = m.Update(tea.KeyMsg{Type: tea.KeyUp})
	if m.focusedField != 0 {
		t.Errorf("expected focusedField=0 after up at boundary, got %d", m.focusedField)
	}
}

// ---------------------------------------------------------------------------
// Test SettingsModel.Update with Ctrl+C
// ---------------------------------------------------------------------------

func TestSettingsModel_UpdateCtrlC(t *testing.T) {
	m := newTestSettingsModel()

	_, cmd := m.Update(tea.KeyMsg{Type: tea.KeyCtrlC})
	if cmd == nil {
		t.Fatal("Ctrl+C should return a cmd")
	}
	// The cmd should be tea.Quit
	msg := cmd()
	if _, ok := msg.(tea.QuitMsg); !ok {
		t.Errorf("expected tea.QuitMsg, got %T", msg)
	}
}

// ---------------------------------------------------------------------------
// Test SettingsModel.Update with right/left while editing
// ---------------------------------------------------------------------------

func TestSettingsModel_UpdateTabWhileEditing(t *testing.T) {
	m := newTestSettingsModel()
	m.activeTab = tabGeneral
	m.focusedField = 0
	m = m.startEdit()

	// Tab while editing should not change tab
	origTab := m.activeTab
	m, _ = m.Update(tea.KeyMsg{Type: tea.KeyTab})
	if m.activeTab != origTab {
		t.Errorf("tab should not change while editing, expected %d, got %d", origTab, m.activeTab)
	}
}

// ---------------------------------------------------------------------------
// Test SettingsModel.renderFooter
// ---------------------------------------------------------------------------

func TestSettingsModel_RenderFooter(t *testing.T) {
	m := newTestSettingsModel()
	m.width = 80

	// Footer should not be empty
	footer := m.renderFooter()
	if footer == "" {
		t.Error("renderFooter should not return empty string")
	}
}

// ---------------------------------------------------------------------------
// Test fmtBool
// ---------------------------------------------------------------------------

func TestFmtBool(t *testing.T) {
	tests := []struct {
		input    bool
		expected string
	}{
		{true, "true"},
		{false, "false"},
	}

	for _, tt := range tests {
		got := fmtBool(tt.input)
		if got != tt.expected {
			t.Errorf("fmtBool(%v) = %q, expected %q", tt.input, got, tt.expected)
		}
	}
}
