package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/ui/tui/streaming"
)

func TestHandleStreamMsg_NilReplModel(t *testing.T) {
	m := &AppState{}
	msg := streaming.StreamMsg{}
	result, cmd := handleStreamMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when replModel is nil")
	}
}

func TestHandleStreamDoneMsg_NilReplModel(t *testing.T) {
	m := &AppState{}
	msg := streaming.StreamDoneMsg{}
	result, cmd := handleStreamDoneMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when replModel is nil")
	}
}

func TestHandleStreamErrorMsg_NilReplModel(t *testing.T) {
	m := &AppState{}
	msg := streaming.StreamErrorMsg{}
	result, cmd := handleStreamErrorMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when replModel is nil")
	}
}

func TestHandleStreamMsg_Signature(t *testing.T) {
	// Verify the function signature matches expected pattern.
	var fn = handleStreamMsg
	_ = fn
}

func TestHandleStreamDoneMsg_Signature(t *testing.T) {
	var fn = handleStreamDoneMsg
	_ = fn
}

func TestHandleStreamErrorMsg_Signature(t *testing.T) {
	var fn = handleStreamErrorMsg
	_ = fn
}
