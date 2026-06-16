package tools

import (
	"testing"

	"github.com/eshanized/M31A/internal/config"
)

func TestDefaultDispatcher_CreatesAllTools(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := t.TempDir()
	sessionsDir := t.TempDir()

	d, err := DefaultDispatcher(workDir, backupDir, sessionsDir, nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if d == nil {
		t.Fatal("expected non-nil dispatcher")
	}

	expectedTools := []string{
		"Bash", "FileRead", "FileWrite", "Edit", "TodoWrite",
		"WebFetch", "WebSearch", "AskUserQuestion",
		"Glob", "Grep", "FileList", "FileDelete", "FileMove", "CodeMap",
	}
	for _, name := range expectedTools {
		if _, ok := d.GetTool(name); !ok {
			t.Errorf("expected tool %q to be registered", name)
		}
	}
}

func TestDefaultDispatcher_WithPermissions(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	backupDir := t.TempDir()
	sessionsDir := t.TempDir()

	cfg := &config.PermissionsConfig{
		DefaultMode: "allow",
	}
	d, err := DefaultDispatcher(workDir, backupDir, sessionsDir, cfg)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if d == nil {
		t.Fatal("expected non-nil dispatcher")
	}
}
