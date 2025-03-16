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

func StartStreamCmd(ctx context.Context, p provider.LLMProvider, req provider.ChatRequest, sessionID string) tea.Cmd {
	streamCh := make(chan tea.Msg, 100)

	go func() {
		defer close(streamCh)

		iterator, err := p.ChatCompletionStream(ctx, req)
		if err != nil {
			streamCh <- StreamErrorMsg{Err: err, ModelID: req.Model}
			return
		}
		defer iterator.Close()

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

	return func() tea.Msg {
		return <-streamCh
	}
}

func StreamTickCmd() tea.Cmd {
	return tea.Tick(time.Second/10, func(t time.Time) tea.Msg {
		return TickMsg{Time: t}
	})
}
