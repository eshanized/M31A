package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/core/config"
)

func TestHandleFirstRunCompleteMsg_NilFirstRunModel(t *testing.T) {
	m := newTestAppState()
	msg := FirstRunCompleteMsg{}
	result, cmd := handleFirstRunCompleteMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when firstRunModel is nil")
	}
}

func TestHandleSettingsSavedMsg_NilConfigModel(t *testing.T) {
	m := newTestAppState()
	msg := SettingsSavedMsg{}
	result, _ := handleSettingsSavedMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	// SettingsSaved may return popScreen cmd
}

func TestHandleResetCompleteMsg_NilRegistry(t *testing.T) {
	m := newTestAppState()
	msg := ResetCompleteMsg{}
	result, _ := handleResetCompleteMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	// ResetComplete navigates to first-run, returns cmd
}

func TestHandleConfigSavedMsg_NilConfigModel(t *testing.T) {
	m := newTestAppState()
	msg := ConfigSavedMsg{}
	result, _ := handleConfigSavedMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	// ConfigSaved returns toast cmd
}

func TestHandleConfigReloadMsg_NilConfig(t *testing.T) {
	m := newTestAppState()
	msg := config.ConfigReloadMsg{}
	result, _ := handleConfigReloadMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	// ConfigReload with nil config and nil error returns nil cmd
}

func TestHandlerConfigSignatures(t *testing.T) {
	var f1 = handleFirstRunCompleteMsg
	var f2 = handleSettingsSavedMsg
	var f3 = handleResetCompleteMsg
	var f4 = handleConfigSavedMsg
	var f5 = handleConfigReloadMsg
	_ = f1
	_ = f2
	_ = f3
	_ = f4
	_ = f5
}
