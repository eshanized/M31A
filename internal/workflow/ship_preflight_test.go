package workflow

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/eshanized/M31A/internal/git"
	m31types "github.com/eshanized/M31A/pkg/types"
)

func TestRunShipPreflight(t *testing.T) {
	dir := t.TempDir()
	e := &Engine{workDir: dir}

	// Create files with issues
	file1 := filepath.Join(dir, "main.go")
	os.WriteFile(file1, []byte(`package main
// TODO: fix this later
func main() {
	console.log("debug")
	fmt.Println("test")
}
`), 0644)

	file2 := filepath.Join(dir, "config.go")
	os.WriteFile(file2, []byte(`package config
const key = "sk-abc123secret"
`), 0644)

	tasks := []m31types.Task{
		{ID: 1, Files: []string{"main.go", "config.go"}},
	}

	result := e.runShipPreflight(tasks)

	// Should find TODO marker
	hasTODO := false
	for _, issue := range result.Issues {
		if strings.Contains(issue, "TODO") {
			hasTODO = true
		}
	}
	if !hasTODO {
		t.Error("expected TODO marker detection")
	}

	// Should find debug statement
	hasDebug := false
	for _, issue := range result.Issues {
		if strings.Contains(issue, "debug statement") {
			hasDebug = true
		}
	}
	if !hasDebug {
		t.Error("expected debug statement detection")
	}

	// Should find hardcoded secret (blocking)
	if result.Passed {
		t.Error("expected preflight to fail due to hardcoded secret")
	}
}

func TestRunShipPreflightClean(t *testing.T) {
	dir := t.TempDir()
	e := &Engine{workDir: dir}

	file := filepath.Join(dir, "clean.go")
	os.WriteFile(file, []byte("package main\nfunc main() {}\n"), 0644)

	tasks := []m31types.Task{
		{ID: 1, Files: []string{"clean.go"}},
	}

	result := e.runShipPreflight(tasks)
	if !result.Passed {
		t.Errorf("clean file should pass preflight, got issues: %v", result.Issues)
	}
}

func TestGenerateChangelog(t *testing.T) {
	e := &Engine{}

	tasks := []m31types.Task{
		{ID: 1, Action: "Create", Description: "Add user API", Category: "API", Status: m31types.StatusDone, Files: []string{"api/users.go"}},
		{ID: 2, Action: "Fix", Description: "Fix auth bug", Category: "Auth", Status: m31types.StatusDone, Files: []string{"auth.go"}},
		{ID: 3, Action: "Delete", Description: "Remove old config", Category: "Cleanup", Status: m31types.StatusDone, Files: []string{"old.yaml"}},
	}

	commits := []git.CommitInfo{
		{Hash: "abc1234def", Message: "feat: add user API"},
		{Hash: "def5678abc", Message: "fix: auth bug"},
	}

	changelog := e.generateChangelog(tasks, commits)

	if !strings.Contains(changelog, "Changelog") {
		t.Error("should contain title")
	}
	if !strings.Contains(changelog, "API") {
		t.Error("should contain API category")
	}
	if !strings.Contains(changelog, "Added") {
		t.Error("Create action should map to 'Added'")
	}
	if !strings.Contains(changelog, "Fixed") {
		t.Error("Fix action should map to 'Fixed'")
	}
	if !strings.Contains(changelog, "Removed") {
		t.Error("Delete action should map to 'Removed'")
	}
	if !strings.Contains(changelog, "abc1234") {
		t.Error("should contain truncated commit hash")
	}
}
