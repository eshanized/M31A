package components

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

type ThinkingBlock struct {
	id           int
	segment      types.MessageSegment
	theme        theme.Theme
	expanded     bool
	focused      bool
	startedAt    time.Time
	scrollOffset int
	// Cached duration to avoid recalculating every render
	lastDurationStr string
	lastDurationAt  time.Time
}

func NewThinkingBlock(segment types.MessageSegment, t theme.Theme, expanded bool, id int) *ThinkingBlock {
	startedAt := segment.StartedAt
	if startedAt.IsZero() {
		startedAt = time.Now()
	}
	return &ThinkingBlock{
		id:        id,
		segment:   segment,
		theme:     t,
		expanded:  expanded,
		focused:   false,
		startedAt: startedAt,
	}
}

// Render renders the thinking block as a compact inline element:
//
// Collapsed:  ▸ Thinking · 1.2s
// Expanded:   full panel with scrollable content
func (b *ThinkingBlock) Render(width int) string {
	contentWidth := width - 4 // account for padding

	if !b.expanded {
		// Collapsed: single compact line, no border
		durStr := b.Duration()
		label := lipgloss.NewStyle().
			Foreground(b.theme.Thinking).
			Render("▸ ") +
			lipgloss.NewStyle().
				Foreground(b.theme.TextMuted).
				Render("Thinking · "+durStr)

		return lipgloss.NewStyle().
			PaddingLeft(2).
			Width(width).
			Render(label)
	}

	// Expanded: panel with header, body content, and footer

	// Header line: toggle + "Thinking" + duration
	headerText := b.Header(contentWidth)
	header := lipgloss.NewStyle().
		Border(theme.ThinBorder).
		BorderForeground(b.theme.Thinking).
		BorderTop(true).BorderBottom(false).BorderLeft(true).BorderRight(true).
		Padding(0, 1).
		Width(contentWidth + 2).
		Render(headerText)

	// Body: italic thinking content with scroll support
	maxContentLines := 20
	lines := strings.Split(b.segment.Content, "\n")
	totalLines := len(lines)

	startLine := b.scrollOffset
	if startLine > totalLines-maxContentLines {
		startLine = totalLines - maxContentLines
	}
	if startLine < 0 {
		startLine = 0
	}
	endLine := startLine + maxContentLines
	if endLine > totalLines {
		endLine = totalLines
	}

	visibleContent := strings.Join(lines[startLine:endLine], "\n")

	bodyContent := lipgloss.NewStyle().
		Foreground(b.theme.Thinking).
		Italic(true).
		Padding(0, 2).
		Width(contentWidth).
		Render(visibleContent)

	// Scroll indicators
	if totalLines > maxContentLines {
		var scrollParts []string
		if startLine > 0 {
			scrollParts = append(scrollParts, fmt.Sprintf("↑ %d lines above", startLine))
		}
		if endLine < totalLines {
			scrollParts = append(scrollParts, fmt.Sprintf("↓ %d lines below", totalLines-endLine))
		}
		if len(scrollParts) > 0 {
			scrollInfo := lipgloss.NewStyle().
				Foreground(b.theme.TextSecondary).
				Padding(0, 2).
				Render(strings.Join(scrollParts, " · "))
			bodyContent += "\n" + scrollInfo
		}
	}

	// Body lines: each line padded
	bodyLines := strings.Split(bodyContent, "\n")
	bodyPadded := make([]string, len(bodyLines))
	for i, line := range bodyLines {
		bodyPadded[i] = lipgloss.NewStyle().
			Padding(0, 1).
			Width(contentWidth + 2).
			Render(line)
	}
	body := strings.Join(bodyPadded, "\n")

	// Footer: duration
	durStr := b.Duration()
	footer := lipgloss.NewStyle().
		Border(theme.ThinBorder).
		BorderForeground(b.theme.Thinking).
		BorderTop(false).BorderBottom(true).BorderLeft(true).BorderRight(true).
		Padding(0, 1).
		Width(contentWidth + 2).
		Render(lipgloss.NewStyle().Foreground(b.theme.TextMuted).Render(durStr))

	return header + "\n" + body + "\n" + footer
}

func (b *ThinkingBlock) Toggle() {
	b.expanded = !b.expanded
}

func (b *ThinkingBlock) SetFocused(f bool) {
	b.focused = f
}

func (b *ThinkingBlock) IsFocused() bool {
	return b.focused
}

func (b *ThinkingBlock) IsExpanded() bool {
	return b.expanded
}

func (b *ThinkingBlock) ID() int {
	return b.id
}

// ScrollUp scrolls the expanded thinking content up by n lines.
func (b *ThinkingBlock) ScrollUp(n int) {
	b.scrollOffset -= n
	if b.scrollOffset < 0 {
		b.scrollOffset = 0
	}
}

// ScrollDown scrolls the expanded thinking content down by n lines.
func (b *ThinkingBlock) ScrollDown(n int) {
	b.scrollOffset += n
	maxContentLines := 20
	lines := strings.Split(b.segment.Content, "\n")
	maxOffset := len(lines) - maxContentLines
	if maxOffset < 0 {
		maxOffset = 0
	}
	if b.scrollOffset > maxOffset {
		b.scrollOffset = maxOffset
	}
}

// ScrollOffset returns the current scroll offset.
func (b *ThinkingBlock) ScrollOffset() int {
	return b.scrollOffset
}

func (b *ThinkingBlock) Duration() string {
	now := time.Now()
	// Return cached value if still valid (within 1 second)
	if b.lastDurationStr != "" && now.Sub(b.lastDurationAt) < time.Second {
		return b.lastDurationStr
	}

	var d time.Duration
	if b.segment.DurationMs > 0 {
		d = time.Duration(b.segment.DurationMs) * time.Millisecond
	} else {
		d = time.Since(b.startedAt)
	}

	totalSecs := d.Seconds()
	var result string
	if totalSecs < 10 {
		result = fmt.Sprintf("%.1fs", totalSecs)
	} else if totalSecs < 60 {
		result = fmt.Sprintf("%.1fs", totalSecs)
	} else {
		mins := int(totalSecs) / 60
		secs := int(totalSecs) % 60
		result = fmt.Sprintf("%dm %ds", mins, secs)
	}

	b.lastDurationStr = result
	b.lastDurationAt = now
	return result
}

// Header returns the toggle + label + duration text for the thinking block header.
func (b *ThinkingBlock) Header(width int) string {
	toggle := "▸"
	hint := ""
	if b.expanded {
		toggle = "▾"
		if b.scrollOffset > 0 {
			hint = " ↑↓"
		}
	}

	durStr := b.Duration()

	toggleStyle := lipgloss.NewStyle().Foreground(b.theme.Thinking)
	if b.focused {
		toggleStyle = toggleStyle.Foreground(b.theme.Brand).Bold(true)
	}

	return toggleStyle.Render(toggle) +
		lipgloss.NewStyle().Foreground(b.theme.TextMuted).Render(fmt.Sprintf(" Thinking · %s", durStr)) +
		lipgloss.NewStyle().Foreground(b.theme.BorderSubtle).Render(hint)
}
