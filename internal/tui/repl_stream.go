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
	}

	m.renderMessages()
	m.viewport.GotoBottom()

	// Fix C-3: continuation cmd reads the next message from the goroutine's
	// channel. StartStreamCmd owns the channel (allocated internally, closed
	// by the goroutine); the REPL stores a read-only reference (m.streamCh)
	// for continuation only. When the goroutine closes streamCh, the next
	// read returns nil and stops.
	// Fix H-14: the cmd is a pure read — no shared mutable state between the
	// streaming goroutine and the BT update loop. All data crosses via tea.Msg
	// values. streamContent and streamSegments live on ReplModel and are only
	// mutated by the BT update loop (this handler), never by the goroutine.
	nextCmd := func() tea.Msg {
		// Fix C-3: m.streamCh is a read-only reference to the channel owned
		// by StartStreamCmd's goroutine. We capture the reference (not the
		// value) so we read from the same channel the goroutine writes to.
		// When the goroutine closes streamCh, msg will be the zero value and
		// ok will be false — we return nil to stop the Bubble Tea cmd chain.
		streamCh := m.streamCh
		msg, ok := <-streamCh
		if !ok {
			return nil
		}
		return msg
	}

	return []tea.Cmd{nextCmd}, false
}

// closeActiveSegment finalizes the current stream segment and appends it to
// the segment list. It is called before switching segment types (H-8 fix)
// and when the stream completes. The method is pure — no goroutines, no
// external state — safe within the single-threaded BT update loop.
func (m *ReplModel) closeActiveSegment() {
	if m.streamContent.Len() == 0 {
		return
	}
	seg := types.MessageSegment{
		Type:    m.activeSegmentType,
		Content: m.streamContent.String(),
		Visible: true,
	}
	// Stamp thinking duration if we have a start time
	if m.activeSegmentType == "thinking" && !m.thinkingStartAt.IsZero() {
		seg.DurationMs = time.Since(m.thinkingStartAt).Milliseconds()
	}
	m.streamSegments = append(m.streamSegments, seg)
	m.streamContent.Reset()
}

func (m *ReplModel) handleStreamDoneMsg(msg StreamDoneMsg) ([]tea.Cmd, bool) {
	// H-8: Close any active segment before finalizing the message.
	m.closeActiveSegment()

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
		// M-17: honor AutoCollapseTools config flag
		if m.cfg != nil && m.cfg.Model.AutoCollapseTools {
			card.SetCollapsed(true)
		}
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

// renderErrorBanner returns a styled error message based on the typed sentinel.
// H-11: distinct banners for known error types.
func renderErrorBanner(err error) string {
	switch {
	case errors.Is(err, m31errors.ErrContextExceeded):
		return lipgloss.NewStyle().Foreground(lipgloss.Color("#FDD663")).Bold(true).
			Render("⚠ Context window exceeded. Use /compress to free space.")
	case errors.Is(err, m31errors.ErrInvalidKey):
		return lipgloss.NewStyle().Foreground(lipgloss.Color("#F28B82")).Bold(true).
			Render("✗ Invalid API key. Run /settings to update.")
	case errors.Is(err, m31errors.ErrRateLimited):
		return lipgloss.NewStyle().Foreground(lipgloss.Color("#FDD663")).Bold(true).
			Render("⚠ Rate limited. Auto-fallback in progress…")
	default:
		return lipgloss.NewStyle().Foreground(lipgloss.Color("#F28B82")).Bold(true).
			Render(fmt.Sprintf("✗ Error: %v", err))
	}
}

// typedErrorName walks the error chain and returns the sentinel name.
// M-24: used in debug-mode logging so operators can see which sentinel fired.
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

func (m *ReplModel) handleStreamErrorMsg(msg StreamErrorMsg) ([]tea.Cmd, bool) {
	// M-24: log typed sentinel name in debug mode
	if os.Getenv("M31A_LOG_LEVEL") == "debug" {
		slog.Debug("stream error",
			"typed", typedErrorName(msg.Err),
			"message", msg.Err.Error())
	}

	// H-11: render styled banner for known error types
	banner := renderErrorBanner(msg.Err)

	errMsg := types.Message{
		Role:    "assistant",
		Content: banner,
		Segments: []types.MessageSegment{{
			Type:    "content",
			Content: banner,
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
