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
}

func NewMessageRenderer(t theme.Theme, width int) (*MessageRenderer, error) {
	mr := &MessageRenderer{
		theme: t,
		width: width,
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
	if ts.IsZero() {
		return ""
	}
	timeStr := ts.Format("15:04")
	prefix := "┤ " + timeStr + " ├"
	prefixWidth := lipgloss.Width(prefix)
	dashCount := width - prefixWidth - 1
	if dashCount < 2 {
		dashCount = 2
	}
	dashes := strings.Repeat("─", dashCount)

	barStyle := lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true)
	return barStyle.Render(prefix + dashes)
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
				var tc types.ToolCall
				if err := json.Unmarshal([]byte(seg.Content), &tc); err == nil {
					card := NewToolCard(tc, nil, ToolRunning, t)
					rendered = append(rendered, card.Render(contentWidth))
				}
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
