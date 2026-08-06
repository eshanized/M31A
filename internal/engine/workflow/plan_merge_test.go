package workflow

import (
	"testing"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

func TestMergeRelatedTasks_NoOverlap(t *testing.T) {
	t.Parallel()
	tasks := []m31types.Task{
		{ID: 1, Action: "create", Files: []string{"a.go"}, AcceptanceCriteria: []string{"c1"}},
		{ID: 2, Action: "create", Files: []string{"b.go"}, AcceptanceCriteria: []string{"c2"}},
	}
	result := mergeRelatedTasks(tasks)
	if len(result) != 2 {
		t.Fatalf("expected 2 tasks, got %d", len(result))
	}
}

func TestMergeRelatedTasks_Overlap(t *testing.T) {
	t.Parallel()
	tasks := []m31types.Task{
		{ID: 1, Action: "create", Files: []string{"a.go", "b.go", "c.go"}, AcceptanceCriteria: []string{"c1"}},
		{ID: 2, Action: "create", Files: []string{"a.go", "b.go", "d.go"}, AcceptanceCriteria: []string{"c2"}},
	}
	result := mergeRelatedTasks(tasks)
	if len(result) != 1 {
		t.Fatalf("expected 1 merged task, got %d", len(result))
	}
	if len(result[0].Files) != 4 {
		t.Errorf("expected 4 unioned files, got %d", len(result[0].Files))
	}
	if len(result[0].AcceptanceCriteria) != 2 {
		t.Errorf("expected 2 acceptance criteria, got %d", len(result[0].AcceptanceCriteria))
	}
}

func TestMergeRelatedTasks_CapExceeded(t *testing.T) {
	t.Parallel()
	// Create two tasks with many files that would exceed cap when merged
	files1 := make([]string, 7)
	files2 := make([]string, 7)
	for i := range files1 {
		files1[i] = "a" + string(rune('0'+i)) + ".go"
		files2[i] = "b" + string(rune('0'+i)) + ".go"
	}
	// Add 3 overlapping files
	files1 = append(files1, "x.go", "y.go", "z.go")
	files2 = append(files2, "x.go", "y.go", "z.go")

	tasks := []m31types.Task{
		{ID: 1, Action: "create", Files: files1, AcceptanceCriteria: []string{"c1"}},
		{ID: 2, Action: "create", Files: files2, AcceptanceCriteria: []string{"c2"}},
	}
	result := mergeRelatedTasks(tasks)
	// Should NOT merge because union would be 17 files > 10
	if len(result) != 2 {
		t.Fatalf("expected 2 tasks (cap exceeded), got %d", len(result))
	}
}

func TestMergeRelatedTasks_PreservesCriteria(t *testing.T) {
	t.Parallel()
	tasks := []m31types.Task{
		{ID: 1, Action: "create", Files: []string{"a.go", "b.go"}, AcceptanceCriteria: []string{"c1", "c2"}},
		{ID: 2, Action: "create", Files: []string{"a.go", "b.go"}, AcceptanceCriteria: []string{"c2", "c3"}},
	}
	result := mergeRelatedTasks(tasks)
	if len(result) != 1 {
		t.Fatalf("expected 1 merged task, got %d", len(result))
	}
	// Should have 3 unique criteria: c1, c2, c3
	if len(result[0].AcceptanceCriteria) != 3 {
		t.Errorf("expected 3 unique criteria, got %d: %v", len(result[0].AcceptanceCriteria), result[0].AcceptanceCriteria)
	}
}

func TestMergeRelatedTasks_Deterministic(t *testing.T) {
	t.Parallel()
	tasks := []m31types.Task{
		{ID: 3, Action: "create", Files: []string{"a.go", "b.go"}, AcceptanceCriteria: []string{"c1"}},
		{ID: 1, Action: "create", Files: []string{"a.go", "b.go"}, AcceptanceCriteria: []string{"c2"}},
		{ID: 2, Action: "create", Files: []string{"a.go", "b.go"}, AcceptanceCriteria: []string{"c3"}},
	}

	// Run merge multiple times and verify same output
	for i := 0; i < 10; i++ {
		result := mergeRelatedTasks(tasks)
		if len(result) != 1 {
			t.Fatalf("iteration %d: expected 1 merged task, got %d", i, len(result))
		}
		// Should keep lowest ID (1) as base
		if result[0].ID != 1 {
			t.Errorf("iteration %d: expected base ID 1, got %d", i, result[0].ID)
		}
	}
}

func TestMergeRelatedTasks_DifferentActions(t *testing.T) {
	t.Parallel()
	tasks := []m31types.Task{
		{ID: 1, Action: "create", Files: []string{"a.go", "b.go"}, AcceptanceCriteria: []string{"c1"}},
		{ID: 2, Action: "modify", Files: []string{"a.go", "b.go"}, AcceptanceCriteria: []string{"c2"}},
	}
	result := mergeRelatedTasks(tasks)
	// Should NOT merge because actions differ
	if len(result) != 2 {
		t.Fatalf("expected 2 tasks (different actions), got %d", len(result))
	}
}

func TestMergeRelatedTasks_Dependencies(t *testing.T) {
	t.Parallel()
	tasks := []m31types.Task{
		{ID: 1, Action: "create", Files: []string{"a.go", "b.go"}, Dependencies: []int{3}, AcceptanceCriteria: []string{"c1"}},
		{ID: 2, Action: "create", Files: []string{"a.go", "b.go"}, Dependencies: []int{3}, AcceptanceCriteria: []string{"c2"}},
	}
	result := mergeRelatedTasks(tasks)
	if len(result) != 1 {
		t.Fatalf("expected 1 merged task, got %d", len(result))
	}
	// Should preserve dependency on 3
	if len(result[0].Dependencies) != 1 || result[0].Dependencies[0] != 3 {
		t.Errorf("expected dependency [3], got %v", result[0].Dependencies)
	}
}

func TestMergeRelatedTasks_InterDependency(t *testing.T) {
	t.Parallel()
	tasks := []m31types.Task{
		{ID: 1, Action: "create", Files: []string{"a.go", "b.go"}, Dependencies: []int{2}, AcceptanceCriteria: []string{"c1"}},
		{ID: 2, Action: "create", Files: []string{"a.go", "b.go"}, AcceptanceCriteria: []string{"c2"}},
	}
	result := mergeRelatedTasks(tasks)
	// Should NOT merge because task 1 depends on task 2
	if len(result) != 2 {
		t.Fatalf("expected 2 tasks (inter-dependency), got %d", len(result))
	}
}

func TestMergeRelatedTasks_Empty(t *testing.T) {
	t.Parallel()
	result := mergeRelatedTasks(nil)
	if len(result) != 0 {
		t.Fatalf("expected 0 tasks, got %d", len(result))
	}
}

func TestMergeRelatedTasks_SingleTask(t *testing.T) {
	t.Parallel()
	tasks := []m31types.Task{
		{ID: 1, Action: "create", Files: []string{"a.go"}},
	}
	result := mergeRelatedTasks(tasks)
	if len(result) != 1 {
		t.Fatalf("expected 1 task, got %d", len(result))
	}
}
