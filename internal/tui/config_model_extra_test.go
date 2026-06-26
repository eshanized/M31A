package tui

import (
	"os"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/testutil"
)

func testConfigModel(t *testing.T) *ConfigModel {
	t.Helper()
	cfg := config.DefaultConfig()
	return NewConfigModel(testTheme(), cfg, "", 80, 24, nil)
}

// ═══ NewConfigModel ═══

func TestNewConfigModel(t *testing.T) {
	cm := testConfigModel(t)
	if cm == nil {
		t.Fatal("nil")
	}
	if len(cm.sections) == 0 {
		t.Error("sections should not be empty")
	}
}

func TestNewConfigModelNilConfig(t *testing.T) {
	cm := NewConfigModel(testTheme(), nil, "", 80, 24, nil)
	if cm == nil {
		t.Fatal("nil")
	}
	// With nil config, it should use a default
	_ = cm.cfg
}

// ═══ Init ═══

func TestConfigModelInit(t *testing.T) {
	cm := testConfigModel(t)
	cmd := cm.Init()
	if cmd != nil {
		t.Error("Init should return nil")
	}
}

// ═══ buildSections ═══

func TestBuildSections(t *testing.T) {
	cm := testConfigModel(t)
	cm.buildSections()
	if len(cm.sections) < 5 {
		t.Errorf("sections=%d, want >= 5", len(cm.sections))
	}
}

// ═══ getFieldValue ═══

func TestConfigGetFieldValueProviderDefault(t *testing.T) {
	cm := testConfigModel(t)
	cm.cfg.Provider.Default = "zen"
	f := cfgField{key: "provider.default"}
	v := cm.getFieldValue(f)
	if v != "zen" {
		t.Errorf("value=%s, want zen", v)
	}
}

func TestConfigGetFieldValueAutoFallback(t *testing.T) {
	cm := testConfigModel(t)
	cm.cfg.Provider.AutoFallback = true
	f := cfgField{key: "provider.auto_fallback"}
	v := cm.getFieldValue(f)
	if v != "yes" {
		t.Errorf("value=%s, want yes", v)
	}
}

func TestConfigGetFieldValueModelDefault(t *testing.T) {
	cm := testConfigModel(t)
	cm.cfg.Model.Default = "gpt-4"
	f := cfgField{key: "model.default"}
	v := cm.getFieldValue(f)
	if v != "gpt-4" {
		t.Errorf("value=%s, want gpt-4", v)
	}
}

func TestConfigGetFieldValueTheme(t *testing.T) {
	cm := testConfigModel(t)
	cm.cfg.UI.Theme = "light"
	f := cfgField{key: "ui.theme"}
	v := cm.getFieldValue(f)
	if v != "light" {
		t.Errorf("value=%s, want light", v)
	}
}

func TestConfigGetFieldValueBoolTrue(t *testing.T) {
	cm := testConfigModel(t)
	cm.cfg.Model.ShowThinkingByDefault = true
	f := cfgField{key: "model.show_thinking_by_default"}
	v := cm.getFieldValue(f)
	if v != "yes" {
		t.Errorf("value=%s, want yes", v)
	}
}

func TestConfigGetFieldValueBoolFalse(t *testing.T) {
	cm := testConfigModel(t)
	cm.cfg.Model.ShowThinkingByDefault = false
	f := cfgField{key: "model.show_thinking_by_default"}
	v := cm.getFieldValue(f)
	if v != "no" {
		t.Errorf("value=%s, want no", v)
	}
}

func TestConfigGetFieldValueNumber(t *testing.T) {
	cm := testConfigModel(t)
	cm.cfg.Features.SessionIDLength = 12
	f := cfgField{key: "features.session_id_length"}
	v := cm.getFieldValue(f)
	if v != "12" {
		t.Errorf("value=%s, want 12", v)
	}
}

func TestConfigGetFieldValueFloat(t *testing.T) {
	cm := testConfigModel(t)
	cm.cfg.Model.ContextWarningThreshold = 0.9
	f := cfgField{key: "model.context_warning_threshold"}
	v := cm.getFieldValue(f)
	if v != "0.90" {
		t.Errorf("value=%s, want 0.90", v)
	}
}

func TestConfigGetFieldValueText(t *testing.T) {
	cm := testConfigModel(t)
	cm.cfg.Git.CommitPrefix = "feat"
	f := cfgField{key: "git.commit_prefix"}
	v := cm.getFieldValue(f)
	if v != "feat" {
		t.Errorf("value=%s, want feat", v)
	}
}

func TestConfigGetFieldValuePassword(t *testing.T) {
	testutil.LoadTestDotEnv(t)

	cm := testConfigModel(t)
	orKey := os.Getenv("OPENROUTER_API_KEY")
	if orKey == "" {
		orKey = "sk-123"
	}
	cm.cfg.Provider.OpenRouter.APIKey = orKey
	f := cfgField{key: "provider.openrouter.api_key", fieldType: cfgPassword}
	v := cm.getFieldValue(f)
	if v != orKey {
		t.Errorf("value=%s, want %s", v, orKey)
	}
}

// ═══ setFieldValue ═══

func TestConfigSetFieldValueProvider(t *testing.T) {
	cm := testConfigModel(t)
	f := cfgField{key: "provider.default", fieldType: cfgChoice}
	cm.setFieldValue(f, "zen")
	if cm.cfg.Provider.Default != "zen" {
		t.Errorf("provider=%s, want zen", cm.cfg.Provider.Default)
	}
}

func TestConfigSetFieldValueAutoFallback(t *testing.T) {
	cm := testConfigModel(t)
	f := cfgField{key: "provider.auto_fallback", fieldType: cfgBool}
	cm.setFieldValue(f, "yes")
	if !cm.cfg.Provider.AutoFallback {
		t.Error("auto_fallback should be true")
	}
	cm.setFieldValue(f, "no")
	if cm.cfg.Provider.AutoFallback {
		t.Error("auto_fallback should be false")
	}
}

func TestConfigSetFieldValueModel(t *testing.T) {
	cm := testConfigModel(t)
	f := cfgField{key: "model.default", fieldType: cfgText}
	cm.setFieldValue(f, "claude-3")
	if cm.cfg.Model.Default != "claude-3" {
		t.Errorf("model=%s, want claude-3", cm.cfg.Model.Default)
	}
}

func TestConfigSetFieldValueTheme(t *testing.T) {
	cm := testConfigModel(t)
	f := cfgField{key: "ui.theme", fieldType: cfgChoice}
	cm.setFieldValue(f, "auto")
	if cm.cfg.UI.Theme != "auto" {
		t.Errorf("theme=%s, want auto", cm.cfg.UI.Theme)
	}
}

func TestConfigSetFieldValueNumber(t *testing.T) {
	cm := testConfigModel(t)
	f := cfgField{key: "features.session_id_length", fieldType: cfgNumber}
	cm.setFieldValue(f, "16")
	if cm.cfg.Features.SessionIDLength != 16 {
		t.Errorf("session_id_length=%d, want 16", cm.cfg.Features.SessionIDLength)
	}
}

func TestConfigSetFieldValueNumberInvalid(t *testing.T) {
	cm := testConfigModel(t)
	cm.cfg.Features.SessionIDLength = 8
	f := cfgField{key: "features.session_id_length", fieldType: cfgNumber}
	cm.setFieldValue(f, "not_a_number")
	if cm.cfg.Features.SessionIDLength != 8 {
		t.Error("should be unchanged")
	}
}

func TestConfigSetFieldValueFloat(t *testing.T) {
	cm := testConfigModel(t)
	f := cfgField{key: "model.context_warning_threshold", fieldType: cfgFloat}
	cm.setFieldValue(f, "0.95")
	if cm.cfg.Model.ContextWarningThreshold != 0.95 {
		t.Errorf("threshold=%f, want 0.95", cm.cfg.Model.ContextWarningThreshold)
	}
}

func TestConfigSetFieldValueFloatInvalid(t *testing.T) {
	cm := testConfigModel(t)
	cm.cfg.Model.ContextWarningThreshold = 0.8
	f := cfgField{key: "model.context_warning_threshold", fieldType: cfgFloat}
	cm.setFieldValue(f, "abc")
	if cm.cfg.Model.ContextWarningThreshold != 0.8 {
		t.Error("should be unchanged")
	}
}

// ═══ activateField ═══

func TestConfigActivateFieldBool(t *testing.T) {
	cm := testConfigModel(t)
	cm.sectionIdx = 0
	cm.fieldIdx = 1 // auto_fallback
	cm.cfg.Provider.AutoFallback = false
	_, _ = cm.activateField()
	if !cm.cfg.Provider.AutoFallback {
		t.Error("should toggle to true")
	}
}

func TestConfigActivateFieldChoice(t *testing.T) {
	cm := testConfigModel(t)
	cm.sectionIdx = 0
	cm.fieldIdx = 0 // provider.default
	cm.cfg.Provider.Default = "zen"
	_, _ = cm.activateField()
	if cm.cfg.Provider.Default != "openrouter" {
		t.Errorf("provider=%s, want openrouter", cm.cfg.Provider.Default)
	}
}

func TestConfigActivateFieldText(t *testing.T) {
	cm := testConfigModel(t)
	cm.sectionIdx = 1 // Model section
	cm.fieldIdx = 0   // model.default
	_, _ = cm.activateField()
	if !cm.editing {
		t.Error("should be editing")
	}
}

// ═══ updateBrowsing ═══

func TestConfigUpdateBrowsingEsc(t *testing.T) {
	cm := testConfigModel(t)
	_, _ = cm.updateBrowsing(tea.KeyMsg{Type: tea.KeyEsc})
}

func TestConfigUpdateBrowsingDown(t *testing.T) {
	cm := testConfigModel(t)
	cm.sectionIdx = 0
	cm.fieldIdx = 0
	_, _ = cm.updateBrowsing(tea.KeyMsg{Type: tea.KeyDown})
	if cm.fieldIdx != 1 {
		t.Errorf("fieldIdx=%d, want 1", cm.fieldIdx)
	}
}

func TestConfigUpdateBrowsingUp(t *testing.T) {
	cm := testConfigModel(t)
	cm.sectionIdx = 0
	cm.fieldIdx = 1
	_, _ = cm.updateBrowsing(tea.KeyMsg{Type: tea.KeyUp})
	if cm.fieldIdx != 0 {
		t.Errorf("fieldIdx=%d, want 0", cm.fieldIdx)
	}
}

func TestConfigUpdateBrowsingUpAtTop(t *testing.T) {
	cm := testConfigModel(t)
	cm.fieldIdx = 0
	_, _ = cm.updateBrowsing(tea.KeyMsg{Type: tea.KeyUp})
	if cm.fieldIdx != 0 {
		t.Errorf("fieldIdx=%d, want 0", cm.fieldIdx)
	}
}

func TestConfigUpdateBrowsingTab(t *testing.T) {
	cm := testConfigModel(t)
	cm.sectionIdx = 0
	cm.fieldIdx = 0
	_, _ = cm.updateBrowsing(tea.KeyMsg{Type: tea.KeyTab})
}

func TestConfigUpdateBrowsingShiftTab(t *testing.T) {
	cm := testConfigModel(t)
	cm.sectionIdx = 0
	cm.fieldIdx = 0
	_, _ = cm.updateBrowsing(tea.KeyMsg{Type: tea.KeyShiftTab})
}

func TestConfigUpdateBrowsingEnter(t *testing.T) {
	cm := testConfigModel(t)
	cm.sectionIdx = 0
	cm.fieldIdx = 1 // auto_fallback (bool)
	cm.cfg.Provider.AutoFallback = false
	_, _ = cm.updateBrowsing(tea.KeyMsg{Type: tea.KeyEnter})
	if !cm.cfg.Provider.AutoFallback {
		t.Error("should toggle to true")
	}
}

func TestConfigUpdateBrowsingNextSection(t *testing.T) {
	cm := testConfigModel(t)
	cm.sectionIdx = 0
	_, _ = cm.updateBrowsing(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{']'}})
	if cm.sectionIdx != 1 {
		t.Errorf("sectionIdx=%d, want 1", cm.sectionIdx)
	}
}

func TestConfigUpdateBrowsingPrevSection(t *testing.T) {
	cm := testConfigModel(t)
	cm.sectionIdx = 1
	_, _ = cm.updateBrowsing(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'['}})
	if cm.sectionIdx != 0 {
		t.Errorf("sectionIdx=%d, want 0", cm.sectionIdx)
	}
}

// ═══ updateEditing ═══

func TestConfigUpdateEditingEsc(t *testing.T) {
	cm := testConfigModel(t)
	cm.editing = true
	_, _ = cm.updateEditing(tea.KeyMsg{Type: tea.KeyEsc})
	if cm.editing {
		t.Error("should not be editing after esc")
	}
}

func TestConfigUpdateEditingEnter(t *testing.T) {
	cm := testConfigModel(t)
	cm.editing = true
	cm.sectionIdx = 1
	cm.fieldIdx = 0
	cm.editInput.SetValue("gpt-4")
	_, _ = cm.updateEditing(tea.KeyMsg{Type: tea.KeyEnter})
	if cm.editing {
		t.Error("should not be editing after enter")
	}
	if cm.cfg.Model.Default != "gpt-4" {
		t.Errorf("model=%s, want gpt-4", cm.cfg.Model.Default)
	}
}

// ═══ View ═══

func TestConfigModelView(t *testing.T) {
	cm := testConfigModel(t)
	r := cm.View()
	if r == "" {
		t.Error("View should not be empty")
	}
}

func TestConfigModelViewEditing(t *testing.T) {
	cm := testConfigModel(t)
	cm.editing = true
	r := cm.View()
	if r == "" {
		t.Error("View while editing should not be empty")
	}
}

// ═══ scrollToField ═══

func TestScrollToField(t *testing.T) {
	cm := testConfigModel(t)
	cm.fieldIdx = 0
	cm.scrollToField()
}

// ═══ render helpers ═══

func TestRenderTabs(t *testing.T) {
	cm := testConfigModel(t)
	r := cm.renderTabs()
	if r == "" {
		t.Error("renderTabs should not be empty")
	}
}

func TestRenderFields(t *testing.T) {
	cm := testConfigModel(t)
	cm.sectionIdx = 0
	r := cm.renderFields()
	if r == "" {
		t.Error("renderFields should not be empty")
	}
}

func TestRenderHint(t *testing.T) {
	cm := testConfigModel(t)
	cm.sectionIdx = 0
	cm.fieldIdx = 0
	r := cm.renderHint()
	if r == "" {
		t.Error("renderHint should not be empty")
	}
}

func TestRenderStatus(t *testing.T) {
	cm := testConfigModel(t)
	r := cm.renderStatus()
	_ = r
}

func TestRenderStatusEmpty(t *testing.T) {
	cm := testConfigModel(t)
	cm.statusMsg = ""
	r := cm.renderStatus()
	_ = r
}

// ═══ Update ═══

func TestConfigModelUpdateWindowSize(t *testing.T) {
	cm := testConfigModel(t)
	result, _ := cm.Update(tea.WindowSizeMsg{Width: 100, Height: 30})
	if result == nil {
		t.Error("should return non-nil")
	}
}

func TestConfigModelUpdateDown(t *testing.T) {
	cm := testConfigModel(t)
	cm.sectionIdx = 0
	cm.fieldIdx = 0
	result, _ := cm.Update(tea.KeyMsg{Type: tea.KeyDown})
	_ = result
	if cm.fieldIdx != 1 {
		t.Errorf("fieldIdx=%d, want 1", cm.fieldIdx)
	}
}

func TestConfigModelUpdateUp(t *testing.T) {
	cm := testConfigModel(t)
	cm.sectionIdx = 0
	cm.fieldIdx = 1
	result, _ := cm.Update(tea.KeyMsg{Type: tea.KeyUp})
	_ = result
	if cm.fieldIdx != 0 {
		t.Errorf("fieldIdx=%d, want 0", cm.fieldIdx)
	}
}
