package workflow

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	m31types "github.com/eshanized/M31A/pkg/types"
)

func TestGenerateVerifyReport(t *testing.T) {
	e := &Engine{sessionID: "test123", state: &WorkflowState{planMarkdown: "# Plan\n## Verification Plan\n### Manual\n- Check the UI"}}

	tasks := []m31types.Task{
		{ID: 1, Description: "Create API", Status: m31types.StatusDone, Files: []string{"api.go"}},
		{ID: 2, Description: "Add tests", Status: m31types.StatusFailed, Files: []string{"test.go"}},
		{ID: 3, Description: "Skip task", Status: m31types.StatusSkipped},
	}

	verifyResults := map[int]VerificationResult{
		1: {TaskID: 1, FilesExist: true, SyntaxOK: true, TestsOK: true},
		2: {TaskID: 2, FilesExist: true, SyntaxOK: false, TestsOK: false, Errors: []string{"build failed"}},
	}

	report := e.generateVerifyReport(tasks, verifyResults)

	if !strings.Contains(report, "Verification Report") {
		t.Error("report should contain title")
	}
	if !strings.Contains(report, "test123") {
		t.Error("report should contain session ID")
	}
	if !strings.Contains(report, "33%") {
		t.Error("report should show 33% pass rate (1/3)")
	}
	if !strings.Contains(report, "build failed") {
		t.Error("report should contain error details")
	}
}

func TestScanSecurityFindings(t *testing.T) {
	dir := t.TempDir()
	e := &Engine{workDir: dir}

	// Create a file with security anti-patterns
	testFile := filepath.Join(dir, "config.go")
	os.WriteFile(testFile, []byte(`package config
const API_KEY = "sk-abc123secret"
func init() {
	eval(userInput)
}
`), 0644)

	tasks := []m31types.Task{
		{ID: 1, Files: []string{"config.go"}},
	}

	findings := e.scanSecurityFindings(tasks)

	if len(findings) == 0 {
		t.Error("expected security findings for hardcoded key and eval()")
	}

	hasKeyFinding := false
	hasEvalFinding := false
	for _, f := range findings {
		if f.Severity == "high" && strings.Contains(f.Pattern, "sk-") {
			hasKeyFinding = true
		}
		if f.Severity == "medium" && f.Pattern == "eval(" {
			hasEvalFinding = true
		}
	}

	if !hasKeyFinding {
		t.Error("expected finding for API key prefix 'sk-'")
	}
	if !hasEvalFinding {
		t.Error("expected finding for eval()")
	}
}

func TestFormatVerifyReport(t *testing.T) {
	report := VerifyReport{
		SessionID: "abc",
		Timestamp: time.Now(),
		Total:     3,
		Passed:    2,
		Failed:    1,
		Results: []VerifyTaskReport{
			{TaskID: 1, Description: "Good task", Status: "done", FilesExist: true, BuildOK: true, TestsOK: true},
			{TaskID: 2, Description: "Bad task", Status: "failed", FilesExist: true, BuildOK: false, TestsOK: false, Errors: []string{"compile error"}},
		},
		Security: []SecurityFinding{
			{Severity: "high", File: "auth.go", Line: 10, Pattern: "password", Message: "Hardcoded password"},
		},
		ManualSteps: []string{"Check the UI renders"},
	}

	output := formatVerifyReport(report)

	if !strings.Contains(output, "66%") {
		t.Error("should show 66% pass rate")
	}
	if !strings.Contains(output, "compile error") {
		t.Error("should contain error details")
	}
	if !strings.Contains(output, "Hardcoded password") {
		t.Error("should contain security finding")
	}
	if !strings.Contains(output, "Check the UI renders") {
		t.Error("should contain manual step")
	}
}
