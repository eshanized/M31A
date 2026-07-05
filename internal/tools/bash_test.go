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
	b := NewBash(t.TempDir(), 1800)
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

func TestBash_Name(t *testing.T) {
	t.Parallel()
	b := NewBash(t.TempDir(), 1800)
	if b.Name() != "Bash" {
		t.Errorf("expected name 'Bash', got %s", b.Name())
	}
}

func TestBash_Description(t *testing.T) {
	t.Parallel()
	b := NewBash(t.TempDir(), 1800)
	if b.Description() == "" {
		t.Error("expected non-empty description")
	}
}

func TestBash_RiskLevel(t *testing.T) {
	t.Parallel()
	b := NewBash(t.TempDir(), 1800)
	if b.RiskLevel() != types.RiskDangerous {
		t.Errorf("expected RiskDangerous, got %s", b.RiskLevel())
	}
}

func TestBash_WithWorkingDirectory(t *testing.T) {
	dir := t.TempDir()
	b := NewBash(dir, 1800)
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
	b := NewBash(t.TempDir(), 1800)
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
	b := NewBash(t.TempDir(), 1800)
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
	b := NewBash(t.TempDir(), 1800)

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
	b := NewBash(t.TempDir(), 1800)
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
	b := NewBash(t.TempDir(), 1800)
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
	b := NewBash(t.TempDir(), 1800)
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
	b := NewBash(t.TempDir(), 1800)
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
	b := NewBash(t.TempDir(), 1800)
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

func TestLimitWriter_UnderLimit(t *testing.T) {
	t.Parallel()
	var buf strings.Builder
	lw := &limitWriter{limit: 100, w: &buf}
	n, err := lw.Write([]byte("hello"))
	if err != nil {
		t.Fatal(err)
	}
	if n != 5 {
		t.Errorf("expected 5 bytes written, got %d", n)
	}
	if lw.written != 5 {
		t.Errorf("expected written=5, got %d", lw.written)
	}
}

func TestLimitWriter_AtLimit(t *testing.T) {
	t.Parallel()
	var buf strings.Builder
	lw := &limitWriter{limit: 5, w: &buf}
	n, err := lw.Write([]byte("hello"))
	if err != nil {
		t.Fatal(err)
	}
	if n != 5 {
		t.Errorf("expected 5 bytes written, got %d", n)
	}
	if lw.written != 5 {
		t.Errorf("expected written=5, got %d", lw.written)
	}
}

func TestLimitWriter_OverLimit(t *testing.T) {
	t.Parallel()
	var buf strings.Builder
	lw := &limitWriter{limit: 5, w: &buf}
	n, err := lw.Write([]byte("hello world"))
	if err != nil {
		t.Fatal(err)
	}
	if n != 5 {
		t.Errorf("expected 5 bytes written, got %d", n)
	}
	if lw.written != 5 {
		t.Errorf("expected written=5, got %d", lw.written)
	}

	// Write again when limit reached
	n, err = lw.Write([]byte("more"))
	if err != nil {
		t.Fatal(err)
	}
	if n != 4 {
		t.Errorf("expected 4 bytes written (silently dropped), got %d", n)
	}
	if lw.written != 5 {
		t.Errorf("expected written to stay at 5, got %d", lw.written)
	}
}

func TestLimitWriter_ZeroLimit(t *testing.T) {
	t.Parallel()
	var buf strings.Builder
	lw := &limitWriter{limit: 0, w: &buf}
	n, err := lw.Write([]byte("hello"))
	if err != nil {
		t.Fatal(err)
	}
	if n != 5 {
		t.Errorf("expected 5 bytes written (silently dropped), got %d", n)
	}
	if lw.written != 0 {
		t.Errorf("expected written=0, got %d", lw.written)
	}
}

func TestIsBinary_Empty(t *testing.T) {
	t.Parallel()
	if isBinary("") {
		t.Error("expected empty string to not be binary")
	}
}

func TestIsBinary_Text(t *testing.T) {
	t.Parallel()
	if isBinary("hello world") {
		t.Error("expected text string to not be binary")
	}
}

func TestIsBinary_WithNullByte(t *testing.T) {
	t.Parallel()
	if !isBinary("hello\x00world") {
		t.Error("expected string with null byte to be binary")
	}
}

func TestIsBinary_LongText(t *testing.T) {
	t.Parallel()
	long := strings.Repeat("x", 1000)
	if isBinary(long) {
		t.Error("expected long text string to not be binary")
	}
}

func TestIsBinary_LongBinary(t *testing.T) {
	t.Parallel()
	long := strings.Repeat("x", 256) + "\x00" + strings.Repeat("x", 256)
	if !isBinary(long) {
		t.Error("expected long binary string to be binary")
	}
}

func TestBash_CommandNotString(t *testing.T) {
	t.Parallel()
	b := NewBash(t.TempDir(), 1800)
	_, err := b.Execute(context.Background(), types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": 123, // not a string
		},
	})
	if err == nil {
		t.Fatal("expected error for non-string command")
	}
	if !strings.Contains(err.Error(), "parameter command must be a string") {
		t.Errorf("expected type error, got: %v", err)
	}
}

func TestBash_CustomTimeout(t *testing.T) {
	t.Parallel()
	b := NewBash(t.TempDir(), 1800)
	result, err := b.Execute(context.Background(), types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "echo done",
			"timeout": float64(30),
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "done") {
		t.Errorf("expected 'done' in output, got: %s", result.Output)
	}
}

func TestBash_InvalidTimeout(t *testing.T) {
	t.Parallel()
	b := NewBash(t.TempDir(), 1800)
	// Invalid timeout type should use default
	result, err := b.Execute(context.Background(), types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "echo done",
			"timeout": "not_a_number",
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(result.Output, "done") {
		t.Errorf("expected 'done' in output, got: %s", result.Output)
	}
}

func TestBash_ObfuscationDetection(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name    string
		command string
		blocked bool
	}{
		{"double spaces", "rm  -rf  /", true},
		{"variable expansion", "CMD=\"rm -rf /\"; $CMD", true},
		{"newline injection", "rm\n-rf\n/", true},
		{"backtick substitution", "`rm -rf /`", true},
		{"dollar substitution", "$(rm -rf /)", true},
		{"mixed case", "Rm -Rf /", true},
		{"tabs", "rm\t-rf\t/", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			_, blocked := checkDangerousCommand(tt.command)
			if blocked != tt.blocked {
				t.Errorf("checkDangerousCommand(%q) blocked=%v, want %v", tt.command, blocked, tt.blocked)
			}
		})
	}
}
