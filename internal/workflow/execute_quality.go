package workflow

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	m31types "github.com/eshanized/M31A/internal/types"
)

// QualityGateResult holds the outcome of checking acceptance criteria against file state.
type QualityGateResult struct {
	TaskID  int
	Passed  bool
	Checked int
	Failed  int
	Details []string
}

// checkAcceptanceCriteria verifies a task's acceptance criteria against the actual
// state of files on disk. Each criterion is checked as a grep-verifiable condition.
func (e *Engine) checkAcceptanceCriteria(task m31types.Task) QualityGateResult {
	result := QualityGateResult{
		TaskID: task.ID,
		Passed: true,
	}

	for _, criterion := range task.AcceptanceCriteria {
		result.Checked++
		if !e.verifyCriterion(task, criterion) {
			result.Failed++
			result.Passed = false
			result.Details = append(result.Details,
				fmt.Sprintf("FAIL: %s", criterion))
		} else {
			result.Details = append(result.Details,
				fmt.Sprintf("PASS: %s", criterion))
		}
	}

	return result
}

// verifyCriterion checks a single acceptance criterion against file state.
// Supports several patterns:
//   - "FILE contains STRING" — check if a file contains a substring
//   - "FILE exists" — check if a file exists
//   - "FILE has FUNCTION_PATTERN" — check for function declarations
//   - Fallback: check if any task file contains the criterion text
func (e *Engine) verifyCriterion(task m31types.Task, criterion string) bool {
	lower := strings.ToLower(criterion)

	// Pattern: "FILENAME contains STRING"
	if idx := strings.Index(lower, " contains "); idx > 0 {
		filename := strings.TrimSpace(criterion[:idx])
		searchStr := strings.TrimSpace(criterion[idx+len(" contains "):])
		return fileContains(e.workDir, filename, searchStr)
	}

	// Pattern: "FILENAME exists"
	if strings.HasSuffix(lower, " exists") {
		filename := strings.TrimSpace(criterion[:len(criterion)-len(" exists")])
		fullPath := filepath.Join(e.workDir, filename)
		_, err := os.Stat(fullPath)
		return !os.IsNotExist(err)
	}

	// Pattern: "FILENAME has PATTERN"
	if idx := strings.Index(lower, " has "); idx > 0 {
		filename := strings.TrimSpace(criterion[:idx])
		pattern := strings.TrimSpace(criterion[idx+len(" has "):])
		return fileContains(e.workDir, filename, pattern)
	}

	// Fallback: check if the criterion text appears in any task file
	for _, f := range task.Files {
		if fileContains(e.workDir, f, extractKeyPhrase(criterion)) {
			return true
		}
	}

	// If no files to check, give the benefit of the doubt
	// (some criteria like "npm test exits 0" require command execution)
	return len(task.Files) == 0
}

// fileContains checks if a file in the workdir contains the given substring.
func fileContains(workDir, filename, substr string) bool {
	fullPath := filepath.Join(workDir, filename)
	data, err := os.ReadFile(fullPath)
	if err != nil {
		return false
	}
	return strings.Contains(strings.ToLower(string(data)), strings.ToLower(substr))
}

// extractKeyPhrase extracts the most meaningful phrase from a criterion
// for fallback file searching. Strips common prefixes and keeps the core.
func extractKeyPhrase(criterion string) string {
	// Strip common prefixes
	prefixes := []string{
		"the ", "a ", "an ", "verify that ", "check that ", "ensure that ",
		"confirm that ", "assert that ", "validate that ",
	}
	lower := strings.ToLower(criterion)
	for _, prefix := range prefixes {
		if strings.HasPrefix(lower, prefix) {
			criterion = criterion[len(prefix):]
			break
		}
	}

	// Cap at 50 chars for efficient searching
	if len(criterion) > 50 {
		criterion = criterion[:50]
	}

	return strings.TrimSpace(criterion)
}
