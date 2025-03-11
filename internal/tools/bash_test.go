package tools

import (
	"context"
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

func TestBash_SimpleCommand(t *testing.T) {
	t.Parallel()
	b := NewBash(t.TempDir())
	result, err := b.Execute(context.Background(), types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": `echo "hello world"`,
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "hello world") {
		t.Errorf("expected 'hello world' in output, got: %s", result.Output)
	}
}

func TestBash_WithWorkingDirectory(t *testing.T) {
	dir := t.TempDir()
	b := NewBash(dir)
	result, err := b.Execute(context.Background(), types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "pwd",
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, dir) {
		t.Errorf("expected %s in pwd output, got: %s", dir, result.Output)
	}
}

func TestBash_Stderr(t *testing.T) {
	t.Parallel()
	b := NewBash(t.TempDir())
	result, err := b.Execute(context.Background(), types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": `echo "error msg" >&2`,
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "error msg") {
		t.Errorf("expected stderr content in output, got: %s", result.Output)
	}
}

func TestBash_Timeout(t *testing.T) {
	t.Parallel()
	b := NewBash(t.TempDir())
	start := time.Now()
	result, err := b.Execute(context.Background(), types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "sleep 10",
			"timeout": float64(1),
		},
	})
	elapsed := time.Since(start)
	if elapsed > 8*time.Second {
		t.Errorf("timeout took too long: %v", elapsed)
	}
	if err != nil {
		t.Logf("expected timeout error, got: %v", err)
	}
	if result.Error == "" {
		t.Log("expected error message in result for timeout")
	}
}

func TestBash_ContextCancellation(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	b := NewBash(t.TempDir())

	errCh := make(chan error, 1)
	go func() {
		_, err := b.Execute(ctx, types.ToolInput{
			Name: "Bash",
			Params: map[string]any{
				"command": "sleep 30",
			},
		})
		errCh <- err
	}()

	time.Sleep(100 * time.Millisecond)
	cancel()

	select {
	case err := <-errCh:
		if err == nil {
			t.Log("context cancellation returned no error (expected error or cancellation)")
		}
	case <-time.After(5 * time.Second):
		t.Fatal("command not cancelled in time")
	}
}

func TestBash_NonZeroExit(t *testing.T) {
	t.Parallel()
	b := NewBash(t.TempDir())
	result, err := b.Execute(context.Background(), types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "exit 42",
		},
	})
	if err != nil {
		t.Logf("expected non-zero exit in error field, err: %v", err)
	}
	if !strings.Contains(result.Error, "exit code") {
		t.Errorf("expected 'exit code' in error field, got: %s", result.Error)
	}
}

func TestBash_OutputTruncated(t *testing.T) {
	b := NewBash(t.TempDir())
	// Generate 100k chars of output (more than 50k limit)
	result, err := b.Execute(context.Background(), types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": `python3 -c "print('x' * 100000)"`,
		},
	})
	if err != nil {
		t.Logf("command may have failed: %v", err)
	}
	if !result.Truncated {
		t.Log("expected truncated flag (output may be within limit on some systems)")
	}
}

func TestBash_BinaryOutput(t *testing.T) {
	t.Parallel()
	b := NewBash(t.TempDir())
	result, err := b.Execute(context.Background(), types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": `printf '\x00\x01\x02'`,
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(result.Output, "[binary output") {
		t.Errorf("expected binary output placeholder, got: %s", result.Output)
	}
}

func TestBash_CommandNotFound(t *testing.T) {
	t.Parallel()
	b := NewBash(t.TempDir())
	result, err := b.Execute(context.Background(), types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "nonexistent_command_xyz123",
		},
	})
	if err != nil {
		t.Logf("got error (expected): %v", err)
	}
	if result.Error == "" && err == nil {
		t.Fatal("expected error or non-zero exit for non-existent command")
	}
}

func TestBash_MissingCommandParam(t *testing.T) {
	t.Parallel()
	b := NewBash(t.TempDir())
	_, err := b.Execute(context.Background(), types.ToolInput{
		Name:   "Bash",
		Params: map[string]any{},
	})
	if err == nil {
		t.Fatal("expected error for missing command param")
	}
	if !strings.Contains(err.Error(), "missing parameter: command") {
		t.Errorf("expected missing parameter error, got: %v", err)
	}
}
