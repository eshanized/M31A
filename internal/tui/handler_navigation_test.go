package tui

import (
	"testing"

	tea "github.com/charmbracelet/bubbletea"
)

func TestHandlePopScreenMsg_EmptyStack(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := PopScreenMsg{}
	result, cmd := handlePopScreenMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	// popScreen returns nil when stack is empty (sets screen to REPL)
	_ = cmd
}

func TestHandleKeyActionMsg(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := KeyActionMsg{Action: "open_home"}
	result, cmd := handleKeyActionMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd == nil {
		t.Fatal("expected non-nil cmd for open_home")
	}
}

func TestHandleLeaderTimeoutMsg_Inactive(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := LeaderTimeoutMsg{}
	result, cmd := handleLeaderTimeoutMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when leader is not active")
	}
}

func TestHandleSlashCommandMsg_NilDispatcher(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := SlashCommandMsg{Command: "test"}
	result, cmd := handleSlashCommandMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when dispatcher is nil")
	}
}

func TestHandleHomeSubmitMsg_NilReplModel(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := HomeSubmitMsg{Text: "test goal"}
	result, _ := handleHomeSubmitMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	// replModel will be created by ensureReplModel
}

func TestHandleIntentClassifiedMsg_Signature(t *testing.T) {
	// Verify function signature matches expected pattern.
	// Full test skipped: handleIntentClassified requires registry setup.
	var f func(*AppState, IntentClassifiedMsg) (tea.Model, tea.Cmd) = handleIntentClassifiedMsg
	_ = f
}

func TestHandleDiscussAnswerMsg_NilEngine(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := DiscussAnswerMsg{}
	result, cmd := handleDiscussAnswerMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when workflow engine is nil")
	}
}

func TestHandleDiscussCompleteMsg_NilEngine(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := DiscussCompleteMsg{}
	result, cmd := handleDiscussCompleteMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when workflow engine is nil")
	}
}

func TestHandlerNavigationSignatures(t *testing.T) {
	var f1 func(*AppState, PopScreenMsg) (tea.Model, tea.Cmd) = handlePopScreenMsg
	var f2 func(*AppState, KeyActionMsg) (tea.Model, tea.Cmd) = handleKeyActionMsg
	var f3 func(*AppState, LeaderTimeoutMsg) (tea.Model, tea.Cmd) = handleLeaderTimeoutMsg
	var f4 func(*AppState, SlashCommandMsg) (tea.Model, tea.Cmd) = handleSlashCommandMsg
	var f5 func(*AppState, HomeSubmitMsg) (tea.Model, tea.Cmd) = handleHomeSubmitMsg
	var f6 func(*AppState, IntentClassifiedMsg) (tea.Model, tea.Cmd) = handleIntentClassifiedMsg
	var f7 func(*AppState, DiscussAnswerMsg) (tea.Model, tea.Cmd) = handleDiscussAnswerMsg
	var f8 func(*AppState, DiscussCompleteMsg) (tea.Model, tea.Cmd) = handleDiscussCompleteMsg
	_ = f1
	_ = f2
	_ = f3
	_ = f4
	_ = f5
	_ = f6
	_ = f7
	_ = f8
}
