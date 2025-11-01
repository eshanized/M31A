// Package tui implements the Bubble Tea TUI for M31A.
//
// STREAMING PIPELINE — GOROUTINE OWNERSHIP MODEL
//
// Architecture: Bubble Tea is single-threaded. All state mutations go through Update().
// The streaming pipeline uses one goroutine per active stream that reads SSE chunks
// and sends typed tea.Msg values through a channel. The TUI's Update() loop receives
// these messages and updates state.
//
// Channel lifecycle (per stream):
//  1. StartStreamCmd creates a buffered channel (chan tea.Msg, cap 64)
//  2. A goroutine reads from the provider's StreamIterator and sends to the channel
//  3. A tea.Cmd (returned to Update()) reads from the channel and returns messages
//  4. When the stream ends (EOF or error), the goroutine closes the channel
//  5. The cmd detects the closed channel and returns nil to stop the chain
//
// CRITICAL RULES:
//   - The goroutine OWNS the channel; it creates and closes it
//   - The TUI only READS from the channel; never closes it
//   - No AppState mutation happens in the goroutine; only tea.Msg emission
package tui

import (
	"context"
	"io"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

// ─── Stream message types ─────────────────────────────────────────────────────

// StreamMsg carries a single streaming chunk from the LLM.
type StreamMsg struct {
	Chunk     *types.StreamChunk
	ModelID   string
	SessionID string
}

// StreamDoneMsg signals that the stream completed successfully.
type StreamDoneMsg struct {
	Message   types.Message
	Usage     *types.Usage
	ModelID   string
	SessionID string
}

// StreamErrorMsg signals that the stream terminated with an error.
type StreamErrorMsg struct {
	Err     error
	ModelID string
}

// TickMsg drives streaming render ticks at ~10fps.
type TickMsg struct {
	Time time.Time
}

// ─── Streaming commands ───────────────────────────────────────────────────────

// StartStreamCmd starts a streaming LLM request in a goroutine.
// The goroutine owns the channel; it creates it, writes to it, and closes it.
// The returned tea.Cmd reads one message per invocation; continuation is
// managed by the caller returning a new cmd from Update().
// The returned read-only channel reference allows the REPL to issue
// continuation reads in handleStreamMsg.
func StartStreamCmd(ctx context.Context, p provider.LLMProvider, req provider.ChatRequest, sessionID string) (tea.Cmd, <-chan tea.Msg) {
	streamCh := make(chan tea.Msg, 64)

	go func() {
		// Goroutine owns streamCh: close it when the goroutine exits.
		// The cmd reader detects closure and returns nil to stop the chain.
		defer close(streamCh)

		iterator, err := p.ChatCompletionStream(ctx, req)
		if err != nil {
			streamCh <- StreamErrorMsg{Err: err, ModelID: req.Model}
			return
		}

		// Unblock iterator.Next() on context cancellation.
		done := make(chan struct{})
		go func() {
			select {
			case <-ctx.Done():
				iterator.Close()
			case <-done:
			}
		}()
		defer func() {
			iterator.Close()
			close(done)
		}()

		var fullContent strings.Builder
		var lastUsage *types.Usage

		for {
			chunk, err := iterator.Next()
			if err == io.EOF {
				msg := types.Message{
					Role:    "assistant",
					Content: fullContent.String(),
					Usage:   lastUsage,
				}
				streamCh <- StreamDoneMsg{
					Message:   msg,
					Usage:     lastUsage,
					ModelID:   req.Model,
					SessionID: sessionID,
				}
				return
			}
			if err != nil {
				streamCh <- StreamErrorMsg{Err: err, ModelID: req.Model}
				return
			}
			if chunk == nil {
				continue
			}

			select {
			case streamCh <- StreamMsg{
				Chunk:     chunk,
				ModelID:   req.Model,
				SessionID: sessionID,
			}:
			case <-ctx.Done():
				return
			}

			if chunk.Usage != nil {
				lastUsage = chunk.Usage
			}
			if chunk.Type == "content" {
				fullContent.WriteString(chunk.Delta)
			}
		}
	}()

	// The initial cmd reads the first message from streamCh.
	// When the goroutine closes streamCh, reads return nil (zero value),
	// which we return as nil to stop the Bubble Tea cmd chain.
	cmd := func() tea.Msg {
		msg, ok := <-streamCh
		if !ok {
			return nil
		}
		return msg
	}

	return cmd, streamCh
}

// StreamTickCmd returns a tea.Cmd that emits a TickMsg at ~10fps.
// Used to drive streaming render updates.
func StreamTickCmd() tea.Cmd {
	return tea.Tick(time.Second/10, func(t time.Time) tea.Msg {
		return TickMsg{Time: t}
	})
}
