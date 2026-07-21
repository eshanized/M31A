package components

import (
	"time"

	"github.com/charmbracelet/lipgloss"
)

// Spinner provides core animation frame cycling for the opencode-style
// spinner character set at a configurable tick rate.
//
// It is NOT a Bubble Tea model — it is a lightweight utility that stores
// frame state. The parent model calls Next() during Update (on each tick)
// and reads the current frame during View. This design avoids embedding
// a full tea.Model inside every consuming component.
type Spinner struct {
	frames []string
	frame  int
	tick   time.Duration
}

// OpenCodeFrames is the 10-frame braille spinner set used across M31A.
var OpenCodeFrames = []string{"⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"}

// ThinkingFrames is the spinner set used for AI thinking state.
// Identical to OpenCodeFrames — exposed with a semantic name for clarity.
var ThinkingFrames = OpenCodeFrames

// SpinnerSets provides multiple spinner frame sets for variety
var SpinnerSets = map[string][]string{
	"braille":  {"⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"},
	"dots":     {"⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"},
	"arc":      {"◜", "◠", "◝", "◞", "◡", "◟"},
	"bouncing": {"⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"},
	"line":     {"|", "/", "-", "\\"},
	"grow":     {"▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"},
	"pulse":    {"◐", "◓", "◑", "◒"},
	// "orbit" is a smooth braille orbital — good for streaming state
	"orbit": {"⠄", "⠆", "⠇", "⠋", "⠙", "⠸", "⠰", "⠠", "⠀"},
}

// GetSpinnerFrames returns frames for the given style, defaulting to braille
func GetSpinnerFrames(style string) []string {
	if frames, ok := SpinnerSets[style]; ok {
		return frames
	}
	return SpinnerSets["braille"]
}

// NewSpinner creates a Spinner with the opencode frame set at 100ms (10fps).
func NewSpinner() Spinner {
	return Spinner{
		frames: OpenCodeFrames,
		frame:  0,
		tick:   100 * time.Millisecond,
	}
}

// NewSpinnerWithFrames creates a Spinner with a custom frame set and tick.
func NewSpinnerWithFrames(frames []string, tick time.Duration) Spinner {
	return Spinner{
		frames: frames,
		frame:  0,
		tick:   tick,
	}
}

// Next returns the current frame and advances to the next one.
// Call this from Update() on each tick — never from View().
func (s *Spinner) Next() string {
	f := s.frames[s.frame]
	s.frame = (s.frame + 1) % len(s.frames)
	return f
}

// Peek returns the current frame without advancing.
// Safe to call from View() — does not mutate state.
func (s *Spinner) Peek() string {
	return s.frames[s.frame]
}

// Tick returns the spinner tick interval.
func (s *Spinner) Tick() time.Duration {
	return s.tick
}

// Reset resets the spinner to frame 0.
func (s *Spinner) Reset() {
	s.frame = 0
}

// SpinnerTickInterval is the default spinner tick interval (10fps).
const SpinnerTickInterval = 100 * time.Millisecond

// RenderSpinner renders the current spinner frame using the given style.
func RenderSpinner(frame string, style lipgloss.Style) string {
	if frame == "" {
		return ""
	}
	return style.Render(frame)
}
