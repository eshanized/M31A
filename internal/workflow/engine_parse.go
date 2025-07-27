package workflow

import (
	"encoding/json"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"sync/atomic"

	m31errors "github.com/eshanized/M31A/internal/errors"
	m31types "github.com/eshanized/M31A/internal/types"
)

// parseTasksFromJSON extracts tasks from LLM response, stripping markdown code blocks.
func parseTasksFromJSON(content string) ([]m31types.Task, error) {
	// Strip markdown code blocks
	content = stripCodeBlocks(content)

	// Try to find JSON array in content
	jsonStr := extractJSONArray(content)
	if jsonStr == "" {
		return nil, fmt.Errorf("no JSON array found in response")
	}

	var tasks []m31types.Task
	if err := json.Unmarshal([]byte(jsonStr), &tasks); err != nil {
		return nil, fmt.Errorf("JSON parse error: %w", err)
	}

	return tasks, nil
}

// stripCodeBlocks removes markdown code fences from content.
func stripCodeBlocks(content string) string {
	// Remove ```<any lang> ... ``` blocks
	re := regexp.MustCompile("(?s)```(?:\\w+)?\\s*\n(.*?)```")
	return re.ReplaceAllString(content, "$1")
}

// extractJSONArray tries to find a JSON array in the content.
// Uses bracket-depth tracking to correctly handle nested structures and
// bracket characters in surrounding text.
func extractJSONArray(content string) string {
	content = strings.TrimSpace(content)

	// If content starts with [, try to parse directly
	if strings.HasPrefix(content, "[") {
		return extractArrayFrom(content)
	}

	// Search for the first [ in content
	idx := strings.Index(content, "[")
	if idx < 0 {
		return ""
	}
	return extractArrayFrom(content[idx:])
}

// extractArrayFrom finds the first complete JSON array in s using
// bracket-depth tracking. It skips brackets inside strings and handles
// escape characters. Returns "" if no complete array is found.
func extractArrayFrom(s string) string {
	depth := 0
	inString := false
	escaped := false

	for i, c := range s {
		if escaped {
			escaped = false
			continue
		}
		switch c {
		case '\\':
			if inString {
				escaped = true
			}
		case '"':
			inString = !inString
		case '[':
			if !inString {
				depth++
			}
		case ']':
			if !inString {
				depth--
				if depth == 0 {
					return s[:i+1]
				}
			}
		}
	}
	return ""
}

// validateTasks checks task list for schema violations.
func validateTasks(tasks []m31types.Task) []string {
	var errs []string
	idSet := make(map[int]bool)

	for _, t := range tasks {
		if t.ID == 0 {
			errs = append(errs, fmt.Sprintf("task: missing ID"))
			continue
		}
		if idSet[t.ID] {
			errs = append(errs, fmt.Sprintf("task %d: duplicate ID", t.ID))
		}
		idSet[t.ID] = true

		for _, dep := range t.Dependencies {
			if dep == t.ID {
				errs = append(errs, fmt.Sprintf("task %d: self-reference", t.ID))
			}
		}

		if t.Description == "" {
			errs = append(errs, fmt.Sprintf("task %d: missing description", t.ID))
		}
		if t.Action == "" {
			errs = append(errs, fmt.Sprintf("task %d: missing action", t.ID))
		}
	}

	// Check all deps reference existing tasks
	for _, t := range tasks {
		for _, dep := range t.Dependencies {
			if !idSet[dep] {
				errs = append(errs, fmt.Sprintf("task %d: references non-existent dependency %d", t.ID, dep))
			}
		}
	}

	// Cycle detection
	if hasCycle(tasks) {
		errs = append(errs, "circular dependency detected")
	}

	return errs
}

// hasCycle detects circular dependencies using iterative DFS with explicit stack.
func hasCycle(tasks []m31types.Task) bool {
	idSet := make(map[int]bool)
	for _, t := range tasks {
		idSet[t.ID] = true
	}

	adj := make(map[int][]int)
	for _, t := range tasks {
		adj[t.ID] = t.Dependencies
	}

	visited := make(map[int]bool)
	inStack := make(map[int]bool)

	for _, t := range tasks {
		if visited[t.ID] {
			continue
		}
		// Iterative DFS using explicit stack
		type stackEntry struct {
			id         int
			depIdx     int
			firstVisit bool
		}
		stack := []stackEntry{{id: t.ID, firstVisit: true}}

		for len(stack) > 0 {
			entry := &stack[len(stack)-1]

			if entry.firstVisit {
				entry.firstVisit = false
				if inStack[entry.id] {
					return true
				}
				if visited[entry.id] {
					stack = stack[:len(stack)-1]
					continue
				}
				visited[entry.id] = true
				inStack[entry.id] = true
			}

			// Process next dependency
			found := false
			for entry.depIdx < len(adj[entry.id]) {
				dep := adj[entry.id][entry.depIdx]
				entry.depIdx++
				if inStack[dep] {
					return true
				}
				if !visited[dep] {
					stack = append(stack, stackEntry{id: dep, firstVisit: true})
					found = true
					break
				}
			}
			if !found {
				inStack[entry.id] = false
				stack = stack[:len(stack)-1]
			}
		}
	}
	return false
}

// detectProjectType scans the working directory for project indicators.
func detectProjectType(workDir string) string {
	detectors := map[string]string{
		"go.mod":           "go",
		"package.json":     "nodejs",
		"Cargo.toml":       "rust",
		"pyproject.toml":   "python",
		"requirements.txt": "python",
		"pom.xml":          "java",
		"Makefile":         "cc",
		"CMakeLists.txt":   "cc",
	}
	for file, typ := range detectors {
		if _, err := os.Stat(filepath.Join(workDir, file)); err == nil {
			return typ
		}
	}
	return "unknown"
}

// parseQuestions extracts numbered questions from LLM response.
func parseQuestions(content string) []string {
	// Match numbered questions: "1. What..." or "2. How..."
	re := regexp.MustCompile(`(\d+)\.\s+(.+)`)
	matches := re.FindAllStringSubmatch(content, -1)
	var questions []string
	for _, m := range matches {
		if len(m) > 2 {
			q := strings.TrimSpace(m[2])
			// Remove trailing content after newline
			if idx := strings.Index(q, "\n"); idx > 0 {
				q = strings.TrimSpace(q[:idx])
			}
			if len(q) > 0 {
				questions = append(questions, q)
			}
		}
	}

	// Fallback: look for question-like lines
	if len(questions) == 0 {
		for _, line := range strings.Split(content, "\n") {
			line = strings.TrimSpace(line)
			if len(line) > 10 && strings.Contains(line, "?") {
				questions = append(questions, line)
			}
		}
	}

	// Cap at 4
	if len(questions) > 4 {
		questions = questions[:4]
	}

	return questions
}

// Fix C-4: Maximum number of tool calls to extract from a single response.
const maxToolsPerCall = 16

// Fix C-4: Maximum bytes to scan when looking for a single JSON object.
// Prevents unbounded scanning of very large LLM replies.
const maxJSONScanBytes = 64 << 10 // 64 KB

// parseToolCalls extracts tool calls from response content.
// Looks for JSON objects with a "name" or "tool" field inside code blocks or inline.
// Returns ErrToolExecution when the LLM emits malformed JSON that looks like a
// tool call but cannot be parsed (H-1). Accepts both array and single-object
// forms (M-3).
func (e *Engine) parseToolCalls(content string) ([]m31types.ToolCall, error) {
	// Fix C-4: reject oversized LLM responses to prevent OOM.
	if len(content) > m31types.MaxLLMResponseBytes {
		slog.Warn("parseToolCalls: oversized LLM response rejected",
			"bytes", len(content), "limit", m31types.MaxLLMResponseBytes)
		return nil, m31errors.ErrToolInputTooLarge
	}

	var calls []m31types.ToolCall
	var parseErrors int
	var totalAttempts int

	// Pattern 1: JSON in code blocks ```<any lang> {...} ```
	blockRe := regexp.MustCompile("(?s)```(?:\\w+)?\\s*\n(.*?)```")
	for _, match := range blockRe.FindAllStringSubmatch(content, -1) {
		if len(match) < 2 {
			continue
		}
		totalAttempts++
		tc, err := parseSingleToolCall(match[1], e.nextCallID())
		if err != nil {
			parseErrors++
			slog.Warn("parseToolCalls: malformed tool call in code block", "error", err)
		} else if tc != nil {
			calls = append(calls, *tc)
		}
	}

	// Pattern 2: Try to find standalone JSON objects with tool call fields
	if len(calls) == 0 {
		// Scan at each { position, use extractJSONObject for proper nesting
		scanLimit := len(content)
		if scanLimit > maxJSONScanBytes {
			scanLimit = maxJSONScanBytes
		}
		for i := 0; i < scanLimit; i++ {
			if content[i] != '{' {
				continue
			}
			obj := extractJSONObject(content[i:])
			if obj == "" {
				continue
			}
			// Quick pre-check: does this object have "name" or "tool" field?
			if !strings.Contains(obj, `"name"`) && !strings.Contains(obj, `"tool"`) {
				i += len(obj) - 1 // skip past this object
				continue
			}
			totalAttempts++
			tc, err := parseSingleToolCall(obj, e.nextCallID())
			if err != nil {
				parseErrors++
				slog.Warn("parseToolCalls: malformed tool call object", "error", err)
			} else if tc != nil {
				calls = append(calls, *tc)
				i += len(obj) - 1 // skip past this object
				continue
			}
		}
	}

	// H-1: If we attempted to parse tool calls but all were malformed, return error.
	if len(calls) == 0 && parseErrors > 0 && totalAttempts > 0 {
		return nil, fmt.Errorf("parse tool calls: %d malformed JSON objects detected: %w", parseErrors, m31errors.ErrToolExecution)
	}

	// Fix C-4: cap tool count to detect model regression.
	if len(calls) > maxToolsPerCall {
		slog.Warn("parseToolCalls: tool count exceeded cap, truncating",
			"count", len(calls), "cap", maxToolsPerCall)
		calls = calls[:maxToolsPerCall]
	}

	return calls, nil
}

// nextCallID returns a monotonically increasing counter for tool call IDs.
func (e *Engine) nextCallID() int64 {
	return atomic.AddInt64(&e.callCounter, 1)
}

// toolCallJSON represents a tool call in JSON format.
type toolCallJSON struct {
	Name  string          `json:"name"`
	Tool  string          `json:"tool"`
	Input json.RawMessage `json:"input"`
}

// parseSingleToolCall attempts to parse a JSON string as a tool call.
// Returns the parsed ToolCall, or an error if the JSON is malformed or
// doesn't contain a valid tool call structure.
func parseSingleToolCall(jsonStr string, callID int64) (*m31types.ToolCall, error) {
	// M-3: Support single object form — if the string starts with '{' and
	// does not start with '[', try wrapping it as a single-element array.
	trimmed := strings.TrimSpace(jsonStr)
	if strings.HasPrefix(trimmed, "{") && !strings.HasPrefix(trimmed, "[") {
		// Already a single object, parse directly below
	} else if strings.HasPrefix(trimmed, "[") {
		// Array form — extract first element
		var arr []toolCallJSON
		if err := json.Unmarshal([]byte(trimmed), &arr); err != nil {
			return nil, fmt.Errorf("unmarshal tool call array: %w", err)
		}
		if len(arr) == 0 {
			return nil, fmt.Errorf("empty tool call array")
		}
		tc := arr[0]
		name := tc.Name
		if name == "" {
			name = tc.Tool
		}
		if name == "" {
			return nil, fmt.Errorf("tool call missing name/tool field")
		}
		name = normalizeToolName(name)
		input := tc.Input
		if input == nil {
			input = json.RawMessage("{}")
		}
		return &m31types.ToolCall{
			ID:    fmt.Sprintf("call_%s_%d", name, callID),
			Name:  name,
			Input: input,
		}, nil
	}

	// Single object form
	var tc toolCallJSON
	if err := json.Unmarshal([]byte(trimmed), &tc); err != nil {
		return nil, fmt.Errorf("unmarshal tool call: %w", err)
	}

	name := tc.Name
	if name == "" {
		name = tc.Tool
	}
	if name == "" {
		return nil, fmt.Errorf("tool call missing name/tool field")
	}

	// Normalize tool names to match registered tool names
	name = normalizeToolName(name)

	input := tc.Input
	if input == nil {
		input = json.RawMessage("{}")
	}

	return &m31types.ToolCall{
		ID:    fmt.Sprintf("call_%s_%d", name, callID),
		Name:  name,
		Input: input,
	}, nil
}

// normalizeToolName maps common LLM tool names to registered tool names.
func normalizeToolName(name string) string {
	lower := strings.ToLower(name)
	switch lower {
	case "bash", "shell", "exec", "run":
		return "Bash"
	case "fileread", "read_file", "read", "cat":
		return "FileRead"
	case "filewrite", "write_file", "write", "save":
		return "FileWrite"
	case "glob", "find_files", "find":
		return "Glob"
	case "grep", "search", "search_files":
		return "Grep"
	case "edit", "search_replace":
		return "FileEdit"
	case "web_fetch", "fetch", "http_get":
		return "WebFetch"
	default:
		return name
	}
}

// stripJSONComments removes // line comments and /* ... */ block comments from
// JSON text while preserving content inside double-quoted string literals.
// Fix H-5: LLM responses may contain comments that cause json.Unmarshal to fail.
func stripJSONComments(s string) string {
	var out []rune
	inString := false
	escaped := false
	i := 0
	runes := []rune(s)
	n := len(runes)
	commentsStripped := false

	for i < n {
		c := runes[i]

		if escaped {
			out = append(out, c)
			escaped = false
			i++
			continue
		}

		if inString {
			if c == '\\' {
				escaped = true
			} else if c == '"' {
				inString = false
			}
			out = append(out, c)
			i++
			continue
		}

		// Not inside a string
		if c == '"' {
			inString = true
			out = append(out, c)
			i++
			continue
		}

		// Check for // line comment
		if c == '/' && i+1 < n && runes[i+1] == '/' {
			commentsStripped = true
			// Skip to end of line
			for i < n && runes[i] != '\n' {
				i++
			}
			continue
		}

		// Check for /* ... */ block comment
		if c == '/' && i+1 < n && runes[i+1] == '*' {
			commentsStripped = true
			i += 2 // skip past /*
			for i < n {
				if runes[i] == '*' && i+1 < n && runes[i+1] == '/' {
					i += 2 // skip past */
					break
				}
				i++
			}
			continue
		}

		out = append(out, c)
		i++
	}

	if commentsStripped {
		slog.Warn("extractJSONObject: stripped comments from LLM response (model regression signal)")
	}

	return string(out)
}

// PERF-3: This function scans from each '{' position, giving O(n*m) complexity
// where n = content length and m = number of JSON objects. The 64KB
// maxJSONScanBytes cap limits worst case. Acceptable for V1 since LLM
// responses are typically <64KB of tool call JSON.
//
// extractJSONObject finds and returns the first complete JSON object starting
// at the beginning of the string.
func extractJSONObject(s string) string {
	if !strings.HasPrefix(strings.TrimSpace(s), "{") {
		return ""
	}
	// Fix H-5: strip comments before scanning for JSON structure.
	s = stripJSONComments(s)
	depth := 0
	inString := false
	escaped := false
	for i, c := range s {
		if escaped {
			escaped = false
			continue
		}
		switch c {
		case '\\':
			if inString {
				escaped = true
			}
		case '"':
			inString = !inString
		case '{':
			if !inString {
				depth++
			}
		case '}':
			if !inString {
				depth--
				if depth == 0 {
					return s[:i+1]
				}
			}
		}
	}
	return ""
}

// formatTaskSummary creates a markdown table of tasks.
func formatTaskSummary(tasks []m31types.Task) string {
	var sb strings.Builder
	sb.WriteString("| ID | Action | Description | Deps | Status |\n")
	sb.WriteString("|----|--------|-------------|------|--------|\n")
	for _, t := range tasks {
		deps := "-"
		if len(t.Dependencies) > 0 {
			ds := make([]string, len(t.Dependencies))
			for i, d := range t.Dependencies {
				ds[i] = fmt.Sprintf("%d", d)
			}
			deps = strings.Join(ds, ", ")
		}
		status := string(t.Status)
		if status == "" {
			status = "pending"
		}
		sb.WriteString(fmt.Sprintf("| %d | %s | %s | %s | %s |\n",
			t.ID, t.Action, t.Description, deps, status))
	}
	return sb.String()
}
