package workflow

import (
	"os"
	"path/filepath"
	"testing"

	m31types "github.com/eshanized/M31A/internal/types"
)

func TestCheckAcceptanceCriteria(t *testing.T) {
	dir := t.TempDir()
	e := &Engine{workDir: dir}

	// Create a test file
	testFile := filepath.Join(dir, "main.go")
	os.WriteFile(testFile, []byte(`package main

func initRouter() {
	// router setup
}

func main() {
	initRouter()
}
`), 0644)

	task := m31types.Task{
		ID:          1,
		Description: "Create router",
		Files:       []string{"main.go"},
		AcceptanceCriteria: []string{
			"main.go contains func initRouter(",
			"main.go exists",
		},
	}

	result := e.checkAcceptanceCriteria(task)
	if !result.Passed {
		t.Errorf("expected all criteria to pass, got %d failures", result.Failed)
		for _, d := range result.Details {
			t.Log(d)
		}
	}
	if result.Checked != 2 {
		t.Errorf("checked = %d, want 2", result.Checked)
	}
}

func TestCheckAcceptanceCriteriaFail(t *testing.T) {
	dir := t.TempDir()
	e := &Engine{workDir: dir}

	// Create a file without the expected content
	testFile := filepath.Join(dir, "api.go")
	os.WriteFile(testFile, []byte("package api\n"), 0644)

	task := m31types.Task{
		ID:    1,
		Files: []string{"api.go"},
		AcceptanceCriteria: []string{
			"api.go contains func HandleRequest(",
		},
	}

	result := e.checkAcceptanceCriteria(task)
	if result.Passed {
		t.Error("expected criteria to fail — func HandleRequest not in file")
	}
	if result.Failed != 1 {
		t.Errorf("failed = %d, want 1", result.Failed)
	}
}

func TestFileContains(t *testing.T) {
	dir := t.TempDir()
	testFile := filepath.Join(dir, "test.go")
	os.WriteFile(testFile, []byte("package main\nfunc Hello() string { return \"world\" }\n"), 0644)

	if !fileContains(dir, "test.go", "func Hello()") {
		t.Error("expected fileContains to find 'func Hello()'")
	}
	if fileContains(dir, "test.go", "func Missing()") {
		t.Error("expected fileContains to NOT find 'func Missing()'")
	}
	if fileContains(dir, "nonexistent.go", "anything") {
		t.Error("expected fileContains to return false for missing file")
	}
}

func TestExtractKeyPhrase(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"verify that the API returns 200", "the API returns 200"},
		{"check that auth middleware works", "auth middleware works"},
		{"simple criterion", "simple criterion"},
		{"ensure that the database connection is properly configured with TLS", "the database connection is properly configured wit"},
	}

	for _, tt := range tests {
		got := extractKeyPhrase(tt.input)
		if got != tt.want {
			t.Errorf("extractKeyPhrase(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}
