package tui

import "time"

// TransitionType controls the visual effect of a screen transition.
type TransitionType int

const (
	TransitionNone       TransitionType = iota
	TransitionSlideLeft                 // forward navigation: old slides left, new enters from right
	TransitionSlideRight                // backward navigation: old slides right, new enters from left
	TransitionFade                      // cross-fade using block density characters
)

// ScreenTransition tracks a visual transition between screens.
type ScreenTransition struct {
	Active     bool
	StartAt    time.Time
	Duration   time.Duration
	Type       TransitionType
	FromScreen Screen
	ToScreen   Screen
	PrevFrame  string // captured render of the old screen
}

const transitionDuration = 150 * time.Millisecond

// transitionFPS is the target frame rate for transition animations.
const transitionFPS = 60

// transitionTickInterval is the interval between animation frames.
const transitionTickInterval = time.Second / transitionFPS

// StartTransition begins a visual screen transition.
func (m *AppState) StartTransition(to Screen, _ string) {
	if m.prevScreen == to {
		m.screen = to
		return
	}

	// Determine transition type based on navigation direction
	tt := TransitionSlideLeft
	if isBackNavigation(m.screen, to) {
		tt = TransitionSlideRight
	}

	// Capture current frame by rendering the current screen directly,
	// avoiding the recursive m.View() call which would check the transition
	// state and potentially cause infinite recursion.
	prevFrame := m.renderFrame()

	m.transition = &ScreenTransition{
		Active:     true,
		StartAt:    time.Now(),
		Duration:   transitionDuration,
		Type:       tt,
		FromScreen: m.screen,
		ToScreen:   to,
		PrevFrame:  prevFrame,
	}
}

// TransitionTick advances the transition animation.
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

// Progress returns the transition progress from 0.0 (start) to 1.0 (complete).
func (t *ScreenTransition) Progress() float64 {
	if t == nil || !t.Active {
		return 1.0
	}
	elapsed := time.Since(t.StartAt)
	p := float64(elapsed) / float64(t.Duration)
	if p > 1.0 {
		return 1.0
	}
	if p < 0.0 {
		return 0.0
	}
	return p
}
