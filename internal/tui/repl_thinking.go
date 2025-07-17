package tui

import (
	"sort"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/types"
)

func (m *ReplModel) toggleAllThinkingBlocks() {
	if len(m.thinkingBlocks) == 0 {
		return
	}

	// Determine current state: all expanded, all collapsed, or mixed
	allExpanded := true
	allCollapsed := true
	for _, block := range m.thinkingBlocks {
		if block.IsExpanded() {
			allCollapsed = false
		} else {
			allExpanded = false
		}
	}

	// If all expanded → collapse all. If all hidden → expand all. Mixed → collapse all.
	collapse := allExpanded || (!allExpanded && !allCollapsed)
	for _, block := range m.thinkingBlocks {
		if collapse {
			if block.IsExpanded() {
				block.Toggle()
			}
		} else {
			if !block.IsExpanded() {
				block.Toggle()
			}
		}
	}
}

// toggleFocusedThinkingBlock toggles the focused block, or focuses the first collapsed one.
func (m *ReplModel) toggleFocusedThinkingBlock() {
	if len(m.thinkingBlocks) == 0 {
		return
	}

	// Clear all focus first
	for _, block := range m.thinkingBlocks {
		block.SetFocused(false)
	}

	// If we have a valid focused block, toggle it
	if m.thinkingFocusIndex >= 0 {
		if block, ok := m.thinkingBlocks[m.thinkingFocusIndex]; ok {
			block.Toggle()
			block.SetFocused(true)
			return
		}
	}

	// No valid focus: find first collapsed block and focus+toggle it
	for idx, block := range m.thinkingBlocks {
		if !block.IsExpanded() {
			block.Toggle()
			block.SetFocused(true)
			m.thinkingFocusIndex = idx
			return
		}
	}

	// All expanded: focus first one
	for idx, block := range m.thinkingBlocks {
		block.SetFocused(false)
		if idx == 0 {
			block.SetFocused(true)
		}
	}
	m.thinkingFocusIndex = 0
}

// cycleThinkingFocus moves focus to the next thinking block.
func (m *ReplModel) cycleThinkingFocus() {
	if len(m.thinkingBlocks) == 0 {
		return
	}

	// Clear current focus
	for _, block := range m.thinkingBlocks {
		block.SetFocused(false)
	}

	// Get sorted indices
	indices := make([]int, 0, len(m.thinkingBlocks))
	for id := range m.thinkingBlocks {
		indices = append(indices, id)
	}
	sort.Ints(indices)

	// Find next index after current focus
	nextIdx := 0
	for i, id := range indices {
		if id == m.thinkingFocusIndex {
			nextIdx = (i + 1) % len(indices)
			break
		}
	}

	m.thinkingFocusIndex = indices[nextIdx]
	m.thinkingBlocks[m.thinkingFocusIndex].SetFocused(true)
}

// cycleThinkingFocusBackward moves focus to the previous thinking block.
func (m *ReplModel) cycleThinkingFocusBackward() {
	if len(m.thinkingBlocks) == 0 {
		return
	}

	for _, block := range m.thinkingBlocks {
		block.SetFocused(false)
	}

	indices := make([]int, 0, len(m.thinkingBlocks))
	for id := range m.thinkingBlocks {
		indices = append(indices, id)
	}
	sort.Ints(indices)

	prevIdx := len(indices) - 1
	for i, id := range indices {
		if id == m.thinkingFocusIndex {
			prevIdx = (i - 1 + len(indices)) % len(indices)
			break
		}
	}

	m.thinkingFocusIndex = indices[prevIdx]
	m.thinkingBlocks[m.thinkingFocusIndex].SetFocused(true)
}

// ShowQuestion displays a question from the AskUserQuestion tool inline in the REPL.
func (m *ReplModel) ShowQuestion(msg QuestionRequestMsg) {
	m.activeQuestion = &msg

	// Add a system message showing the question
	questionText := components.FormatQuestion(msg.Question, msg.Header, msg.Options, m.width, m.theme, msg.TimeoutSecs)
	questionMsg := types.Message{
		Role:      "system",
		Content:   questionText,
		CreatedAt: time.Now(),
	}
	m.messages = append(m.messages, questionMsg)
	m.renderMessages()
	m.viewport.GotoBottom()

	// Focus the textarea for user input
	m.textarea.Focus()
	m.textarea.Placeholder = "Type your answer and press Enter..."
}

// HandleQuestionInput processes user input when a question is active.
// Returns a tea.Cmd that sends the answer back to the tool, or nil if no answer.
func (m *ReplModel) HandleQuestionInput() tea.Cmd {
	if m.activeQuestion == nil {
		return nil
	}

	answer := strings.TrimSpace(m.textarea.Value())
	if answer == "" {
		return nil
	}

	m.textarea.Reset()
	m.activeQuestion = nil
	m.textarea.Placeholder = "Type a message, /command, or goal..."
	m.renderMessages()
	m.viewport.GotoBottom()

	return func() tea.Msg {
		return QuestionResponseMsg{Answer: answer}
	}
}
