package tui

import (
	"time"
)

// ScreenTransition tracks a brief timing gap between screen switches.
// It is used only for timing — no visual overlay is rendered.
type ScreenTransition struct {
	Active     bool
	StartAt    time.Time
	Duration   time.Duration
	FromScreen Screen
	ToScreen   Screen
}

// transitionDuration is the duration of the screen switch delay.
const transitionDuration = 200 * time.Millisecond

// StartTransition begins a screen transition.
func (m *AppState) StartTransition(to Screen, _ string) {
	if m.prevScreen == to {
		m.screen = to
		return
	}
	m.transition = &ScreenTransition{
		Active:     true,
		StartAt:    time.Now(),
		Duration:   transitionDuration,
		FromScreen: m.screen,
		ToScreen:   to,
	}
}

// TransitionTick advances a running transition.
// Returns true when the transition is complete.
func (t *ScreenTransition) TransitionTick() bool {
	if t == nil || !t.Active {
		return true
	}
	elapsed := time.Since(t.StartAt)
	if elapsed >= t.Duration {
		t.Active = false
		return true
	}
	return false
}
