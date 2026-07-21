package tui

import (
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

func TestReplWidth(t *testing.T) {
	m := ReplModel{width: 80, sidebarWidth: 0}
	if got := m.replWidth(); got != 80 {
		t.Errorf("want 80, got %d", got)
	}
	m.sidebarWidth = 30
	if got := m.replWidth(); got != 50 {
		t.Errorf("want 50, got %d", got)
	}
	m.width = 10
	m.sidebarWidth = 20
	if got := m.replWidth(); got != 20 {
		t.Errorf("want 20 (min clamp), got %d", got)
	}
}

func TestReplSettersGetters(t *testing.T) {
	rm := ReplModel{}

	rm.SetDispatcher(nil)
	rm.SetCommandRegistry(nil)
	rm.SetFrecentHistory(nil)
	rm.SetKeyRegistry(nil)
	rm.SetSessionSparkline("abc")
	if rm.sessionSparkline != "abc" {
		t.Error("sessionSparkline not set")
	}

	now := time.Now()
	rm.SetLastActivity(now)
	if !rm.lastActivity.Equal(now) {
		t.Error("lastActivity not set")
	}

	rm.SetStreaming(true)
	if !rm.streaming {
		t.Error("streaming not set")
	}
	rm.SetThinking(true)
	if !rm.thinking {
		t.Error("thinking not set")
	}

	rm.SetSessionID("sess-123")
	if rm.sessionID != "sess-123" {
		t.Error("sessionID not set")
	}

	rm.SetChangedFiles(5)
	if rm.changedFiles != 5 {
		t.Error("changedFiles not set")
	}
}

func TestReplSetCwd(t *testing.T) {
	rm := ReplModel{cwd: "/old"}
	rm.mentionCompleter = &MentionCompleter{}
	rm.SetCwd("/new")
	if rm.cwd != "/new" {
		t.Error("cwd not set")
	}
	if rm.mentionCompleter != nil {
		t.Error("mentionCompleter should be nil after cwd change")
	}

	rm.mentionCompleter = &MentionCompleter{}
	rm.SetCwd("/new")
	if rm.mentionCompleter == nil {
		t.Error("mentionCompleter should not be nil when cwd unchanged")
	}
}

func TestReplGetStatusText(t *testing.T) {
	rm := ReplModel{}
	if got := rm.GetStatusText(); got != "" {
		t.Errorf("empty: want '', got %q", got)
	}
	rm.streaming = true
	if got := rm.GetStatusText(); got != "streaming…" {
		t.Errorf("streaming: want 'streaming…', got %q", got)
	}
	rm.streaming = false
	rm.thinking = true
	if got := rm.GetStatusText(); got != "thinking…" {
		t.Errorf("thinking: want 'thinking…', got %q", got)
	}
	rm.thinking = false
	rm.lastStatus = "Building..."
	if got := rm.GetStatusText(); got != "Building..." {
		t.Errorf("lastStatus: want 'Building...', got %q", got)
	}
}

func TestReplLastUsageCost(t *testing.T) {
	rm := ReplModel{}
	if rm.LastUsage() != nil {
		t.Error("want nil usage")
	}
	if rm.LastCost() != 0 {
		t.Error("want 0 cost")
	}
	usage := &types.Usage{PromptTokens: 10}
	rm.lastUsage = usage
	rm.lastCost = 0.5
	if rm.LastUsage() != usage {
		t.Error("usage mismatch")
	}
	if rm.LastCost() != 0.5 {
		t.Error("cost mismatch")
	}
}

func TestReplMessages(t *testing.T) {
	rm := ReplModel{}
	if rm.Messages() != nil {
		t.Error("want nil messages")
	}
	msgs := []types.Message{
		{Role: "user", Content: "hello"},
		{Role: "assistant", Content: "world"},
	}
	rm.messages = msgs
	if len(rm.Messages()) != 2 {
		t.Error("want 2 messages")
	}
}

func TestReplSetTheme(t *testing.T) {
	rm := ReplModel{theme: testTheme()}
	rm.msgRenderer = nil
	rm.SetTheme(testTheme())
	if rm.msgRenderer != nil {
		t.Error("should not create renderer when nil")
	}
}

func TestHandleProviderModelsFetchedNil(t *testing.T) {
	rm := ReplModel{modelValid: true}
	rm.handleProviderModelsFetched(ProviderModelsFetchedMsg{Err: nil, Model: nil})
	if !rm.modelValid {
		t.Error("should stay valid on nil model")
	}
}

func TestHandleProviderModelsFetchedNoMatch(t *testing.T) {
	rm := ReplModel{
		modelValid: true,
		registry:   nil,
	}
	rm.handleProviderModelsFetched(ProviderModelsFetchedMsg{
		Models: []types.ModelInfo{{ID: "a"}, {ID: "b"}},
		Model:  &types.ModelInfo{ID: "nonexistent"},
	})
	if rm.modelValid {
		t.Error("should be invalid when model not found")
	}
}

func TestHandleProviderModelsFetchedMatch(t *testing.T) {
	target := types.ModelInfo{ID: "target", Name: "Target"}
	rm := ReplModel{
		registry: nil,
	}
	rm.handleProviderModelsFetched(ProviderModelsFetchedMsg{
		Models: []types.ModelInfo{target},
		Model:  &types.ModelInfo{ID: "target"},
	})
	if rm.activeModel == nil || rm.activeModel.ID != "target" {
		t.Error("should set activeModel to matched model")
	}
}
