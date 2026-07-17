package tui

import (
	"context"
	"testing"

	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tools"
	"github.com/eshanized/M31A/internal/workflow"
)

func setupTestDispatcher(t *testing.T) (*tools.Dispatcher, func()) {
	t.Helper()
	d, err := tools.NewDispatcher("", "", "", &config.PermissionsConfig{}, &config.ToolsConfig{})
	if err != nil {
		t.Fatalf("NewDispatcher failed: %v", err)
	}
	// Pre-approve permissions for testing
	_ = d.SetPermission("Bash", true, false)
	_ = d.SetPermission("FileRead", true, false)
	_ = d.SetPermission("FileWrite", true, false)
	_ = d.SetPermission("FileEdit", true, false)
	_ = d.SetPermission("FileList", true, false)
	_ = d.SetPermission("FileDelete", true, false)
	_ = d.SetPermission("FileMove", true, false)
	_ = d.SetPermission("Glob", true, false)
	_ = d.SetPermission("Grep", true, false)
	_ = d.SetPermission("WebFetch", true, false)
	_ = d.SetPermission("WebSearch", true, false)
	_ = d.SetPermission("Task", true, false)
	_ = d.SetPermission("Edit", true, false)
	_ = d.SetPermission("TodoWrite", true, false)
	_ = d.SetPermission("TodoRead", true, false)
	_ = d.SetPermission("Git", true, false)
	_ = d.SetPermission("DevServer", true, false)
	_ = d.SetPermission("CodeMap", true, false)
	_ = d.SetPermission("CodeComplexity", true, false)
	_ = d.SetPermission("HTTPCheck", true, false)
	_ = d.SetPermission("AskUserQuestion", true, false)

	return d, func() {}
}

func TestEmitter_ConcurrentEvents(t *testing.T) {
	t.Parallel()

	d, cleanup := setupTestDispatcher(t)
	defer cleanup()

	engine, err := workflow.NewEngine(context.Background(), d, nil, "", "", nil, nil)
	if err != nil {
		t.Fatalf("NewEngine failed: %v", err)
	}
	defer engine.Close()

	// Test concurrent event emission
	const numGoroutines = 10
	const eventsPerGoroutine = 100

	var wg sync.WaitGroup
	wg.Add(numGoroutines)

	for i := 0; i < numGoroutines; i++ {
		go func(id int) {
			defer wg.Done()
			for j := 0; j < eventsPerGoroutine; j++ {
				engine.Emit(workflow.TaskStartMsg{TaskID: id*100 + j, Description: "test"})
				engine.Emit(workflow.TaskUpdateMsg{TaskID: id*100 + j, Status: workflow.TaskStatusRunning})
				engine.Emit(workflow.ToolStartMsg{ToolCall: types.ToolCall{ID: "call", Name: "test", Input: []byte("{}")}})
				engine.Emit(workflow.ToolCompleteMsg{ToolCall: types.ToolCall{ID: "call", Name: "test"}, Err: nil, DurationMs: 1})
			}
		}(i)
	}

	wg.Wait()
}
