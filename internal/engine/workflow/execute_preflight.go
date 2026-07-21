package workflow

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

// ExecutePreflightResult holds the outcome of pre-execution validation.
type ExecutePreflightResult struct {
	Passed bool
	Issues []string
}

// runExecutePreflight validates the task list before execution begins.
// Checks: dependency IDs reference existing tasks, "Modify" target files exist,
// no orphaned tasks.
func (e *Engine) runExecutePreflight(tasks []m31types.Task) ExecutePreflightResult {
	result := ExecutePreflightResult{Passed: true}

	idSet := make(map[int]bool)
	for _, t := range tasks {
		idSet[t.ID] = true
	}

	for _, task := range tasks {
		// Check dependency references
		for _, dep := range task.Dependencies {
			if !idSet[dep] {
				result.Issues = append(result.Issues,
					fmt.Sprintf("Task %d depends on non-existent task %d", task.ID, dep))
				result.Passed = false
			}
		}

		// Check that "Modify" action targets have existing files
		actionLower := strings.ToLower(task.Action)
		if actionLower == "modify" || actionLower == "update" || actionLower == "edit" {
			for _, f := range task.Files {
				fullPath := filepath.Join(e.workDir, f)
				if _, err := os.Stat(fullPath); os.IsNotExist(err) {
					result.Issues = append(result.Issues,
						fmt.Sprintf("Task %d modifies %q but file does not exist", task.ID, f))
					// Warning only — the file might be created by a dependency task
				}
			}
		}

		// Check for tasks with no files AND no acceptance criteria
		if len(task.Files) == 0 && len(task.AcceptanceCriteria) == 0 {
			result.Issues = append(result.Issues,
				fmt.Sprintf("Task %d has no files and no acceptance criteria", task.ID))
		}
	}

	if len(result.Issues) == 0 {
		result.Passed = true
	}

	return result
}

// toolCallSignature creates a fingerprint for a tool call to detect loops.
type toolCallSignature struct {
	Name string
	Args string // first 200 chars of input
}

// makeSignature creates a signature from a tool call for loop detection.
func makeSignature(name string, input string) toolCallSignature {
	args := input
	if len(args) > 200 {
		args = args[:200]
	}
	return toolCallSignature{Name: name, Args: args}
}

// ToolCallTracker tracks tool call patterns across iterations to detect loops.
type ToolCallTracker struct {
	history []toolCallSignature
	maxLoop int // threshold: same sig N times = loop
}

// NewToolCallTracker creates a tracker with the given loop threshold.
func NewToolCallTracker(maxLoop int) *ToolCallTracker {
	if maxLoop <= 0 {
		maxLoop = 3
	}
	return &ToolCallTracker{maxLoop: maxLoop}
}

// Record adds a tool call to the history and returns true if a loop is detected.
func (t *ToolCallTracker) Record(name, input string) bool {
	sig := makeSignature(name, input)
	t.history = append(t.history, sig)

	// Check last N entries for repeated signatures
	if len(t.history) >= t.maxLoop {
		recent := t.history[len(t.history)-t.maxLoop:]
		allSame := true
		for i := 1; i < len(recent); i++ {
			if recent[i] != recent[0] {
				allSame = false
				break
			}
		}
		return allSame
	}
	return false
}

// LastLoopTool returns the tool name involved in the most recent loop, or empty.
func (t *ToolCallTracker) LastLoopTool() string {
	if len(t.history) == 0 {
		return ""
	}
	return t.history[len(t.history)-1].Name
}

// Count returns total tool calls recorded.
func (t *ToolCallTracker) Count() int {
	return len(t.history)
}
