package tui

import (
	"encoding/json"
	"fmt"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/types"
)

// AppendStreamChunk appends a streamed token from the Discuss phase
// to the REPL's current streaming content buffer.
func (m *ReplModel) AppendStreamChunk(chunk *types.StreamChunk) {
	if chunk == nil || chunk.Delta == "" {
		return
	}
	m.streaming = true
	m.streamContent.WriteString(chunk.Delta)
}

func (m *ReplModel) handleStreamMsg(msg StreamMsg) ([]tea.Cmd, bool) {
	chunk := msg.Chunk
	if chunk == nil {
		return nil, false
	}

	m.streaming = true

	switch chunk.Type {
	case "content":
		if m.activeSegmentType == "thinking" && m.streamContent.Len() > 0 {
			m.streamSegments = append(m.streamSegments, types.MessageSegment{
				Type:    "thinking",
				Content: m.streamContent.String(),
				Visible: true,
			})
			m.streamContent.Reset()
		}
		m.activeSegmentType = "content"
		m.thinking = false
		m.streamContent.WriteString(chunk.Delta)
	case "thinking":
		if m.activeSegmentType == "content" && m.streamContent.Len() > 0 {
			m.streamSegments = append(m.streamSegments, types.MessageSegment{
				Type:    "content",
				Content: m.streamContent.String(),
				Visible: true,
			})
			m.streamContent.Reset()
		}
		m.activeSegmentType = "thinking"
		m.thinking = true
		if m.thinkingStartAt.IsZero() {
			m.thinkingStartAt = time.Now()
		}

		if chunk.Delta != "" {
			m.streamContent.WriteString(chunk.Delta)
		}
	case "done":
	}

	m.renderMessages()
	m.viewport.GotoBottom()

	// Continuation: schedule next read from stream channel.
	// Must also watch streamDone so the cmd exits when the stream
	// goroutine terminates (normal completion or cancellation).
	streamCh := m.streamCh
	streamDone := m.streamDone
	streamCtx := m.streamCtx
	nextCmd := func() tea.Msg {
		select {
		case msg := <-streamCh:
			return msg
		case <-streamDone:
			// Stream goroutine exited — drain any remaining messages from
			// the channel, then return nil to stop the continuation chain.
			select {
			case msg := <-streamCh:
				return msg
			default:
				return nil
			}
		case <-streamCtx.Done():
			// Context cancelled (e.g. user pressed Ctrl+C) — stop continuation.
			return nil
		}
	}

	return []tea.Cmd{nextCmd}, false
}

func (m *ReplModel) handleStreamDoneMsg(msg StreamDoneMsg) ([]tea.Cmd, bool) {
	if m.streamContent.Len() > 0 {
		m.streamSegments = append(m.streamSegments, types.MessageSegment{
			Type:    "content",
			Content: m.streamContent.String(),
			Visible: true,
		})
		m.streamContent.Reset()
	}

	msg.Message.Segments = m.streamSegments
	if msg.Message.ToolCalls == nil || len(msg.Message.ToolCalls) == 0 {
		msg.Message.ToolCalls = m.getToolCallsFromSegments()
	}

	m.messages = append(m.messages, msg.Message)

	// Populate thinking blocks from finalized segments
	m.thinkingBlocks = make(map[int]*components.ThinkingBlock)
	for i, seg := range m.streamSegments {
		if seg.Type == "thinking" {
			tb := components.NewThinkingBlock(seg, m.theme, false, i)
			m.thinkingBlocks[i] = tb
		}
	}

	// Populate tool cards from tool calls
	m.toolCards = make(map[int]*components.ToolCard)
	for i, tc := range msg.Message.ToolCalls {
		card := components.NewToolCard(tc, nil, components.ToolRunning, m.theme)
		m.toolCards[i] = card
	}

	m.streaming = false
	m.thinking = false
	m.activeSegmentType = ""
	m.streamSegments = nil
	m.textarea.Focus()

	// Capture usage and cost from stream
	if msg.Usage != nil {
		m.lastUsage = msg.Usage
		if m.activeModel != nil {
			p := m.activeModel.Pricing
			m.lastCost = (p.InputPerMToken * float64(msg.Usage.PromptTokens) / 1e6) +
				(p.OutputPerMToken * float64(msg.Usage.CompletionTokens) / 1e6)
		}
	}

	m.renderMessages()
	m.viewport.GotoBottom()

	var cmds []tea.Cmd
	return cmds, true
}

func (m *ReplModel) handleStreamErrorMsg(msg StreamErrorMsg) ([]tea.Cmd, bool) {
	errMsg := types.Message{
		Role:    "assistant",
		Content: fmt.Sprintf("Error during streaming: %v", msg.Err),
		Segments: []types.MessageSegment{{
			Type:    "content",
			Content: fmt.Sprintf("Error during streaming: %v", msg.Err),
			Visible: true,
		}},
		CreatedAt: time.Now(),
	}
	m.messages = append(m.messages, errMsg)

	m.streaming = false
	m.thinking = false
	m.activeSegmentType = ""
	m.streamSegments = nil
	m.streamContent.Reset()
	m.thinkingBlocks = make(map[int]*components.ThinkingBlock)
	m.toolCards = make(map[int]*components.ToolCard)
	m.textarea.Focus()

	m.renderMessages()
	m.viewport.GotoBottom()

	var cmds []tea.Cmd
	return cmds, true
}

func (m *ReplModel) streamTickCmds() ([]tea.Cmd, bool) {
	var cmds []tea.Cmd
	cmds = append(cmds, StreamTickCmd())
	return cmds, false
}

func (m *ReplModel) getToolCallsFromSegments() []types.ToolCall {
	var toolCalls []types.ToolCall
	for _, seg := range m.streamSegments {
		if seg.Type == "tool_use" && seg.Content != "" {
			var tc types.ToolCall
			if err := json.Unmarshal([]byte(seg.Content), &tc); err == nil {
				toolCalls = append(toolCalls, tc)
			}
		}
	}
	return toolCalls
}
