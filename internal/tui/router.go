package tui

import (
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ScreenID identifies a screen for routing purposes.
// Using the existing Screen type from tuitypes for consistency.
type ScreenID = Screen

// Router manages screen registration, active-screen tracking, and View() delegation.
// Update() dispatch is handled by screenUpdaters in app_routing.go, not by the Router.
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

// SwitchTo changes the active screen and syncs its dimensions/theme.
// Init() is NOT called here — ensureSubModel already calls Init() during model creation.
func (r *Router) SwitchTo(id ScreenID) {
	if id == 0 {
		return
	}
	if s, ok := r.screens[id]; ok {
		r.activeID = id
		r.active = s
		if r.active != nil {
			r.active.SetDimensions(r.width, r.height)
			r.active.SetTheme(r.theme)
		}
	}
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

// ActiveID returns the currently active screen ID.
func (r *Router) ActiveID() ScreenID {
	return r.activeID
}
