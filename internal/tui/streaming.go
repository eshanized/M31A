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
// never creates or disposes channels — they are allocated and closed within
// this function. The goroutine owns the write side; the returned channel
// is the read side for the BT update loop's continuation cmd.
//
// Fix M-21: The goroutine closes streamCh when it exits. There is no
// separate streamDone channel — closing streamCh IS the done signal.
// The cmd reads from streamCh; when closed, reads return the zero value
// (nil for tea.Msg interface), which we detect and return as nil to stop
// the Bubble Tea continuation chain.
//
// Double-invocation is safe: each call allocates fresh channels.
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

		var segments []types.MessageSegment
		var activeContent strings.Builder
		var lastUsage *types.Usage

		for {
			chunk, err := iterator.Next()
			if err == io.EOF {
				if activeContent.Len() > 0 {
					segments = append(segments, types.MessageSegment{
						Type:    "content",
						Content: activeContent.String(),
						Visible: true,
					})
				}

				var fullContent strings.Builder
				for _, seg := range segments {
					if seg.Type == "content" {
						fullContent.WriteString(seg.Content)
					}
				}

				msg := types.Message{
					Role:     "assistant",
					Content:  fullContent.String(),
					Segments: segments,
					Usage:    lastUsage,
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

			switch chunk.Type {
			case "content":
				activeContent.WriteString(chunk.Delta)
			case "thinking":
				if activeContent.Len() > 0 {
					segments = append(segments, types.MessageSegment{
						Type:    "content",
						Content: activeContent.String(),
						Visible: true,
					})
					activeContent.Reset()
				}
				segments = append(segments, types.MessageSegment{
					Type:       "thinking",
					Content:    chunk.Delta,
					DurationMs: chunk.ThinkingDuration,
					Visible:    true,
				})
			case "done":
				lastUsage = &types.Usage{}
			}
		}
	}()

	// Fix C-3: the cmd reads from streamCh. When the goroutine closes
	// streamCh, the channel read returns the zero value (nil for tea.Msg
	// interface), which we return as nil to stop the Bubble Tea cmd chain.
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
