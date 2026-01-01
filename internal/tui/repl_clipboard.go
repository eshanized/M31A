package tui

import (
	"time"

	"github.com/atotto/clipboard"
	tea "github.com/charmbracelet/bubbletea"
)

// copyLastAssistantMessage copies the content of the last assistant message
// in the conversation to the system clipboard.
func (m *ReplModel) copyLastAssistantMessage() tea.Cmd {
	var lastContent string
	for i := len(m.messages) - 1; i >= 0; i-- {
		if m.messages[i].Role == "assistant" && m.messages[i].Content != "" {
			lastContent = m.messages[i].Content
			break
		}
	}
	if lastContent == "" {
		return func() tea.Msg {
			return ToastMsg{Text: "No assistant message to copy", Duration: 3 * time.Second, Type: "warning"}
		}
	}
	if err := clipboard.WriteAll(lastContent); err != nil {
		return func() tea.Msg {
			return ToastMsg{Text: "Clipboard unavailable: " + err.Error(), Duration: 4 * time.Second, Type: "error"}
		}
	}
	return func() tea.Msg {
		return ToastMsg{Text: "Copied last assistant message to clipboard", Duration: 3 * time.Second, Type: "success"}
	}
}

// copyLastError copies the last error message from the conversation to the clipboard.
func (m *ReplModel) copyLastError() tea.Cmd {
	var lastErr string
	for i := len(m.messages) - 1; i >= 0; i-- {
		if m.messages[i].Role == "assistant" && len(m.messages[i].Content) > 7 && m.messages[i].Content[:7] == "Error: " {
			lastErr = m.messages[i].Content[7:]
			break
		}
	}
	if lastErr == "" {
		return func() tea.Msg {
			return ToastMsg{Text: "No error message to copy", Duration: 3 * time.Second, Type: "warning"}
		}
	}
	if err := clipboard.WriteAll(lastErr); err != nil {
		return func() tea.Msg {
			return ToastMsg{Text: "Clipboard unavailable: " + err.Error(), Duration: 4 * time.Second, Type: "error"}
		}
	}
	return func() tea.Msg {
		return ToastMsg{Text: "Copied error to clipboard", Duration: 3 * time.Second, Type: "success"}
	}
}
