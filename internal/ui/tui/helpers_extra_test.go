package tui

import (
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/core/types"
)

func TestActiveModelID(t *testing.T) {
	tests := []struct {
		name  string
		model *types.ModelInfo
		want  string
	}{
		{"nil", nil, ""},
		{"empty", &types.ModelInfo{}, ""},
		{"with ID", &types.ModelInfo{ID: "gpt-4"}, "gpt-4"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := activeModelID(tt.model); got != tt.want {
				t.Errorf("activeModelID() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestMakeAssistantMsg(t *testing.T) {
	msg := MakeAssistantMsg("hello")

	if msg.Role != "assistant" {
		t.Errorf("Role = %q, want %q", msg.Role, "assistant")
	}
	if msg.Content != "hello" {
		t.Errorf("Content = %q, want %q", msg.Content, "hello")
	}
	if len(msg.Segments) != 1 {
		t.Fatalf("Segments length = %d, want 1", len(msg.Segments))
	}
	if msg.Segments[0].Type != "content" {
		t.Errorf("Segment Type = %q, want %q", msg.Segments[0].Type, "content")
	}
	if msg.Segments[0].Content != "hello" {
		t.Errorf("Segment Content = %q, want %q", msg.Segments[0].Content, "hello")
	}
	if !msg.Segments[0].Visible {
		t.Error("Segment Visible = false, want true")
	}
	if msg.CreatedAt.IsZero() {
		t.Error("CreatedAt is zero")
	}
}

func TestMakeErrorBannerMsg(t *testing.T) {
	tests := []struct {
		name     string
		text     string
		provider string
		want     string
	}{
		{"with provider", "error occurred", "openai", "error occurred (openai)"},
		{"no provider", "error occurred", "", "error occurred"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			msg := MakeErrorBannerMsg(tt.text, tt.provider)
			if msg.Content != tt.want {
				t.Errorf("Content = %q, want %q", msg.Content, tt.want)
			}
			if len(msg.Segments) != 1 {
				t.Fatalf("Segments length = %d, want 1", len(msg.Segments))
			}
			if msg.Segments[0].Type != "error" {
				t.Errorf("Segment Type = %q, want %q", msg.Segments[0].Type, "error")
			}
		})
	}
}

func TestMakeUserMsg(t *testing.T) {
	msg := MakeUserMsg("hello")

	if msg.Role != "user" {
		t.Errorf("Role = %q, want %q", msg.Role, "user")
	}
	if msg.Content != "hello" {
		t.Errorf("Content = %q, want %q", msg.Content, "hello")
	}
	if msg.SkipForLLM {
		t.Error("SkipForLLM = true, want false")
	}
	if len(msg.Segments) != 1 {
		t.Fatalf("Segments length = %d, want 1", len(msg.Segments))
	}
	if msg.Segments[0].Type != "content" {
		t.Errorf("Segment Type = %q, want %q", msg.Segments[0].Type, "content")
	}
}

func TestMakeUserMsgWithSkip(t *testing.T) {
	tests := []struct {
		name    string
		content string
		skip    bool
	}{
		{"no skip", "hello", false},
		{"with skip", "hello", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			msg := MakeUserMsgWithSkip(tt.content, tt.skip)
			if msg.Content != tt.content {
				t.Errorf("Content = %q, want %q", msg.Content, tt.content)
			}
			if msg.SkipForLLM != tt.skip {
				t.Errorf("SkipForLLM = %v, want %v", msg.SkipForLLM, tt.skip)
			}
		})
	}
}

func TestFormatSI(t *testing.T) {
	tests := []struct {
		n    int
		want string
	}{
		{0, "0"},
		{1, "1"},
		{999, "999"},
		{1000, "1K"},
		{1500, "2K"},
		{999999, "1000K"},
		{1000000, "1.0M"},
		{1500000, "1.5M"},
		{10000000, "10.0M"},
	}

	for _, tt := range tests {
		t.Run(tt.want, func(t *testing.T) {
			if got := formatSI(tt.n); got != tt.want {
				t.Errorf("formatSI(%d) = %q, want %q", tt.n, got, tt.want)
			}
		})
	}
}

func TestFormatDurationMs(t *testing.T) {
	tests := []struct {
		ms   int64
		want string
	}{
		{-1, "0s"},
		{0, "0s"},
		{500, "0s"},
		{1000, "1s"},
		{5000, "5s"},
		{60000, "1m0s"},
		{90000, "1m30s"},
		{3600000, "1h0m"},
		{3661000, "1h1m"},
	}

	for _, tt := range tests {
		t.Run(tt.want, func(t *testing.T) {
			if got := formatDurationMs(tt.ms); got != tt.want {
				t.Errorf("formatDurationMs(%d) = %q, want %q", tt.ms, got, tt.want)
			}
		})
	}
}

func TestProviderShortName(t *testing.T) {
	tests := []struct {
		name string
		want string
	}{
		{"openrouter", "OR"},
		{"OPENROUTER", "OR"},
		{"zen", "Zen"},
		{"zen-gateway", "Zen"},
		{"openai", "OAI"},
		{"anthropic", "AC"},
		{"nvidia", "NV"},
		{"nim", "NV"},
		{"custom", "cust"},
		{"ab", "ab"},
		{"a", "a"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := ProviderShortName(tt.name); got != tt.want {
				t.Errorf("ProviderShortName(%q) = %q, want %q", tt.name, got, tt.want)
			}
		})
	}
}

func TestRenderSectionHeader(t *testing.T) {
	result := renderSectionHeader("Title", 20)
	if result == "" {
		t.Error("renderSectionHeader() returned empty string")
	}
	if len(result) < 20 {
		t.Errorf("renderSectionHeader() length = %d, want >= 20", len(result))
	}
}

func TestScreenTransition_TransitionTick(t *testing.T) {
	// nil transition
	var nilTrans *ScreenTransition
	if !nilTrans.TransitionTick() {
		t.Error("nil TransitionTick() should return true")
	}

	// completed transition
	trans := &ScreenTransition{
		Active:   false,
		StartAt:  time.Now(),
		Duration: 100 * time.Millisecond,
	}
	if !trans.TransitionTick() {
		t.Error("inactive TransitionTick() should return true")
	}

	// active transition not yet complete
	trans2 := &ScreenTransition{
		Active:   true,
		StartAt:  time.Now(),
		Duration: 10 * time.Second,
	}
	if trans2.TransitionTick() {
		t.Error("active TransitionTick() should return false")
	}
}

func TestScreenTransition_Progress(t *testing.T) {
	// nil transition
	var nilTrans *ScreenTransition
	if nilTrans.Progress() != 1.0 {
		t.Errorf("nil Progress() = %f, want 1.0", nilTrans.Progress())
	}

	// inactive transition
	trans := &ScreenTransition{
		Active:   false,
		StartAt:  time.Now(),
		Duration: 100 * time.Millisecond,
	}
	if trans.Progress() != 1.0 {
		t.Errorf("inactive Progress() = %f, want 1.0", trans.Progress())
	}

	// active transition
	trans2 := &ScreenTransition{
		Active:   true,
		StartAt:  time.Now(),
		Duration: 10 * time.Second,
	}
	p := trans2.Progress()
	if p < 0.0 || p > 1.0 {
		t.Errorf("active Progress() = %f, want 0.0-1.0", p)
	}
}

func TestNewKeyRegistry(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	if r == nil {
		t.Fatal("NewKeyRegistry() returned nil")
	}
	if r.leaderKey != "ctrl+x" {
		t.Errorf("leaderKey = %q, want %q", r.leaderKey, "ctrl+x")
	}
	if r.leaderTimeout != 2*time.Second {
		t.Errorf("leaderTimeout = %v, want %v", r.leaderTimeout, 2*time.Second)
	}
}

func TestNewKeyRegistry_CustomOpts(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{
		LeaderKey:     "ctrl+z",
		LeaderTimeout: 5 * time.Second,
	})
	if r.leaderKey != "ctrl+z" {
		t.Errorf("leaderKey = %q, want %q", r.leaderKey, "ctrl+z")
	}
	if r.leaderTimeout != 5*time.Second {
		t.Errorf("leaderTimeout = %v, want %v", r.leaderTimeout, 5*time.Second)
	}
}

func TestKeyRegistry_Register(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	r.Register(CtxGlobal, "ctrl+c", "quit", nil)

	if len(r.bindings[CtxGlobal]) != 1 {
		t.Fatalf("bindings length = %d, want 1", len(r.bindings[CtxGlobal]))
	}

	b := r.bindings[CtxGlobal][0]
	if b.Key != "ctrl+c" {
		t.Errorf("Key = %q, want %q", b.Key, "ctrl+c")
	}
	if b.Description != "quit" {
		t.Errorf("Description = %q, want %q", b.Description, "quit")
	}
}

func TestKeyRegistry_Handle_SimpleBinding(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	actionFired := false
	r.Register(CtxREPL, "enter", "submit", func() tea.Cmd {
		actionFired = true
		return nil
	})

	handled, _ := r.Handle("enter", CtxREPL)
	if !handled {
		t.Error("Handle() returned false, want true")
	}
	if !actionFired {
		t.Error("action did not fire")
	}
}

func TestKeyRegistry_Handle_NoBinding(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})

	handled, _ := r.Handle("unknown", CtxREPL)
	if handled {
		t.Error("Handle() returned true for unknown key, want false")
	}
}

func TestKeyRegistry_Handle_GlobalBinding(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	actionFired := false
	r.Register(CtxGlobal, "ctrl+c", "quit", func() tea.Cmd {
		actionFired = true
		return nil
	})

	handled, _ := r.Handle("ctrl+c", CtxREPL)
	if !handled {
		t.Error("Handle() returned false for global binding")
	}
	if !actionFired {
		t.Error("action did not fire")
	}
}

func TestKeyRegistry_Handle_LeaderKey(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{
		LeaderKey:     "ctrl+x",
		LeaderTimeout: 5 * time.Second,
	})

	handled, cmd := r.Handle("ctrl+x", CtxREPL)
	if !handled {
		t.Error("Handle() returned false for leader key")
	}
	if cmd == nil {
		t.Error("Handle() returned nil cmd for leader key")
	}
	if !r.leaderActive {
		t.Error("leaderActive = false, want true after leader key")
	}
}

func TestKeyRegistry_Handle_ChordBinding(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{
		LeaderKey:     "ctrl+x",
		LeaderTimeout: 5 * time.Second,
	})
	actionFired := false
	r.Register(CtxGlobal, "ctrl+x c", "compress", func() tea.Cmd {
		actionFired = true
		return nil
	})

	// Activate leader
	r.Handle("ctrl+x", CtxREPL)

	// Fire chord
	handled, _ := r.Handle("c", CtxREPL)
	if !handled {
		t.Error("Handle() returned false for chord binding")
	}
	if !actionFired {
		t.Error("chord action did not fire")
	}
	if r.leaderActive {
		t.Error("leaderActive = true, want false after chord")
	}
}

func TestKeyRegistry_Handle_LeaderTimeout(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{
		LeaderKey:     "ctrl+x",
		LeaderTimeout: 5 * time.Second,
	})

	// Activate leader
	r.Handle("ctrl+x", CtxREPL)
	if !r.leaderActive {
		t.Error("leaderActive = false, want true")
	}

	// Simulate timeout
	r.leaderActive = false

	// Now regular key should work
	handled, _ := r.Handle("enter", CtxREPL)
	if handled {
		t.Error("Handle() returned true after timeout")
	}
}

func TestKeyRegistry_IsLeaderActive(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})

	if r.IsLeaderActive() {
		t.Error("IsLeaderActive() = true, want false")
	}

	r.leaderActive = true
	if !r.IsLeaderActive() {
		t.Error("IsLeaderActive() = false, want true")
	}
}

func TestKeyRegistry_GetContextBindings(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{})
	r.Register(CtxREPL, "enter", "submit", nil)
	r.Register(CtxGlobal, "ctrl+c", "quit", nil)
	r.Register(CtxSettings, "tab", "next", nil)

	bindings := r.GetContextBindings(CtxREPL)
	// Should include context-specific + global
	if len(bindings) < 2 {
		t.Errorf("GetContextBindings() length = %d, want >= 2", len(bindings))
	}
}

func TestKeyRegistry_Handle_UnknownChord(t *testing.T) {
	r := NewKeyRegistry(KeyRegistryOpts{
		LeaderKey: "ctrl+x",
	})

	// Activate leader
	r.Handle("ctrl+x", CtxREPL)

	// Unknown chord should still be handled (consumed)
	handled, cmd := r.Handle("z", CtxREPL)
	if !handled {
		t.Error("Handle() returned false for unknown chord")
	}
	if cmd == nil {
		t.Error("Handle() returned nil cmd for unknown chord")
	}
}

func TestKeyContext_Constants(t *testing.T) {
	if CtxGlobal != "global" {
		t.Errorf("CtxGlobal = %q, want %q", CtxGlobal, "global")
	}
	if CtxREPL != "repl" {
		t.Errorf("CtxREPL = %q, want %q", CtxREPL, "repl")
	}
}

func TestTransitionType_Constants(t *testing.T) {
	if TransitionNone != 0 {
		t.Errorf("TransitionNone = %d, want 0", TransitionNone)
	}
	if TransitionSlideLeft != 1 {
		t.Errorf("TransitionSlideLeft = %d, want 1", TransitionSlideLeft)
	}
	if TransitionSlideRight != 2 {
		t.Errorf("TransitionSlideRight = %d, want 2", TransitionSlideRight)
	}
	if TransitionFade != 3 {
		t.Errorf("TransitionFade = %d, want 3", TransitionFade)
	}
}

func TestRenderTransition(t *testing.T) {
	th := theme.Dark()

	// Test with progress >= 1.0
	result := RenderTransition("old", "new", 1.0, TransitionNone, 80, 24, th)
	if result != "new" {
		t.Errorf("RenderTransition() with progress=1.0 = %q, want %q", result, "new")
	}

	// Test with progress <= 0.0
	result = RenderTransition("old", "new", 0.0, TransitionNone, 80, 24, th)
	if result != "old" {
		t.Errorf("RenderTransition() with progress=0.0 = %q, want %q", result, "old")
	}

	// Test with TransitionNone
	result = RenderTransition("old", "new", 0.5, TransitionNone, 80, 24, th)
	if result != "new" {
		t.Errorf("RenderTransition() with TransitionNone = %q, want %q", result, "new")
	}
}
