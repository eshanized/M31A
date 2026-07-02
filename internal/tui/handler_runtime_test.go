package tui

import (
	"testing"

	tea "github.com/charmbracelet/bubbletea"
)

func TestHandleHealthCheckTickMsg_NilRegistry(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := HealthCheckTickMsg{}
	result, cmd := handleHealthCheckTickMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	// Will return cmds for NextHealthTick
	_ = cmd
}

func TestHandleHealthCheckResultMsg_NilReplModel(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := HealthCheckResultMsg{}
	result, cmd := handleHealthCheckResultMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when replModel is nil")
	}
}

func TestHandleRefreshCacheMsg_NilRegistry(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := RefreshCacheMsg{}
	result, cmd := handleRefreshCacheMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when registry is nil")
	}
}

func TestHandleCacheRefreshResultMsg_NilNextCmd(t *testing.T) {
	m := newTestAppStateForWorkflow()
	msg := CacheRefreshResultMsg{}
	result, cmd := handleCacheRefreshResultMsg(m, msg)
	if result == nil {
		t.Fatal("expected non-nil model")
	}
	if cmd != nil {
		t.Fatal("expected nil cmd when NextCmd is nil")
	}
}

func TestHandlerRuntimeSignatures(t *testing.T) {
	var f1 func(*AppState, HealthCheckTickMsg) (tea.Model, tea.Cmd) = handleHealthCheckTickMsg
	var f2 func(*AppState, HealthCheckResultMsg) (tea.Model, tea.Cmd) = handleHealthCheckResultMsg
	var f3 func(*AppState, RefreshCacheMsg) (tea.Model, tea.Cmd) = handleRefreshCacheMsg
	var f4 func(*AppState, CacheRefreshResultMsg) (tea.Model, tea.Cmd) = handleCacheRefreshResultMsg
	_ = f1
	_ = f2
	_ = f3
	_ = f4
}
