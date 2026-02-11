package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ScreenTransition captures a dim-and-reveal transition between screens.
type ScreenTransition struct {
	Active     bool
	StartAt    time.Time
	Duration   time.Duration
	FromScreen Screen
	ToScreen   Screen
	prevView   string // cached view of the from-screen at transition start
}

// transitionDuration is the duration of the screen dim effect.
const transitionDuration = 200 * time.Millisecond

// StartTransition begins a screen transition effect.
// It captures the current view as the fading-out frame.
func (m *AppState) StartTransition(to Screen, prevView string) {
	if m.prevScreen == to {
		// No transition needed for same screen
		m.screen = to
		return
	}
	m.transition = &ScreenTransition{
		Active:     true,
		StartAt:    time.Now(),
		Duration:   transitionDuration,
		FromScreen: m.screen,
		ToScreen:   to,
		prevView:   prevView,
	}
	// Don't switch m.screen yet; the View() will overlay the dim effect
	// on the current view. When the transition completes, we switch.
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

// Progress returns the transition progress as 0.0–1.0.
func (t *ScreenTransition) Progress() float64 {
	if t == nil || !t.Active || t.Duration <= 0 {
		return 1.0
	}
	elapsed := time.Since(t.StartAt)
	pct := float64(elapsed) / float64(t.Duration)
	if pct > 1.0 {
		return 1.0
	}
	return pct
}

// renderTransitionOverlay renders the dim overlay on top of the previous screen.
func (t *ScreenTransition) renderTransitionOverlay(th theme.Theme, width, height int) string {
	if t == nil || !t.Active {
		return ""
	}

	pct := t.Progress()
	// Inverse opacity: start fully opaque (dimmed), fade to transparent
	opacity := 1.0 - pct
	if opacity < 0 {
		opacity = 0
	}

	// Build a dim overlay using the theme's surface color as the base
	dimChar := "░"
	dimColor := lipgloss.Color(th.Surface)

	overlayStyle := lipgloss.NewStyle().
		Foreground(dimColor).
		Background(dimColor)

	overlayLines := height
	if overlayLines > 40 {
		overlayLines = 40
	}
	if overlayLines < 5 {
		overlayLines = 5
	}

	var overlay strings.Builder
	for i := 0; i < overlayLines; i++ {
		line := strings.Repeat(dimChar, width)
		overlay.WriteString(overlayStyle.Render(line))
		if i < overlayLines-1 {
			overlay.WriteString("\n")
		}
	}

	// Screen name centered in the overlay
	screenName := t.ToScreen.Label()
	label := fmt.Sprintf(" %s ", screenName)
	labelStyle := lipgloss.NewStyle().
		Foreground(lipgloss.Color("#ffffff")).
		Background(lipgloss.Color(dimColor)).
		Bold(true)

	centeredLabel := centerText(label, labelStyle, width)
	// Replace middle line with centered label
	lines := strings.Split(overlay.String(), "\n")
	midIdx := len(lines) / 2
	lines[midIdx] = centeredLabel

	return strings.Join(lines, "\n")
}

// centerText centers a styled text within a given width.
func centerText(text string, style lipgloss.Style, width int) string {
	rendered := style.Render(text)
	textWidth := lipgloss.Width(rendered)
	padding := width - textWidth
	if padding <= 0 {
		return rendered
	}
	leftPad := padding / 2
	rightPad := padding - leftPad
	return strings.Repeat(" ", leftPad) + rendered + strings.Repeat(" ", rightPad)
}
