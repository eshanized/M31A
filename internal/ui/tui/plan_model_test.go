package tui

import (
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

func testTasks() []types.Task {
	return []types.Task{
		{ID: 1, Action: "Setup project", Description: "Initialize project structure", Dependencies: []int{}},
		{ID: 2, Action: "Implement core", Description: "Build core functionality", Dependencies: []int{1}},
		{ID: 3, Action: "Add tests", Description: "Write comprehensive tests", Dependencies: []int{1, 2}},
	}
}

func TestPlanCollapse(t *testing.T) {
	tasks := testTasks()
	pm := NewPlanModel(tasks, theme.Default(), "", "", "", 0.0, "", 80, 24)

	// Initially all expanded - should show all tasks
	result := pm.renderTasks()
	if !strings.Contains(result, "Setup project") {
		t.Error("expected expanded view to show all tasks")
	}
	if !strings.Contains(result, "Implement core") {
		t.Error("expected expanded view to show Implement core")
	}

	// Collapse wave 0
	pm.toggleCollapse(0)
	result = pm.renderTasks()
	if !strings.Contains(result, "▸") {
		t.Error("expected collapsed view to show ▸ indicator")
	}
	if strings.Contains(result, "Setup project") {
		t.Error("expected collapsed view to hide individual tasks")
	}

	// Expand wave 0 again
	pm.toggleCollapse(0)
	result = pm.renderTasks()
	if !strings.Contains(result, "Setup project") {
		t.Error("expected expanded view to show tasks after expand")
	}
}

func TestPlanTimeEstimates(t *testing.T) {
	tasks := []types.Task{
		{ID: 1, Action: "Simple task", Description: "Short", Dependencies: []int{}},
		{ID: 2, Action: "Medium task", Description: "Has deps", Dependencies: []int{1}},
		{ID: 3, Action: "Complex task", Description: strings.Repeat("x", 150), Dependencies: []int{1, 2, 3}},
	}
	pm := NewPlanModel(tasks, theme.Default(), "", "", "", 0.0, "", 80, 24)

	result := pm.renderTasks()
	// Simple task should have ~2 min
	if !strings.Contains(result, "~2 min") {
		t.Errorf("expected '~2 min' for simple task, got %q", result)
	}
	// Medium task should have ~5 min
	if !strings.Contains(result, "~5 min") {
		t.Errorf("expected '~5 min' for medium task, got %q", result)
	}
	// Complex task should have ~8 min
	if !strings.Contains(result, "~8 min") {
		t.Errorf("expected '~8 min' for complex task, got %q", result)
	}
}

func TestPlanCollapseAll(t *testing.T) {
	tasks := testTasks()
	pm := NewPlanModel(tasks, theme.Default(), "", "", "", 0.0, "", 80, 24)

	pm.collapseAll()
	result := pm.renderTasks()
	if !strings.Contains(result, "▸") {
		t.Error("expected collapseAll to show collapsed indicator")
	}
	if strings.Contains(result, "Setup project") {
		t.Error("expected collapseAll to hide tasks")
	}

	pm.expandAll()
	result = pm.renderTasks()
	if !strings.Contains(result, "Setup project") {
		t.Error("expected expandAll to show tasks")
	}
}

func TestComputeTaskDuration(t *testing.T) {
	tests := []struct {
		task     types.Task
		expected time.Duration
	}{
		{types.Task{Dependencies: []int{}, Description: "short"}, 2 * time.Minute},
		{types.Task{Dependencies: []int{1}, Description: "has deps"}, 5 * time.Minute},
		{types.Task{Dependencies: []int{}, Description: strings.Repeat("x", 150)}, 8 * time.Minute},
		{types.Task{Dependencies: []int{1, 2, 3}, Description: "complex"}, 8 * time.Minute},
	}
	for _, tt := range tests {
		got := computeTaskDuration(tt.task)
		if got != tt.expected {
			t.Errorf("computeTaskDuration(%v) = %v, want %v", tt.task, got, tt.expected)
		}
	}
}

func TestFormatDurationShort(t *testing.T) {
	tests := []struct {
		input    time.Duration
		expected string
	}{
		{0, "~1 min"},
		{30 * time.Second, "~1 min"},
		{2 * time.Minute, "~2 min"},
		{5 * time.Minute, "~5 min"},
		{8 * time.Minute, "~8 min"},
	}
	for _, tt := range tests {
		got := formatDurationShort(tt.input)
		if got != tt.expected {
			t.Errorf("formatDurationShort(%v) = %q, want %q", tt.input, got, tt.expected)
		}
	}
}
