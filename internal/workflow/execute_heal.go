package workflow

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/decision"
	"github.com/eshanized/M31A/internal/taskrunner"
	m31types "github.com/eshanized/M31A/internal/types"
)

func (e *Engine) healTask(ctx context.Context, task m31types.Task, failure string, goal string) taskrunner.TaskResult {
	start := time.Now()

	healPrompt := e.buildSystemPrompt(e.promptOrGet("tool-use"), e.promptOrGet("self-heal"), e.promptOrGet("code-quality"))
	if goal != "" {
		healPrompt += "\n\n## Original Goal\n" + goal
	}

	// Build enhanced heal context with acceptance criteria and diagnostic info
	healCtx := fmt.Sprintf(
		"## Self-Heal Request\n\nTask %d failed with the following error:\n\n%s\n\n"+
			"## Task Specification\n"+
			"- Action: %s\n"+
			"- Description: %s\n"+
			"- Files: %v\n"+
			"- Acceptance Criteria: %s\n"+
			"- Heal Attempts: %d/%d\n\n"+
			"## Current File State\n\n%s\n",
		task.ID, failure,
		task.Action, task.Description, task.Files,
		strings.Join(task.AcceptanceCriteria, "; "),
		task.HealsAttempted, m31types.MaxHealAttempts,
		e.readTaskFiles(task.Files),
	)

	// Add git diff of recent changes if available
	if e.git != nil {
		if diff, diffErr := e.git.Run("diff", "--stat", "HEAD~3..HEAD"); diffErr == nil && strings.TrimSpace(diff) != "" {
			healCtx += "\n## Recent Changes (last 3 commits)\n\n```\n" + diff + "\n```\n"
		}
	}

	// Add codebase intelligence context for the task files
	if ci := e.getCodeIntel(ctx); ci != nil {
		ciCtx := ci.FormatContext(task.Files, task.Description, 5, 2000)
		if ciCtx != "" {
			healCtx += "\n" + ciCtx + "\n"
		}
	}

	healCtx += "\nDiagnose the root cause using the diagnostic steps in your instructions, then apply a fix using your available tools." +
		" Prefer Edit for targeted changes; use FileWrite only when rewriting a file entirely."

	messages := []m31types.Message{
		{Role: "system", Content: healPrompt},
		{Role: "user", Content: healCtx},
	}

	content, toolCalls, err := e.streamLLMWithTools(ctx, messages)
	if err != nil {
		return taskrunner.TaskResult{Success: false, Error: err.Error(), DurationMs: time.Since(start).Milliseconds()}
	}

	// Fall back to text-based parsing if no native tool calls
	if len(toolCalls) == 0 {
		toolCalls, err = e.parseToolCalls(content)
		if err != nil {
			return taskrunner.TaskResult{
				Success:    false,
				Error:      fmt.Sprintf("heal: tool call parsing failed: %v", err),
				DurationMs: time.Since(start).Milliseconds(),
			}
		}
	}
	if len(toolCalls) == 0 {
		return taskrunner.TaskResult{
			Success:    false,
			Error:      "heal: no tool calls generated",
			DurationMs: time.Since(start).Milliseconds(),
		}
	}
	// Execute heal tool calls in parallel for faster healing
	type healResult struct {
		result m31types.ToolResult
		err    error
	}
	healResults := make([]healResult, len(toolCalls))
	var wg sync.WaitGroup
	for i, tc := range toolCalls {
		wg.Add(1)
		go func(idx int, call m31types.ToolCall) {
			defer wg.Done()
			r, err := e.dispatcher.Execute(ctx, call)
			healResults[idx] = healResult{result: r, err: err}
		}(i, tc)
	}
	wg.Wait()
	for i, hr := range healResults {
		if hr.err != nil {
			return taskrunner.TaskResult{
				Success:    false,
				Error:      fmt.Sprintf("heal tool %s: %v", toolCalls[i].Name, hr.err),
				DurationMs: time.Since(start).Milliseconds(),
			}
		}
		// Check for tool-level errors and truncation that aren't surfaced as Go errors
		if hr.result.Error != "" {
			return taskrunner.TaskResult{
				Success:    false,
				Error:      fmt.Sprintf("heal tool %s returned error: %s", toolCalls[i].Name, hr.result.Error),
				DurationMs: time.Since(start).Milliseconds(),
			}
		}
		if hr.result.Truncated {
			e.logger.Warn("heal tool produced truncated output", "tool", toolCalls[i].Name, "task", task.ID)
		}
	}

	// Commit fix scoped to task files
	var commitHash string
	if len(task.Files) > 0 && e.git != nil {
		hash, err := e.git.CommitWithFiles(
			fmt.Sprintf("%s: %s", e.gitConfig().FixPrefix, task.Description),
			task.Files...,
		)
		if err != nil {
			e.logger.Warn("heal commit failed", "task", task.ID, "error", err)
		} else {
			commitHash = hash
		}
	}

	// Verify the fix was actually applied by re-checking task files
	for _, f := range task.Files {
		path := filepath.Join(e.workDir, f)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			e.logger.Warn("heal did not create expected file", "task", task.ID, "file", f)
		}
	}

	// Generate HealReport for self-heal explanation
	errorType := classifyError(failure)
	healReport := &m31types.HealReport{
		TaskID:     task.ID,
		Attempt:    task.HealsAttempted,
		Success:    true,
		ErrorType:  errorType,
		ErrorMsg:   truncateForLog(failure, 500),
		Strategy:   describeHealStrategy(toolCalls),
		FilesUsed:  task.Files,
		DurationMs: time.Since(start).Milliseconds(),
		Timestamp:  time.Now(),
	}
	e.state.lastHealReport = healReport

	// Log decision for self-heal
	e.LogDecision(decision.DecisionReceipt{
		Decision:  fmt.Sprintf("self-heal succeeded for task %d (attempt %d)", task.ID, task.HealsAttempted),
		Rationale: fmt.Sprintf("error type: %s, strategy: %s", errorType, healReport.Strategy),
		Category:  decision.CategoryRetry,
		Cost: decision.Cost{
			Duration:   float64(healReport.DurationMs) / 1000.0,
			Attempts:   task.HealsAttempted,
			RetryCount: task.HealsAttempted,
		},
	})

	return taskrunner.TaskResult{
		Success:    true,
		Output:     content,
		CommitHash: commitHash,
		DurationMs: time.Since(start).Milliseconds(),
	}
}

func summarizeInput(input json.RawMessage, maxChars int) string {
	s := string(input)
	if len(s) <= maxChars {
		return s
	}
	return s[:maxChars] + "...(truncated)"
}

func (e *Engine) healCreatedExpectedFiles(task *m31types.Task) bool {
	if len(task.Files) == 0 {
		return true
	}
	for _, f := range task.Files {
		if _, err := os.Stat(filepath.Join(e.workDir, f)); os.IsNotExist(err) {
			return false
		}
	}
	return true
}

// classifyError categorizes an error message for the HealReport.
func classifyError(errMsg string) string {
	lower := strings.ToLower(errMsg)
	switch {
	case strings.Contains(lower, "build") || strings.Contains(lower, "compile"):
		return "build_error"
	case strings.Contains(lower, "test") || strings.Contains(lower, "fail"):
		return "test_failure"
	case strings.Contains(lower, "syntax") || strings.Contains(lower, "parse"):
		return "syntax_error"
	case strings.Contains(lower, "undefined") || strings.Contains(lower, "unresolved"):
		return "undefined_symbol"
	case strings.Contains(lower, "import") || strings.Contains(lower, "require"):
		return "import_error"
	case strings.Contains(lower, "permission") || strings.Contains(lower, "denied"):
		return "permission_error"
	case strings.Contains(lower, "timeout") || strings.Contains(lower, "deadline"):
		return "timeout"
	case strings.Contains(lower, "context window") || strings.Contains(lower, "token"):
		return "context_overflow"
	default:
		return "unknown"
	}
}

// describeHealStrategy returns a human-readable description of the heal tool calls.
func describeHealStrategy(toolCalls []m31types.ToolCall) string {
	if len(toolCalls) == 0 {
		return "no tools used"
	}
	var tools []string
	for _, tc := range toolCalls {
		tools = append(tools, tc.Name)
	}
	return fmt.Sprintf("used %s", strings.Join(tools, ", "))
}

// looksLikeCode detects when an LLM has printed code as plain text instead of
// using FileWrite/Edit tools. Uses heuristic pattern matching on common code
// structures. Returns true when the output looks like source code but contains
// no tool calls.
func looksLikeCode(content string) bool {
	if len(content) < 100 {
		return false
	}

	lower := strings.ToLower(content)

	// Go code patterns
	goPatterns := []string{
		"package main",
		"import (",
		"func main(",
		"func (",
		":=",
		"fmt.println(",
		"fmt.printf(",
		"if err != nil {",
		"return fmt.errorf(",
		"cobra.command{",
		"&cobra.command{",
	}

	// Python code patterns
	pythonPatterns := []string{
		"import ",
		"def main(",
		"if __name__",
		"print(",
		"class ",
		"def __init__",
	}

	// JavaScript/TypeScript code patterns
	jsPatterns := []string{
		"const ",
		"let ",
		"function ",
		"export ",
		"import {",
		"from '",
		"require(",
	}

	// Generic code indicators
	genericPatterns := []string{
		"{\n",
		"}\n",
		"()\n",
		"  }",
		"  {",
		"  return ",
		"  if (",
	}

	// Count matches across all pattern families
	score := 0
	for _, p := range goPatterns {
		if strings.Contains(lower, p) {
			score++
		}
	}
	for _, p := range pythonPatterns {
		if strings.Contains(lower, p) {
			score++
		}
	}
	for _, p := range jsPatterns {
		if strings.Contains(lower, p) {
			score++
		}
	}
	for _, p := range genericPatterns {
		if strings.Contains(content, p) {
			score++
		}
	}

	// Also check for code fences with language tags (model wrapping code in ```go etc.)
	if strings.Contains(content, "```go") || strings.Contains(content, "```python") ||
		strings.Contains(content, "```javascript") || strings.Contains(content, "```typescript") ||
		strings.Contains(content, "```js") || strings.Contains(content, "```ts") {
		score += 3
	}

	// Threshold: 3+ code-like patterns means this is code, not tool calls
	return score >= 3
}
