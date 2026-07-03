package tui

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tui/theme"
)

func testTheme() theme.Theme {
	return theme.Dark()
}

func testKeyMsg(key string) tea.KeyMsg {
	switch key {
	case "y":
		return tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'y'}}
	case "n":
		return tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'n'}}
	case "esc":
		return tea.KeyMsg{Type: tea.KeyEscape}
	case "enter":
		return tea.KeyMsg{Type: tea.KeyEnter}
	default:
		return tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune(key)}
	}
}

// testConfig is a minimal config.Config for tests.
type testConfig = config.Config

func newTestRegistry() *provider.Registry {
	return provider.NewRegistry()
}
