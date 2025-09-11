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

// GutterWidth is the fixed width for the role gutter (│ + label + space).
// "│ M31A" = 7 chars + 1 space = 8 chars total.
const GutterWidth = 8

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

	barStyle := lipgloss.NewStyle().Foreground(t.TextMuted)
	return barStyle.Render(prefix + dashes)
}

// renderUserMessage renders a user message with a role gutter.
// Format:
// │ USER
//   <content>
func (r *MessageRenderer) renderUserMessage(msg types.Message, width int) string {
	gutterStyle := lipgloss.NewStyle().Foreground(r.theme.TextSecondary)
	contentWidth := width - GutterWidth
	if contentWidth < 20 {
		contentWidth = 20
	}

	// Gutter line
	gutter := gutterStyle.Render("│ USER")

	// Content with left padding
	content := r.renderUserContent(msg.Content, contentWidth)

	return lipgloss.JoinVertical(lipgloss.Top, gutter, content)
}

// renderUserContent renders user message content with a split-border left edge.
func (r *MessageRenderer) renderUserContent(content string, width int) string {
	if content == "" {
		return ""
	}

	// Left gutter marker (│)
	gutterChar := lipgloss.NewStyle().Foreground(r.theme.TextSecondary).Render("│")

	// Content styled with padding
	contentStyle := lipgloss.NewStyle().
		Foreground(r.theme.Text).
		PaddingLeft(2).
		Width(width - 4).
		MaxWidth(width - 4)

	renderedContent := contentStyle.Render(content)

	// Join gutter + content
	return lipgloss.JoinHorizontal(lipgloss.Top, gutterChar, renderedContent)
}

// renderAssistantMessage renders assistant content with a role gutter.
// Format:
// │ M31A
//   <content>
func (r *MessageRenderer) renderAssistantMessage(msg types.Message, width int) string {
	contentWidth := width - GutterWidth
	if contentWidth < 20 {
		contentWidth = 20
	}

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
				tb := NewThinkingBlock(seg, r.theme, false, 0)
				rendered = append(rendered, tb.Render(contentWidth))
			case "tool_use":
				var tc types.ToolCall
				if err := json.Unmarshal([]byte(seg.Content), &tc); err == nil {
					card := NewToolCard(tc, nil, ToolRunning, r.theme)
					rendered = append(rendered, card.Render(contentWidth))
				}
			}
		}
	}

	if len(msg.ToolCalls) > 0 {
		for _, tc := range msg.ToolCalls {
			card := NewToolCard(tc, nil, ToolRunning, r.theme)
			rendered = append(rendered, card.Render(contentWidth))
		}
	}

	if len(rendered) == 0 {
		rendered = append(rendered, "")
	}

	// Join gutter with content
	content := lipgloss.JoinVertical(lipgloss.Top, rendered...)
	gutterChar := lipgloss.NewStyle().Foreground(r.theme.Brand).Render("│")

	return lipgloss.JoinHorizontal(lipgloss.Top,
		gutterChar,
		lipgloss.NewStyle().PaddingLeft(2).Width(contentWidth-4).Render(content),
	)
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
	contentWidth := width - GutterWidth
	if contentWidth < 20 {
		contentWidth = 20
	}
	for i, line := range lines {
		result[i] = lipgloss.JoinHorizontal(lipgloss.Top,
			gutterChar.Render("│"),
			lipgloss.NewStyle().PaddingLeft(2).Width(contentWidth-4).Render(line),
		)
	}
	return strings.Join(result, "\n")
}
