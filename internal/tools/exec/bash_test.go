package exec

import (
	"context"
	"os"
	"path/filepath"
	"strings"
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

func TestCheckDangerousCommand_RmRfRoot(t *testing.T) {
	t.Parallel()
	reason, blocked := CheckDangerousCommand("rm -rf /", nil, nil)
	if !blocked {
		t.Error("expected rm -rf / to be blocked")
	}
	if !Contains(reason, "recursive root deletion") {
		t.Errorf("expected reason to mention root deletion, got %q", reason)
	}
}

func TestCheckDangerousCommand_RmRfWildcard(t *testing.T) {
	t.Parallel()
	_, blocked := CheckDangerousCommand("rm -rf /*", nil, nil)
	if !blocked {
		t.Error("expected rm -rf /* to be blocked")
	}
}

func TestCheckDangerousCommand_Mkfs(t *testing.T) {
	t.Parallel()
	reason, blocked := CheckDangerousCommand("mkfs.ext4 /dev/sda1", nil, nil)
	if !blocked {
		t.Error("expected mkfs to be blocked")
	}
	if !Contains(reason, "filesystem formatting") {
		t.Errorf("expected filesystem formatting reason, got %q", reason)
	}
}

func TestCheckDangerousCommand_Dd(t *testing.T) {
	t.Parallel()
	_, blocked := CheckDangerousCommand("dd if=/dev/zero of=/dev/sda", nil, nil)
	if !blocked {
		t.Error("expected dd to be blocked")
	}
}

func TestCheckDangerousCommand_ForkBomb(t *testing.T) {
	t.Parallel()
	_, blocked := CheckDangerousCommand(":(){ :|:& };:", nil, nil)
	if !blocked {
		t.Error("expected fork bomb to be blocked")
	}
}

func TestCheckDangerousCommand_Shutdown(t *testing.T) {
	t.Parallel()
	for _, cmd := range []string{"shutdown", "reboot", "halt", "poweroff", "init 0", "init 6"} {
		t.Run(cmd, func(t *testing.T) {
			_, blocked := CheckDangerousCommand(cmd, nil, nil)
			if !blocked {
				t.Errorf("expected %q to be blocked", cmd)
			}
		})
	}
}

func TestCheckDangerousCommand_Killall(t *testing.T) {
	t.Parallel()
	_, blocked := CheckDangerousCommand("killall -9", nil, nil)
	if !blocked {
		t.Error("expected killall to be blocked")
	}
}

func TestCheckDangerousCommand_CurlToShell(t *testing.T) {
	t.Parallel()
	for _, cmd := range []string{"curl | sh", "wget | bash", "curl | bash"} {
		t.Run(cmd, func(t *testing.T) {
			_, blocked := CheckDangerousCommand(cmd, nil, nil)
			if !blocked {
				t.Errorf("expected %q to be blocked", cmd)
			}
		})
	}
}

func TestCheckDangerousCommand_Obfuscation_Base64(t *testing.T) {
	t.Parallel()
	reason, blocked := CheckDangerousCommand("echo Y2F0IC9ldGMvcGFzc3dk | base64 -d | sh", nil, nil)
	if !blocked {
		t.Error("expected base64 decode pipe to shell to be blocked")
	}
	if !Contains(reason, "base64 decode") {
		t.Errorf("expected base64 decode reason, got %q", reason)
	}
}

func TestCheckDangerousCommand_Obfuscation_Eval(t *testing.T) {
	t.Parallel()
	_, blocked := CheckDangerousCommand("eval $(echo rm -rf /)", nil, nil)
	if !blocked {
		t.Error("expected eval to be blocked")
	}
	// Command is blocked - exact reason may vary based on detection logic
}

func TestCheckDangerousCommand_CommandChaining(t *testing.T) {
	t.Parallel()
	reason, blocked := CheckDangerousCommand("echo hello; rm -rf /", nil, nil)
	if !blocked {
		t.Error("expected command chaining with rm to be blocked")
	}
	if !Contains(reason, "command in chain") {
		t.Errorf("expected chain reason, got %q", reason)
	}
}

func TestCheckDangerousCommand_CommandSubstitution(t *testing.T) {
	t.Parallel()
	_, blocked := CheckDangerousCommand("echo $(rm -rf /)", nil, nil)
	if !blocked {
		t.Error("expected command substitution with rm to be blocked")
	}
	// Command is blocked - exact reason may vary based on detection logic
}

func TestCheckDangerousCommand_BacktickSubstitution(t *testing.T) {
	t.Parallel()
	_, blocked := CheckDangerousCommand("echo `rm -rf /`", nil, nil)
	if !blocked {
		t.Error("expected backtick substitution with rm to be blocked")
	}
	// Command is blocked - exact reason may vary based on detection logic
}

func TestCheckDangerousCommand_VariableExpansion(t *testing.T) {
	t.Parallel()
	reason, blocked := CheckDangerousCommand("echo $HOME", nil, nil)
	if !blocked {
		t.Error("expected variable expansion to be blocked")
	}
	if !Contains(reason, "variable expansion") {
		t.Errorf("expected variable expansion reason, got %q", reason)
	}
}

func TestCheckDangerousCommand_CustomBlocklist(t *testing.T) {
	t.Parallel()
	reason, blocked := CheckDangerousCommand("customcmd arg1", []string{"customcmd"}, nil)
	if !blocked {
		t.Error("expected custom blocklist to block command")
	}
	if !Contains(reason, "customcmd") {
		t.Errorf("expected customcmd in reason, got %q", reason)
	}
}

func TestCheckDangerousCommand_CustomObfuscation(t *testing.T) {
	t.Parallel()
	reason, blocked := CheckDangerousCommand("evil-command arg", nil, []string{"evil"})
	if !blocked {
		t.Error("expected custom obfuscation to block command")
	}
	if !Contains(reason, "evil") {
		t.Errorf("expected evil in reason, got %q", reason)
	}
}

func TestCheckDangerousCommand_AllowedCommand(t *testing.T) {
	t.Parallel()
	reason, blocked := CheckDangerousCommand("echo hello world", nil, nil)
	if blocked {
		t.Errorf("expected echo to be allowed, got blocked: %s", reason)
	}
}

func TestNormalizeCommand(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input    string
		expected string
	}{
		{"echo hello world", "echo hello world"},
		{"  echo   hello  ", "echo hello"},
		{"ECHO HELLO", "echo hello"},
		{"echo # comment\nhello", "echo hello"},
		{"echo\t\thello", "echo hello"},
	}
	for _, tc := range tests {
		t.Run(tc.input, func(t *testing.T) {
			result := normalizeCommand(tc.input)
			if result != tc.expected {
				t.Errorf("normalizeCommand(%q) = %q, want %q", tc.input, result, tc.expected)
			}
		})
	}
}

func TestValidateCommandSyntax_Valid(t *testing.T) {
	t.Parallel()
	validCommands := []string{
		"echo hello",
		"echo 'hello world'",
		"echo \"hello world\"",
		"echo hello; echo world",
		"ls -la",
	}
	for _, cmd := range validCommands {
		t.Run(cmd, func(t *testing.T) {
			err := validateCommandSyntax(cmd)
			if err != nil {
				t.Errorf("expected valid command %q, got error: %v", cmd, err)
			}
		})
	}
}

func TestValidateCommandSyntax_UnbalancedSingleQuote(t *testing.T) {
	t.Parallel()
	err := validateCommandSyntax("echo 'unclosed")
	if err == nil {
		t.Error("expected error for unbalanced single quote")
	}
	if !Contains(err.Error(), "unbalanced single quotes") {
		t.Errorf("expected unbalanced single quotes error, got %q", err.Error())
	}
}

func TestValidateCommandSyntax_UnbalancedDoubleQuote(t *testing.T) {
	t.Parallel()
	err := validateCommandSyntax("echo \"unclosed")
	if err == nil {
		t.Error("expected error for unbalanced double quote")
	}
	if !Contains(err.Error(), "unbalanced double quotes") {
		t.Errorf("expected unbalanced double quotes error, got %q", err.Error())
	}
}

func TestContainsVariableExpansion(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input    string
		expected bool
	}{
		{"echo $VAR", true},
		{"echo ${VAR}", true},
		{"echo $((1+1))", true},
		{"echo $(cmd)", false}, // command substitution, not variable
		{"echo `cmd`", false},  // backtick substitution, not variable
		{"echo hello", false},
	}
	for _, tc := range tests {
		t.Run(tc.input, func(t *testing.T) {
			result := containsVariableExpansion(tc.input)
			if result != tc.expected {
				t.Errorf("containsVariableExpansion(%q) = %v, want %v", tc.input, result, tc.expected)
			}
		})
	}
}

func TestLimitWriter(t *testing.T) {
	t.Parallel()
	var buf strings.Builder
	lw := &limitWriter{limit: 10, w: &buf}

	// Write within limit
	n, err := lw.Write([]byte("hello"))
	if err != nil {
		t.Fatalf("Write failed: %v", err)
	}
	if n != 5 {
		t.Errorf("expected 5 bytes written, got %d", n)
	}
	if buf.String() != "hello" {
		t.Errorf("expected 'hello', got %q", buf.String())
	}

	// Write exceeding limit
	n, err = lw.Write([]byte("world!"))
	if err != nil {
		t.Fatalf("Write failed: %v", err)
	}
	if n != 5 { // only 5 bytes remaining (10 - 5 = 5)
		t.Errorf("expected 5 bytes written (limit), got %d", n)
	}
	if buf.String() != "helloworld" {
		t.Errorf("expected 'helloworld', got %q", buf.String())
	}

	// Write beyond limit (should silently drop)
	n, err = lw.Write([]byte("extra"))
	if err != nil {
		t.Fatalf("Write failed: %v", err)
	}
	if n != 0 {
		t.Errorf("expected 0 bytes written (over limit), got %d", n)
	}
	if buf.String() != "helloworld" {
		t.Errorf("expected 'helloworld' (no change), got %q", buf.String())
	}
}

// TestBash_WorkDir_SymlinkTraversal verifies that a symlink pointing outside
// the workspace is rejected by the workdir containment check. Regression test
// for M31A-AUDIT-006 (R5.3).
func TestBash_WorkDir_SymlinkTraversal(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	outsideDir := t.TempDir()

	// Create a symlink inside workDir that points outside
	symlinkPath := filepath.Join(workDir, "escape")
	if err := os.Symlink(outsideDir, symlinkPath); err != nil {
		t.Fatalf("failed to create symlink: %v", err)
	}

	b := NewBash(workDir, 60, nil, nil)

	_, err := b.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"command": "pwd",
			"workdir": symlinkPath,
		},
	})
	if err == nil {
		t.Fatal("expected error for symlink escaping workspace, got nil")
	}
	if !strings.Contains(err.Error(), "within the project directory") {
		t.Errorf("expected containment error, got: %v", err)
	}
}

// TestBash_WorkDir_SymlinkInsideWorkspace verifies that a symlink pointing to
// a directory within the workspace is allowed. Regression test for R5.3.
func TestBash_WorkDir_SymlinkInsideWorkspace(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	targetDir := filepath.Join(workDir, "real-subdir")
	if err := os.MkdirAll(targetDir, 0755); err != nil {
		t.Fatalf("failed to create target dir: %v", err)
	}

	symlinkPath := filepath.Join(workDir, "link-to-subdir")
	if err := os.Symlink(targetDir, symlinkPath); err != nil {
		t.Fatalf("failed to create symlink: %v", err)
	}

	b := NewBash(workDir, 60, nil, nil)

	result, err := b.Execute(context.Background(), types.ToolInput{
		Params: map[string]any{
			"command": "pwd",
			"workdir": symlinkPath,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error for symlink inside workspace: %v", err)
	}
	// Should resolve to the real path (within workspace)
	if !strings.Contains(result.Output, "real-subdir") {
		t.Errorf("expected resolved path containing 'real-subdir', got %q", result.Output)
	}
}
