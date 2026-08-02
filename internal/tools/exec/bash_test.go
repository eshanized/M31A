package exec

import (
	"context"
	"os"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

func TestBash_ExecuteSimpleCommand(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()

	b := NewBash(workDir, 60, nil, nil)

	result, err := b.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"command": "echo hello world",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Output != "hello world\n" {
		t.Errorf("expected 'hello world\\n', got %q", result.Output)
	}
	if result.Error != "" {
		t.Errorf("expected empty error, got %q", result.Error)
	}
}

func TestBash_ExecuteCommandWithExitCode(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()

	b := NewBash(workDir, 60, nil, nil)

	result, err := b.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"command": "false",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Error == "" {
		t.Errorf("expected error for 'false' command")
	}
	if result.DurationMs <= 0 {
		t.Errorf("expected positive duration")
	}
}

func TestBash_ExecuteCommandWithWorkDir(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	subDir := workDir + "/subdir"

	// Create subdir
	_ = os.Mkdir(subDir, 0755)

	b := NewBash(workDir, 60, nil, nil)

	result, err := b.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"command": "pwd",
			"workdir": subDir,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Output != subDir+"\n" {
		t.Errorf("expected %q, got %q", subDir+"\n", result.Output)
	}
}

func TestBash_ExecuteCommandTimeout(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()

	b := NewBash(workDir, 1, nil, nil) // 1 second max timeout

	result, err := b.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"command": "sleep 10",
			"timeout": 1,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// The command should be killed due to timeout
	if result.Error == "" {
		t.Errorf("expected timeout error")
	}
}

func TestBash_ExecuteBlockedCommand(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()

	b := NewBash(workDir, 60, []string{"rm"}, nil)

	_, err := b.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"command": "rm -rf /tmp/test",
		},
	})
	if err == nil {
		t.Errorf("expected blocked command error")
	}
}

func TestBash_ExecuteInvalidSyntax(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()

	b := NewBash(workDir, 60, nil, nil)

	_, err := b.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"command": "echo 'unclosed quote",
		},
	})
	if err == nil {
		t.Errorf("expected syntax error")
	}
}

func TestBash_RiskLevel(t *testing.T) {
	t.Parallel()
	b := NewBash("", 60, nil, nil)
	if b.RiskLevel() != types.RiskDangerous {
		t.Errorf("expected RiskDangerous, got %v", b.RiskLevel())
	}
}

func TestBash_Name(t *testing.T) {
	t.Parallel()
	b := NewBash("", 60, nil, nil)
	if b.Name() != "Bash" {
		t.Errorf("expected 'Bash', got %q", b.Name())
	}
}