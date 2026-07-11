package tui

import (
	tea "github.com/charmbracelet/bubbletea"

	"github.com/eshanized/M31A/internal/tui/theme"
)

// Screenable is the interface that all TUI screens must implement.
// This enables a router-based architecture where screens are
// registered and delegated to, rather than using giant switch statements.
type Screenable interface {
	Init() tea.Cmd
	Update(msg tea.Msg) (Screenable, tea.Cmd)
	View() string
	SetDimensions(w, h int)
	SetTheme(theme.Theme)
}
