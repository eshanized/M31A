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
// "┃  M31A  " ≈ 9 chars — enough for the brand label + split border.
const GutterWidth = 9

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

// RenderTimestampBar renders a timestamp separator between conversation turns.
// Format: ┤ HH:MM ├──────────────────────────────────
func RenderTimestampBar(t theme.Theme, ts time.Time, width int) string {
	return RenderTimestampBarWithSummary(t, ts, width, "")
}

// RenderTimestampBarWithSummary renders a timestamp separator with an optional
// trailing summary suffix. When summary is non-empty, the dashes fill the gap
// between the time prefix and the summary; when empty, behaves like
// RenderTimestampBar.
//
//	┤ 13:08 ├────────────────────── iter 3 · 3 tools
func RenderTimestampBarWithSummary(t theme.Theme, ts time.Time, width int, summary string) string {
	if ts.IsZero() {
		return ""
	}
	timeStr := ts.Format("15:04")
	prefix := "┤ " + timeStr + " ├"
	prefixWidth := lipgloss.Width(prefix)

	barStyle := lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true)

	if summary == "" {
		dashCount := width - prefixWidth - 1
		if dashCount < 2 {
			dashCount = 2
		}
		return barStyle.Render(prefix + strings.Repeat("─", dashCount))
	}

	// Leave a space, dashes, space, summary.
	suffix := " " + summary
	suffixWidth := lipgloss.Width(suffix)
	gap := width - prefixWidth - suffixWidth - 2
	if gap < 2 {
		gap = 2
	}
	dashes := strings.Repeat("─", gap)
	return barStyle.Render(prefix + dashes + " " + summary)
}

// renderUserMessage renders a user message with an opencode-style right-leaning bubble.
//
// Layout:
//
//	┃ user
//	┃   <content in muted right-aligned style>
func (r *MessageRenderer) renderUserMessage(msg types.Message, width int) string {
	t := r.theme
	contentWidth := calcContentWidth(width)

	// Gutter: thick split border + "user" role label (lowercase, muted, no bold)
	gutterStyle := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Bold(false)
	borderChar := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Render("┃")
	roleLabel := gutterStyle.Render(" user")
	gutter := lipgloss.JoinHorizontal(lipgloss.Top, borderChar, roleLabel)

	// Content: user input in a subtly styled block
	if msg.Content == "" {
		return lipgloss.JoinVertical(lipgloss.Top, gutter, "")
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

	return lipgloss.JoinVertical(lipgloss.Top, gutter, contentLine, "")
}

// renderAssistantMessage renders assistant content with an opencode-style thick left border.
//
// Layout:
//
//	┃ M31A
//	┃   <segments...>
func (r *MessageRenderer) renderAssistantMessage(msg types.Message, width int) string {
	t := r.theme
	contentWidth := calcContentWidth(width)

	// Gutter header: thick brand-colored border + "M31A" label
	borderChar := lipgloss.NewStyle().Foreground(t.Brand).Render("┃")
	roleLabel := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(" M31A")
	gutter := lipgloss.JoinHorizontal(lipgloss.Top, borderChar, roleLabel)

	// Render segments
	var rendered []string
	if len(msg.Segments) == 0 && msg.Content != "" {
		rendered = append(rendered, r.renderContentSegment(msg.Content, contentWidth))
	} else {
		for _, seg := range msg.Segments {
			switch seg.Type {
			case "content":
				rendered = append(rendered, r.renderContentSegment(seg.Content, contentWidth))
			case "thinking":
				tb := NewThinkingBlock(seg, t, false, 0)
				rendered = append(rendered, tb.Render(contentWidth))
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
					card := NewToolCard(*tc, nil, ToolRunning, t)
					rendered = append(rendered, card.Render(contentWidth))
				}
			case "error":
				rendered = append(rendered, r.renderErrorSegment(seg.Content, contentWidth))
			}
		}
	}

	if len(msg.ToolCalls) > 0 {
		for _, tc := range msg.ToolCalls {
			card := NewToolCard(tc, nil, ToolRunning, t)
			rendered = append(rendered, card.Render(contentWidth))
		}
	}

	if len(rendered) == 0 {
		rendered = append(rendered, "")
	}

	// Join content lines with thick left border on each
	content := lipgloss.JoinVertical(lipgloss.Top, rendered...)
	contentLines := strings.Split(content, "\n")
	borderedLines := make([]string, len(contentLines))
	for i, line := range contentLines {
		borderedLines[i] = lipgloss.JoinHorizontal(lipgloss.Top,
			lipgloss.NewStyle().Foreground(t.Brand).Render("┃"),
			lipgloss.NewStyle().PaddingLeft(2).Width(contentWidth-2).Render(line),
		)
	}

	return lipgloss.JoinVertical(lipgloss.Top, gutter, strings.Join(borderedLines, "\n"), "")
}

func (r *MessageRenderer) renderContentSegment(content string, width int) string {
	if content == "" {
		return ""
	}

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

// renderAgentIteration renders an agent-loop iteration summary as a bordered
// card with tool badges:
//
//	╭ Agent iteration 3 ──────────────────╮
//	│ [FileRead] [FileRead] [FileRead]     │
//	╰──────────────────────────────────────╯
//
// Input format: "**Agent iteration N** — tools: X, Y, Z"
// Returns "" if the content doesn't match the expected format.
func (r *MessageRenderer) renderAgentIteration(content string, width int) string {
	t := r.theme

	idx := strings.Index(content, "** — tools: ")
	if idx < 0 {
		return ""
	}
	title := strings.TrimPrefix(content[:idx], "**")
	toolsList := strings.TrimSpace(content[idx+len("** — tools: "):])
	if toolsList == "" {
		return ""
	}

	toolNames := strings.Split(toolsList, ", ")
	var badges []Badge
	for _, name := range toolNames {
		name = strings.TrimSpace(name)
		if name == "" {
			continue
		}
		badges = append(badges, NewBadge(name, BadgeBrandPreset, t))
	}
	if len(badges) == 0 {
		return ""
	}

	// Reserve 4 columns for the card's left+right border + inner padding.
	innerWidth := width - 4
	if innerWidth < 8 {
		innerWidth = 8
	}
	body := RenderBadges(badges)
	if lipgloss.Width(body) > innerWidth {
		body = TruncateEnd(body, innerWidth)
	}

	cardW := width
	if cardW < 20 {
		cardW = 20
	}

	card := Card{
		Title:   title,
		Content: body,
		Width:   cardW,
		Border:  theme.ThinBorder,
		Style:   CardBrand,
		Theme:   t,
	}.Render()

	return lipgloss.NewStyle().Width(width).Render(card)
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
