package tui

import (
	"testing"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
)

func TestSettingsGetFieldValueAllKeys(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.Provider.Default = "zen"
	cfg.Provider.AutoFallback = true
	cfg.Model.Default = "test-model"
	cfg.Model.AutoCollapseTools = true
	cfg.Model.ShowThinkingByDefault = true
	cfg.Model.AutoArbitrage = true
	cfg.Model.DefaultContextLength = 99999
	cfg.UI.Theme = "light"
	cfg.UI.ShowCostEstimate = true
	cfg.UI.ShowTokenUsage = true
	cfg.UI.CompactMode = true
	cfg.UI.SidebarWidth = 42
	cfg.UI.LeaderKey = "ctrl+g"
	cfg.UI.MaxMessageHistory = 777
	cfg.UI.MaxIterations = 200
	cfg.Provider.OpenRouter.APIKey = "sk-or-xxx"
	cfg.Provider.Zen.APIKey = "sk-zen-xxx"
	cfg.Permissions.DefaultMode = "ask"
	cfg.Permissions.TimeoutSeconds = 45
	cfg.Features.AutoBackup = true

	s := &SettingsModel{config: cfg}

	allKeys := []struct {
		key  string
		want string
	}{
		{"provider", "zen"},
		{"auto_fallback", "yes"},
		{"model", "test-model"},
		{"auto_collapse", "yes"},
		{"show_thinking", "yes"},
		{"auto_arbitrage", "yes"},
		{"context_length", "99999"},
		{"theme", "light"},
		{"show_cost", "yes"},
		{"show_tokens", "yes"},
		{"compact_mode", "yes"},
		{"sidebar_width", "42"},
		{"leader_key", "ctrl+g"},
		{"max_history", "777"},
		{"apikey_or", "sk-or-xxx"},
		{"apikey_zen", "sk-zen-xxx"},
		{"perm_mode", "ask"},
		{"perm_timeout", "45"},
		{"max_iterations", "200"},
		{"auto_backup", "yes"},
	}
	for _, tt := range allKeys {
		f := settingsField{key: tt.key}
		got := s.getFieldValue(f)
		if got != tt.want {
			t.Errorf("getFieldValue(%q) = %q, want %q", tt.key, got, tt.want)
		}
	}
}

func TestSettingsGetFieldValueNilConfig(t *testing.T) {
	s := &SettingsModel{config: nil}
	f := settingsField{key: "provider"}
	if got := s.getFieldValue(f); got != "" {
		t.Errorf("nil config: want '', got %q", got)
	}
}

func TestSettingsSetFieldValueAllKeys(t *testing.T) {
	cfg := config.DefaultConfig()
	s := &SettingsModel{config: cfg}
	s.editValue = textinput.New()

	setTests := []struct {
		key   string
		val   string
		check func() bool
	}{
		{"provider", "zen", func() bool { return cfg.Provider.Default == "zen" }},
		{"auto_fallback", "yes", func() bool { return cfg.Provider.AutoFallback }},
		{"model", "gpt-5", func() bool { return cfg.Model.Default == "gpt-5" }},
		{"auto_collapse", "yes", func() bool { return cfg.Model.AutoCollapseTools }},
		{"show_thinking", "yes", func() bool { return cfg.Model.ShowThinkingByDefault }},
		{"auto_arbitrage", "yes", func() bool { return cfg.Model.AutoArbitrage }},
		{"context_length", "50000", func() bool { return cfg.Model.DefaultContextLength == 50000 }},
		{"theme", "dark", func() bool { return cfg.UI.Theme == "dark" }},
		{"show_cost", "yes", func() bool { return cfg.UI.ShowCostEstimate }},
		{"show_tokens", "yes", func() bool { return cfg.UI.ShowTokenUsage }},
		{"compact_mode", "yes", func() bool { return cfg.UI.CompactMode }},
		{"sidebar_width", "35", func() bool { return cfg.UI.SidebarWidth == 35 }},
		{"leader_key", "ctrl+z", func() bool { return cfg.UI.LeaderKey == "ctrl+z" }},
		{"max_history", "200", func() bool { return cfg.UI.MaxMessageHistory == 200 }},
		{"apikey_or", "sk-new", func() bool { return cfg.Provider.OpenRouter.APIKey == "sk-new" }},
		{"apikey_zen", "sk-zen-new", func() bool { return cfg.Provider.Zen.APIKey == "sk-zen-new" }},
		{"perm_mode", "auto", func() bool { return cfg.Permissions.DefaultMode == "auto" }},
		{"perm_timeout", "60", func() bool { return cfg.Permissions.TimeoutSeconds == 60 }},
		{"max_iterations", "100", func() bool { return cfg.UI.MaxIterations == 100 }},
		{"auto_backup", "no", func() bool { return !cfg.Features.AutoBackup }},
	}
	for _, tt := range setTests {
		s.setFieldValue(settingsField{key: tt.key, label: tt.key}, tt.val)
		if !tt.check() {
			t.Errorf("setFieldValue(%q, %q) failed", tt.key, tt.val)
		}
	}
}

func TestSettingsSetFieldValueNilConfig(t *testing.T) {
	s := &SettingsModel{config: nil}
	s.editValue = textinput.New()
	s.setFieldValue(settingsField{key: "provider"}, "zen")
	if s.config == nil {
		t.Error("should auto-create config")
	}
}

func TestSettingsSetFieldValueInvalidNumber(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.Model.DefaultContextLength = 128000
	s := &SettingsModel{config: cfg}
	s.editValue = textinput.New()
	s.setFieldValue(settingsField{key: "context_length"}, "not-a-number")
	if cfg.Model.DefaultContextLength != 128000 {
		t.Error("should not change on invalid number")
	}
}

func TestSettingsCycleChoiceWrapping(t *testing.T) {
	cfg := config.DefaultConfig()
	cfg.UI.Theme = "auto"
	s := &SettingsModel{config: cfg}
	s.fields = []settingsField{
		{key: "theme", fieldType: "choice", choices: []string{"dark", "light", "auto"}},
	}
	s.fieldCursor = 0

	s, _ = s.cycleChoice(1)
	if cfg.UI.Theme != "dark" {
		t.Errorf("want dark, got %s", cfg.UI.Theme)
	}
	s, _ = s.cycleChoice(1)
	if cfg.UI.Theme != "light" {
		t.Errorf("want light, got %s", cfg.UI.Theme)
	}
	s, _ = s.cycleChoice(1)
	if cfg.UI.Theme != "auto" {
		t.Errorf("want auto (wrap), got %s", cfg.UI.Theme)
	}
	s, _ = s.cycleChoice(-1)
	if cfg.UI.Theme != "light" {
		t.Errorf("want light (back), got %s", cfg.UI.Theme)
	}
}

func TestSettingsActivateFieldEmpty(t *testing.T) {
	s := &SettingsModel{config: config.DefaultConfig()}
	s, _ = s.activateField()
	if s.editing {
		t.Error("should not edit with empty fields")
	}
}

func TestSettingsActivateFieldOOB(t *testing.T) {
	s := &SettingsModel{config: config.DefaultConfig()}
	s.fields = []settingsField{{key: "x"}}
	s.fieldCursor = 5
	s, _ = s.activateField()
	if s.editing {
		t.Error("should not edit with OOB cursor")
	}
}

func TestSettingsUpdateEditingOtherKey(t *testing.T) {
	cfg := config.DefaultConfig()
	s := &SettingsModel{editing: true, editField: "model", config: cfg}
	s.editValue = textinput.New()
	msg := tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'a'}}
	s, _ = s.updateEditing(msg)
	if !s.editing {
		t.Error("should still be editing on regular key")
	}
}
