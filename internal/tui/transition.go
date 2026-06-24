package tui

import (
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

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

	// Capture current frame before switching
	prevFrame := m.View()

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

// RenderTransition composites the old and new frames with a visual effect.
func RenderTransition(prevFrame, nextFrame string, progress float64, tt TransitionType, width, height int, t theme.Theme) string {
	if progress >= 1.0 {
		return nextFrame
	}
	if progress <= 0.0 {
		return prevFrame
	}

	switch tt {
	case TransitionSlideLeft:
		return renderSlide(prevFrame, nextFrame, progress, width, height, true)
	case TransitionSlideRight:
		return renderSlide(prevFrame, nextFrame, progress, width, height, false)
	case TransitionFade:
		return renderFade(prevFrame, nextFrame, progress, width, height)
	default:
		return nextFrame
	}
}

// renderSlide performs a horizontal slide transition.
// If leftward is true, old content slides left and new enters from right.
func renderSlide(prev, next string, progress float64, width, height int, leftward bool) string {
	prevLines := padFrameLines(strings.Split(prev, "\n"), width, height)
	nextLines := padFrameLines(strings.Split(next, "\n"), width, height)

	offset := int(float64(width) * progress)
	if offset > width {
		offset = width
	}

	var result []string
	for y := 0; y < height; y++ {
		prevLine := ""
		if y < len(prevLines) {
			prevLine = prevLines[y]
		}
		nextLine := ""
		if y < len(nextLines) {
			nextLine = nextLines[y]
		}

		if leftward {
			// Old slides left: take tail of prev + head of next
			if offset >= width {
				result = append(result, nextLine)
			} else {
				tail := sliceVisibleTail(prevLine, width-offset)
				head := sliceVisibleHead(nextLine, offset)
				result = append(result, tail+head)
			}
		} else {
			// Old slides right: take tail of next + head of prev
			if offset >= width {
				result = append(result, nextLine)
			} else {
				head := sliceVisibleHead(prevLine, width-offset)
				tail := sliceVisibleTail(nextLine, offset)
				result = append(result, tail+head)
			}
		}
	}

	return strings.Join(result, "\n")
}

// renderFade performs a cross-fade using Unicode block density characters.
func renderFade(prev, next string, progress float64, width, height int) string {
	prevLines := padFrameLines(strings.Split(prev, "\n"), width, height)
	nextLines := padFrameLines(strings.Split(next, "\n"), width, height)

	// Density characters for blending
	densityChars := []rune{' ', '░', '▒', '▓', '█'}

	var result []string
	for y := 0; y < height; y++ {
		prevLine := prevLines[y]
		nextLine := nextLines[y]

		// At low progress, show more prev; at high progress, show more next
		if progress < 0.5 {
			// Blend prev with density characters
			density := int(progress * 2.0 * float64(len(densityChars)-1))
			if density >= len(densityChars) {
				density = len(densityChars) - 1
			}
			blendChar := densityChars[density]
			result = append(result, blendLine(prevLine, blendChar, width))
		} else {
			// Blend next emerging from density
			density := int((1.0 - progress) * 2.0 * float64(len(densityChars)-1))
			if density >= len(densityChars) {
				density = len(densityChars) - 1
			}
			blendChar := densityChars[density]
			result = append(result, blendLine(nextLine, blendChar, width))
		}
	}

	return strings.Join(result, "\n")
}

// blendLine replaces visible characters in a line with a blend character,
// preserving spacing structure.
func blendLine(line string, blendChar rune, width int) string {
	var b strings.Builder
	col := 0
	inEsc := false

	for _, r := range line {
		if inEsc {
			b.WriteRune(r)
			if r == 'm' {
				inEsc = false
			}
			continue
		}
		if r == '\x1b' {
			inEsc = true
			b.WriteRune(r)
			continue
		}
		if r == ' ' || r == '\t' {
			b.WriteRune(r)
		} else {
			b.WriteRune(blendChar)
		}
		col++
	}

	// Pad to width (only visible characters count toward width)
	for col < width {
		b.WriteRune(' ')
		col++
	}

	return b.String()
}

// padFrameLines ensures exactly `height` lines each of visible width `width`.
func padFrameLines(lines []string, width, height int) []string {
	result := make([]string, height)
	for i := 0; i < height; i++ {
		if i < len(lines) {
			line := lines[i]
			lw := lipgloss.Width(line)
			if lw < width {
				line += strings.Repeat(" ", width-lw)
			}
			result[i] = line
		} else {
			result[i] = strings.Repeat(" ", width)
		}
	}
	return result
}

// sliceVisibleTail returns the last `n` visible-width cells of a styled string.
func sliceVisibleTail(s string, n int) string {
	if n <= 0 {
		return ""
	}
	w := lipgloss.Width(s)
	if w <= n {
		if w < n {
			return strings.Repeat(" ", n-w) + s
		}
		return s
	}
	// Skip first (w-n) visible characters
	skip := w - n
	return skipVisible(s, skip)
}

// sliceVisibleHead returns the first `n` visible-width cells of a styled string.
func sliceVisibleHead(s string, n int) string {
	if n <= 0 {
		return ""
	}
	return transitionTruncate(s, n)
}

// transitionTruncate truncates a styled string to at most maxW visible cells,
// preserving ANSI escape sequences.
func transitionTruncate(s string, maxW int) string {
	var out strings.Builder
	visible := 0
	truncated := false
	inEsc := false
	esc := strings.Builder{}
	for _, r := range s {
		if inEsc {
			esc.WriteRune(r)
			if r == 'm' {
				out.WriteString(esc.String())
				esc.Reset()
				inEsc = false
			}
			continue
		}
		if r == '\x1b' {
			inEsc = true
			esc.WriteRune(r)
			continue
		}
		if visible >= maxW {
			truncated = true
			break
		}
		out.WriteRune(r)
		visible++
	}
	if truncated {
		out.WriteString("\x1b[0m")
	}
	return out.String()
}

// skipVisible skips `n` visible characters from a styled string, preserving ANSI codes.
// Escape sequences in the skipped region are discarded; only those at or after
// position n are forwarded to the output. The last active style from the skipped
// region is applied to the first output character to maintain visual continuity.
func skipVisible(s string, n int) string {
	var b strings.Builder
	skipped := 0
	inEsc := false
	pendingEsc := strings.Builder{}
	lastStyle := ""       // tracks the most recent complete SGR sequence
	styleEmitted := false // whether we've emitted the style for the output portion

	for _, r := range s {
		if inEsc {
			pendingEsc.WriteRune(r)
			if r == 'm' {
				lastStyle = pendingEsc.String()
				pendingEsc.Reset()
				inEsc = false
			}
			continue
		}
		if r == '\x1b' {
			inEsc = true
			pendingEsc.WriteRune(r)
			continue
		}
		if skipped < n {
			skipped++
			continue
		}
		// Emit the last active style before the first visible output character
		if !styleEmitted && lastStyle != "" {
			b.WriteString(lastStyle)
			styleEmitted = true
		}
		b.WriteRune(r)
	}

	return b.String()
}

// screenOrder maps screens to a navigation depth for forward/back detection.
// The workflow sequence is: Home → GoalInput → Discuss → Plan → Execute → Verify → RuntimeCheck → Ship
var screenOrder = map[Screen]int{
	ScreenHome:          -1,
	ScreenREPL:          0,
	ScreenDashboard:     1,
	ScreenGoalInput:     2,
	ScreenDiscuss:       3,
	ScreenPlan:          4,
	ScreenExecute:       5,
	ScreenVerify:        6,
	ScreenRuntimeCheck:  7,
	ScreenShip:          8,
	ScreenSettings:      10,
	ScreenModelSelector: 10,
	ScreenResume:        11,
	ScreenSessionDetail: 12,
	ScreenHelp:          13,
	ScreenLedger:        14,
	ScreenRollback:      15,
	ScreenMetrics:       16,
	ScreenConfig:        17,
	ScreenBisect:        18,
	ScreenThemePicker:   19,
	ScreenNotifications: 20,
	ScreenChatHistory:   21,
	ScreenFileExplorer:  22,
	ScreenToolDetail:    23,
	ScreenPhaseModelPicker: 24,
	ScreenGhostPicker:   25,
	ScreenGhostOutput:   26,
}

// isBackNavigation returns true if navigating from `from` to `to` is
// a backward navigation (escape/back action).
func isBackNavigation(from, to Screen) bool {
	if to == ScreenREPL && from != ScreenREPL {
		return true
	}
	fromOrder, fromOK := screenOrder[from]
	toOrder, toOK := screenOrder[to]
	if fromOK && toOK {
		return toOrder < fromOrder
	}
	return false
}
