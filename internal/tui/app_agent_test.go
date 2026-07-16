package tui

import (
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/pkg/types"
)

func TestExtractToolInputSnippet_EmptyInput(t *testing.T) {
	tc := types.ToolCall{Input: []byte("{}")}
	snippet := extractToolInputSnippet(tc)
	if snippet != "" {
		t.Errorf("snippet=%q, want empty", snippet)
	}
}

func TestExtractToolInputSnippet_Path(t *testing.T) {
	tc := types.ToolCall{Input: []byte(`{"path": "/home/user/file.go"}`)}
	snippet := extractToolInputSnippet(tc)
	if snippet != "/home/user/file.go" {
		t.Errorf("snippet=%q, want /home/user/file.go", snippet)
	}
}

func TestExtractToolInputSnippet_Command(t *testing.T) {
	tc := types.ToolCall{Input: []byte(`{"command": "go build ./..."}`)}
	snippet := extractToolInputSnippet(tc)
	if snippet != "go build ./..." {
		t.Errorf("snippet=%q, want 'go build ./...'", snippet)
	}
}

func TestExtractToolInputSnippet_Pattern(t *testing.T) {
	tc := types.ToolCall{Input: []byte(`{"pattern": "*.go"}`)}
	snippet := extractToolInputSnippet(tc)
	if snippet != "*.go" {
		t.Errorf("snippet=%q, want '*.go'", snippet)
	}
}

func TestExtractToolInputSnippet_Query(t *testing.T) {
	tc := types.ToolCall{Input: []byte(`{"query": "find usage"}`)}
	snippet := extractToolInputSnippet(tc)
	if snippet != "find usage" {
		t.Errorf("snippet=%q, want 'find usage'", snippet)
	}
}

func TestExtractToolInputSnippet_URL(t *testing.T) {
	tc := types.ToolCall{Input: []byte(`{"url": "https://example.com"}`)}
	snippet := extractToolInputSnippet(tc)
	if snippet != "https://example.com" {
		t.Errorf("snippet=%q, want 'https://example.com'", snippet)
	}
}

func TestExtractToolInputSnippet_Truncate(t *testing.T) {
	longPath := "/home/user/very/long/path/to/some/file/that/is/definitely/way/too/long.txt"
	tc := types.ToolCall{Input: []byte(`{"path": "` + longPath + `"}`)}
	snippet := extractToolInputSnippet(tc)
	if len(snippet) > 40 {
		t.Errorf("snippet len=%d, want <= 40", len(snippet))
	}
}

func TestExtractToolInputSnippet_Newline(t *testing.T) {
	tc := types.ToolCall{Input: []byte(`{"command": "echo\nhello"}`)}
	snippet := extractToolInputSnippet(tc)
	if snippet != "echo hello" {
		t.Errorf("snippet=%q, want 'echo hello'", snippet)
	}
}

func TestExtractToolInputSnippet_InvalidJSON(t *testing.T) {
	tc := types.ToolCall{Input: []byte("not json")}
	snippet := extractToolInputSnippet(tc)
	if snippet != "" {
		t.Errorf("snippet=%q, want empty for invalid JSON", snippet)
	}
}

func TestExtractToolInputSnippet_EmptyValue(t *testing.T) {
	tc := types.ToolCall{Input: []byte(`{"path": ""}`)}
	snippet := extractToolInputSnippet(tc)
	if snippet != "" {
		t.Errorf("snippet=%q, want empty for empty path", snippet)
	}
}

func TestExtractToolInputSnippet_NoMatchingKey(t *testing.T) {
	tc := types.ToolCall{Input: []byte(`{"foo": "bar"}`)}
	snippet := extractToolInputSnippet(tc)
	if snippet != "" {
		t.Errorf("snippet=%q, want empty for no matching key", snippet)
	}
}

func TestReadAgentCh_Nil(t *testing.T) {
	m := &AppState{agentCh: nil}
	cmd := m.readAgentCh()
	if cmd != nil {
		t.Error("nil channel should return nil")
	}
}

func TestReadAgentCh_WithMessage(t *testing.T) {
	ch := make(chan tea.Msg, 1)
	ch <- tea.WindowSizeMsg{Width: 80, Height: 24}
	m := &AppState{agentCh: ch}
	cmd := m.readAgentCh()
	if cmd == nil {
		t.Error("should return non-nil cmd")
	}
	// Execute the cmd to verify it returns the message
	msg := cmd()
	if msg == nil {
		t.Error("cmd should return non-nil msg")
	}
}

func TestReadAgentCh_ClosedChannel(t *testing.T) {
	ch := make(chan tea.Msg)
	close(ch)
	m := &AppState{agentCh: ch}
	cmd := m.readAgentCh()
	if cmd == nil {
		t.Error("should return non-nil cmd")
	}
	msg := cmd()
	if msg != nil {
		t.Error("closed channel should return nil msg")
	}
}

func TestSaveAgentSession_NilManager(t *testing.T) {
	m := &AppState{sessionManager: nil}
	// Should not panic
	m.saveAgentSession()
}

func TestSaveAgentSession_EmptySessionID(t *testing.T) {
	m := &AppState{sessionID: ""}
	// Should not panic
	m.saveAgentSession()
}

func TestSaveAgentSession_NilReplModel(t *testing.T) {
	m := &AppState{replModel: nil}
	// Should not panic
	m.saveAgentSession()
}

func TestSaveAgentSession_AllNil(t *testing.T) {
	m := &AppState{}
	// Should not panic
	m.saveAgentSession()
}

func TestAttemptAutoFallback_NilRegistry(t *testing.T) {
	m := &AppState{registry: nil}
	cmd := m.attemptAutoFallback(nil)
	if cmd != nil {
		t.Error("nil registry should return nil cmd")
	}
}

func TestAttemptAutoFallback_EmptyProvider(t *testing.T) {
	m := &AppState{activeProvider: ""}
	cmd := m.attemptAutoFallback(nil)
	if cmd != nil {
		t.Error("empty provider should return nil cmd")
	}
}

func TestReRegisterProvidersFromConfig_NilRegistry(t *testing.T) {
	m := &AppState{registry: nil}
	// Should not panic
	m.reRegisterProvidersFromConfig()
}

func TestReRegisterProvidersFromConfig_NilConfig(t *testing.T) {
	m := &AppState{config: nil}
	// Should not panic
	m.reRegisterProvidersFromConfig()
}

func TestReRegisterProvidersFromConfig_AllNil(t *testing.T) {
	m := &AppState{}
	// Should not panic
	m.reRegisterProvidersFromConfig()
}

// Verify signatures
func TestExtractToolInputSnippet_Signature(t *testing.T) {
	var fn = extractToolInputSnippet
	_ = fn
}

func TestReadAgentCh_Signature(t *testing.T) {
	var fn = (&AppState{}).readAgentCh
	_ = fn
}

func TestSaveAgentSession_Signature(t *testing.T) {
	var fn = (&AppState{}).saveAgentSession
	_ = fn
}

func TestAttemptAutoFallback_Signature(t *testing.T) {
	var fn = (&AppState{}).attemptAutoFallback
	_ = fn
}

func TestReRegisterProvidersFromConfig_Signature(t *testing.T) {
	var fn = (&AppState{}).reRegisterProvidersFromConfig
	_ = fn
}
