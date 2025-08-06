package tui

import (
	"sync/atomic"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// --- H-8: Segment transition tests ---

func TestSegmentTransition_NoRace(t *testing.T) {
	m := NewReplModel(theme.Dark())
	m.width = 120
	m.height = 40
	m.activeSegmentType = "thinking"

	// Feed alternating thinking/content chunks — enough to exercise transitions
	// without triggering expensive lipgloss rendering per iteration.
	for i := 0; i < 20; i++ {
		var chunkType string
		if i%2 == 0 {
			chunkType = "thinking"
		} else {
			chunkType = "content"
		}
		m.Update(StreamMsg{Chunk: &types.StreamChunk{
			Type:  chunkType,
			Delta: "x",
		}})
	}

	// Simulate stream done
	m.Update(StreamDoneMsg{})

	if m.activeSegmentType != "" {
		t.Errorf("expected activeSegmentType empty after Done, got %q", m.activeSegmentType)
	}
	// Segments are moved to the message's Segments field after Done
	if len(m.messages) == 0 {
		t.Error("expected at least one message after Done")
	} else if len(m.messages[0].Segments) == 0 {
		t.Error("expected at least one segment in message after Done")
	}
}

func TestSegmentTransition_StuckThinking(t *testing.T) {
	m := NewReplModel(theme.Dark())
	m.width = 120
	m.height = 40
	m.activeSegmentType = "thinking"

	// Send a thinking chunk then Done (no content chunk)
	m.Update(StreamMsg{Chunk: &types.StreamChunk{
		Type:  "thinking",
		Delta: "let me think",
	}})
	m.Update(StreamDoneMsg{})

	// The thinking segment should be closed and moved to the message
	if len(m.messages) == 0 {
		t.Fatal("no message after Done")
	}
	found := false
	for _, seg := range m.messages[0].Segments {
		if seg.Type == "thinking" {
			found = true
			if seg.Content != "let me think" {
				t.Errorf("thinking content = %q, want %q", seg.Content, "let me think")
			}
			// Duration should be >= 0 (we just want to confirm it was stamped)
			if seg.DurationMs < 0 {
				t.Errorf("thinking duration = %d, want >= 0", seg.DurationMs)
			}
		}
	}
	if !found {
		t.Error("no thinking segment found after Done; segment was stuck")
	}
}

func TestStreamShutdown_OrderingCancelFirst(t *testing.T) {
	// Verify that StartStreamCmd's cleanup goroutine drains the channel
	// before closing it. We simulate the pattern: send chunks, then
	// verify all chunks arrive before the channel close.
	streamCh := make(chan types.StreamChunk, 10)
	done := make(chan struct{})
	var received atomic.Int32

	// Simulate the drain pattern from StartStreamCmd
	go func() {
		<-done // wait for signal
		for range streamCh {
			received.Add(1)
		}
	}()

	// Send 5 chunks
	for i := 0; i < 5; i++ {
		streamCh <- types.StreamChunk{Type: "content", Delta: "x"}
	}

	// Signal done and close channel (same order as StartStreamCmd)
	close(done)
	close(streamCh)

	// Give the goroutine time to drain
	time.Sleep(50 * time.Millisecond)

	if got := received.Load(); got != 5 {
		t.Errorf("received %d chunks, want 5 (chunks lost during shutdown)", got)
	}
}

// --- H-10/M-26: Header cache tests ---

func TestHeaderCache_HitReturnsSameString(t *testing.T) {
	app := &AppState{
		activeProvider: "openrouter",
		activeModel: &types.ModelInfo{
			ID:   "test-model",
			Name: "Test Model",
		},
		healthStatus: types.HealthStatus{Status: "live"},
		width:        120,
		themeManager: theme.NewManager(theme.ModeDark),
	}

	first := app.CachedRenderHeader(1000, 100000)
	second := app.CachedRenderHeader(1000, 100000)

	if first != second {
		t.Error("CachedRenderHeader returned different strings on second call (cache miss)")
	}
	if !app.headerCacheValid {
		t.Error("headerCacheValid should be true after first call")
	}
}

func TestHeaderCache_InvalidatedOnModelChange(t *testing.T) {
	app := &AppState{
		activeProvider: "openrouter",
		activeModel: &types.ModelInfo{
			ID:   "model-a",
			Name: "Model A",
		},
		healthStatus: types.HealthStatus{Status: "live"},
		width:        120,
		themeManager: theme.NewManager(theme.ModeDark),
	}

	first := app.CachedRenderHeader(1000, 100000)

	// Change model
	app.activeModel = &types.ModelInfo{
		ID:   "model-b",
		Name: "Model B",
	}

	second := app.CachedRenderHeader(1000, 100000)

	if first == second {
		t.Error("CachedRenderHeader returned same string after model change (cache not invalidated)")
	}
}

func TestHeaderCache_InvalidatedOnHealthChange(t *testing.T) {
	app := &AppState{
		activeProvider: "openrouter",
		activeModel: &types.ModelInfo{
			ID:   "test-model",
			Name: "Test Model",
		},
		healthStatus: types.HealthStatus{Status: "live"},
		width:        120,
		themeManager: theme.NewManager(theme.ModeDark),
	}

	first := app.CachedRenderHeader(1000, 100000)

	// Change health status
	app.healthStatus = types.HealthStatus{Status: "offline"}

	second := app.CachedRenderHeader(1000, 100000)

	if first == second {
		t.Error("CachedRenderHeader returned same string after health change (cache not invalidated)")
	}
}

func TestHeaderCache_KeyIncludesLogLevel(t *testing.T) {
	app := &AppState{
		activeProvider: "openrouter",
		activeModel: &types.ModelInfo{
			ID:   "test-model",
			Name: "Test Model",
		},
		healthStatus: types.HealthStatus{Status: "live"},
		width:        120,
		themeManager: theme.NewManager(theme.ModeDark),
	}

	// Set log level to debug
	t.Setenv("M31A_LOG_LEVEL", "debug")
	_ = app.CachedRenderHeader(1000, 100000)
	firstKey := app.headerCacheKey

	// Set log level to info
	t.Setenv("M31A_LOG_LEVEL", "info")
	app.headerCacheValid = false // force recomputation
	_ = app.CachedRenderHeader(1000, 100000)
	secondKey := app.headerCacheKey

	if firstKey == secondKey {
		t.Errorf("cache key did not change when M31A_LOG_LEVEL changed (both = %d)", firstKey)
	}
}

// --- H-13: Verify model single-task selection tests ---

func TestVerify_HealKey_SingleTask(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "task-1", Status: types.StatusFailed},
	}
	m := NewVerifyModel(tasks, nil, theme.Dark(), 120, 40)
	m.selected = 0

	// First 'h' sets confirmation gate
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'h'}})
	if m.tasks[0].Status != types.StatusFailed {
		t.Errorf("first 'h' should set confirmation gate, got status = %v", m.tasks[0].Status)
	}

	// 'y' confirms self-heal
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'y'}})

	if m.tasks[0].Status != types.StatusPending {
		t.Errorf("task status = %v, want StatusPending (heal should reset failed task)", m.tasks[0].Status)
	}
}

func TestVerify_SkipKey_SingleTask(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "task-1", Status: types.StatusFailed},
	}
	m := NewVerifyModel(tasks, nil, theme.Dark(), 120, 40)
	m.selected = 0

	// Send 's' key
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'s'}})

	if m.tasks[0].Status != types.StatusSkipped {
		t.Errorf("task status = %v, want StatusSkipped", m.tasks[0].Status)
	}
}

func TestVerify_HealKey_OutOfRange(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Description: "task-1", Status: types.StatusFailed},
	}
	m := NewVerifyModel(tasks, nil, theme.Dark(), 120, 40)
	m.selected = 5 // out of range

	// Should not panic
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'h'}})

	// Task status should be unchanged
	if m.tasks[0].Status != types.StatusFailed {
		t.Errorf("task status changed unexpectedly = %v", m.tasks[0].Status)
	}
}
