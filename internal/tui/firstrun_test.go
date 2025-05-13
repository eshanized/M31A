package tui

import (
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
)

func TestFirstRun_InitialState(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	if m.State() != FirstRunWelcome {
		t.Errorf("Expected FirstRunWelcome, got %d", m.State())
	}
}

func TestFirstRun_WelcomeToProviderSelect(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if m.State() != FirstRunProviderSelect {
		t.Errorf("Expected FirstRunProviderSelect, got %d", m.State())
	}
}

func TestFirstRun_WelcomeSpace(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	cmds, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{' '}})
	if m.State() != FirstRunProviderSelect {
		t.Errorf("Space should advance to ProviderSelect, got %d", m.State())
	}
	if appMsg != nil {
		t.Error("Should not emit AppMsg from welcome")
	}
	_ = cmds
}

func TestFirstRun_SelectOpenRouter(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunProviderSelect
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'1'}})
	if m.State() != FirstRunKeyInput {
		t.Errorf("Expected FirstRunKeyInput, got %d", m.State())
	}
	providers := m.SelectedProviders()
	if len(providers) != 1 || providers[0] != "openrouter" {
		t.Errorf("Expected [openrouter], got %v", providers)
	}
}

func TestFirstRun_SelectZen(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunProviderSelect
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'2'}})
	providers := m.SelectedProviders()
	if len(providers) != 1 || providers[0] != "zen" {
		t.Errorf("Expected [zen], got %v", providers)
	}
}

func TestFirstRun_SelectBoth(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunProviderSelect
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'3'}})
	providers := m.SelectedProviders()
	if len(providers) != 2 || providers[0] != "openrouter" || providers[1] != "zen" {
		t.Errorf("Expected [openrouter zen], got %v", providers)
	}
}

func TestFirstRun_SelectSkip(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunProviderSelect
	cmds, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'4'}})
	if m.State() != FirstRunComplete {
		t.Errorf("Expected FirstRunComplete, got %d", m.State())
	}
	if appMsg == nil || appMsg.Screen != ScreenREPL {
		t.Error("Skip should emit AppMsg with ScreenREPL")
	}
	_ = cmds
}

func TestFirstRun_SelectSkipWithS(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunProviderSelect
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'s'}})
	if m.State() != FirstRunComplete {
		t.Errorf("Expected FirstRunComplete, got %d", m.State())
	}
}

func TestFirstRun_KeyInputValidation(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunKeyInput
	m.providers = []string{"openrouter"}
	m.apiKeyInput.SetValue("sk-or-v1-1234567890abcdef")
	cmds, _ := m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if m.State() != FirstRunValidating {
		t.Errorf("Expected FirstRunValidating, got %d", m.State())
	}
	if len(cmds) == 0 {
		t.Error("Expected validation command")
	}
}

func TestFirstRun_KeyInputValidationShortKey(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunKeyInput
	m.apiKeyInput.SetValue("short")
	cmds, _ := m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if m.State() != FirstRunValidating {
		t.Errorf("Expected FirstRunValidating, got %d", m.State())
	}
	if len(cmds) == 0 {
		t.Fatal("Expected validation command")
	}

	cmd := cmds[0]
	msg := cmd()
	result, ok := msg.(validationResultMsg)
	if !ok {
		t.Fatal("Expected validationResultMsg")
	}
	if result.valid {
		t.Error("Short key should be invalid")
	}
	if result.err != "API key too short" {
		t.Errorf("Expected 'API key too short', got %q", result.err)
	}
}

func TestFirstRun_KeyInputValidationLongKey(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunKeyInput
	m.apiKeyInput.SetValue("sk-or-v1-1234567890abcdef")
	cmds, _ := m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	cmd := cmds[0]
	msg := cmd()
	result, ok := msg.(validationResultMsg)
	if !ok {
		t.Fatal("Expected validationResultMsg")
	}
	if !result.valid {
		t.Errorf("Long key should be valid, got error: %s", result.err)
	}
}

func TestFirstRun_ValidationSuccessToKeychainPrompt(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunValidating
	m.apiKeyValue = "sk-or-v1-1234567890abcdef"
	cmds, appMsg := m.Update(validationResultMsg{valid: true})
	if m.State() != FirstRunKeychainPrompt {
		t.Errorf("Expected FirstRunKeychainPrompt, got %d", m.State())
	}
	if appMsg != nil {
		t.Error("Should not emit AppMsg from validation success")
	}
	_ = cmds
}

func TestFirstRun_ValidationFailureToKeyInput(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunValidating
	cmds, appMsg := m.Update(validationResultMsg{valid: false, err: "bad key"})
	if m.State() != FirstRunKeyInput {
		t.Errorf("Expected FirstRunKeyInput, got %d", m.State())
	}
	if m.validationErr != "bad key" {
		t.Errorf("Expected validationErr='bad key', got %q", m.validationErr)
	}
	if appMsg != nil {
		t.Error("Should not emit AppMsg from validation failure")
	}
	_ = cmds
}

func TestFirstRun_KeychainPromptYes(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunKeychainPrompt
	m.apiKeyValue = "sk-or-v1-1234567890abcdef"
	cmds, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'y'}})
	if m.State() != FirstRunComplete {
		t.Errorf("Expected FirstRunComplete after yes, got %d", m.State())
	}
	if appMsg == nil || appMsg.Screen != ScreenREPL {
		t.Error("Should emit AppMsg with ScreenREPL")
	}
	_ = cmds
}

func TestFirstRun_KeychainPromptEnter(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunKeychainPrompt
	cmds, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if m.State() != FirstRunComplete {
		t.Errorf("Expected FirstRunComplete after Enter, got %d", m.State())
	}
	if appMsg == nil || appMsg.Screen != ScreenREPL {
		t.Error("Should emit AppMsg with ScreenREPL")
	}
	_ = cmds
}

func TestFirstRun_KeychainPromptNo(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunKeychainPrompt
	cmds, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'n'}})
	if m.State() != FirstRunComplete {
		t.Errorf("Expected FirstRunComplete after no, got %d", m.State())
	}
	if appMsg == nil || appMsg.Screen != ScreenREPL {
		t.Error("Should emit AppMsg with ScreenREPL")
	}
	_ = cmds
}

func TestFirstRun_ViewNotEmpty(t *testing.T) {
	states := []FirstRunState{
		FirstRunWelcome,
		FirstRunProviderSelect,
		FirstRunKeyInput,
		FirstRunValidating,
		FirstRunKeychainPrompt,
		FirstRunComplete,
	}
	for _, s := range states {
		m := NewFirstRunModel(theme.Dark(), "/tmp/test")
		m.state = s
		m.width = 80
		m.height = 24
		if s == FirstRunKeyInput {
			m.providers = []string{"openrouter"}
		}
		v := m.View()
		if v == "" {
			t.Errorf("View() should not be empty for state %d", s)
		}
	}
}

func TestFirstRun_EscGoesBack(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunKeyInput
	m.providers = []string{"openrouter"}
	m.apiKeyInput.SetValue("some-key")
	cmds, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyEsc})
	if m.State() != FirstRunProviderSelect {
		t.Errorf("Expected FirstRunProviderSelect after Esc, got %d", m.State())
	}
	if appMsg != nil {
		t.Error("Should not emit AppMsg from Esc")
	}
	_ = cmds
}

func TestFirstRun_EmptyKeyEnter(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunKeyInput
	m.providers = []string{"openrouter"}
	cmds, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if m.State() != FirstRunKeyInput {
		t.Errorf("Should stay in KeyInput with empty key, got %d", m.State())
	}
	if appMsg != nil {
		t.Error("Should not emit AppMsg")
	}
	_ = cmds
}

func TestFirstRun_CtrlC(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	cmds, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyCtrlC})
	if appMsg != nil {
		t.Error("Should not emit AppMsg on ctrl+c")
	}
	_ = cmds
}

func TestFirstRun_APIKeyGetter(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunKeyInput
	m.apiKeyInput.SetValue("sk-or-v1-secret")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if m.APIKey() != "sk-or-v1-secret" {
		t.Errorf("APIKey() = %q, want %q", m.APIKey(), "sk-or-v1-secret")
	}
}

func TestFirstRun_ProviderSelectWithEnter(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunProviderSelect
	m.cursor = 1
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if m.State() != FirstRunKeyInput {
		t.Errorf("Expected KeyInput after enter with cursor=1, got %d", m.State())
	}
	providers := m.SelectedProviders()
	if len(providers) != 1 || providers[0] != "zen" {
		t.Errorf("Expected [zen], got %v", providers)
	}
}

func TestFirstRun_ProviderSelectUpDown(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunProviderSelect
	m.providers = []string{"openrouter", "zen", "other1", "other2"}

	m.Update(tea.KeyMsg{Type: tea.KeyUp})
	if m.cursor != 3 {
		t.Errorf("Expected cursor=3 (wrap), got %d", m.cursor)
	}

	m.Update(tea.KeyMsg{Type: tea.KeyDown})
	if m.cursor != 0 {
		t.Errorf("Expected cursor=0, got %d", m.cursor)
	}
}

func TestFirstRun_WindowSize(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.Update(tea.WindowSizeMsg{Width: 120, Height: 40})
	if m.width != 120 || m.height != 40 {
		t.Errorf("Expected 120x40, got %dx%d", m.width, m.height)
	}
}

func TestFirstRun_CompleteStateEmitsOnce(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	m.state = FirstRunComplete
	cmds, appMsg := m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if appMsg == nil || appMsg.Screen != ScreenREPL {
		t.Error("Complete state should emit AppMsg with ScreenREPL")
	}
	_ = cmds
}

func TestFirstRun_SetTheme(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/test")
	light := theme.Light()
	m.SetTheme(light)
	if m.theme.Mode != light.Mode {
		t.Error("SetTheme did not update theme")
	}
}

func TestFirstRun_ConfigPath(t *testing.T) {
	m := NewFirstRunModel(theme.Dark(), "/tmp/.m31a/config.toml")
	if m.configPath != "/tmp/.m31a/config.toml" {
		t.Errorf("Expected /tmp/.m31a/config.toml, got %q", m.configPath)
	}
}
