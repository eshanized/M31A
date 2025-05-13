package components

import (
	"encoding/json"

	"github.com/charmbracelet/glamour"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/tui/theme"
)

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
		glamour.WithWordWrap(r.width),
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

func (r *MessageRenderer) RenderMessage(msg types.Message, width int) string {
	switch msg.Role {
	case "user":
		return r.renderUserMessage(msg.Content, width)
	case "assistant":
		return r.renderAssistantMessage(msg, width)
	default:
		return r.renderAssistantMessage(msg, width)
	}
}

// renderUserMessage uses OpenCode-style left-bordered block instead of rounded bubble.
func (r *MessageRenderer) renderUserMessage(content string, width int) string {
	contentWidth := width - 4 // account for paddingLeft=2 paddingRight=2
	if contentWidth < 20 {
		contentWidth = 20
	}

	borderStyle := lipgloss.NewStyle().
		Border(theme.SplitBorder, true, false, false, false).
		BorderForeground(r.theme.Border).
		Background(r.theme.BackgroundPanel).
		Padding(1, 2).
		MaxWidth(contentWidth).
		Width(contentWidth)

	bubble := borderStyle.Render(content)

	return lipgloss.NewStyle().
		Width(width).
		Render(bubble)
}

// renderAssistantMessage renders assistant content with left indentation.
func (r *MessageRenderer) renderAssistantMessage(msg types.Message, width int) string {
	contentWidth := width - 4

	if len(msg.Segments) == 0 && msg.Content != "" {
		return r.renderContentSegment(msg.Content, contentWidth)
	}

	var rendered []string

	for _, seg := range msg.Segments {
		switch seg.Type {
		case "content":
			rendered = append(rendered, r.renderContentSegment(seg.Content, contentWidth))
		case "thinking":
			tb := NewThinkingBlock(seg, r.theme, false, 0)
			rendered = append(rendered, tb.Render(contentWidth))
		case "tool_use":
			// Parse tool call from segment JSON
			var tc types.ToolCall
			if err := json.Unmarshal([]byte(seg.Content), &tc); err == nil {
				card := NewToolCard(tc, nil, ToolRunning, r.theme)
				rendered = append(rendered, card.Render(contentWidth))
			}
		}
	}

	if len(msg.ToolCalls) > 0 {
		for _, tc := range msg.ToolCalls {
			card := NewToolCard(tc, nil, ToolRunning, r.theme)
			rendered = append(rendered, card.Render(contentWidth))
		}
	}

	return lipgloss.JoinVertical(lipgloss.Top, rendered...)
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
			PaddingLeft(3).
			Render(content)
	}

	return lipgloss.NewStyle().
		Width(width).
		PaddingLeft(3).
		Render(rendered)
}
