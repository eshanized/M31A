package tui

import (
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tui/theme"
)

func newTestSettingsModel() *SettingsModel {
	m := NewSettingsModel(config.DefaultConfig(), theme.Dark(), nil)
	m.width = 80
	m.height = 24
	return m
}

func TestSettings_InitialRender(t *testing.T) {
	m := newTestSettingsModel()
	v := m.View()
	if v == "" {
		t.Error("View() should not be empty")
	}
	if !strings.Contains(v, "General") {
		t.Errorf("View() should contain 'General' tab, got: %s", v)
	}
}

func TestSettings_TabCycling(t *testing.T) {
	m := newTestSettingsModel()

	// Verify initial state is tab 0
	if m.activeTab != 0 {
		t.Fatalf("Expected initial activeTab=0, got %d", m.activeTab)
	}

	// Cycle with Tab: 0→1
	m.Update(tea.KeyMsg{Type: tea.KeyTab})
	if m.activeTab != 1 {
		t.Fatalf("After 1st Tab, expected activeTab=1, got %d", m.activeTab)
	}

	// Cycle with Tab: 1→2
	m.Update(tea.KeyMsg{Type: tea.KeyTab})
	if m.activeTab != 2 {
		t.Fatalf("After 2nd Tab, expected activeTab=2, got %d", m.activeTab)
	}

	// Cycle with Tab: 2→3
	m.Update(tea.KeyMsg{Type: tea.KeyTab})
	if m.activeTab != 3 {
		t.Fatalf("After 3rd Tab, expected activeTab=3, got %d", m.activeTab)
	}

	// Cycle with Tab: 3→0 (wrap)
	m.Update(tea.KeyMsg{Type: tea.KeyTab})
	if m.activeTab != 0 {
		t.Fatalf("After 4th Tab (wrap), expected activeTab=0, got %d", m.activeTab)
	}

	// Test shift+tab reverses: 0→3
	m.Update(tea.KeyMsg{Type: tea.KeyShiftTab})
	if m.activeTab != 3 {
		t.Errorf("After shift+tab from 0, expected activeTab=3, got %d", m.activeTab)
	}

	// Test shift+tab again: 3→2
	m.Update(tea.KeyMsg{Type: tea.KeyShiftTab})
	if m.activeTab != 2 {
		t.Errorf("After second shift+tab from 3, expected activeTab=2, got %d", m.activeTab)
	}
}

func TestSettings_TabContent(t *testing.T) {
	m := newTestSettingsModel()

	// Tab labels and expected content keywords
	tabs := []struct {
		tab      int
		keywords []string
	}{
		{0, []string{"General", "Theme", "Compact", "Token", "Iterations"}},
		{1, []string{"Provider", "Default", "Fallback", "API Key", "Keychain"}},
		{2, []string{"Permissions", "Default Mode", "Timeout"}},
		{3, []string{"Features", "AutoDream", "Subagent", "Backup", "Resume"}},
	}

	for _, tt := range tabs {
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

func TestSettings_APIMasking(t *testing.T) {
	m := newTestSettingsModel()
	m.activeTab = 1

	// Set API key and verify it's masked
	m.config.Provider.OpenRouter.APIKey = "sk-or-v1-secret-key-value"
	v := m.View()

	if strings.Contains(v, "sk-or-v1-secret-key-value") {
		t.Error("API key should not appear in plaintext in View()")
	}
	if !strings.Contains(v, "••••••••") {
		t.Error("API key should be masked as •••••••• in View()")
	}
}

func TestSettings_SaveAction(t *testing.T) {
	m := newTestSettingsModel()
	cmds, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if appMsg == nil {
		t.Error("Enter should return an AppMsg")
	} else if appMsg.Screen != ScreenREPL {
		t.Errorf("Expected ScreenREPL after save, got %d", appMsg.Screen)
	}
	_ = cmds
}

func TestSettings_DiscardOnEsc(t *testing.T) {
	m := newTestSettingsModel()
	cmds, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if appMsg == nil {
		t.Error("Esc should return an AppMsg")
	} else if appMsg.Screen != ScreenREPL {
		t.Errorf("Expected ScreenREPL after discard, got %d", appMsg.Screen)
	}
	_ = cmds
}

func TestSettings_WindowSize(t *testing.T) {
	m := NewSettingsModel(config.DefaultConfig(), theme.Dark(), nil)
	if m.width != 0 || m.height != 0 {
		t.Error("Initial width/height should be 0")
	}

	m.Update(tea.WindowSizeMsg{Width: 120, Height: 40})
	if m.width != 120 {
		t.Errorf("Expected width=120, got %d", m.width)
	}
	if m.height != 40 {
		t.Errorf("Expected height=40, got %d", m.height)
	}
}
