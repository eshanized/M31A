package chat_history

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
)

// renderChatHistory renders the chat history table screen content.
func (ch *ChatHistoryModel) renderChatHistory() string {
	t := ch.theme

	title := components.ScreenTitle{Text: "Chat History", Theme: t}.Render()

	if !ch.loaded {
		return lipgloss.JoinVertical(lipgloss.Left, "",
			title, "",
			components.LoadingIndicator{Label: "Loading...", Theme: t, Inline: true}.Render())
	}

	if len(ch.messages) == 0 {
		return lipgloss.JoinVertical(lipgloss.Left, "",
			title, "",
			components.InlineEmptyState{Message: "No messages in this session yet.", Theme: t}.Render())
	}

	// Update viewport content
	ch.viewport.SetContent(ch.renderTable())

	// Status line
	statusLine := components.ScrollIndicator{Cursor: ch.cursor, Total: len(ch.messages), Theme: t}.Render()

	hints := components.HintBar{
		Hints: []string{"↑↓ navigate", "enter continue from here", "g top", "G bottom", "q back"},
		Theme: t,
	}.Render()

	parts := []string{
		"",
		title,
		"",
		ch.viewport.View(),
		"",
		statusLine,
		hints,
	}

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// renderTable converts messages to a styled table with cursor indicator.
func (ch *ChatHistoryModel) renderTable() string {
	t := ch.theme

	// Header
	header := lipgloss.NewStyle().Foreground(t.TextMuted).Bold(true).
		Render(fmt.Sprintf("  %-4s  %-10s  %-16s  %-8s  %s",
			"#", "Role", "Time", "Tokens", "Preview"))
	divRow := lipgloss.NewStyle().Foreground(t.Border).
		Render(strings.Repeat("─", ch.width-2))

	var rows []string
	rows = append(rows, header, divRow)

	for i, msg := range ch.messages {
		role := msg.Role
		if role == "" {
			role = "unknown"
		}

		ts := ""
		if !msg.CreatedAt.IsZero() {
			ts = msg.CreatedAt.Format("15:04:05")
		}

		tokens := ""
		if msg.Usage != nil {
			tokens = fmt.Sprintf("%d", msg.Usage.TotalTokens)
		}

		preview := msg.Content
		// Strip newlines for single-line display
		preview = strings.ReplaceAll(preview, "\n", " ")
		preview = strings.ReplaceAll(preview, "\r", " ")
		if len(preview) > 50 {
			preview = preview[:47] + "..."
		}
		if preview == "" {
			// Show segment info if no content
			for _, seg := range msg.Segments {
				if seg.Type == "thinking" {
					preview = "[thinking]"
					break
				}
				if seg.Type == "tool_use" {
					preview = "[tool call]"
					break
				}
				if seg.Type == "error" {
					preview = "[error]"
					break
				}
			}
			if preview == "" {
				preview = "(empty)"
			}
		}

		row := fmt.Sprintf("  %-4d  %-10s  %-16s  %-8s  %s",
			i+1,
			role,
			ts,
			tokens,
			preview,
		)

		if i == ch.cursor {
			// Selected row — highlighted
			rows = append(rows, components.CursorIndicator{Selected: true, Theme: t}.Render()+
				strings.TrimPrefix(row, "  "))
		} else {
			rows = append(rows, components.CursorIndicator{Selected: false, Theme: t}.Render()+
				lipgloss.NewStyle().Foreground(t.Text).Render(row))
		}
	}

	return strings.Join(rows, "\n")
}
