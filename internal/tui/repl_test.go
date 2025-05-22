package tui

import (
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

func TestNewReplModel_Components(t *testing.T) {
	t.Run("NonZeroViewport", func(t *testing.T) {
		m := NewReplModel(theme.Dark())
		if m.viewport.Width == 0 && m.viewport.Height == 0 {
			t.Error("NewReplModel viewport should have non-zero dimensions")
		}
	})
	t.Run("NonZeroTextarea", func(t *testing.T) {
		m := NewReplModel(theme.Dark())
		if !m.textarea.Focused() {
			t.Error("NewReplModel textarea should be focused")
		}
	})
}

func TestReplModel_EnterSendsMessage(t *testing.T) {
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	v := m.View()
	if v == "" {
		t.Error("View() should not return empty string after resize")
	}
}

func TestReplModel_AddMessage(t *testing.T) {
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())

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
	m := NewReplModel(theme.Dark())
	m.textarea.SetValue("some text")
	m.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'e', 's', 'c'}})
	m.Update(tea.KeyMsg{Type: tea.KeyEsc})

	if m.textarea.Value() != "" {
		t.Errorf("Expected cleared input after Esc, got %q", m.textarea.Value())
	}
}

func TestReplModel_PgUpPgDown(t *testing.T) {
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())

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
	m := NewReplModel(theme.Dark())
	m.textarea.SetValue("  hello  ")

	if v := m.InputValue(); v != "hello" {
		t.Errorf("InputValue() = %q, want %q", v, "hello")
	}
}

func TestReplModel_DownEmptyHistory(t *testing.T) {
	m := NewReplModel(theme.Dark())

	cmds, sent := m.Update(tea.KeyMsg{Type: tea.KeyDown})
	if len(m.inputHistory) != 0 {
		t.Errorf("Expected empty history, got %d", len(m.inputHistory))
	}
	_ = cmds
	_ = sent
}

func TestReplModel_UpEmptyHistory(t *testing.T) {
	m := NewReplModel(theme.Dark())

	cmds, sent := m.Update(tea.KeyMsg{Type: tea.KeyUp})
	if len(m.inputHistory) != 0 {
		t.Errorf("Expected empty history, got %d", len(m.inputHistory))
	}
	_ = cmds
	_ = sent
}

func TestReplModel_RenderMessages(t *testing.T) {
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
	cmd := m.SpinnerTick()
	if cmd == nil {
		t.Error("SpinnerTick() should return non-nil command")
	}
}

func TestReplModel_StatusTextPriority(t *testing.T) {
	m := NewReplModel(theme.Dark())
	m.SetStreaming(true)
	m.SetThinking(true)

	if s := m.GetStatusText(); s != "Streaming..." {
		t.Errorf("Streaming should take priority, got %q", s)
	}
}

func TestReplModel_MessageRoleMapping(t *testing.T) {
	m := NewReplModel(theme.Dark())
	m.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	m.AddMessage(types.Message{Role: "system", Content: "system message"})
	content := m.viewport.View()
	if len(content) == 0 {
		t.Error("Viewport should have content after AddMessage")
	}
}

func TestReplModel_MultipleRenders(t *testing.T) {
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
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
	m := NewReplModel(theme.Dark())
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
