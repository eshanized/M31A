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
	segment   types.MessageSegment
	theme     theme.Theme
	expanded  bool
	startedAt time.Time
}

func NewThinkingBlock(segment types.MessageSegment, t theme.Theme, expanded bool) *ThinkingBlock {
	return &ThinkingBlock{
		segment:   segment,
		theme:     t,
		expanded:  expanded,
		startedAt: time.Now(),
	}
}

func (b *ThinkingBlock) Render(width int) string {
	header := b.Header(width)
	if !b.expanded {
		return b.theme.ThinkingBlock.
			Width(width).
			Render(header)
	}

	contentWidth := width - 6
	content := lipgloss.NewStyle().
		Foreground(b.theme.TextSecondary).
		Width(contentWidth).
		Padding(0, 1).
		Render(b.segment.Content)

	separator := lipgloss.NewStyle().
		Foreground(b.theme.Border).
		Render(strings.Repeat("─", contentWidth))

	return b.theme.ThinkingBlock.
		Width(width).
		Render(lipgloss.JoinVertical(lipgloss.Top,
			header,
			separator,
			content,
		))
}

func (b *ThinkingBlock) Toggle() {
	b.expanded = !b.expanded
}

func (b *ThinkingBlock) IsExpanded() bool {
	return b.expanded
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
	toggle := "▼"
	if b.expanded {
		toggle = "▲"
	}

	label := fmt.Sprintf("[%s] Thinking (%s)", toggle, b.Duration())

	if lipgloss.Width(label) > width-4 {
		maxWidth := width - 7
		if maxWidth < 10 {
			maxWidth = 10
		}
		label = label[:maxWidth] + "..."
	}

	toggleStyle := lipgloss.NewStyle().
		Foreground(b.theme.Thinking)
	durationStyle := lipgloss.NewStyle().
		Foreground(b.theme.TextSecondary)

	// Split label into parts around the duration for styling
	beforeDur := fmt.Sprintf("[%s] Thinking (", toggle)
	afterDur := ")"
	durStr := b.Duration()

	return lipgloss.JoinHorizontal(lipgloss.Top,
		toggleStyle.Render(beforeDur),
		durationStyle.Render(durStr),
		toggleStyle.Render(afterDur),
	)
}
