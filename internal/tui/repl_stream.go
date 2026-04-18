package tui

import (
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"os"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// AppendStreamChunk appends a streamed token from a workflow phase to the REPL's streaming buffer.
func (m *ReplModel) AppendStreamChunk(chunk *types.StreamChunk) {
	if chunk == nil || chunk.Delta == "" {
		return
	}
	m.streaming = true
	m.streamContent.WriteString(chunk.Delta)
}

// handleStreamMsg processes a StreamMsg (token chunk) from the stream goroutine.
// Returns commands to continue the stream and a bool indicating "done".
func (m *ReplModel) handleStreamMsg(msg StreamMsg) []tea.Cmd {
	chunk := msg.Chunk
	if chunk == nil {
		return nil
	}

	m.streaming = true
	m.awaitingResponse = false

	switch chunk.Type {
	case "content":
		if m.activeSegmentType == "thinking" {
			m.closeActiveSegment()
		}
		m.activeSegmentType = "content"
		m.thinking = false
		m.streamContent.WriteString(chunk.Delta)
	case "thinking":
		if m.activeSegmentType == "content" {
			m.closeActiveSegment()
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
		// no-op
	}

	m.renderMessages()
	m.autoScrollConditionally()

	// Continuation: read next message from the channel
	nextCmd := func() tea.Msg {
		streamCh := m.streamCh
		msg, ok := <-streamCh
		if !ok {
			return nil
		}
		return msg
	}
	// Append streaming tick to drive 10fps rendering
	return []tea.Cmd{nextCmd, StreamTickCmd()}
}

// closeActiveSegment finalizes the current stream segment and appends it.
func (m *ReplModel) closeActiveSegment() {
	if m.streamContent.Len() == 0 {
		return
	}
	seg := types.MessageSegment{
		Type:    m.activeSegmentType,
		Content: m.streamContent.String(),
		Visible: true,
	}
	if m.activeSegmentType == "thinking" && !m.thinkingStartAt.IsZero() {
		seg.DurationMs = time.Since(m.thinkingStartAt).Milliseconds()
	}
	m.streamSegments = append(m.streamSegments, seg)
	m.streamContent.Reset()
}

// handleStreamDoneMsg finalizes the completed stream.
func (m *ReplModel) handleStreamDoneMsg(msg StreamDoneMsg) {
	m.closeActiveSegment()

	msg.Message.Segments = m.streamSegments
	if len(msg.Message.ToolCalls) == 0 {
		msg.Message.ToolCalls = m.getToolCallsFromSegments()
	}

	m.messages = append(m.messages, msg.Message)

	// Build thinking blocks from finalized segments
	m.thinkingBlocks = make(map[int]*components.ThinkingBlock)
	for i, seg := range m.streamSegments {
		if seg.Type == "thinking" {
			tb := components.NewThinkingBlock(seg, m.theme, false, i)
			if m.cfg == nil || !m.cfg.Model.ShowThinkingByDefault {
				tb.Toggle()
			}
			m.thinkingBlocks[i] = tb
		}
	}

	// Build tool cards from tool calls
	m.toolCards = make(map[int]*components.ToolCard)
	for i, tc := range msg.Message.ToolCalls {
		card := components.NewToolCard(tc, nil, components.ToolRunning, m.theme)
		if m.cfg != nil && m.cfg.Model.AutoCollapseTools {
			card.SetCollapsed(true)
		}
		m.toolCards[i] = card
	}

	m.streaming = false
	m.thinking = false
	m.activeSegmentType = ""
	m.streamSegments = nil
	m.thinkingStartAt = time.Time{}
	m.textarea.Focus()

	if msg.Usage != nil {
		m.lastUsage = msg.Usage
		if m.activeModel != nil {
			p := m.activeModel.Pricing
			m.lastCost = (p.InputPerMToken * float64(msg.Usage.PromptTokens) / 1e6) +
				(p.OutputPerMToken * float64(msg.Usage.CompletionTokens) / 1e6)
		}
	}

	m.renderMessages()
	m.autoScrollConditionally()
}

// renderErrorBanner returns a styled error message based on the typed sentinel.
func renderErrorBanner(err error, t theme.Theme, providerName string) string {
	providerSuffix := ""
	if providerName != "" {
		providerSuffix = fmt.Sprintf(" (%s)", providerName)
	}
	switch {
	case errors.Is(err, m31errors.ErrContextExceeded):
		return lipgloss.NewStyle().Foreground(t.Warning).Bold(true).
			Render("⚠ Context window exceeded. Use /compress to free space.")
	case errors.Is(err, m31errors.ErrInvalidKey):
		return lipgloss.NewStyle().Foreground(t.Error).Bold(true).
			Render(fmt.Sprintf("✗ Invalid API key%s. Run /settings to update.", providerSuffix))
	case errors.Is(err, m31errors.ErrRateLimited):
		return lipgloss.NewStyle().Foreground(t.Warning).Bold(true).
			Render(fmt.Sprintf("⚠ Rate limited%s. Auto-fallback in progress or retry in a moment.", providerSuffix))
	case errors.Is(err, m31errors.ErrProviderUnreachable):
		return lipgloss.NewStyle().Foreground(t.Warning).Bold(true).
			Render(fmt.Sprintf("⚠ Provider unreachable%s — check connection or try /fallback.", providerSuffix))
	case errors.Is(err, m31errors.ErrStreamTruncated):
		return lipgloss.NewStyle().Foreground(t.Warning).Bold(true).
			Render("⚠ Stream interrupted — try sending your message again.")
	case errors.Is(err, m31errors.ErrModelNotFound):
		return lipgloss.NewStyle().Foreground(t.Error).Bold(true).
			Render(fmt.Sprintf("✗ Model not found%s — run /models to see available models.", providerSuffix))
	default:
		return lipgloss.NewStyle().Foreground(t.Error).Bold(true).
			Render("✗ " + m31errors.UserMessage(err))
	}
}

// plainErrorBanner returns the same textual content as renderErrorBanner but
// without any lipgloss styling. Callers that persist the banner as a message
// segment should use this variant so ANSI escape codes are applied once at
// render time (avoiding glamour's markdown pipeline mangling raw ANSI).
func plainErrorBanner(err error, providerName string) string {
	providerSuffix := ""
	if providerName != "" {
		providerSuffix = fmt.Sprintf(" (%s)", providerName)
	}
	switch {
	case errors.Is(err, m31errors.ErrContextExceeded):
		return "⚠ Context window exceeded. Use /compress to free space."
	case errors.Is(err, m31errors.ErrInvalidKey):
		return fmt.Sprintf("✗ Invalid API key%s. Run /settings to update.", providerSuffix)
	case errors.Is(err, m31errors.ErrRateLimited):
		return fmt.Sprintf("⚠ Rate limited%s. Auto-fallback in progress or retry in a moment.", providerSuffix)
	case errors.Is(err, m31errors.ErrProviderUnreachable):
		return fmt.Sprintf("⚠ Provider unreachable%s — check connection or try /fallback.", providerSuffix)
	case errors.Is(err, m31errors.ErrStreamTruncated):
		return "⚠ Stream interrupted — try sending your message again."
	case errors.Is(err, m31errors.ErrModelNotFound):
		return fmt.Sprintf("✗ Model not found%s — run /models to see available models.", providerSuffix)
	default:
		return "✗ " + m31errors.UserMessage(err)
	}
}

// typedErrorName returns the sentinel name for debug logging.
func typedErrorName(err error) string {
	switch {
	case errors.Is(err, m31errors.ErrContextExceeded):
		return "ErrContextExceeded"
	case errors.Is(err, m31errors.ErrInvalidKey):
		return "ErrInvalidKey"
	case errors.Is(err, m31errors.ErrRateLimited):
		return "ErrRateLimited"
	case errors.Is(err, m31errors.ErrProviderUnreachable):
		return "ErrProviderUnreachable"
	case errors.Is(err, m31errors.ErrModelNotFound):
		return "ErrModelNotFound"
	case errors.Is(err, m31errors.ErrToolExecution):
		return "ErrToolExecution"
	case errors.Is(err, m31errors.ErrPermissionDenied):
		return "ErrPermissionDenied"
	default:
		return "unknown"
	}
}

// handleStreamErrorMsg processes a StreamErrorMsg.
func (m *ReplModel) handleStreamErrorMsg(msg StreamErrorMsg) {
	if os.Getenv("M31A_LOG_LEVEL") == "debug" {
		slog.Debug("stream error",
			"typed", typedErrorName(msg.Err),
			"provider", msg.ProviderName,
			"message", msg.Err.Error())
	}

	m.messages = append(m.messages, makeErrorBannerMsg(plainErrorBanner(msg.Err, msg.ProviderName), msg.ProviderName))

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
}

// getToolCallsFromSegments extracts tool calls from the segments list.
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
