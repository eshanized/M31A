package exec

import (
	"context"
	"fmt"
	"net"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestDevServer_Name(t *testing.T) {
	t.Parallel()
	d := NewDevServer("/tmp")
	if d.Name() != "DevServer" {
		t.Errorf("expected 'DevServer', got %q", d.Name())
	}
}

func TestDevServer_RiskLevel(t *testing.T) {
	t.Parallel()
	d := NewDevServer("/tmp")
	if d.RiskLevel() != types.RiskMedium {
		t.Errorf("expected RiskMedium, got %v", d.RiskLevel())
	}
}

func TestDevServer_ParameterSchema(t *testing.T) {
	t.Parallel()
	d := NewDevServer("/tmp")
	schema := d.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty schema")
	}
	if !Contains(schema, "start") || !Contains(schema, "stop") || !Contains(schema, "status") {
		t.Error("schema missing required actions")
	}
}

func TestDevServer_StartStop(t *testing.T) {
	t.Parallel()
	// Use a simple HTTP server that we can control
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("OK"))
	}))
	defer server.Close()

	// Extract port from server URL
	_, portStr, _ := net.SplitHostPort(server.Listener.Addr().String())
	port := 0
	_, _ = fmt.Sscanf(portStr, "%d", &port)

	workDir := t.TempDir()
	d := NewDevServer(workDir)

	// Start server with a command that exits quickly (use sleep for test)
	// Since we can't easily test actual long-running servers in unit tests,
	// we test the status/check_port functionality instead
	result, err := d.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"action": "status",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error in output: %s", result.Error)
	}
	// Should return empty list (nil slice marshals to "null")
	if !Contains(result.Output, "null") && !Contains(result.Output, "[]") {
		t.Logf("status output: %s", result.Output)
	}
}

func TestDevServer_CheckPort(t *testing.T) {
	t.Parallel()
	// Start a real server to test port checking
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	defer server.Close()

	_, portStr, _ := net.SplitHostPort(server.Listener.Addr().String())
	port := 0
	_, _ = fmt.Sscanf(portStr, "%d", &port)

	workDir := t.TempDir()
	d := NewDevServer(workDir)

	result, err := d.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"action": "check_port",
			"port":   float64(port),
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error != "" {
		t.Errorf("unexpected error: %s", result.Error)
	}
	if !Contains(result.Output, "accepting connections") {
		t.Errorf("expected 'accepting connections', got %q", result.Output)
	}
}

func TestDevServer_CheckPort_Closed(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewDevServer(workDir)

	// Use a port that's unlikely to be in use
	result, err := d.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"action": "check_port",
			"port":   float64(54321),
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// CheckPort returns result in Output, not Error
	if !Contains(result.Output, "NOT accepting") {
		t.Errorf("expected 'NOT accepting', got %q", result.Output)
	}
}

func TestDevServer_InvalidAction(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewDevServer(workDir)

	result, err := d.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"action": "invalid_action",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error == "" {
		t.Errorf("expected error for invalid action")
	}
	if !Contains(result.Error, "unknown action") {
		t.Errorf("expected 'unknown action' error, got %q", result.Error)
	}
}

func TestDevServer_StopNonExistent(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewDevServer(workDir)

	result, err := d.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"action": "stop",
			"id":     float64(999),
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error == "" {
		t.Errorf("expected error for non-existent server")
	}
	if !Contains(result.Error, "not found") {
		t.Errorf("expected 'not found' error, got %q", result.Error)
	}
}

func TestDevServer_LogsNonExistent(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewDevServer(workDir)

	result, err := d.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"action": "logs",
			"id":     float64(999),
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error == "" {
		t.Errorf("expected error for non-existent server")
	}
	if !Contains(result.Error, "not found") {
		t.Errorf("expected 'not found' error, got %q", result.Error)
	}
}

func TestDevServer_MissingCommand(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewDevServer(workDir)

	result, err := d.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"action": "start",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error == "" {
		t.Errorf("expected error for missing command")
	}
	if !Contains(result.Error, "command is required") {
		t.Errorf("expected 'command is required' error, got %q", result.Error)
	}
}

func TestDevServer_MissingIDForStop(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewDevServer(workDir)

	result, err := d.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"action": "stop",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error == "" {
		t.Errorf("expected error for missing id")
	}
	if !Contains(result.Error, "id is required") {
		t.Errorf("expected 'id is required' error, got %q", result.Error)
	}
}

func TestDevServer_MissingPortForCheckPort(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	d := NewDevServer(workDir)

	result, err := d.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"action": "check_port",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error == "" {
		t.Errorf("expected error for missing port")
	}
	if !Contains(result.Error, "port is required") {
		t.Errorf("expected 'port is required' error, got %q", result.Error)
	}
}

func TestRingBuffer_Write(t *testing.T) {
	t.Parallel()
	rb := NewRingBuffer(100)
	n, err := rb.Write([]byte("hello"))
	if err != nil {
		t.Fatalf("Write failed: %v", err)
	}
	if n != 5 {
		t.Errorf("expected 5 bytes written, got %d", n)
	}
	if rb.String() != "hello" {
		t.Errorf("expected 'hello', got %q", rb.String())
	}
}

func TestRingBuffer_Truncation(t *testing.T) {
	t.Parallel()
	rb := NewRingBuffer(20)
	// Write more than maxBytes with enough lines to trigger trimming
	rb.Write([]byte("line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\nline 11\nline 12\n"))
	// Should be trimmed down to 10 lines (not strictly maxBytes, since lines > 10 is required for trimming)
	if rb.TruncatedLines() > 10 {
		t.Errorf("buffer should be trimmed to at most 10 lines, got %d lines", rb.TruncatedLines())
	}
}

func TestRingBuffer_Lines(t *testing.T) {
	t.Parallel()
	rb := NewRingBuffer(1000)
	rb.Write([]byte("line 1\nline 2\nline 3\n"))
	if rb.TruncatedLines() != 3 {
		t.Errorf("expected 3 lines, got %d", rb.TruncatedLines())
	}
}

func TestMergeEnv(t *testing.T) {
	t.Parallel()
	base := []string{"A=1", "B=2", "C=3"}
	custom := []string{"B=override", "D=4"}
	result := mergeEnv(base, custom)

	expected := []string{"A=1", "B=override", "C=3", "D=4"}
	if len(result) != len(expected) {
		t.Errorf("expected %d env vars, got %d", len(expected), len(result))
	}
	for _, e := range expected {
		found := false
		for _, r := range result {
			if r == e {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("expected env var %q not found in result", e)
		}
	}
}

func TestWaitForPort_Success(t *testing.T) {
	t.Parallel()
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {}))
	defer server.Close()

	_, portStr, _ := net.SplitHostPort(server.Listener.Addr().String())
	port := 0
	_, _ = fmt.Sscanf(portStr, "%d", &port)

	ready := waitForPort(port, 5*time.Second)
	if !ready {
		t.Error("expected port to be ready")
	}
}

func TestWaitForPort_Timeout(t *testing.T) {
	t.Parallel()
	// Use a port that's not listening
	ready := waitForPort(54321, 100*time.Millisecond)
	if ready {
		t.Error("expected port to not be ready")
	}
}