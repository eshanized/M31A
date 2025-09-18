package tools

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

var sessionIDRe = regexp.MustCompile(`^[a-zA-Z0-9_-]+$`)

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

// ParameterSchema returns the JSON Schema for TodoWrite tool parameters.
func (t *TodoWrite) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"todos": {
				"type": "array",
				"items": {
					"type": "object",
					"properties": {
						"content": {"type": "string"},
						"status": {"type": "string", "enum": ["pending", "in_progress", "completed"]},
						"priority": {"type": "string", "enum": ["high", "medium", "low"]}
					}
				},
				"description": "List of todo items"
			}
		},
		"required": ["todos"]
	}`
}

func (t *TodoWrite) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	todosRaw, ok := input.Params["todos"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: missing parameter: todos", m31errors.ErrToolExecution)
	}
	todosSlice, ok := todosRaw.([]any)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: parameter todos must be an array", m31errors.ErrToolExecution)
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
			return types.ToolResult{}, fmt.Errorf("%w: todo item %d must be an object", m31errors.ErrToolExecution, i)
		}

		content, ok := tMap["content"].(string)
		if !ok {
			return types.ToolResult{}, fmt.Errorf("%w: todo item %d missing content field", m31errors.ErrToolExecution, i)
		}

		status := "pending"
		if s, ok := tMap["status"].(string); ok {
			switch s {
			case "pending", "in_progress", "completed", "cancelled":
				status = s
			default:
				return types.ToolResult{}, fmt.Errorf("%w: todo item %d has invalid status: %s", m31errors.ErrToolExecution, i, s)
			}
		}

		priority := "medium"
		if p, ok := tMap["priority"].(string); ok {
			switch p {
			case "high", "medium", "low":
				priority = p
			default:
				return types.ToolResult{}, fmt.Errorf("%w: todo item %d has invalid priority: %s", m31errors.ErrToolExecution, i, p)
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
	if !sessionIDRe.MatchString(t.sessionID) {
		return types.ToolResult{}, fmt.Errorf("%w: invalid session ID: must be alphanumeric", m31errors.ErrToolExecution)
	}
	sessionDir := filepath.Join(t.sessionsDir, t.sessionID)
	if err := os.MkdirAll(sessionDir, DirPermission); err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: cannot create session directory: %v", m31errors.ErrToolExecution, err)
	}

	todoPath := filepath.Join(sessionDir, "TODO.md")
	content := []byte(b.String())

	randBytes := make([]byte, 8)
	if _, err := rand.Read(randBytes); err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: cannot generate temp name: %v", m31errors.ErrToolExecution, err)
	}
	tmpPath := filepath.Join(sessionDir, ".m31a_tmp_"+hex.EncodeToString(randBytes))
	if err := os.WriteFile(tmpPath, content, FilePermission); err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: cannot write temp file: %v", m31errors.ErrToolExecution, err)
	}
	if err := os.Rename(tmpPath, todoPath); err != nil {
		os.Remove(tmpPath)
		return types.ToolResult{}, fmt.Errorf("%w: cannot write TODO.md: %v", m31errors.ErrToolExecution, err)
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
