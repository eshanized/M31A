package components

import (
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
	style := "dark"
	if r.theme.Mode == theme.ModeLight {
		style = "light"
	}
	renderer, err := glamour.NewTermRenderer(
		glamour.WithStandardStyle(style),
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

func (r *MessageRenderer) renderUserMessage(content string, width int) string {
	maxWidth := int(float64(width) * 0.7)
	if maxWidth < 20 {
		maxWidth = 20
	}
	if maxWidth > width-4 {
		maxWidth = width - 4
	}

	bubble := r.theme.UserBubble.
		Width(maxWidth).
		MaxWidth(maxWidth).
		Render(content)

	return lipgloss.NewStyle().
		Width(width).
		Align(lipgloss.Right).
		Render(bubble)
}

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
			tb := NewThinkingBlock(seg, r.theme, false)
			rendered = append(rendered, tb.Render(contentWidth))
		}
	}

	if len(msg.ToolCalls) > 0 {
		for _, tc := range msg.ToolCalls {
			tcName := tc.Name
			if tcName == "" {
				tcName = "Tool"
			}
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
			Foreground(r.theme.TextPrimary).
			Width(width).
			Render(content)
	}

	return lipgloss.NewStyle().
		Width(width).
		Padding(0, 1).
		Render(rendered)
}

