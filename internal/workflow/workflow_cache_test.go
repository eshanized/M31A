package workflow

import (
	"fmt"
	"sync"
	"testing"

	"github.com/eshanized/M31A/internal/provider"
	m31types "github.com/eshanized/M31A/internal/types"
)

func TestNewWorkflowCache(t *testing.T) {
	wc := NewWorkflowCache()
	if wc == nil {
		t.Fatal("expected non-nil WorkflowCache")
	}
	if wc.fullPrompts == nil {
		t.Error("expected fullPrompts to be initialized")
	}
	if wc.contextSnapshot == nil {
		t.Error("expected contextSnapshot to be initialized")
	}
}

func TestWorkflowCache_GetToolDefs_CallsBuildFnOnce(t *testing.T) {
	wc := NewWorkflowCache()
	callCount := 0
	buildFn := func() []provider.ToolDefinition {
		callCount++
		return []provider.ToolDefinition{{Name: "test"}}
	}

	result1 := wc.GetToolDefs(buildFn)
	result2 := wc.GetToolDefs(buildFn)

	if callCount != 1 {
		t.Errorf("expected buildFn to be called once, got %d", callCount)
	}
	if len(result1) != 1 || result1[0].Name != "test" {
		t.Errorf("unexpected result: %v", result1)
	}
	if len(result2) != 1 || result2[0].Name != "test" {
		t.Errorf("unexpected cached result: %v", result2)
	}
}

func TestWorkflowCache_GetBasePrompt_CallsBuildFnOnce(t *testing.T) {
	wc := NewWorkflowCache()
	callCount := 0
	buildFn := func() string {
		callCount++
		return "base prompt"
	}

	result1 := wc.GetBasePrompt(buildFn)
	result2 := wc.GetBasePrompt(buildFn)

	if callCount != 1 {
		t.Errorf("expected buildFn to be called once, got %d", callCount)
	}
	if result1 != "base prompt" {
		t.Errorf("unexpected result: %s", result1)
	}
	if result2 != "base prompt" {
		t.Errorf("unexpected cached result: %s", result2)
	}
}

func TestWorkflowCache_GetFullPrompt_CachesByKey(t *testing.T) {
	wc := NewWorkflowCache()
	callCount := 0
	buildFn := func() string {
		callCount++
		return "full prompt"
	}

	result1 := wc.GetFullPrompt("key1", buildFn)
	result2 := wc.GetFullPrompt("key1", buildFn)
	result3 := wc.GetFullPrompt("key2", buildFn)

	if callCount != 2 {
		t.Errorf("expected buildFn to be called twice, got %d", callCount)
	}
	if result1 != "full prompt" {
		t.Errorf("unexpected result: %s", result1)
	}
	if result2 != "full prompt" {
		t.Errorf("unexpected cached result: %s", result2)
	}
	if result3 != "full prompt" {
		t.Errorf("unexpected result for key2: %s", result3)
	}
}

func TestWorkflowCache_SetProject_GetProject_MatchingID(t *testing.T) {
	wc := NewWorkflowCache()
	project := &m31types.ProjectState{Goal: "test"}

	wc.SetProject("sess1", project)

	result := wc.GetProject("sess1")
	if result == nil {
		t.Fatal("expected non-nil project")
	}
	if result != project {
		t.Error("expected same project instance")
	}
}

func TestWorkflowCache_SetProject_GetProject_MismatchingID(t *testing.T) {
	wc := NewWorkflowCache()
	project := &m31types.ProjectState{Goal: "test"}

	wc.SetProject("sess1", project)

	result := wc.GetProject("sess2")
	if result != nil {
		t.Error("expected nil project for mismatching ID")
	}
}

func TestWorkflowCache_SetProjectShared_GetProjectShared_MatchingID(t *testing.T) {
	wc := NewWorkflowCache()
	project := &m31types.ProjectState{Goal: "test"}

	wc.SetProjectShared("sess1", project)

	result := wc.GetProjectShared("sess1")
	if result == nil {
		t.Fatal("expected non-nil project")
	}
	if result != project {
		t.Error("expected same project instance")
	}
}

func TestWorkflowCache_SetProjectShared_GetProjectShared_MismatchingID(t *testing.T) {
	wc := NewWorkflowCache()
	project := &m31types.ProjectState{Goal: "test"}

	wc.SetProjectShared("sess1", project)

	result := wc.GetProjectShared("sess2")
	if result != nil {
		t.Error("expected nil project for mismatching ID")
	}
}

func TestWorkflowCache_SetPlan_GetPlan_MatchingMD5(t *testing.T) {
	wc := NewWorkflowCache()
	plan := &m31types.Plan{}

	wc.SetPlan("md5hash", plan)

	result := wc.GetPlan("md5hash")
	if result == nil {
		t.Fatal("expected non-nil plan")
	}
	if result != plan {
		t.Error("expected same plan instance")
	}
}

func TestWorkflowCache_SetPlan_GetPlan_MismatchingMD5(t *testing.T) {
	wc := NewWorkflowCache()
	plan := &m31types.Plan{}

	wc.SetPlan("md5hash", plan)

	result := wc.GetPlan("differentmd5")
	if result != nil {
		t.Error("expected nil plan for mismatching MD5")
	}
}

func TestWorkflowCache_InvalidateAll(t *testing.T) {
	wc := NewWorkflowCache()

	// Set some cached data
	wc.GetToolDefs(func() []provider.ToolDefinition {
		return []provider.ToolDefinition{{Name: "test"}}
	})
	wc.GetBasePrompt(func() string { return "prompt" })
	wc.SetProject("sess1", &m31types.ProjectState{})
	wc.SetPlan("md5", &m31types.Plan{})

	// Invalidate
	wc.InvalidateAll()

	// Verify caches are cleared
	if wc.project != nil {
		t.Error("expected project to be nil after invalidation")
	}
	if wc.projectID != "" {
		t.Error("expected projectID to be empty after invalidation")
	}
	if wc.plan != nil {
		t.Error("expected plan to be nil after invalidation")
	}
	if wc.planMD5 != "" {
		t.Error("expected planMD5 to be empty after invalidation")
	}
}

func TestWorkflowCache_ConcurrentAccess(t *testing.T) {
	wc := NewWorkflowCache()
	var wg sync.WaitGroup

	// Concurrent GetToolDefs calls
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			wc.GetToolDefs(func() []provider.ToolDefinition {
				return []provider.ToolDefinition{{Name: "test"}}
			})
		}()
	}

	// Concurrent GetFullPrompt calls
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			wc.GetFullPrompt("key", func() string { return "prompt" })
		}()
	}

	// Concurrent SetProject/GetProject calls
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			wc.SetProject("sess1", &m31types.ProjectState{})
			wc.GetProject("sess1")
		}()
	}

	wg.Wait()
}

func TestWorkflowCache_SetDynamicContext(t *testing.T) {
	wc := NewWorkflowCache()
	snapshot := map[string]string{"key": "value"}

	wc.SetDynamicContext(snapshot, "context string")

	if wc.GetDynamicContext() != "context string" {
		t.Error("unexpected dynamic context")
	}
	if wc.GetContextSnapshot()["key"] != "value" {
		t.Error("unexpected context snapshot")
	}
}

func TestWorkflowCache_ConcurrentDynamicContext(t *testing.T) {
	wc := NewWorkflowCache()

	var wg sync.WaitGroup
	numGoroutines := 100
	numIterations := 1000

	// Concurrent writers
	for i := 0; i < numGoroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < numIterations; j++ {
				snapshot := map[string]string{
					"id":   fmt.Sprintf("%d", id),
					"iter": fmt.Sprintf("%d", j),
				}
				wc.SetDynamicContext(snapshot, fmt.Sprintf("context-%d-%d", id, j))
			}
		}(i)
	}

	// Concurrent readers
	for i := 0; i < numGoroutines; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < numIterations; j++ {
				_ = wc.GetDynamicContext()
				_ = wc.GetContextSnapshot()
			}
		}()
	}

	wg.Wait()
}
