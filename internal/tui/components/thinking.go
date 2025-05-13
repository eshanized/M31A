package components

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/tui/theme"
)

type ThinkingBlock struct {
	id        int
	segment   types.MessageSegment
	theme     theme.Theme
	expanded  bool
	focused   bool
	startedAt time.Time
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
	var d time.Duration
	if b.segment.DurationMs > 0 {
		d = time.Duration(b.segment.DurationMs) * time.Millisecond
	} else {
		d = time.Since(b.startedAt)
	}

	totalSecs := d.Seconds()
	if totalSecs < 10 {
		return fmt.Sprintf("%.1fs", totalSecs)
	}
	if totalSecs < 60 {
		return fmt.Sprintf("%.1fs", totalSecs)
	}
	mins := int(totalSecs) / 60
	secs := int(totalSecs) % 60
	return fmt.Sprintf("%dm %ds", mins, secs)
}

func (b *ThinkingBlock) Header(width int) string {
	toggle := "+"
	if b.expanded {
		toggle = "−"
	}

	toggleStyle := lipgloss.NewStyle().Foreground(b.theme.Thinking)
	if b.focused {
		toggleStyle = toggleStyle.Bold(true)
	}

	label := fmt.Sprintf("[%s] Thinking (%s)", toggle, b.Duration())

	if lipgloss.Width(label) > width-4 {
		maxWidth := width - 7
		if maxWidth < 10 {
			maxWidth = 10
		}
		label = label[:maxWidth] + "..."
	}

	beforeDur := fmt.Sprintf("[%s] Thinking (", toggle)
	afterDur := ")"
	durStr := b.Duration()

	return lipgloss.JoinHorizontal(lipgloss.Top,
		toggleStyle.Render(beforeDur),
		lipgloss.NewStyle().Foreground(b.theme.TextMuted).Render(durStr),
		toggleStyle.Render(afterDur),
	)
}
