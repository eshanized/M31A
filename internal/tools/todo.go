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
	"sync/atomic"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// Compile-time interface check
var _ types.Tool = (*TodoWrite)(nil)

var sessionIDRe = regexp.MustCompile(`^[a-zA-Z0-9_-]+$`)

type TodoWrite struct {
	sessionsDir string
	sessionID   atomic.Value           // stores string
	onUpdate    func(items []TodoItem) // callback for sidebar updates
}

// TodoItem represents a parsed todo item from the TodoWrite tool.
type TodoItem struct {
	Content  string
	Status   string
	Priority string
}

func NewTodoWrite(sessionsDir, sessionID string) *TodoWrite {
	t := &TodoWrite{sessionsDir: sessionsDir}
	t.sessionID.Store(sessionID)
	return t
}

// SetOnUpdate sets the callback invoked after successful todo writes.
func (t *TodoWrite) SetOnUpdate(fn func(items []TodoItem)) {
	t.onUpdate = fn
}

func (t *TodoWrite) SetSessionID(id string) {
	t.sessionID.Store(id)
}

func (t *TodoWrite) getSessionID() string {
	if v, ok := t.sessionID.Load().(string); ok {
		return v
	}
	return ""
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
						"status": {"type": "string", "enum": ["pending", "in_progress", "completed", "cancelled"]},
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

	if err := t.writeTodoFile(items); err != nil {
		return types.ToolResult{}, err
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

	// Notify sidebar of todo updates
	if t.onUpdate != nil {
		callbackItems := make([]TodoItem, len(items))
		for i, item := range items {
			callbackItems[i] = TodoItem{
				Content:  item.Content,
				Status:   item.Status,
				Priority: item.Priority,
			}
		}
		t.onUpdate(callbackItems)
	}

	elapsed := time.Since(start).Milliseconds()
	return types.ToolResult{
		Output:     summary,
		DurationMs: elapsed,
	}, nil
}

// SyncTodoFromTasks generates TODO.md from a task runner's current state.
// This bridges the gap between the task runner (which tracks execution state)
// and the TODO.md file (which provides user-visible progress tracking).
// It is called automatically after each execution group and at phase boundaries.
func (t *TodoWrite) SyncTodoFromTasks(tasks []types.Task) error {
	items := make([]TodoItem, len(tasks))
	for i, task := range tasks {
		status := taskStatusToTodoStatus(task.Status)
		priority := taskActionToPriority(task.Action)
		items[i] = TodoItem{
			Content:  fmt.Sprintf("[Task %d] %s", task.ID, task.Description),
			Status:   status,
			Priority: priority,
		}
	}

	if err := t.writeTodoFile(items); err != nil {
		return err
	}

	// Notify sidebar
	if t.onUpdate != nil {
		t.onUpdate(items)
	}

	return nil
}

// taskStatusToTodoStatus maps a task runner status to a TODO status string.
func taskStatusToTodoStatus(s types.TaskStatus) string {
	switch s {
	case types.StatusDone:
		return "completed"
	case types.StatusRunning:
		return "in_progress"
	case types.StatusFailed, types.StatusUnrecoverable:
		return "cancelled"
	case types.StatusSkipped:
		return "cancelled"
	default:
		return "pending"
	}
}

// taskActionToPriority maps a task action to a TODO priority string.
func taskActionToPriority(action string) string {
	switch strings.ToLower(action) {
	case "delete", "modify":
		return "high"
	case "create", "add":
		return "medium"
	default:
		return "medium"
	}
}

// writeTodoFile writes items as a Markdown table to TODO.md in the session directory.
// Uses atomic write (temp file + rename) to prevent partial reads.
func (t *TodoWrite) writeTodoFile(items []TodoItem) error {
	var b strings.Builder
	b.WriteString("# TODO\n\n")
	b.WriteString("| # | Status | Priority | Content |\n")
	b.WriteString("|---|--------|----------|---------|\n")

	for i, item := range items {
		fmt.Fprintf(&b, "| %d | %s | %s | %s |\n", i+1, statusIcon(item.Status), item.Priority, item.Content)
	}

	sid := t.getSessionID()
	if !sessionIDRe.MatchString(sid) {
		return fmt.Errorf("%w: invalid session ID: must be alphanumeric", m31errors.ErrToolExecution)
	}
	sessionDir := filepath.Join(t.sessionsDir, sid)
	if err := os.MkdirAll(sessionDir, DirPermission); err != nil {
		return fmt.Errorf("%w: cannot create session directory: %w", m31errors.ErrToolExecution, err)
	}

	todoPath := filepath.Join(sessionDir, "TODO.md")
	content := []byte(b.String())

	randBytes := make([]byte, 8)
	if _, err := rand.Read(randBytes); err != nil {
		return fmt.Errorf("%w: cannot generate temp name: %w", m31errors.ErrToolExecution, err)
	}
	tmpPath := filepath.Join(sessionDir, ".m31a_tmp_"+hex.EncodeToString(randBytes))
	if err := os.WriteFile(tmpPath, content, FilePermission); err != nil {
		return fmt.Errorf("%w: cannot write temp file: %w", m31errors.ErrToolExecution, err)
	}
	if err := os.Rename(tmpPath, todoPath); err != nil {
		_ = os.Remove(tmpPath)
		return fmt.Errorf("%w: cannot write TODO.md: %w", m31errors.ErrToolExecution, err)
	}
	return nil
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
