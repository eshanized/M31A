package tools

import (
	"context"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/types"
	m31errors "github.com/eshanized/M31A/internal/errors"
)

type mockTool struct {
	name      string
	riskLevel types.RiskLevel
	execFunc  func(ctx context.Context, input types.ToolInput) (types.ToolResult, error)
}

func (m *mockTool) Name() string                                        { return m.name }
func (m *mockTool) Description() string                                 { return "mock tool for testing" }
func (m *mockTool) RiskLevel() types.RiskLevel                          { return m.riskLevel }
func (m *mockTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	if m.execFunc != nil {
		return m.execFunc(ctx, input)
	}
	return types.ToolResult{Output: "ok"}, nil
}

func TestDispatcher_RegisterAndExecute(t *testing.T) {
	t.Parallel()
	d := NewDispatcher()
	d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe})

	result, err := d.Execute(context.Background(), types.ToolCall{
		ID:   "call1",
		Name: "test",
	})
	if err != nil {
		t.Fatal(err)
	}
	if result.Output != "ok" {
		t.Errorf("expected 'ok', got %q", result.Output)
	}
	if result.ToolCallID != "call1" {
		t.Errorf("expected ToolCallID 'call1', got %q", result.ToolCallID)
	}
}

func TestDispatcher_UnknownTool(t *testing.T) {
	t.Parallel()
	d := NewDispatcher()
	_, err := d.Execute(context.Background(), types.ToolCall{
		ID:   "call1",
		Name: "nonexistent",
	})
	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(err.Error(), "unknown tool") {
		t.Errorf("expected 'unknown tool', got: %v", err)
	}
}

func TestDispatcher_SafeToolNoPermission(t *testing.T) {
	t.Parallel()
	d := NewDispatcher()
	d.Register(&mockTool{name: "safe", riskLevel: types.RiskSafe})

	_, err := d.Execute(context.Background(), types.ToolCall{
		ID:   "call1",
		Name: "safe",
	})
	if err != nil {
		t.Errorf("safe tool should execute without permission, got: %v", err)
	}
}

func TestDispatcher_DangerousToolPermissionGranted(t *testing.T) {
	d := NewDispatcher()
	d.Register(&mockTool{name: "bash", riskLevel: types.RiskDangerous})

	errCh := make(chan error, 1)
	go func() {
		_, err := d.Execute(context.Background(), types.ToolCall{
			ID:   "call1",
			Name: "bash",
		})
		errCh <- err
	}()

	req := <-d.RequestCh()
	if req.ToolName != "bash" {
		t.Errorf("expected request for 'bash', got %q", req.ToolName)
	}
	d.ApprovePermission(true, false)

	if err := <-errCh; err != nil {
		t.Errorf("expected nil error after approval, got: %v", err)
	}
}

func TestDispatcher_DangerousToolPermissionDenied(t *testing.T) {
	d := NewDispatcher()
	d.Register(&mockTool{name: "bash", riskLevel: types.RiskDangerous})

	errCh := make(chan error, 1)
	go func() {
		_, err := d.Execute(context.Background(), types.ToolCall{
			ID:   "call1",
			Name: "bash",
		})
		errCh <- err
	}()

	<-d.RequestCh()
	d.ApprovePermission(false, false)

	if err := <-errCh; err != m31errors.ErrPermissionDenied {
		t.Errorf("expected ErrPermissionDenied, got: %v", err)
	}
}

func TestDispatcher_RememberedPermission(t *testing.T) {
	d := NewDispatcher()
	d.Register(&mockTool{name: "bash", riskLevel: types.RiskDangerous})

	// First call — approve with remember=true
	errCh1 := make(chan error, 1)
	go func() {
		_, err := d.Execute(context.Background(), types.ToolCall{
			ID:   "call1",
			Name: "bash",
		})
		errCh1 <- err
	}()

	<-d.RequestCh()
	d.ApprovePermission(true, true)

	if err := <-errCh1; err != nil {
		t.Fatalf("first call failed: %v", err)
	}

	// Second call — should not ask for permission
	result, err := d.Execute(context.Background(), types.ToolCall{
		ID:   "call2",
		Name: "bash",
	})
	if err != nil {
		t.Errorf("remembered call should not need permission, got: %v", err)
	}
	if result.Output != "ok" {
		t.Errorf("expected 'ok', got %q", result.Output)
	}
}

func TestDispatcher_List(t *testing.T) {
	t.Parallel()
	d := NewDispatcher()
	d.Register(&mockTool{name: "zzz", riskLevel: types.RiskSafe})
	d.Register(&mockTool{name: "aaa", riskLevel: types.RiskSafe})
	d.Register(&mockTool{name: "mmm", riskLevel: types.RiskSafe})

	names := d.List()
	if len(names) != 3 {
		t.Errorf("expected 3 tools, got %d", len(names))
	}
	if names[0] != "aaa" || names[1] != "mmm" || names[2] != "zzz" {
		t.Errorf("expected sorted [aaa mmm zzz], got %v", names)
	}
}
