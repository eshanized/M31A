package tui

import (
	"context"
	"os"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/tests/testutil"
	"github.com/eshanized/M31A/internal/tui/theme"
)

func testSettingsModel(t *testing.T) *SettingsModel {
	t.Helper()
	cfg := config.DefaultConfig()
	th := theme.NewManager(theme.ModeDark)
	return NewSettingsModel(cfg, nil, th.Current(), "/tmp/test-settings.toml", "0.1.0", nil, context.Background())
}

// ═══ NewSettingsModel ═══

func TestNewSettingsModel(t *testing.T) {
	s := testSettingsModel(t)
	if s == nil {
		t.Fatal("nil")
	}
	if s.activeTab != TabProvider {
		t.Errorf("activeTab=%d, want TabProvider", s.activeTab)
	}
	if len(s.fields) != 2 {
		t.Errorf("fields=%d, want 2", len(s.fields))
	}
	if s.fieldCursor != 0 {
		t.Errorf("fieldCursor=%d, want 0", s.fieldCursor)
	}
}

func TestNewSettingsModelNilConfig(t *testing.T) {
	s := NewSettingsModel(nil, nil, testTheme(), "", "0.1.0", nil, context.Background())
	if s == nil {
		t.Fatal("nil")
	}
	// With nil config, s.config will be nil
	_ = s.config
}

// ═══ SetConfig / SetTheme ═══

func TestSettingsSetConfig(t *testing.T) {
	s := testSettingsModel(t)
	newCfg := config.DefaultConfig()
	newCfg.Model.Default = "claude-3"
	s.SetConfig(newCfg)
	if s.config.Model.Default != "claude-3" {
		t.Error("config not updated")
	}
}

func TestSettingsSetTheme2(t *testing.T) {
	s := testSettingsModel(t)
	s.SetTheme(testTheme())
}

// ═══ buildFields ═══

func TestBuildFieldsTabProvider(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabProvider
	s.buildFields()
	if len(s.fields) != 2 {
		t.Errorf("fields=%d, want 2", len(s.fields))
	}
	if s.fields[0].key != "provider" {
		t.Errorf("field[0].key=%s, want provider", s.fields[0].key)
	}
}

func TestBuildFieldsTabModel(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabModel
	s.buildFields()
	if len(s.fields) != 5 {
		t.Errorf("fields=%d, want 5", len(s.fields))
	}
}

func TestBuildFieldsTabUI(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabUI
	s.buildFields()
	if len(s.fields) != 6 {
		t.Errorf("fields=%d, want 6", len(s.fields))
	}
}

func TestBuildFieldsTabKeys(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabKeys
	s.buildFields()
	if len(s.fields) != 3 {
		t.Errorf("fields=%d, want 3", len(s.fields))
	}
}

func TestBuildFieldsTabWorkflow(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabWorkflow
	s.buildFields()
	if len(s.fields) != 4 {
		t.Errorf("fields=%d, want 4", len(s.fields))
	}
}

func TestBuildFieldsTabAbout(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabAbout
	s.buildFields()
	if len(s.fields) != 0 {
		t.Errorf("fields=%d, want 0", len(s.fields))
	}
}

func TestBuildFieldsCursorReset(t *testing.T) {
	s := testSettingsModel(t)
	s.fieldCursor = 10
	s.buildFields()
	if s.fieldCursor != 0 {
		t.Errorf("fieldCursor=%d, want 0 (reset)", s.fieldCursor)
	}
}

// ═══ getFieldValue ═══

func TestGetFieldValueProvider(t *testing.T) {
	s := testSettingsModel(t)
	v := s.getFieldValue(s.fields[0])
	_ = v
}

func TestGetFieldValueAutoFallback(t *testing.T) {
	s := testSettingsModel(t)
	s.config.Provider.AutoFallback = true
	v := s.getFieldValue(s.fields[1])
	if v != "yes" {
		t.Errorf("value=%s, want yes", v)
	}
}

func TestGetFieldValueModel(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabModel
	s.buildFields()
	s.config.Model.Default = "gpt-4"
	v := s.getFieldValue(s.fields[0])
	if v != "gpt-4" {
		t.Errorf("value=%s, want gpt-4", v)
	}
}

func TestGetFieldValueShowCost(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabUI
	s.buildFields()
	s.config.UI.ShowCostEstimate = true
	for _, f := range s.fields {
		if f.key == "show_cost" {
			v := s.getFieldValue(f)
			if v != "yes" {
				t.Errorf("value=%s, want yes", v)
			}
			return
		}
	}
	t.Skip("show_cost field not found")
}

func TestGetFieldValueContextLength(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabModel
	s.buildFields()
	s.config.Model.DefaultContextLength = 200000
	v := s.getFieldValue(s.fields[4])
	if v != "200000" {
		t.Errorf("value=%s, want 200000", v)
	}
}

// ═══ setFieldValue ═══

func TestSetFieldValueProvider(t *testing.T) {
	s := testSettingsModel(t)
	_, _ = s.setFieldValue(s.fields[0], "zen")
	if s.config.Provider.Default != "zen" {
		t.Errorf("provider=%s, want zen", s.config.Provider.Default)
	}
}

func TestSetFieldValueAutoFallback(t *testing.T) {
	s := testSettingsModel(t)
	_, _ = s.setFieldValue(s.fields[1], "yes")
	if !s.config.Provider.AutoFallback {
		t.Error("auto_fallback should be true")
	}
	_, _ = s.setFieldValue(s.fields[1], "no")
	if s.config.Provider.AutoFallback {
		t.Error("auto_fallback should be false")
	}
}

func TestSetFieldValueContextLength(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabModel
	s.buildFields()
	_, _ = s.setFieldValue(s.fields[4], "64000")
	if s.config.Model.DefaultContextLength != 64000 {
		t.Errorf("context_length=%d, want 64000", s.config.Model.DefaultContextLength)
	}
}

func TestSetFieldValueContextLengthInvalid(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabModel
	s.buildFields()
	s.config.Model.DefaultContextLength = 128000
	_, _ = s.setFieldValue(s.fields[4], "not_a_number")
	if s.config.Model.DefaultContextLength != 128000 {
		t.Error("context_length should be unchanged")
	}
}

func TestSetFieldValueShowCost(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabUI
	s.buildFields()
	// Find the show_cost field
	for _, f := range s.fields {
		if f.key == "show_cost" {
			cmd, _ := s.setFieldValue(f, "no")
			_ = cmd
			if s.config.UI.ShowCostEstimate {
				t.Error("show_cost should be false after setting to 'no'")
			}
			return
		}
	}
	t.Skip("show_cost field not found in UI tab")
}

func TestSetFieldValueMaxIterations(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabWorkflow
	s.buildFields()
	_, _ = s.setFieldValue(s.fields[2], "50")
	if s.config.UI.MaxIterations != 50 {
		t.Errorf("max_iterations=%d, want 50", s.config.UI.MaxIterations)
	}
}

// ═══ toggleBool ═══

func TestToggleBool(t *testing.T) {
	s := testSettingsModel(t)
	s.config.Provider.AutoFallback = false
	_, _ = s.toggleBool(s.fields[1])
	if !s.config.Provider.AutoFallback {
		t.Error("should toggle to true")
	}
	_, _ = s.toggleBool(s.fields[1])
	if s.config.Provider.AutoFallback {
		t.Error("should toggle to false")
	}
}

// ═══ cycleChoice ═══

func TestCycleChoice2(t *testing.T) {
	s := testSettingsModel(t)
	s.config.Provider.Default = "openrouter"
	_, _ = s.cycleChoice(1)
	if s.config.Provider.Default != "zen" {
		t.Errorf("provider=%s, want zen", s.config.Provider.Default)
	}
	_, _ = s.cycleChoice(1)
	if s.config.Provider.Default != "nvidia" {
		t.Errorf("provider=%s, want nvidia", s.config.Provider.Default)
	}
}

func TestCycleChoiceBackward(t *testing.T) {
	s := testSettingsModel(t)
	s.config.Provider.Default = "openrouter"
	_, _ = s.cycleChoice(-1)
	if s.config.Provider.Default != "nvidia" {
		t.Errorf("provider=%s, want nvidia", s.config.Provider.Default)
	}
}

// ═══ isCurrentFieldChoice ═══

func TestIsCurrentFieldChoice(t *testing.T) {
	s := testSettingsModel(t)
	s.fieldCursor = 0
	if !s.isCurrentFieldChoice() {
		t.Error("provider should be a choice")
	}
	s.activeTab = TabModel
	s.buildFields()
	s.fieldCursor = 0
	if s.isCurrentFieldChoice() {
		t.Error("model should not be a choice")
	}
}

// ═══ activateField ═══

func TestActivateFieldBool(t *testing.T) {
	s := testSettingsModel(t)
	s.config.Provider.AutoFallback = false
	s.fieldCursor = 1
	_, _ = s.activateField()
	if !s.config.Provider.AutoFallback {
		t.Error("should toggle to true")
	}
}

func TestActivateFieldChoice(t *testing.T) {
	s := testSettingsModel(t)
	s.config.Provider.Default = "openrouter"
	s.fieldCursor = 0
	_, _ = s.activateField()
	if s.config.Provider.Default != "zen" {
		t.Errorf("provider=%s, want zen", s.config.Provider.Default)
	}
}

func TestActivateFieldText(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabModel
	s.buildFields()
	s.fieldCursor = 0
	_, _ = s.activateField()
	if !s.editing {
		t.Error("should be editing")
	}
	if s.editField != "model" {
		t.Errorf("editField=%s, want model", s.editField)
	}
}

// ═══ updateEditing ═══

func TestUpdateEditingEsc(t *testing.T) {
	s := testSettingsModel(t)
	s.editing = true
	s.editField = "model"
	_, _ = s.updateEditing(tea.KeyMsg{Type: tea.KeyEsc})
	if s.editing {
		t.Error("should not be editing after esc")
	}
}

func TestUpdateEditingEnter(t *testing.T) {
	s := testSettingsModel(t)
	s.activeTab = TabModel
	s.buildFields()
	s.editing = true
	s.editField = "model"
	s.editValue.SetValue("claude-3")
	_, _ = s.updateEditing(tea.KeyMsg{Type: tea.KeyEnter})
	if s.editing {
		t.Error("should not be editing after enter")
	}
	if s.config.Model.Default != "claude-3" {
		t.Errorf("model=%s, want claude-3", s.config.Model.Default)
	}
}

// ═══ Init ═══

func TestSettingsInitNilRegistry(t *testing.T) {
	s := testSettingsModel(t)
	s.registry = nil
	cmd := s.Init()
	if cmd != nil {
		t.Error("Init with nil registry should return nil")
	}
}

// ═══ Update ═══

func TestSettingsUpdateWindowSize(t *testing.T) {
	s := testSettingsModel(t)
	result, _ := s.Update(tea.WindowSizeMsg{Width: 100, Height: 30})
	if result == nil {
		t.Error("should return non-nil")
	}
}

func TestSettingsUpdateTab(t *testing.T) {
	s := testSettingsModel(t)
	// Tab on a non-choice field does nothing (tab only cycles choice fields)
	result, _ := s.Update(tea.KeyMsg{Type: tea.KeyTab})
	_ = result
	if s.activeTab != TabProvider {
		t.Errorf("activeTab=%d, want TabProvider (no change)", s.activeTab)
	}
}

func TestSettingsUpdateTabChoice(t *testing.T) {
	s := testSettingsModel(t)
	// Tab on a choice field should cycle the choice
	s.fieldCursor = 0
	s.config.Provider.Default = "openrouter"
	result, _ := s.Update(tea.KeyMsg{Type: tea.KeyTab})
	_ = result
	if s.config.Provider.Default != "zen" {
		t.Errorf("provider=%s, want zen", s.config.Provider.Default)
	}
}

func TestSettingsUpdateShiftTab(t *testing.T) {
	s := testSettingsModel(t)
	// Shift+Tab on a non-choice field does nothing
	result, _ := s.Update(tea.KeyMsg{Type: tea.KeyShiftTab})
	_ = result
	if s.activeTab != TabProvider {
		t.Errorf("activeTab=%d, want TabProvider (no change)", s.activeTab)
	}
}

func TestSettingsUpdateShiftTabChoice(t *testing.T) {
	s := testSettingsModel(t)
	// Shift+Tab on a choice field should cycle backward
	s.fieldCursor = 0
	s.config.Provider.Default = "openrouter"
	result, _ := s.Update(tea.KeyMsg{Type: tea.KeyShiftTab})
	_ = result
	if s.config.Provider.Default != "nvidia" {
		t.Errorf("provider=%s, want nvidia", s.config.Provider.Default)
	}
}

func TestSettingsUpdateNextTab(t *testing.T) {
	s := testSettingsModel(t)
	result, _ := s.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{']'}})
	_ = result
	if s.activeTab != TabModel {
		t.Errorf("activeTab=%d, want TabModel", s.activeTab)
	}
}

func TestSettingsUpdatePrevTab(t *testing.T) {
	s := testSettingsModel(t)
	result, _ := s.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'['}})
	_ = result
	if s.activeTab != TabAbout {
		t.Errorf("activeTab=%d, want TabAbout", s.activeTab)
	}
}

func TestSettingsUpdateCursorDown(t *testing.T) {
	s := testSettingsModel(t)
	result, _ := s.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'j'}})
	_ = result
	if s.fieldCursor != 1 {
		t.Errorf("fieldCursor=%d, want 1", s.fieldCursor)
	}
}

func TestSettingsUpdateCursorUp(t *testing.T) {
	s := testSettingsModel(t)
	s.fieldCursor = 1
	result, _ := s.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'k'}})
	_ = result
	if s.fieldCursor != 0 {
		t.Errorf("fieldCursor=%d, want 0", s.fieldCursor)
	}
}

func TestSettingsUpdateCursorUpAtTop(t *testing.T) {
	s := testSettingsModel(t)
	s.fieldCursor = 0
	result, _ := s.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'k'}})
	_ = result
	if s.fieldCursor != 0 {
		t.Errorf("fieldCursor=%d, want 0", s.fieldCursor)
	}
}

func TestSettingsUpdateDirectTab(t *testing.T) {
	s := testSettingsModel(t)
	for i, key := range []rune{'1', '2', '3', '4', '5', '6'} {
		result, _ := s.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{key}})
		_ = result
		expected := SettingsTab(i)
		if s.activeTab != expected {
			t.Errorf("key=%c activeTab=%d, want %d", key, s.activeTab, expected)
		}
	}
}

func TestSettingsUpdateActivate(t *testing.T) {
	s := testSettingsModel(t)
	s.config.Provider.AutoFallback = false
	s.fieldCursor = 1 // auto_fallback is a bool field
	result, _ := s.Update(tea.KeyMsg{Type: tea.KeyEnter})
	_ = result
	if !s.config.Provider.AutoFallback {
		t.Error("should toggle to true")
	}
}

func TestSettingsUpdateActivateSpace(t *testing.T) {
	s := testSettingsModel(t)
	s.config.Provider.AutoFallback = true
	s.fieldCursor = 1
	result, _ := s.Update(tea.KeyMsg{Type: tea.KeyEnter})
	_ = result
	if s.config.Provider.AutoFallback {
		t.Error("should toggle to false")
	}
}

func TestSettingsUpdateQ(t *testing.T) {
	s := testSettingsModel(t)
	result, _ := s.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'q'}})
	_ = result
}

func TestSettingsUpdateSaveNoPath(t *testing.T) {
	s := testSettingsModel(t)
	s.configPath = ""
	result, _ := s.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'s'}})
	_ = result
}

func TestSettingsUpdateRefresh(t *testing.T) {
	s := testSettingsModel(t)
	s.registry = nil
	result, _ := s.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'r'}})
	_ = result
}

// ═══ View ═══

func TestSettingsView(t *testing.T) {
	s := testSettingsModel(t)
	s.width = 80
	s.height = 24
	r := s.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestSettingsViewAllTabs(t *testing.T) {
	s := testSettingsModel(t)
	s.width = 80
	s.height = 24
	for tab := TabProvider; tab <= TabAbout; tab++ {
		s.activeTab = tab
		s.buildFields()
		r := s.View()
		if r == "" {
			t.Errorf("View for tab %d should not be empty", tab)
		}
	}
}

func TestSettingsViewEditing(t *testing.T) {
	s := testSettingsModel(t)
	s.width = 80
	s.height = 24
	s.editing = true
	s.editField = "model"
	r := s.View()
	if r == "" {
		t.Error("View while editing should not be empty")
	}
}

func TestSettingsViewStatus(t *testing.T) {
	s := testSettingsModel(t)
	s.width = 80
	s.height = 24
	s.statusMsg = "Config saved"
	s.statusTime = time.Now()
	r := s.View()
	_ = r
}

// ═══ keySource ═══

func TestKeySource(t *testing.T) {
	testutil.LoadTestDotEnv(t)

	s := testSettingsModel(t)
	orKey := os.Getenv("OPENROUTER_API_KEY")
	if orKey == "" {
		orKey = "sk-123"
	}
	s.config.Provider.OpenRouter.APIKey = orKey
	v := s.keySource("apikey_or")
	if v != "config" {
		t.Errorf("keySource=%s, want config", v)
	}
}

func TestKeySourceEmpty(t *testing.T) {
	s := testSettingsModel(t)
	v := s.keySource("apikey_or")
	if v != "" {
		t.Errorf("keySource=%s, want empty", v)
	}
}

func TestKeySourceUnknown(t *testing.T) {
	s := testSettingsModel(t)
	v := s.keySource("unknown_key")
	if v != "" {
		t.Errorf("keySource=%s, want empty", v)
	}
}
