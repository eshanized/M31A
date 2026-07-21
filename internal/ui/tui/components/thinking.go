package components

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

type ThinkingBlock struct {
	id           int
	segment      types.MessageSegment
	theme        theme.Theme
	styles       theme.SemanticStyles
	expanded     bool
	focused      bool
	startedAt    time.Time
	scrollOffset int
	// phase is the current workflow phase (e.g. "execute", "plan")
	phase string
	// taskAction is the current task action (if any)
	taskAction string
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
		styles:    theme.BuildSemanticStyles(t),
		expanded:  expanded,
		focused:   false,
		startedAt: startedAt,
	}
}

// SetContext sets the workflow phase and task action for intent labels.
func (b *ThinkingBlock) SetContext(phase, taskAction string) {
	b.phase = phase
	b.taskAction = taskAction
}

// Render renders the thinking block as a compact inline element:
//
// Collapsed:  ╭─ ⠹ Thinking · 1.2s ──────────────────╮
// Expanded:   full panel with ┃ left-gutter content
func (b *ThinkingBlock) Render(width int) string {
	s := b.styles
	contentWidth := width - 4 // account for padding

	// Get intent label instead of timer
	elapsed := b.Elapsed()
	intentLabel := thinkingLabel(b.phase, b.taskAction, elapsed)

	if !b.expanded {
		// Collapsed: styled capsule with content preview
		spinner := "⠹" // static thinking indicator; parent spinner provides animation
		spinnerStyled := s.ThinkingToggle.Bold(true).Render(spinner)
		labelStyled := s.ThinkingLabel.Render(" " + intentLabel)

		// Content preview: first line truncated to fit
		preview := ""
		if b.segment.Content != "" {
			firstLine := strings.SplitN(b.segment.Content, "\n", 2)[0]
			firstLine = strings.TrimSpace(firstLine)
			if firstLine != "" {
				maxPreview := contentWidth - lipgloss.Width(spinnerStyled) - lipgloss.Width(labelStyled) - 12
				if maxPreview > 10 {
					if len(firstLine) > maxPreview {
						firstLine = firstLine[:maxPreview-1] + "…"
					}
					preview = " " + s.ThinkingMuted.Render(firstLine)
				}
			}
		}

		inner := spinnerStyled + labelStyled + preview

		// Build capsule: ╭─ [inner] ──────╮
		innerW := lipgloss.Width(inner)
		prefixRaw := "╭─ "
		suffixRaw := " "
		// trailing dashes fill to contentWidth
		fixedW := lipgloss.Width(prefixRaw) + innerW + lipgloss.Width(suffixRaw) + 2 // +2 for ╮ and left pad
		dashW := contentWidth - fixedW
		if dashW < 1 {
			dashW = 1
		}

		// Animated dashes: alternate between ╌ and ┄ for a subtle breathing effect
		dashChar := "╌"
		if time.Now().UnixMilli()/500%2 == 1 {
			dashChar = "┄"
		}
		dashes := strings.Repeat(dashChar, dashW)

		line := s.ThinkingBorder.Render(prefixRaw) + inner + s.ThinkingBorder.Render(suffixRaw+dashes+"╮")

		return lipgloss.NewStyle().PaddingLeft(2).Width(width).Render(line)
	}

	// Expanded: panel with ┃ left-gutter border + italic thinking content + footer

	// Header line: toggle + intent label + duration
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

	bodyContent := s.Thinking.
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
			scrollInfo := s.SecondaryText.
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

	// Footer: duration (shown only when expanded for tooltip/expandable)
	durStr := b.Duration()
	footer := lipgloss.NewStyle().
		Border(theme.ThinBorder).
		BorderForeground(b.theme.Thinking).
		BorderTop(false).BorderBottom(true).BorderLeft(true).BorderRight(true).
		Padding(0, 1).
		Width(contentWidth + 2).
		Render(s.ThinkingMuted.Render(durStr))

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
	s := b.styles
	toggle := "▸"
	hint := ""
	if b.expanded {
		toggle = "▾"
		if b.scrollOffset > 0 {
			hint = " ↑↓"
		}
	}

	// Use intent label instead of "Thinking · duration"
	elapsed := b.Elapsed()
	intent := thinkingLabel(b.phase, b.taskAction, elapsed)

	toggleStyle := s.ThinkingToggle
	if b.focused {
		toggleStyle = s.BrandBold
	}

	return toggleStyle.Render(toggle) +
		s.ThinkingLabel.Render(" "+intent) +
		s.ThinkingMuted.Render(hint)
}

// Elapsed returns the elapsed time since thinking started.
func (b *ThinkingBlock) Elapsed() time.Duration {
	if b.segment.DurationMs > 0 {
		return time.Duration(b.segment.DurationMs) * time.Millisecond
	}
	return time.Since(b.startedAt)
}

// thinkingLabel returns an intent-based label for the thinking state.
func thinkingLabel(phase, taskAction string, elapsed time.Duration) string {
	intent := classifyThinkingIntent(phase, taskAction, elapsed)
	return intentLabel(intent, elapsed)
}

// ThinkingLabel is the exported version of thinkingLabel for use in other packages.
func ThinkingLabel(phase, taskAction string, elapsed time.Duration) string {
	return thinkingLabel(phase, taskAction, elapsed)
}

// ThinkingIntent represents the type of work the AI is doing.
type ThinkingIntent int

const (
	IntentUnknown ThinkingIntent = iota
	IntentPlanning
	IntentImplementing
	IntentVerifying
	IntentAnalyzing
	IntentRefining
	IntentResearching
	IntentSynthesizing
)

func classifyThinkingIntent(phase, taskAction string, elapsed time.Duration) ThinkingIntent {
	phase = strings.ToLower(phase)

	switch phase {
	case "plan":
		if elapsed < 2*time.Second {
			return IntentAnalyzing
		}
		return IntentPlanning
	case "execute":
		if taskAction != "" {
			return IntentImplementing
		}
		if elapsed < 3*time.Second {
			return IntentAnalyzing
		}
		return IntentImplementing
	case "verify":
		return IntentVerifying
	case "discuss":
		return IntentPlanning
	case "ship":
		return IntentSynthesizing
	case "runtime":
		return IntentAnalyzing
	default:
		if elapsed < 1*time.Second {
			return IntentAnalyzing
		}
		if elapsed < 5*time.Second {
			return IntentRefining
		}
		return IntentSynthesizing
	}
}

func intentLabel(intent ThinkingIntent, elapsed time.Duration) string {
	switch intent {
	case IntentPlanning:
		return "Planning implementation"
	case IntentImplementing:
		return "Implementing changes"
	case IntentVerifying:
		return "Verifying results"
	case IntentAnalyzing:
		if elapsed < 500*time.Millisecond {
			return "Analyzing"
		}
		return "Analyzing code"
	case IntentRefining:
		return "Refining approach"
	case IntentResearching:
		return "Researching"
	case IntentSynthesizing:
		return "Synthesizing"
	default:
		if elapsed < 1*time.Second {
			return "Thinking"
		}
		return "Thinking…"
	}
}
