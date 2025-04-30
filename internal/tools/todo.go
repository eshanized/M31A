package tools

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

type TodoWrite struct {
	sessionsDir string
	sessionID   string
}

func NewTodoWrite(sessionsDir, sessionID string) *TodoWrite {
	return &TodoWrite{sessionsDir: sessionsDir, sessionID: sessionID}
}

func (t *TodoWrite) SetSessionID(id string) {
	t.sessionID = id
}

func (t *TodoWrite) Name() string {
	return "TodoWrite"
}

func (t *TodoWrite) Description() string {
	return "Write the complete todo list to TODO.md in the session directory."
}

func (t *TodoWrite) RiskLevel() types.RiskLevel {
	return types.RiskSafe
}

func (t *TodoWrite) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	todosRaw, ok := input.Params["todos"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: todos")
	}
	todosSlice, ok := todosRaw.([]any)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter todos must be an array")
	}

	type TodoItem struct {
		Content  string
		Status   string
		Priority string
	}

	var items []TodoItem
	for i, tRaw := range todosSlice {
		tMap, ok := tRaw.(map[string]any)
		if !ok {
			return types.ToolResult{}, fmt.Errorf("todo item %d must be an object", i)
		}

		content, ok := tMap["content"].(string)
		if !ok {
			return types.ToolResult{}, fmt.Errorf("todo item %d missing content field", i)
		}

		status := "pending"
		if s, ok := tMap["status"].(string); ok {
			switch s {
			case "pending", "in_progress", "completed", "cancelled":
				status = s
			default:
				return types.ToolResult{}, fmt.Errorf("todo item %d has invalid status: %s", i, s)
			}
		}

		priority := "medium"
		if p, ok := tMap["priority"].(string); ok {
			switch p {
			case "high", "medium", "low":
				priority = p
			default:
				return types.ToolResult{}, fmt.Errorf("todo item %d has invalid priority: %s", i, p)
			}
		}

		items = append(items, TodoItem{
			Content:  content,
			Status:   status,
			Priority: priority,
		})
	}

	// Build markdown table
	var b strings.Builder
	b.WriteString("# TODO\n\n")
	b.WriteString("| # | Status | Priority | Content |\n")
	b.WriteString("|---|--------|----------|---------|\n")

	for i, item := range items {
		statusIcon := statusIcon(item.Status)
		b.WriteString(fmt.Sprintf("| %d | %s | %s | %s |\n", i+1, statusIcon, item.Priority, item.Content))
	}

	// Write to session directory
	sessionDir := filepath.Join(t.sessionsDir, t.sessionID)
	if err := os.MkdirAll(sessionDir, 0755); err != nil {
		return types.ToolResult{}, fmt.Errorf("cannot create session directory: %w", err)
	}

	todoPath := filepath.Join(sessionDir, "TODO.md")
	if err := os.WriteFile(todoPath, []byte(b.String()), 0644); err != nil {
		return types.ToolResult{}, fmt.Errorf("cannot write TODO.md: %w", err)
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

	summary := fmt.Sprintf("TODO updated: %d total (%d pending, %d in progress, %d completed, %d cancelled)",
		len(items), pending, inProgress, completed, cancelled)

	elapsed := time.Since(start).Milliseconds()
	return types.ToolResult{
		Output:     summary,
		DurationMs: elapsed,
	}, nil
}

func statusIcon(status string) string {
	switch status {
	case "completed":
		return "[x]"
	case "in_progress":
		return "[~]"
	case "cancelled":
		return "[-]"
	default:
		return "[ ]"
	}
}
