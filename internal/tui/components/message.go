package components

import (
	"encoding/json"
	"strings"
	"time"

	"github.com/charmbracelet/glamour"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// GutterWidth is the fixed width for the role gutter.
// "┃ ◆  " ≈ 6 chars — compact brand icon + split border.
const GutterWidth = 6

// calcContentWidth returns the available content width, clamped to a minimum of 20 columns.
func calcContentWidth(width int) int {
	w := width - GutterWidth
	if w < 20 {
		w = 20
	}
	return w
}

type MessageRenderer struct {
	theme    theme.Theme
	renderer *glamour.TermRenderer
	width    int

	// toolCallCache maps segment content string → pre-parsed ToolCall (TU-3 fix).
	// Avoids json.Unmarshal on every render for unchanged tool_use segments.
	toolCallCache map[string]*types.ToolCall
}

func NewMessageRenderer(t theme.Theme, width int) (*MessageRenderer, error) {
	mr := &MessageRenderer{
		theme:         t,
		width:         width,
		toolCallCache: make(map[string]*types.ToolCall),
	}
	if err := mr.createGlamourRenderer(); err != nil {
		return nil, err
	}
	return mr, nil
}

func (r *MessageRenderer) createGlamourRenderer() error {
	style := glamour.WithStylePath("dark")
	if r.theme.Mode == theme.ModeLight {
		style = glamour.WithStylePath("light")
	}
	renderer, err := glamour.NewTermRenderer(
		style,
		glamour.WithWordWrap(r.width-GutterWidth),
	)
	if err != nil {
		return err
	}
	r.renderer = renderer
	return nil
}

func (r *MessageRenderer) SetWidth(width int) error {
	if width == r.width {
		return nil
	}
	r.width = width
	r.renderer.Close()
	return r.createGlamourRenderer()
}

// RenderMessage renders a message with a role gutter and optional timestamp.
func (r *MessageRenderer) RenderMessage(msg types.Message, width int) string {
	switch msg.Role {
	case "user":
		return r.renderUserMessage(msg, width)
	case "assistant":
		return r.renderAssistantMessage(msg, width)
	default:
		return r.renderAssistantMessage(msg, width)
	}
}

// RenderTimestampBar renders a compact timestamp between conversation turns.
// Format: ── 15:04 ──
func RenderTimestampBar(t theme.Theme, ts time.Time, width int) string {
	return RenderTimestampBarWithSummary(t, ts, width, "")
}

// RenderTimestampBarWithSummary renders a compact timestamp separator with an
// optional trailing summary suffix.
func RenderTimestampBarWithSummary(t theme.Theme, ts time.Time, width int, summary string) string {
	if ts.IsZero() {
		return ""
	}
	timeStr := ts.Format("15:04")

	barStyle := lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true)

	if summary == "" {
		return barStyle.Render("── " + timeStr + " ──")
	}

	return barStyle.Render("── " + timeStr + " ── " + summary)
}

// renderUserMessage renders a user message with a compact right-aligned bubble.
//
// Layout:
//
//	┃ ▸ you
//	┃   <content>
func (r *MessageRenderer) renderUserMessage(msg types.Message, width int) string {
	t := r.theme
	contentWidth := calcContentWidth(width)

	borderChar := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Render("┃")
	roleLabel := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Render(" ▸ you")
	gutter := lipgloss.JoinHorizontal(lipgloss.Top, borderChar, roleLabel)

	if msg.Content == "" {
		return ""
	}

	contentStyle := lipgloss.NewStyle().
		Foreground(t.TextPrimary).
		PaddingLeft(2).
		Width(contentWidth - 2).
		MaxWidth(contentWidth - 2)

	contentLine := lipgloss.JoinHorizontal(lipgloss.Top,
		lipgloss.NewStyle().Foreground(t.TextSecondary).Render("┃"),
		contentStyle.Render(msg.Content),
	)

	return lipgloss.JoinVertical(lipgloss.Top, gutter, contentLine)
}

// renderAssistantMessage renders assistant content with a compact left border.
//
// Layout:
//
//	┃ ◆ assistant
//	  <segments...>
//
// Tool-only messages (no text content) are collapsed into a single summary line.
func (r *MessageRenderer) renderAssistantMessage(msg types.Message, width int) string {
	t := r.theme
	contentWidth := calcContentWidth(width)

	borderChar := lipgloss.NewStyle().Foreground(t.Brand).Render("┃")
	roleLabel := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(" ◆")
	gutter := lipgloss.JoinHorizontal(lipgloss.Top, borderChar, roleLabel)

	// Render segments
	var contentSegments []string
	var toolSegments []string
	hasContent := false

	if len(msg.Segments) == 0 && msg.Content != "" {
		stripped := StripANSI(msg.Content)
		contentSegments = append(contentSegments, r.renderContentSegment(stripped, contentWidth))
		hasContent = true
	} else {
		for _, seg := range msg.Segments {
			switch seg.Type {
			case "content":
				stripped := StripANSI(seg.Content)
				rendered := r.renderContentSegment(stripped, contentWidth)
				if rendered != "" {
					contentSegments = append(contentSegments, rendered)
					hasContent = true
				}
			case "thinking":
				tb := NewThinkingBlock(seg, t, false, 0)
				contentSegments = append(contentSegments, tb.Render(contentWidth))
				hasContent = true
			case "tool_use":
				tc, ok := r.toolCallCache[seg.Content]
				if !ok {
					var parsed types.ToolCall
					if err := json.Unmarshal([]byte(seg.Content), &parsed); err == nil {
						tc = &parsed
						r.toolCallCache[seg.Content] = tc
					}
				}
				if tc != nil {
					toolSegments = append(toolSegments, tc.Name)
					if hasContent {
						card := NewToolCard(*tc, nil, ToolRunning, t)
						contentSegments = append(contentSegments, card.Render(contentWidth))
					}
				}
			case "error":
				contentSegments = append(contentSegments, r.renderErrorSegment(seg.Content, contentWidth))
				hasContent = true
			}
		}
	}

	if len(msg.ToolCalls) > 0 {
		for _, tc := range msg.ToolCalls {
			toolSegments = append(toolSegments, tc.Name)
			if hasContent {
				card := NewToolCard(tc, nil, ToolRunning, t)
				contentSegments = append(contentSegments, card.Render(contentWidth))
			}
		}
	}

	// Collapse tool-only messages into a compact summary
	if !hasContent && len(toolSegments) > 0 {
		toolBadges := strings.Join(toolSegments, " ")
		summary := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Faint(true).
			Render("  " + toolBadges)
		return lipgloss.JoinVertical(lipgloss.Top, gutter, summary)
	}

	if len(contentSegments) == 0 {
		return ""
	}

	// Join content lines with left border
	content := lipgloss.JoinVertical(lipgloss.Top, contentSegments...)
	contentLines := strings.Split(content, "\n")
	borderedLines := make([]string, len(contentLines))
	for i, line := range contentLines {
		borderedLines[i] = lipgloss.JoinHorizontal(lipgloss.Top,
			lipgloss.NewStyle().Foreground(t.Brand).Render("┃"),
			lipgloss.NewStyle().PaddingLeft(2).Width(contentWidth-2).Render(line),
		)
	}

	return lipgloss.JoinVertical(lipgloss.Top, gutter, strings.Join(borderedLines, "\n"))
}

func (r *MessageRenderer) renderContentSegment(content string, width int) string {
	if content == "" {
		return ""
	}

	// Strip any ANSI escape codes before glamour to prevent mangling
	content = StripANSI(content)

	// Special-case: "Agent iteration N — tools: X, Y" → card + tool chips.
	if strings.HasPrefix(content, "**Agent iteration") {
		if out := r.renderAgentIteration(content, width); out != "" {
			return out
		}
	}

	rendered, err := r.renderer.Render(content)
	if err != nil {
		return lipgloss.NewStyle().
			Foreground(r.theme.Text).
			Width(width).
			PaddingLeft(2).
			Render(content)
	}

	return lipgloss.NewStyle().
		Width(width).
		PaddingLeft(2).
		Render(rendered)
}

// renderAgentIteration renders an agent-loop iteration as a compact inline line.
//
//	 ³ FileRead FileRead FileRead
//
// Input format: "**Agent iteration N** — tools: X, Y, Z"
// Returns "" if the content doesn't match the expected format.
func (r *MessageRenderer) renderAgentIteration(content string, width int) string {
	t := r.theme

	idx := strings.Index(content, "** — tools: ")
	if idx < 0 {
		return ""
	}
	toolsList := strings.TrimSpace(content[idx+len("** — tools: "):])
	if toolsList == "" {
		return ""
	}

	toolNames := strings.Split(toolsList, ", ")
	var badges []string
	for _, name := range toolNames {
		name = strings.TrimSpace(name)
		if name == "" {
			continue
		}
		badges = append(badges, name)
	}
	if len(badges) == 0 {
		return ""
	}

	// Extract iteration number
	title := strings.TrimPrefix(content[:idx], "**")
	iterNum := strings.TrimPrefix(title, "Agent iteration ")

	// Compact single-line: superscript iteration number + tool names
	superscript := ""
	for _, ch := range iterNum {
		switch ch {
		case '0':
			superscript += "⁰"
		case '1':
			superscript += "¹"
		case '2':
			superscript += "²"
		case '3':
			superscript += "³"
		case '4':
			superscript += "⁴"
		case '5':
			superscript += "⁵"
		case '6':
			superscript += "⁶"
		case '7':
			superscript += "⁷"
		case '8':
			superscript += "⁸"
		case '9':
			superscript += "⁹"
		default:
			superscript += string(ch)
		}
	}

	line := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Faint(true).
		PaddingLeft(2).
		Render(superscript + " " + strings.Join(badges, " "))

	return lipgloss.NewStyle().Width(width).Render(line)
}

// renderErrorSegment styles an error banner directly via lipgloss, bypassing
// glamour so that plain-text error content (with ✗/⚠ glyphs) is rendered
// without the markdown pipeline mangling any escape sequences.
func (r *MessageRenderer) renderErrorSegment(content string, width int) string {
	if content == "" {
		return ""
	}
	styled := lipgloss.NewStyle().
		Foreground(r.theme.Error).
		Bold(true).
		Render(content)
	return lipgloss.NewStyle().
		Width(width).
		PaddingLeft(2).
		Render(styled)
}

// FormatTimeBar formats a timestamp for use in timestamp bars.
func FormatTimeBar(ts time.Time) string {
	if ts.IsZero() {
		return ""
	}
	return ts.Format("15:04")
}

// WrapWithGutter wraps content with a gutter character on each line.
func WrapWithGutter(content string, gutterChar lipgloss.Style, width int) string {
	lines := strings.Split(content, "\n")
	result := make([]string, len(lines))
	contentWidth := calcContentWidth(width)
	for i, line := range lines {
		result[i] = lipgloss.JoinHorizontal(lipgloss.Top,
			gutterChar.Render("┃"),
			lipgloss.NewStyle().PaddingLeft(2).Width(contentWidth-4).Render(line),
		)
	}
	return strings.Join(result, "\n")
}
