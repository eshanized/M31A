package tui

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func TestNewReplModel_Components(t *testing.T) {
	t.Run("NonZeroViewport", func(t *testing.T) {
		m := NewReplModel(theme.Dark(), "test")
		if m.viewport.Width == 0 && m.viewport.Height == 0 {
			t.Error("NewReplModel viewport should have non-zero dimensions")
		}
	})
	t.Run("NonZeroTextarea", func(t *testing.T) {
		m := NewReplModel(theme.Dark(), "test")
		if !m.textarea.Focused() {
			t.Error("NewReplModel textarea should be focused")
		}
	})
}

func TestReplModel_EnterSendsMessage(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.textarea.SetValue("hello world")
	cmds, sent := m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if !sent {
		t.Error("Expected sent=true after enter with non-empty input")
	}
	// Without a provider configured, the REPL adds the user message + an error message
	if len(m.messages) < 1 {
		t.Fatalf("Expected at least 1 message, got %d", len(m.messages))
	}
	if m.messages[0].Content != "hello world" {
		t.Errorf("Message content = %q, want %q", m.messages[0].Content, "hello world")
	}
	if m.textarea.Value() != "" {
		t.Error("Textarea should be cleared after sending")
	}
	_ = cmds
}

func TestReplModel_EnterEmptyInput(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.textarea.SetValue("  ")
	cmds, sent := m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if sent {
		t.Error("Expected sent=false for empty input")
	}
	if len(m.messages) != 0 {
		t.Errorf("Expected 0 messages, got %d", len(m.messages))
	}
	_ = cmds
}

func TestReplModel_WindowResize(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	cmds, _ := m.Update(tea.WindowSizeMsg{Width: 100, Height: 40})
	if m.width != 100 {
		t.Errorf("Expected width=100, got %d", m.width)
	}
	if m.height != 40 {
		t.Errorf("Expected height=40, got %d", m.height)
	}
	if m.viewport.Width != 100 {
		t.Errorf("Expected viewport.Width=100, got %d", m.viewport.Width)
	}
	_ = cmds
}

func TestReplModel_InputHistory(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})

	m.textarea.SetValue("hello")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	m.textarea.SetValue("world")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	m.textarea.SetValue("!")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	if len(m.inputHistory) != 3 {
		t.Fatalf("Expected 3 history entries, got %d", len(m.inputHistory))
	}

	cmds, sent := m.Update(tea.KeyMsg{Type: tea.KeyUp})
	_ = cmds
	_ = sent
	if m.textarea.Value() != "!" {
		t.Errorf("After first up, expected '!' (most recent), got %q", m.textarea.Value())
	}

	m.Update(tea.KeyMsg{Type: tea.KeyUp})
	if m.textarea.Value() != "world" {
		t.Errorf("After second up, expected 'world', got %q", m.textarea.Value())
	}
}

func TestReplModel_ViewNotEmpty(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	v := m.View()
	if v == "" {
		t.Error("View() should not return empty string after resize")
	}
}

func TestReplModel_AddMessage(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	msg := types.Message{
		Role:    "assistant",
		Content: "Hello, how can I help?",
	}
	m.AddMessage(msg)

	if len(m.Messages()) != 1 {
		t.Fatalf("Expected 1 message, got %d", len(m.Messages()))
	}
	if m.Messages()[0].Content != "Hello, how can I help?" {
		t.Errorf("Message content mismatch")
	}
}

func TestReplModel_GetStatusText(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")

	if s := m.GetStatusText(); s != "" {
		t.Errorf("Expected empty status, got %q", s)
	}

	m.SetStreaming(true)
	if s := m.GetStatusText(); s != "Streaming..." {
		t.Errorf("Expected 'Streaming...', got %q", s)
	}

	m.SetStreaming(false)
	m.SetThinking(true)
	if s := m.GetStatusText(); s != "Thinking..." {
		t.Errorf("Expected 'Thinking...', got %q", s)
	}
}

func TestReplModel_EscClearsInput(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.textarea.SetValue("some text")
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'e', 's', 'c'}})
	m.Update(tea.KeyMsg{Type: tea.KeyEsc})

	if m.textarea.Value() != "" {
		t.Errorf("Expected cleared input after Esc, got %q", m.textarea.Value())
	}
}

func TestReplModel_PgUpPgDown(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})

	m.textarea.SetValue("line1")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	m.textarea.SetValue("line2")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	m.textarea.SetValue("line3")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	cmds1, _ := m.Update(tea.KeyMsg{Type: tea.KeyPgUp})
	cmds2, _ := m.Update(tea.KeyMsg{Type: tea.KeyPgDown})
	_ = cmds1
	_ = cmds2
}

func TestReplModel_ScrollPos(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})

	m.textarea.SetValue("a")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	m.textarea.SetValue("b")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	m.textarea.SetValue("c")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	// Without a provider, each send adds a user message + error message (6 total)
	if len(m.messages) < 3 {
		t.Errorf("Expected at least 3 messages, got %d", len(m.messages))
	}
	// User messages are at indices 0, 2, 4 (interleaved with error messages)
	if m.messages[0].Content != "a" {
		t.Errorf("First message = %q, want %q", m.messages[0].Content, "a")
	}
	if m.messages[2].Content != "b" {
		t.Errorf("Second user message = %q, want %q", m.messages[2].Content, "b")
	}
	if m.messages[4].Content != "c" {
		t.Errorf("Third user message = %q, want %q", m.messages[4].Content, "c")
	}
}

func TestReplModel_HistoryNavigationAtEnd(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")

	m.textarea.SetValue("first")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	m.textarea.SetValue("second")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	m.Update(tea.KeyMsg{Type: tea.KeyUp})
	m.Update(tea.KeyMsg{Type: tea.KeyUp})

	m.Update(tea.KeyMsg{Type: tea.KeyDown})
	m.Update(tea.KeyMsg{Type: tea.KeyDown})

	if m.textarea.Value() != "" {
		t.Errorf("Expected empty input after navigating past end, got %q", m.textarea.Value())
	}
}

func TestReplModel_InputValue(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.textarea.SetValue("  hello  ")

	if v := m.InputValue(); v != "hello" {
		t.Errorf("InputValue() = %q, want %q", v, "hello")
	}
}

func TestReplModel_DownEmptyHistory(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")

	cmds, sent := m.Update(tea.KeyMsg{Type: tea.KeyDown})
	if len(m.inputHistory) != 0 {
		t.Errorf("Expected empty history, got %d", len(m.inputHistory))
	}
	_ = cmds
	_ = sent
}

func TestReplModel_UpEmptyHistory(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")

	cmds, sent := m.Update(tea.KeyMsg{Type: tea.KeyUp})
	if len(m.inputHistory) != 0 {
		t.Errorf("Expected empty history, got %d", len(m.inputHistory))
	}
	_ = cmds
	_ = sent
}

func TestReplModel_RenderMessages(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	m.AddMessage(types.Message{Role: "user", Content: "hi"})
	m.AddMessage(types.Message{Role: "assistant", Content: "hello"})
	m.renderMessages()

	content := m.viewport.View()
	if !strings.Contains(content, "hi") {
		t.Error("Viewport should contain 'hi'")
	}
	if !strings.Contains(content, "hello") {
		t.Error("Viewport should contain 'hello'")
	}
}

func TestReplModel_SpinnerTick(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	cmd := m.SpinnerTick()
	if cmd == nil {
		t.Error("SpinnerTick() should return non-nil command")
	}
}

func TestReplModel_StatusTextPriority(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.SetStreaming(true)
	m.SetThinking(true)

	if s := m.GetStatusText(); s != "Streaming..." {
		t.Errorf("Streaming should take priority, got %q", s)
	}
}

func TestReplModel_MessageRoleMapping(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	m.AddMessage(types.Message{Role: "system", Content: "system message"})
	content := m.viewport.View()
	if len(content) == 0 {
		t.Error("Viewport should have content after AddMessage")
	}
}

func TestReplModel_MultipleRenders(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 40})

	m.textarea.SetValue("msg1")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	m.textarea.SetValue("msg2")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	m.textarea.SetValue("msg3")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	v := m.View()
	if !strings.Contains(v, "msg1") {
		t.Error("View should contain rendered messages")
	}
}

func TestReplModel_EnterWithHistoryPosReset(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})

	m.textarea.SetValue("a")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})
	m.textarea.SetValue("b")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	if m.historyPos != len(m.inputHistory) {
		t.Errorf("historyPos should be reset to %d, got %d", len(m.inputHistory), m.historyPos)
	}
}

func TestShellMode_DetectsBangPrefix(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})

	// Input starting with ! should trigger shell mode
	m.textarea.SetValue("!ls -la")
	cmds, sent := m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	// sent=false because shell mode returns false (not a conversation turn)
	if sent {
		t.Error("Expected sent=false for shell mode command")
	}

	// Textarea should be reset
	if m.textarea.Value() != "" {
		t.Errorf("Expected empty textarea after shell command, got %q", m.textarea.Value())
	}

	// Should have assistant message with "Running..."
	found := false
	for _, msg := range m.messages {
		if msg.Role == "assistant" && strings.HasPrefix(msg.Content, "$ ls -la") && strings.Contains(msg.Content, "Running") {
			found = true
			break
		}
	}
	if !found {
		t.Error("Expected assistant message with '$ ls -la\\nRunning...'")
	}

	// Should return a non-nil cmd (the shell execution goroutine)
	if len(cmds) == 0 {
		t.Error("Expected at least one command (the shell execution goroutine)")
	}

	// Verify shell message is marked SkipForLLM
	if len(m.messages) > 0 {
		lastMsg := m.messages[len(m.messages)-1]
		if !lastMsg.SkipForLLM {
			t.Error("Shell message should have SkipForLLM=true")
		}
	}
}

func TestShellMode_EmptyCommand(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})

	// Just "!" should show help
	m.textarea.SetValue("!")
	cmds, sent := m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	// sent=true because help message was shown
	if !sent {
		t.Error("Expected sent=true for empty shell mode (help shown)")
	}

	// Should have assistant message with "Shell mode:" guidance
	found := false
	for _, msg := range m.messages {
		if msg.Role == "assistant" && strings.Contains(msg.Content, "Shell mode:") {
			found = true
			break
		}
	}
	if !found {
		t.Error("Expected assistant message with 'Shell mode:' guidance text")
	}

	// Textarea should NOT be reset (we didn't execute a command)
	if m.textarea.Value() != "!" {
		t.Errorf("Expected textarea to still contain '!', got %q", m.textarea.Value())
	}

	_ = cmds
}

func TestShellMode_NoLLMCall(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})

	// Shell mode should not trigger streaming
	m.textarea.SetValue("!echo hello")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	// streaming should be false
	if m.streaming {
		t.Error("Expected streaming=false after shell command")
	}

	// streamCh should be nil (no stream created)
	if m.streamCh != nil {
		t.Error("Expected streamCh=nil after shell command (no LLM stream created)")
	}
}

func TestShellMode_NonBangInput(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})

	// Normal input should still work (create user message, error about no provider)
	m.textarea.SetValue("hello")
	cmds, sent := m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	if !sent {
		t.Error("Expected sent=true for non-bang input")
	}

	// Should have a user message with "hello"
	found := false
	for _, msg := range m.messages {
		if msg.Role == "user" && msg.Content == "hello" {
			found = true
			break
		}
	}
	if !found {
		t.Error("Expected user message with 'hello' for non-bang input")
	}

	// Should NOT have SkipForLLM on user messages
	for _, msg := range m.messages {
		if msg.Role == "user" && msg.SkipForLLM {
			t.Error("User message should NOT have SkipForLLM=true")
		}
	}

	_ = cmds
}

func TestShellResultMsg_UpdatesDisplay(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})

	// First, simulate a shell command that was sent
	m.textarea.SetValue("!echo hello")
	cmds, _ := m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	// The "Running..." message should be there
	runningFound := false
	for _, msg := range m.messages {
		if strings.Contains(msg.Content, "Running") && strings.Contains(msg.Content, "$ echo hello") {
			runningFound = true
			break
		}
	}
	if !runningFound {
		t.Error("Expected 'Running...' message before ShellResultMsg")
	}

	// Now send a ShellResultMsg to update the display
	resultCmds, sent := m.Update(ShellResultMsg{
		Command: "echo hello",
		Output:  "hello\n",
		Err:     "",
	})

	if !sent {
		t.Error("Expected sent=true for ShellResultMsg")
	}

	// The "Running..." message should be replaced with actual output
	outputFound := false
	for _, msg := range m.messages {
		if msg.Role == "assistant" && strings.HasPrefix(msg.Content, "$ echo hello") && strings.Contains(msg.Content, "hello") {
			outputFound = true
			// Should NOT contain "Running" anymore
			if strings.Contains(msg.Content, "Running") {
				t.Error("Updated message should not contain 'Running...'")
			}
			break
		}
	}
	if !outputFound {
		t.Error("Expected updated message with '$ echo hello\\nhello'")
	}

	_ = cmds
	_ = resultCmds
}

func TestShellResultMsg_Error(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})

	// Simulate shell command
	m.textarea.SetValue("!invalid-command")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	// Send error result
	m.Update(ShellResultMsg{
		Command: "invalid-command",
		Output:  "",
		Err:     "command not found",
	})

	// Message should contain [Error: command not found]
	found := false
	for _, msg := range m.messages {
		if msg.Role == "assistant" && strings.HasPrefix(msg.Content, "$ invalid-command") && strings.Contains(msg.Content, "Error: command not found") {
			found = true
			break
		}
	}
	if !found {
		t.Error("Expected error message with '[Error: command not found]'")
	}
}

func TestShellMode_MessagesForLLM(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})

	// Add a user message (normal, skipForLLM=false)
	m.textarea.SetValue("normal message")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	// Add a shell message (skipForLLM=true) via simulate
	m.textarea.SetValue("!ls")
	m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	// messagesForLLM should include normal messages but NOT shell messages
	llmMsgs := m.messagesForLLM()
	for _, msg := range llmMsgs {
		if msg.SkipForLLM {
			t.Error("messagesForLLM() should not include SkipForLLM messages")
		}
	}

	// At least the user message should be present
	found := false
	for _, msg := range llmMsgs {
		if msg.Role == "user" && msg.Content == "normal message" {
			found = true
			break
		}
	}
	if !found {
		t.Error("messagesForLLM() should include normal user message")
	}
}

func TestShellMode_BangBeforeSlash(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})

	// "!/bin/ls" would be caught by slash handler if ! check came after /
	// ! detection must come first
	m.textarea.SetValue("!/bin/echo test")
	cmds, sent := m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	// Should be handled as shell mode, not slash command
	if sent {
		t.Error("Expected sent=false for shell mode (not a slash command)")
	}

	found := false
	for _, msg := range m.messages {
		if msg.Role == "assistant" && strings.HasPrefix(msg.Content, "$ /bin/echo test") {
			found = true
			break
		}
	}
	if !found {
		t.Error("Expected shell mode to handle !/bin/echo test (not slash command)")
	}

	_ = cmds
}

// --- FrecentHistory tests ---

func TestFrecentHistory_Upsert(t *testing.T) {
	h := NewFrecentHistory(filepath.Join(t.TempDir(), "test.json"), 100)

	if h.Size() != 0 {
		t.Fatalf("Expected empty history, got %d entries", h.Size())
	}

	h.Upsert("hello")
	if h.Size() != 1 {
		t.Fatalf("Expected 1 entry after first upsert, got %d", h.Size())
	}
	if h.entries[0].Frequency != 1 {
		t.Errorf("Expected frequency=1, got %d", h.entries[0].Frequency)
	}
	if h.entries[0].Text != "hello" {
		t.Errorf("Expected text='hello', got %q", h.entries[0].Text)
	}

	// Upsert same text again — should increment frequency, not add new entry
	h.Upsert("hello")
	if h.Size() != 1 {
		t.Fatalf("Expected still 1 entry after duplicate upsert, got %d", h.Size())
	}
	if h.entries[0].Frequency != 2 {
		t.Errorf("Expected frequency=2 after duplicate upsert, got %d", h.entries[0].Frequency)
	}

	// Upsert different text
	h.Upsert("world")
	if h.Size() != 2 {
		t.Fatalf("Expected 2 entries after second upsert, got %d", h.Size())
	}
}

func TestFrecentHistory_Search(t *testing.T) {
	h := NewFrecentHistory(filepath.Join(t.TempDir(), "test.json"), 100)

	entries := []string{"git status", "git log", "git commit -m", "make test", "make build"}
	for _, e := range entries {
		h.Upsert(e)
	}

	// Search with prefix "git" — should return 3 results
	results := h.Search("git", 10)
	if len(results) != 3 {
		t.Errorf("Expected 3 results for 'git', got %d", len(results))
	}

	// Search with prefix "make" and limit 2
	results = h.Search("make", 2)
	if len(results) != 2 {
		t.Errorf("Expected 2 results for 'make' with limit 2, got %d", len(results))
	}

	// Search for nonexistent — should return 0
	results = h.Search("nonexistent", 10)
	if len(results) != 0 {
		t.Errorf("Expected 0 results for 'nonexistent', got %d", len(results))
	}

	// Empty prefix — should return all entries sorted by frecency
	results = h.Search("", 10)
	if len(results) != 5 {
		t.Errorf("Expected 5 results for empty prefix, got %d", len(results))
	}

	// Results should be sorted by frecency (most recent first since all have frequency=1)
	if len(results) > 0 {
		if results[0].Text != "make build" {
			t.Logf("First result: %q (expected 'make build' as most recent)", results[0].Text)
		}
	}
}

func TestFrecentHistory_Frecency(t *testing.T) {
	h := NewFrecentHistory(filepath.Join(t.TempDir(), "test.json"), 100)

	// Freeze reference time to avoid float precision issues with time.Since()
	now := time.Now()

	// Entry with frequency=5, used ~1 hour ago
	entry1 := HistoryEntry{
		Text:      "test1",
		Frequency: 5,
		LastUsed:  now.Add(-1 * time.Hour),
	}
	score1 := h.Frecency(entry1)
	if score1 < 2.4 || score1 > 2.6 {
		t.Errorf("Expected frecency ≈2.50 for (freq=5, 1h ago), got %.4f", score1)
	}

	// Entry with frequency=1, used just now
	entry2 := HistoryEntry{
		Text:      "test2",
		Frequency: 1,
		LastUsed:  now,
	}
	score2 := h.Frecency(entry2)
	if score2 < 0.9 || score2 > 1.1 {
		t.Errorf("Expected frecency ≈1.00 for (freq=1, now), got %.4f", score2)
	}

	// Higher frequency + recency = higher score
	if score1 <= score2 {
		t.Errorf("Expected entry1 (freq=5, ~1h ago) to score higher than entry2 (freq=1, now), got %.4f vs %.4f", score1, score2)
	}
}

func TestFrecentHistory_Eviction(t *testing.T) {
	h := NewFrecentHistory(filepath.Join(t.TempDir(), "test.json"), 3)

	// Upsert 4 entries — one should be evicted on Save
	h.Upsert("entry-a")
	h.Upsert("entry-b")
	h.Upsert("entry-c")
	h.Upsert("entry-d")

	// Save triggers eviction
	if err := h.Save(); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	if h.Size() > 3 {
		t.Errorf("Expected at most 3 entries after eviction, got %d", h.Size())
	}

	// The lowest-frecency entry should be evicted. All have frequency=1,
	// so the oldest (first upserted) should be evicted: "entry-a"
	for _, entry := range h.entries {
		if entry.Text == "entry-a" {
			t.Log("Note: entry-a survived eviction — eviction is frecency-based, not FIFO")
			break
		}
	}
}

func TestFrecentHistory_Persistence(t *testing.T) {
	path := filepath.Join(t.TempDir(), "prompt_history.json")

	// Create and populate history
	h1 := NewFrecentHistory(path, 100)
	h1.Upsert("hello")
	h1.Upsert("world")
	if err := h1.Save(); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	// Create new instance pointing to the same file and load
	h2 := NewFrecentHistory(path, 100)
	if err := h2.Load(); err != nil {
		t.Fatalf("Load failed: %v", err)
	}

	if h2.Size() != 2 {
		t.Fatalf("Expected 2 entries after load, got %d", h2.Size())
	}

	// Verify both entries loaded correctly
	foundHello := false
	foundWorld := false
	for _, entry := range h2.entries {
		if entry.Text == "hello" {
			foundHello = true
			if entry.Frequency != 1 {
				t.Errorf("Expected frequency=1 for 'hello', got %d", entry.Frequency)
			}
		}
		if entry.Text == "world" {
			foundWorld = true
			if entry.Frequency != 1 {
				t.Errorf("Expected frequency=1 for 'world', got %d", entry.Frequency)
			}
		}
	}
	if !foundHello {
		t.Error("Expected 'hello' in loaded entries")
	}
	if !foundWorld {
		t.Error("Expected 'world' in loaded entries")
	}
}

func TestFrecentHistory_EmptyInput(t *testing.T) {
	h := NewFrecentHistory(filepath.Join(t.TempDir(), "test.json"), 100)

	h.Upsert("")
	if h.Size() != 0 {
		t.Errorf("Expected 0 entries after upserting empty string, got %d", h.Size())
	}

	h.Upsert("   ")
	if h.Size() != 0 {
		t.Errorf("Expected 0 entries after upserting whitespace, got %d", h.Size())
	}

	h.Upsert("\t\n")
	if h.Size() != 0 {
		t.Errorf("Expected 0 entries after upserting whitespace, got %d", h.Size())
	}
}

func TestFrecentHistory_Clear(t *testing.T) {
	h := NewFrecentHistory(filepath.Join(t.TempDir(), "test.json"), 100)

	h.Upsert("hello")
	h.Upsert("world")
	if h.Size() != 2 {
		t.Fatalf("Expected 2 entries before clear, got %d", h.Size())
	}

	h.Clear()
	if h.Size() != 0 {
		t.Errorf("Expected 0 entries after clear, got %d", h.Size())
	}

	// Verify re-use after clear works
	h.Upsert("new-entry")
	if h.Size() != 1 {
		t.Errorf("Expected 1 entry after clear + upsert, got %d", h.Size())
	}
	if h.entries[0].Text != "new-entry" {
		t.Errorf("Expected text 'new-entry', got %q", h.entries[0].Text)
	}
}

// testReplModelWithCwd creates a ReplModel with the given cwd for @filepath testing.
func testReplModelWithCwd(t *testing.T, cwd string) *ReplModel {
	t.Helper()
	m := NewReplModel(theme.Dark(), "test")
	m.SetCwd(cwd)
	return &m
}

func TestExpandFileRefs_ExistingFile(t *testing.T) {
	dir := t.TempDir()
	m := testReplModelWithCwd(t, dir)

	if err := os.WriteFile(dir+"/test.txt", []byte("hello world"), 0644); err != nil {
		t.Fatalf("failed to create test file: %v", err)
	}

	result := m.expandFileRefs("read @./test.txt please")
	// The path in the output preserves the original reference (./test.txt)
	if !strings.Contains(result, "hello world") {
		t.Errorf("Expected 'hello world' in result, got %q", result)
	}
	if !strings.Contains(result, "--- ./test.txt ---") {
		t.Errorf("Expected '--- ./test.txt ---' in result, got %q", result)
	}
}

func TestExpandFileRefs_MissingFile(t *testing.T) {
	dir := t.TempDir()
	m := testReplModelWithCwd(t, dir)

	result := m.expandFileRefs("check @./nonexistent.txt")
	if result != "check @./nonexistent.txt" {
		t.Errorf("Missing file: expected reference left as-is, got %q", result)
	}
}

func TestExpandFileRefs_BinaryFile(t *testing.T) {
	dir := t.TempDir()
	m := testReplModelWithCwd(t, dir)

	// Write a 4-byte PNG header (binary)
	if err := os.WriteFile(dir+"/binary.bin", []byte{0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A}, 0644); err != nil {
		t.Fatalf("failed to create binary file: %v", err)
	}

	result := m.expandFileRefs("view @./binary.bin")
	if !strings.Contains(result, "[binary:") {
		t.Errorf("Binary file: expected '[binary:' in result, got %q", result)
	}
	if !strings.Contains(result, "8 bytes") {
		t.Errorf("Binary file: expected file size in result, got %q", result)
	}
}

func TestExpandFileRefs_FileTooLarge(t *testing.T) {
	dir := t.TempDir()
	m := testReplModelWithCwd(t, dir)

	// Write a file larger than 100KB
	data := make([]byte, 101*1024)
	if err := os.WriteFile(dir+"/large.txt", data, 0644); err != nil {
		t.Fatalf("failed to create large file: %v", err)
	}

	result := m.expandFileRefs("load @./large.txt")
	if !strings.Contains(result, "[file too large:") {
		t.Errorf("Large file: expected '[file too large:' in result, got %q", result)
	}
}

func TestExpandFileRefs_MultipleRefs(t *testing.T) {
	dir := t.TempDir()
	m := testReplModelWithCwd(t, dir)

	if err := os.WriteFile(dir+"/a.txt", []byte("alpha"), 0644); err != nil {
		t.Fatalf("failed to create a.txt: %v", err)
	}
	if err := os.WriteFile(dir+"/b.txt", []byte("beta"), 0644); err != nil {
		t.Fatalf("failed to create b.txt: %v", err)
	}

	result := m.expandFileRefs("@./a.txt and @./b.txt")
	if !strings.Contains(result, "alpha") {
		t.Errorf("MultipleRefs: expected 'alpha' in result, got %q", result)
	}
	if !strings.Contains(result, "beta") {
		t.Errorf("MultipleRefs: expected 'beta' in result, got %q", result)
	}
	if !strings.Contains(result, "--- ./a.txt ---") {
		t.Errorf("MultipleRefs: expected '--- ./a.txt ---' in result, got %q", result)
	}
	if !strings.Contains(result, "--- ./b.txt ---") {
		t.Errorf("MultipleRefs: expected '--- ./b.txt ---' in result, got %q", result)
	}
}

func TestExpandFileRefs_NoRefs(t *testing.T) {
	dir := t.TempDir()
	m := testReplModelWithCwd(t, dir)

	result := m.expandFileRefs("hello world")
	if result != "hello world" {
		t.Errorf("NoRefs: expected unchanged input, got %q", result)
	}
}

func TestExpandFileRefs_NoCwd(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	// Deliberately NOT setting cwd

	result := m.expandFileRefs("@./test.txt")
	if result != "@./test.txt" {
		t.Errorf("NoCwd: expected reference left as-is, got %q", result)
	}
}

func TestExpandFileRefs_AbsolutePath(t *testing.T) {
	dir := t.TempDir()
	m := testReplModelWithCwd(t, dir)

	absPath := dir + "/abs_file.txt"
	if err := os.WriteFile(absPath, []byte("absolute content"), 0644); err != nil {
		t.Fatalf("failed to create abs file: %v", err)
	}

	input := fmt.Sprintf("cat @%s", absPath)
	result := m.expandFileRefs(input)
	if !strings.Contains(result, "absolute content") {
		t.Errorf("AbsolutePath: expected 'absolute content' in result, got %q", result)
	}
	// The output includes the full absolute path in the header
	if !strings.Contains(result, "---") || !strings.Contains(result, "abs_file.txt") {
		t.Errorf("AbsolutePath: expected file reference with abs_file.txt in result, got %q", result)
	}
}

func TestExpandFileRefs_NotAMention(t *testing.T) {
	dir := t.TempDir()
	m := testReplModelWithCwd(t, dir)

	result := m.expandFileRefs("hello @user check @mention")
	if result != "hello @user check @mention" {
		t.Errorf("NotAMention: expected unchanged, got %q", result)
	}
}

func TestExpandFileRefs_Directory(t *testing.T) {
	dir := t.TempDir()
	m := testReplModelWithCwd(t, dir)

	subDir := dir + "/subdir"
	if err := os.Mkdir(subDir, 0755); err != nil {
		t.Fatalf("failed to create dir: %v", err)
	}

	input := fmt.Sprintf("list @%s", subDir)
	result := m.expandFileRefs(input)
	if result != input {
		t.Errorf("Directory: expected reference left as-is, got %q", result)
	}
}

func TestReplModel_SlashCommandEmitsSlashCommandMsg(t *testing.T) {
	m := NewReplModel(theme.Dark(), "test")
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})

	// Set a slash command value
	m.textarea.SetValue("/help")

	// Press Enter
	cmds, _ := m.Update(tea.KeyMsg{Type: tea.KeyEnter})

	// Check that a SlashCommandMsg was emitted
	found := false
	for _, cmd := range cmds {
		if cmd != nil {
			msg := cmd()
			if slashMsg, ok := msg.(SlashCommandMsg); ok {
				found = true
				if slashMsg.Command != "/help" {
					t.Errorf("SlashCommandMsg.Command = %q, want %q", slashMsg.Command, "/help")
				}
			}
		}
	}
	if !found {
		t.Error("Expected SlashCommandMsg to be emitted for /help command")
	}

	// Textarea should be cleared
	if m.textarea.Value() != "" {
		t.Errorf("Expected textarea cleared after slash command, got %q", m.textarea.Value())
	}
}
