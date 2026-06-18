package commands

import (
	"testing"

	"github.com/eshanized/M31A/internal/tools"
)

func TestHandleComplexity_NilDispatcher(t *testing.T) {
	t.Parallel()
	result := handleComplexity(nil, CommandContext{})
	if result.Success {
		t.Error("should fail without dispatcher")
	}
	if result.Message != "Tool dispatcher not available." {
		t.Errorf("unexpected message: %s", result.Message)
	}
}

func TestHandleComplexity_ToolNotRegistered(t *testing.T) {
	t.Parallel()
	d := tools.NewDispatcher(nil)
	result := handleComplexity(nil, CommandContext{Dispatcher: d})
	if result.Success {
		t.Error("should fail when CodeComplexity not registered")
	}
	if result.Message != "CodeComplexity tool not registered." {
		t.Errorf("unexpected message: %s", result.Message)
	}
}

func TestHandleComplexity_Success(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	d := tools.NewDispatcher(nil)
	tool := tools.NewCodeComplexity(dir, nil)
	if err := d.Register(tool); err != nil {
		t.Fatalf("register: %v", err)
	}

	result := handleComplexity(nil, CommandContext{Dispatcher: d})
	if !result.Success {
		t.Errorf("should succeed, got: %s", result.Message)
	}
	if result.Message == "" {
		t.Error("expected non-empty message")
	}
}

func TestHandleComplexity_RegisteredInDefaultCommands(t *testing.T) {
	t.Parallel()
	r := DefaultCommands()
	if _, ok := r.Get("complexity"); !ok {
		t.Error("complexity command not registered in DefaultCommands")
	}
}
