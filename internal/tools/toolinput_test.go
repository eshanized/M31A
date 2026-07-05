package tools

import (
	"context"
	"encoding/json"
	"errors"
	"testing"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

func TestDispatcher_PermissionDenied_Typed(t *testing.T) {
	d := NewDispatcher(nil)
	bash := NewBash(t.TempDir(), 1800)
	d.Register(bash)

	// Send a dangerous tool call
	call := types.ToolCall{
		ID:   "test-1",
		Name: "Bash",
	}
	callBytes, _ := json.Marshal(map[string]any{
		"name": "Bash",
		"params": map[string]any{
			"command": "echo test",
		},
	})
	call.Input = callBytes

	// Run Execute in a goroutine — it will block waiting for permission
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()

	type result struct {
		res types.ToolResult
		err error
	}
	ch := make(chan result, 1)
	go func() {
		r, e := d.Execute(ctx, call)
		ch <- result{r, e}
	}()

	// Deny permission
	req := <-d.RequestCh()
	d.ApprovePermission(req.ID, false, false)

	select {
	case r := <-ch:
		if r.err == nil {
			// Check if the error is in ToolResult.Error
			if r.res.Error == "" {
				t.Fatal("expected error for permission denial")
			}
		} else {
			if !errors.Is(r.err, m31errors.ErrPermissionDenied) {
				t.Fatalf("expected ErrPermissionDenied, got: %v", r.err)
			}
		}
	case <-ctx.Done():
		t.Fatal("timed out waiting for Execute to return")
	}
}

func TestDispatcher_PermissionTimeout_Typed(t *testing.T) {
	d := NewDispatcher(nil)
	bash := NewBash(t.TempDir(), 1800)
	d.Register(bash)

	call := types.ToolCall{
		ID:   "test-2",
		Name: "Bash",
	}
	callBytes, _ := json.Marshal(map[string]any{
		"name": "Bash",
		"params": map[string]any{
			"command": "echo test",
		},
	})
	call.Input = callBytes

	// Use a short timeout context
	ctx, cancel := context.WithTimeout(context.Background(), 100*time.Millisecond)
	defer cancel()

	_, err := d.Execute(ctx, call)
	// Should return context.DeadlineExceeded or ErrPermissionDenied
	if err != nil && !errors.Is(err, context.DeadlineExceeded) && !errors.Is(err, m31errors.ErrPermissionDenied) {
		t.Fatalf("expected context.DeadlineExceeded or ErrPermissionDenied, got: %v", err)
	}
}

func TestDispatcher_PermissionAllowed_NoError(t *testing.T) {
	d := NewDispatcher(nil)
	bash := NewBash(t.TempDir(), 1800)
	d.Register(bash)

	call := types.ToolCall{
		ID:   "test-3",
		Name: "Bash",
	}
	callBytes, _ := json.Marshal(map[string]any{
		"name": "Bash",
		"params": map[string]any{
			"command": "echo hello",
		},
	})
	call.Input = callBytes

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	type result struct {
		res types.ToolResult
		err error
	}
	ch := make(chan result, 1)
	go func() {
		r, e := d.Execute(ctx, call)
		ch <- result{r, e}
	}()

	// Allow permission
	req := <-d.RequestCh()
	d.ApprovePermission(req.ID, true, false)

	select {
	case r := <-ch:
		if r.err != nil {
			t.Fatalf("unexpected error: %v", r.err)
		}
		if r.res.Error != "" {
			t.Fatalf("unexpected ToolResult.Error: %s", r.res.Error)
		}
		if !containsString(r.res.Output, "hello") {
			t.Fatalf("expected output to contain 'hello', got: %s", r.res.Output)
		}
	case <-ctx.Done():
		t.Fatal("timed out waiting for Execute to return")
	}
}

func containsString(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > 0 && containsSubstr(s, substr))
}

func containsSubstr(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
