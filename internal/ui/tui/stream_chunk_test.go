package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

// TestStreamChunkMsgHandled verifies that Update() routes StreamChunkMsg to the REPL
// and appends the chunk content to the streaming buffer.
func TestStreamChunkMsgHandled(t *testing.T) {
	var rm ReplModel
	rm.Init()
	m := &AppState{
		replModel: &rm,
	}

	chunk := &types.StreamChunk{
		Type:  "content",
		Delta: "hello world",
	}
	msg := StreamChunkMsg{Chunk: chunk}

	_, _ = m.Update(msg)

	// Verify the chunk was appended to the REPL streaming buffer
	if rm.streamContent.Len() == 0 {
		t.Error("expected streamContent to have content after StreamChunkMsg")
	}
	if got := rm.streamContent.String(); got != "hello world" {
		t.Errorf("streamContent = %q, want %q", got, "hello world")
	}
}

// TestStreamChunkMsgNilModel verifies StreamChunkMsg with nil replModel doesn't panic.
func TestStreamChunkMsgNilModel(t *testing.T) {
	m := &AppState{
		replModel: nil,
	}

	chunk := &types.StreamChunk{
		Type:  "content",
		Delta: "hello",
	}
	msg := StreamChunkMsg{Chunk: chunk}

	// Should not panic
	_, _ = m.Update(msg)
}

// TestStreamChunkMsgNilChunk verifies StreamChunkMsg with nil chunk doesn't panic.
func TestStreamChunkMsgNilChunk(t *testing.T) {
	var rm ReplModel
	rm.Init()
	m := &AppState{
		replModel: &rm,
	}

	msg := StreamChunkMsg{Chunk: nil}

	// Should not panic
	_, _ = m.Update(msg)
}
