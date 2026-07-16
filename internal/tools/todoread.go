package tools

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"sync/atomic"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/pkg/types"
)

// Compile-time interface check
var _ types.Tool = (*TodoRead)(nil)

type TodoRead struct {
	sessionsDir string
	sessionID   atomic.Value // stores string
}

// NewTodoRead creates a new TodoRead tool instance.
func NewTodoRead(sessionsDir, sessionID string) *TodoRead {
	t := &TodoRead{sessionsDir: sessionsDir}
	t.sessionID.Store(sessionID)
	return t
}

func (t *TodoRead) SetSessionID(id string) {
	t.sessionID.Store(id)
}

func (t *TodoRead) getSessionID() string {
	if v, ok := t.sessionID.Load().(string); ok {
		return v
	}
	return ""
}

func (t *TodoRead) Name() string {
	return "TodoRead"
}

func (t *TodoRead) Description() string {
	return "Read the current todo list from TODO.md. Returns all items with their status, priority, and content."
}

func (t *TodoRead) RiskLevel() types.RiskLevel {
	return types.RiskSafe
}

// ParameterSchema returns the JSON Schema for TodoRead tool parameters.
func (t *TodoRead) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"status": {
				"type": "string",
				"enum": ["pending", "in_progress", "completed", "cancelled", "all"],
				"description": "Filter todos by status (default: all)"
			}
		},
		"required": []
	}`
}

func (t *TodoRead) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	sid := t.getSessionID()
	if !sessionIDRe.MatchString(sid) {
		return types.ToolResult{}, fmt.Errorf("%w: invalid session ID", m31errors.ErrToolExecution)
	}

	todoPath := filepath.Join(t.sessionsDir, sid, "TODO.md")
	data, err := os.ReadFile(todoPath)
	if err != nil {
		if os.IsNotExist(err) {
			return types.ToolResult{
				Output:     "No TODO.md found for this session. Use TodoWrite to create one.",
				DurationMs: time.Since(start).Milliseconds(),
			}, nil
		}
		return types.ToolResult{}, fmt.Errorf("%w: cannot read TODO.md: %w", m31errors.ErrToolExecution, err)
	}

	// Parse the markdown table
	items := parseTodoMarkdown(string(data))

	// Filter by status if requested
	statusFilter := "all"
	if s, ok := input.Params["status"].(string); ok {
		statusFilter = s
	}

	var filtered []TodoItem
	for _, item := range items {
		if statusFilter != "all" && item.Status != statusFilter {
			continue
		}
		filtered = append(filtered, item)
	}

	// Build summary
	pending, inProgress, completed, cancelled := 0, 0, 0, 0
	for _, item := range items {
		switch item.Status {
		case "pending":
			pending++
		case "in_progress":
			inProgress++
		case "completed":
			completed++
		case "cancelled":
			cancelled++
		}
	}

	var sb strings.Builder
	fmt.Fprintf(&sb, "# TODO Summary\n\n")
	fmt.Fprintf(&sb, "**Total:** %d | **Pending:** %d | **In Progress:** %d | **Completed:** %d | **Cancelled:** %d\n\n",
		len(items), pending, inProgress, completed, cancelled)

	if statusFilter != "all" {
		fmt.Fprintf(&sb, "Showing: %s (%d items)\n\n", statusFilter, len(filtered))
	}

	if len(filtered) == 0 {
		sb.WriteString("No todo items match the filter.\n")
	} else {
		fmt.Fprintf(&sb, "| # | Status | Priority | Content |\n")
		fmt.Fprintf(&sb, "|---|--------|----------|---------|\n")
		for i, item := range filtered {
			fmt.Fprintf(&sb, "| %d | %s | %s | %s |\n", i+1, statusIcon(item.Status), item.Priority, item.Content)
		}
	}

	elapsed := time.Since(start).Milliseconds()
	return types.ToolResult{
		Output:     sb.String(),
		DurationMs: elapsed,
	}, nil
}

var todoRowRe = regexp.MustCompile(`^\|\s*\d+\s*\|\s*\[([^\]]+)\]\s*\|\s*(\w+)\s*\|\s*(.+?)\s*\|$`)

// parseTodoMarkdown parses the TODO.md markdown table format back into TodoItems.
func parseTodoMarkdown(content string) []TodoItem {
	var items []TodoItem
	lines := strings.Split(content, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if !strings.HasPrefix(line, "|") || strings.HasPrefix(line, "| #") || strings.HasPrefix(line, "|--") {
			continue
		}
		matches := todoRowRe.FindStringSubmatch(line)
		if len(matches) < 4 {
			continue
		}
		status := parseStatusIcon(matches[1])
		items = append(items, TodoItem{
			Status:   status,
			Priority: matches[2],
			Content:  matches[3],
		})
	}
	return items
}

// parseStatusIcon converts [x], [~], [-], [ ] back to status strings.
func parseStatusIcon(icon string) string {
	switch icon {
	case "x":
		return "completed"
	case "~":
		return "in_progress"
	case "-":
		return "cancelled"
	default:
		return "pending"
	}
}
