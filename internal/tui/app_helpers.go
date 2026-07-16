package tui

// Tool card state management and utility functions for AppState.

import (
	"encoding/json"
	"strings"

	"github.com/eshanized/M31A/pkg/types"
)

// toggleToolCardCollapsed toggles the collapsed state of a tool card by ID (M8).
func (m *AppState) toggleToolCardCollapsed(toolID string) {
	if m.replModel == nil || m.replModel.msgRenderer == nil {
		return
	}
	// Toggle in the renderer's collapsed state map
	current := m.replModel.msgRenderer.IsToolCardCollapsed(toolID)
	m.replModel.msgRenderer.SetToolCardCollapsed(toolID, !current)
	// Also update the tool card in the REPL's map if it exists
	for _, card := range m.replModel.toolCards {
		if card.ToolID() == toolID {
			card.SetCollapsed(!current)
			break
		}
	}
	// Re-render to reflect the change
	m.replModel.renderMessages()
}

// collapseAllToolCards collapses all expanded tool cards (M8).
func (m *AppState) collapseAllToolCards() {
	if m.replModel == nil || m.replModel.msgRenderer == nil {
		return
	}
	m.replModel.msgRenderer.SetAllToolCardsCollapsed(true)
	for _, card := range m.replModel.toolCards {
		card.SetCollapsed(true)
	}
	m.replModel.renderMessages()
}

// ensureToolDetailModel lazily creates the tool-detail model with the current
// theme and REPL dimensions so the detail screen is ready to display.
func (m *AppState) ensureToolDetailModel() {
	if m.toolDetailModel != nil {
		return
	}
	t := m.themeManager.Current()
	w := m.width
	h := m.height
	if w < 40 {
		w = 40
	}
	if h < 10 {
		h = 10
	}
	m.toolDetailModel = NewToolDetailModel(t, w, h)
}

// extractToolDetail returns (title, body) for the first tool_use segment in
// messages[messageIndex] whose Name matches toolName. Falls back to the first
// tool_use segment when toolName is empty or unmatched. Returns ("", "") when
// no tool_use segment exists in the message.
func (m *AppState) extractToolDetail(messageIndex int, toolName string) (string, string) {
	if m.replModel == nil {
		return "", ""
	}
	msgs := m.replModel.messages
	if messageIndex < 0 || messageIndex >= len(msgs) {
		return "", ""
	}
	msg := msgs[messageIndex]

	type candidate struct {
		name  string
		input string
		body  string
	}
	var fallback *candidate

	for _, seg := range msg.Segments {
		if seg.Type != "tool_use" {
			continue
		}
		name := extractToolName(seg.Content)
		body := seg.Content
		c := &candidate{name: name, input: seg.Content, body: body}
		if toolName != "" && name == toolName {
			return name, c.body
		}
		if fallback == nil {
			fallback = c
		}
	}

	if fallback != nil {
		return fallback.name, fallback.body
	}
	return "", ""
}

// extractToolInputSnippet returns a short human-readable description of a tool call's input.
// Extracts the most relevant parameter (path, command, pattern) and truncates to 40 chars.
func extractToolInputSnippet(tc types.ToolCall) string {
	var params map[string]any
	if err := json.Unmarshal(tc.Input, &params); err != nil {
		return ""
	}
	// Priority order for display: path > command > pattern > query > description
	for _, key := range []string{"path", "command", "pattern", "query", "url"} {
		if v, ok := params[key]; ok {
			if s, ok := v.(string); ok && s != "" {
				snippet := strings.ReplaceAll(s, "\n", " ")
				if len(snippet) > 40 {
					snippet = snippet[:37] + "\u2026"
				}
				return snippet
			}
		}
	}
	return ""
}
