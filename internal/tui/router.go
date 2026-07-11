package tui

import (
	tea "github.com/charmbracelet/bubbletea"

	"github.com/eshanized/M31A/internal/tui/theme"
)

// ScreenID identifies a screen for routing purposes.
// Using the existing Screen type from tuitypes for consistency.
type ScreenID = Screen

// Router manages screen transitions and delegates messages to the active screen.
type Router struct {
	screens  map[ScreenID]Screenable
	activeID ScreenID
	active   Screenable
	theme    theme.Theme
	width    int
	height   int
}

// NewRouter creates a new Router.
func NewRouter() *Router {
	return &Router{
		screens: make(map[ScreenID]Screenable),
	}
}

// Register adds a screen to the router.
func (r *Router) Register(id ScreenID, s Screenable) {
	if s == nil {
		return
	}
	r.screens[id] = s
	s.SetDimensions(r.width, r.height)
	s.SetTheme(r.theme)
}

// SwitchTo changes the active screen.
func (r *Router) SwitchTo(id ScreenID) tea.Cmd {
	if id == 0 {
		return nil
	}
	if s, ok := r.screens[id]; ok {
		r.activeID = id
		r.active = s
		// Initialize new screen with current dimensions and theme
		if r.active != nil {
			r.active.SetDimensions(r.width, r.height)
			r.active.SetTheme(r.theme)
		}
		return r.active.Init()
	}
	return nil
}

// Update delegates the message to the active screen.
func (r *Router) Update(msg tea.Msg) tea.Cmd {
	if r.active == nil {
		return nil
	}
	newActive, cmd := r.active.Update(msg)
	if newActive != nil {
		r.screens[r.activeID] = newActive
		r.active = newActive
	}
	return cmd
}

// View returns the view of the active screen.
func (r *Router) View() string {
	if r.active == nil {
		return ""
	}
	return r.active.View()
}

// SetTheme updates the theme for all registered screens.
func (r *Router) SetTheme(t theme.Theme) {
	r.theme = t
	for _, s := range r.screens {
		if s != nil {
			s.SetTheme(t)
		}
	}
}

// SetDimensions updates dimensions for all registered screens.
func (r *Router) SetDimensions(w, h int) {
	r.width = w
	r.height = h
	for _, s := range r.screens {
		if s != nil {
			s.SetDimensions(w, h)
		}
	}
}

// ActiveScreen returns the currently active screen.
func (r *Router) ActiveScreen() Screenable {
	return r.active
}

// ActiveID returns the currently active screen ID.
func (r *Router) ActiveID() ScreenID {
	return r.activeID
}