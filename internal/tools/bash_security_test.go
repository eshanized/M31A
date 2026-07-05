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
