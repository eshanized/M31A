//go:build !windows

package shell

import (
	"context"
	"testing"
	"time"
)

func TestCommandContext(t *testing.T) {
	ctx := context.Background()
	cmd := CommandContext(ctx, "echo hello")

	if cmd == nil {
		t.Fatal("CommandContext() returned nil")
	}
	if cmd.Args[0] != "sh" {
		t.Errorf("Args[0] = %q, want %q", cmd.Args[0], "sh")
	}
	if cmd.Args[1] != "-c" {
		t.Errorf("Args[1] = %q, want %q", cmd.Args[1], "-c")
	}
	if cmd.Args[2] != "echo hello" {
		t.Errorf("Args[2] = %q, want %q", cmd.Args[2], "echo hello")
	}
}

func TestCommandContext_Execution(t *testing.T) {
	ctx := context.Background()
	cmd := CommandContext(ctx, "echo test123")

	output, err := cmd.Output()
	if err != nil {
		t.Fatalf("cmd.Output() error = %v", err)
	}

	if string(output) != "test123\n" {
		t.Errorf("output = %q, want %q", string(output), "test123\n")
	}
}

func TestCommandContext_Cancellation(t *testing.T) {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Millisecond)
	defer cancel()

	cmd := CommandContext(ctx, "sleep 10")
	err := cmd.Run()
	if err == nil {
		t.Error("expected error for cancelled context")
	}
}

func TestCommand(t *testing.T) {
	cmd := Command("echo hello")

	if cmd == nil {
		t.Fatal("Command() returned nil")
	}
	if cmd.Args[0] != "sh" {
		t.Errorf("Args[0] = %q, want %q", cmd.Args[0], "sh")
	}
	if cmd.Args[1] != "-c" {
		t.Errorf("Args[1] = %q, want %q", cmd.Args[1], "-c")
	}
	if cmd.Args[2] != "echo hello" {
		t.Errorf("Args[2] = %q, want %q", cmd.Args[2], "echo hello")
	}
}

func TestCommand_Execution(t *testing.T) {
	cmd := Command("echo test456")

	output, err := cmd.Output()
	if err != nil {
		t.Fatalf("cmd.Output() error = %v", err)
	}

	if string(output) != "test456\n" {
		t.Errorf("output = %q, want %q", string(output), "test456\n")
	}
}

func TestCommand_Pipe(t *testing.T) {
	cmd := Command("echo hello | tr a-z A-Z")

	output, err := cmd.Output()
	if err != nil {
		t.Fatalf("cmd.Output() error = %v", err)
	}

	if string(output) != "HELLO\n" {
		t.Errorf("output = %q, want %q", string(output), "HELLO\n")
	}
}

func TestCommand_Redirect(t *testing.T) {
	cmd := Command("echo test > /dev/null")

	err := cmd.Run()
	if err != nil {
		t.Errorf("cmd.Run() error = %v", err)
	}
}

func TestCommand_ExitCode(t *testing.T) {
	cmd := Command("exit 1")

	err := cmd.Run()
	if err == nil {
		t.Error("expected error for exit code 1")
	}
}
