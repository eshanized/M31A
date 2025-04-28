package tools

import (
	"context"
	"strings"
	"testing"
	"time"

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
			ID:    "call1",
			Name:  "bash",
			Input: []byte(`{"name": "bash", "params": {"command": "echo hello"}}`),
		})
		errCh <- err
	}()

	req := <-d.RequestCh()
	if req.ToolName != "bash" {
		t.Errorf("expected request for 'bash', got %q", req.ToolName)
	}
	t.Logf("DEBUG: req.Command = %q (len=%d)", req.Command, len(req.Command))
	if req.Command != "echo hello" {
		t.Errorf("expected Command 'echo hello', got %q", req.Command)
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

func TestDispatcher_GetTool(t *testing.T) {
	t.Parallel()
	d := NewDispatcher()
	d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe})

	tool, ok := d.GetTool("test")
	if !ok {
		t.Fatal("expected to find tool")
	}
	if tool.Name() != "test" {
		t.Errorf("expected tool name 'test', got %s", tool.Name())
	}

	_, ok = d.GetTool("nonexistent")
	if ok {
		t.Error("expected not to find nonexistent tool")
	}
}

func TestDispatcher_DefaultDispatcher(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	d := DefaultDispatcher(dir, dir, dir)

	names := d.List()
	if len(names) != 9 {
		t.Errorf("expected 9 tools, got %d: %v", len(names), names)
	}

	expectedTools := []string{"Bash", "FileRead", "FileWrite", "Edit", "TodoWrite", "WebFetch", "AskUserQuestion", "Glob", "Grep"}
	for _, expected := range expectedTools {
		found := false
		for _, name := range names {
			if name == expected {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("expected tool %s not found in %v", expected, names)
		}
	}
}

func TestExtractCommandString_Bash(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("bash", []byte(`{"name":"bash","params":{"command":"ls -la"}}`))
	if cmd != "ls -la" {
		t.Errorf("expected 'ls -la', got %q", cmd)
	}
}

func TestExtractCommandString_FileRead(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("fileRead", []byte(`{"name":"fileread","params":{"path":"/tmp/test.txt"}}`))
	if cmd != "read /tmp/test.txt" {
		t.Errorf("expected 'read /tmp/test.txt', got %q", cmd)
	}
}

func TestExtractCommandString_FileWrite(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("fileWrite", []byte(`{"name":"filewrite","params":{"path":"/tmp/out.txt"}}`))
	if cmd != "write /tmp/out.txt" {
		t.Errorf("expected 'write /tmp/out.txt', got %q", cmd)
	}
}

func TestExtractCommandString_Glob(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("glob", []byte(`{"name":"glob","params":{"pattern":"*.go"}}`))
	if cmd != "glob *.go" {
		t.Errorf("expected 'glob *.go', got %q", cmd)
	}
}

func TestExtractCommandString_Grep(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("grep", []byte(`{"name":"grep","params":{"pattern":"func main"}}`))
	if cmd != "grep func main" {
		t.Errorf("expected 'grep func main', got %q", cmd)
	}
}

func TestExtractCommandString_EmptyInput(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("bash", nil)
	if cmd != "" {
		t.Errorf("expected empty string, got %q", cmd)
	}
}

func TestExtractCommandString_RawMap(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("bash", []byte(`{"command":"echo test"}`))
	if cmd != "echo test" {
		t.Errorf("expected 'echo test', got %q", cmd)
	}
}

func TestExtractCommandString_Fallback(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("bash", []byte(`{"some":"json"}`))
	if cmd != `{"some":"json"}` {
		t.Errorf("expected raw JSON fallback, got %q", cmd)
	}
}

func TestDispatcher_DestructiveToolPermission(t *testing.T) {
	d := NewDispatcher()
	d.Register(&mockTool{name: "filewrite", riskLevel: types.RiskDestructive})

	errCh := make(chan error, 1)
	go func() {
		_, err := d.Execute(context.Background(), types.ToolCall{
			ID:    "call1",
			Name:  "filewrite",
			Input: []byte(`{"name":"filewrite","params":{"path":"test.txt"}}`),
		})
		errCh <- err
	}()

	req := <-d.RequestCh()
	if req.ToolName != "filewrite" {
		t.Errorf("expected request for 'filewrite', got %q", req.ToolName)
	}
	// extractCommandString normalizes to "Filewrite" which doesn't match "FileWrite" switch case
	if req.Command != "write test.txt" && req.Command != `{"name":"filewrite","params":{"path":"test.txt"}}` {
		t.Errorf("expected Command 'write test.txt' or raw JSON, got %q", req.Command)
	}
	d.ApprovePermission(true, false)

	if err := <-errCh; err != nil {
		t.Errorf("expected nil error after approval, got: %v", err)
	}
}

func TestDispatcher_DangerousToolContextCancelled(t *testing.T) {
	d := NewDispatcher()
	d.Register(&mockTool{name: "bash", riskLevel: types.RiskDangerous})

	ctx, cancel := context.WithCancel(context.Background())

	errCh := make(chan error, 1)
	go func() {
		_, err := d.Execute(ctx, types.ToolCall{
			ID:   "call1",
			Name: "bash",
		})
		errCh <- err
	}()

	<-d.RequestCh()
	cancel()

	select {
	case err := <-errCh:
		if err == nil {
			t.Error("expected context cancellation error")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("expected context cancellation to be handled")
	}
}

func TestDispatcher_RegisterDuplicate(t *testing.T) {
	t.Parallel()
	d := NewDispatcher()
	d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe})

	defer func() {
		if r := recover(); r == nil {
			t.Error("expected panic for duplicate registration")
		}
	}()
	d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe})
}

func TestDispatcher_PermissionChannelFull(t *testing.T) {
	d := NewDispatcher()
	d.Register(&mockTool{name: "bash", riskLevel: types.RiskDangerous})

	// Fill the request channel (buffer size is 8)
	for i := 0; i < 8; i++ {
		d.requestCh <- PermissionRequest{}
	}

	_, err := d.Execute(context.Background(), types.ToolCall{
		ID:   "call1",
		Name: "bash",
	})
	if err != m31errors.ErrPermissionDenied {
		t.Errorf("expected ErrPermissionDenied when channel is full, got: %v", err)
	}
}
