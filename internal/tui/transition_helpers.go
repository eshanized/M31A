package tui

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
)

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
