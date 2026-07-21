package tui

import (
	"testing"
)

func TestHandleSessionDetailRequestMsg_NilModel(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := SessionDetailRequestMsg{}
	result, cmd := handleSessionDetailRequestMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd")
	}
}

func TestHandleDiffScreenMsg_NilDiffModel(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := DiffScreenMsg{Diff: "test diff", Title: "Test"}
	result, cmd := handleDiffScreenMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd")
	}
}

func TestHandleDiffCloseMsg_NilSidebar(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := DiffCloseMsg{}
	result, cmd := handleDiffCloseMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	// Will return cmd from popScreen()
	_ = cmd
}

func TestHandleToastExpiryMsg(t *testing.T) {
	m := newTestAppStateForWorkflow()
	// Add a toast first
	m.addToast("test", "info")
	msg := ToastExpiryMsg{ToastID: m.nextToastID}
	result, cmd := handleToastExpiryMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd")
	}
}

func TestHandleDismissToastMsg(t *testing.T) {
	m := newTestAppStateForWorkflow()
	m.addToast("test", "info")
	msg := DismissToastMsg{ToastID: m.nextToastID}
	result, cmd := handleDismissToastMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd")
	}
}

func TestHandlerModalSignatures(t *testing.T) {
	var f1 = handleSessionDetailRequestMsg
	var f2 = handleDiffScreenMsg
	var f3 = handleDiffCloseMsg
	var f4 = handleToastExpiryMsg
	var f5 = handleDismissToastMsg
	_ = f1
	_ = f2
	_ = f3
	_ = f4
	_ = f5
}
