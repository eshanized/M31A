package components

import (
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// Context carries layout and theme information to components.
// It replaces the pattern of passing width/height/theme as separate parameters.
type Context struct {
	Width   int
	Height  int
	Theme   theme.Theme
	Cache   *theme.StyleCache
	Focused bool
	Hovered bool
}

// NewContext creates a Context with the given dimensions and theme.
func NewContext(w, h int, t theme.Theme) Context {
	return Context{
		Width:  w,
		Height: h,
		Theme:  t,
		Cache:  theme.NewStyleCache(t),
	}
}

// Component is the standard interface for TUI components.
// Components receive a Context instead of raw parameters, enabling
// consistent access to theme, cache, focus, and hover state.
type Component interface {
	Init() tea.Cmd
	Update(ctx Context, msg tea.Msg) (Component, tea.Cmd)
	View(ctx Context) string
}

// StatelessComponent is a simplified interface for components that don't
// need Init/Update lifecycle — they only render.
type StatelessComponent interface {
	View(ctx Context) string
}

// RenderFunc adapts a plain function to StatelessComponent.
type RenderFunc func(ctx Context) string

func (f RenderFunc) View(ctx Context) string {
	return f(ctx)
}
