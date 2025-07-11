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
	id        int
	segment   types.MessageSegment
	theme     theme.Theme
	expanded  bool
	focused   bool
	startedAt time.Time
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

func (b *ThinkingBlock) Render(width int) string {
	contentWidth := width - 4 // account for paddingLeft=2 paddingRight=2

	header := b.Header(contentWidth)

	if !b.expanded {
		// Collapsed: single-line with left border, muted text
		style := lipgloss.NewStyle().
			Border(theme.SplitBorder, true, false, false, false).
			BorderForeground(b.theme.BorderSubtle).
			Background(b.theme.BackgroundPanel).
			Padding(0, 2).
			Foreground(b.theme.TextMuted).
			Width(width)
		return style.Render(header)
	}

	// Expanded: left-bordered block with content
	content := lipgloss.NewStyle().
		Foreground(b.theme.TextMuted).
		Italic(true).
		Width(contentWidth).
		Padding(0, 1).
		Render(b.segment.Content)

	separator := lipgloss.NewStyle().
		Foreground(b.theme.Border).
		Render(strings.Repeat("─", contentWidth))

	blockContent := lipgloss.JoinVertical(lipgloss.Top, header, separator, content)

	borderColor := b.theme.BorderSubtle
	if b.focused {
		borderColor = b.theme.Thinking
	}

	style := lipgloss.NewStyle().
		Border(theme.SplitBorder, true, false, false, false).
		BorderForeground(borderColor).
		Background(b.theme.BackgroundPanel).
		Padding(0, 2).
		MarginTop(1).
		Width(width)

	return style.Render(blockContent)
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

func (b *ThinkingBlock) Header(width int) string {
	toggle := "+"
	if b.expanded {
		toggle = "−"
	}

	toggleStyle := lipgloss.NewStyle().Foreground(b.theme.Brand)
	if b.focused {
		toggleStyle = toggleStyle.Bold(true)
	}

	hint := " [T] to expand"
	if b.expanded {
		hint = " [T] to collapse"
	}
	label := fmt.Sprintf("[%s] Thinking (%s)%s", toggle, b.Duration(), hint)

	if lipgloss.Width(label) > width-4 {
		maxWidth := width - 7
		if maxWidth < 10 {
			maxWidth = 10
		}
		label = label[:maxWidth] + "..."
	}

	beforeDur := fmt.Sprintf("[%s] Thinking (", toggle)
	hintSuffix := fmt.Sprintf(")%s", hint)
	durStr := b.Duration()

	return lipgloss.JoinHorizontal(lipgloss.Top,
		toggleStyle.Render(beforeDur),
		lipgloss.NewStyle().Foreground(b.theme.TextMuted).Render(durStr),
		toggleStyle.Render(hintSuffix),
	)
}
