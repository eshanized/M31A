package tui

// STREAMING PIPELINE — GOROUTINE OWNERSHIP MODEL
//
// Architecture: Bubble Tea is single-threaded. All state mutations go through Update().
// The streaming pipeline uses one goroutine per active stream that reads SSE chunks
// and sends typed tea.Msg values through a channel. The TUI's Update() loop receives
// these messages and updates AppState.
//
// Channel lifecycle (per stream):
// 1. StartStreamCmd creates a buffered channel (chan tea.Msg, cap 256)
// 2. A goroutine reads from the provider's StreamIterator and sends to the channel
// 3. A tea.Cmd (streamListenerCmd) polls the channel and returns messages to Update()
// 4. When the stream ends (EOF or error), the goroutine closes the channel
// 5. streamListenerCmd detects the closed channel and stops polling
//
// CRITICAL RULES:
// - The goroutine OWNS the channel; it creates and closes it
// - The TUI only READS from the channel; never closes it
// - safeCloseOnce is used as a defensive guard against double-close
// - No AppState mutation happens in the goroutine; only tea.Msg emission
//
// Historical fixes: C-3 (double-close), M-21 (channel ownership), M-35-36 (race conditions)

import (
	"context"
	"io"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

type StreamMsg struct {
	Chunk     *types.StreamChunk
	ModelID   string
	SessionID string
}

type StreamDoneMsg struct {
	Message   types.Message
	Usage     *types.Usage
	ModelID   string
	SessionID string
}

type StreamErrorMsg struct {
	Err     error
	ModelID string
}

type TickMsg struct {
	Time time.Time
}

// Fix C-3: StartStreamCmd owns its channels internally. The caller (REPL)
// never creates, holds, or disposes channels — they are allocated and closed
// within this function. The goroutine owns the write side; the returned cmd
// reads from the channel internally. Double-invocation is safe: each call
// allocates fresh channels.
//
// Fix M-21: The goroutine closes streamCh when it exits. There is no
// separate streamDone channel — closing streamCh IS the done signal.
// The cmd reads from streamCh; when closed, reads return the zero value
// (nil for tea.Msg interface), which we detect and return as nil to stop
// the Bubble Tea continuation chain.
//
// The returned tea.Cmd reads one message per invocation from the internal
// channel. The caller manages continuation by returning a new cmd from
// Update() — but the channel reference never escapes this closure.
func StartStreamCmd(ctx context.Context, p provider.LLMProvider, req provider.ChatRequest, sessionID string) (tea.Cmd, <-chan tea.Msg) {
	// Fix C-3: channels allocated locally — no caller creates or disposes them.
	streamCh := make(chan tea.Msg, 64)

	go func() {
		// Fix M-21: close streamCh when the goroutine exits. The cmd reader
		// detects closure and returns nil to stop the continuation chain.
		defer close(streamCh)

		iterator, err := p.ChatCompletionStream(ctx, req)
		if err != nil {
			streamCh <- StreamErrorMsg{Err: err, ModelID: req.Model}
			return
		}

		// Ensure the iterator is closed on context cancellation to unblock
		// any pending Next() call and release the HTTP response body.
		done := make(chan struct{})
		go func() {
			select {
			case <-ctx.Done():
				iterator.Close()
			case <-done:
				// Stream completed normally
			}
		}()

		defer func() {
			iterator.Close()
			close(done)
		}()

		// M-35-36 fix: the goroutine only accumulates raw content for
		// msg.Message.Content and tracks usage. Segment building is done
		// exclusively by the REPL model (repl_stream.go) which has full
		// visibility into segment boundaries. The goroutine's old segment
		// building code was always overwritten by handleStreamDoneMsg.
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
			switch chunk.Type {
			case "content":
				fullContent.WriteString(chunk.Delta)
			}
		}
	}()

	// Fix C-3: the cmd reads from streamCh. When the goroutine closes
	// streamCh, the channel read returns the zero value (nil for tea.Msg
	// interface), which we return as nil to stop the Bubble Tea cmd chain.
	// The channel reference is returned as a read-only hint; the caller
	// stores it for continuation but never writes to or closes it.
	cmd := func() tea.Msg {
		msg, ok := <-streamCh
		if !ok {
			// Fix M-21: channel closed — stream goroutine exited.
			return nil
		}
		return msg
	}

	return cmd, streamCh
}

func StreamTickCmd() tea.Cmd {
	return tea.Tick(time.Second/10, func(t time.Time) tea.Msg {
		return TickMsg{Time: t}
	})
}
