package narrative

import (
	"testing"
	"time"
)

func TestGrouperAddAndFlush(t *testing.T) {
	g := NewGrouper(GroupConfig{
		Window:    5 * time.Second,
		MaxItems:  10,
		FlushIdle: 5 * time.Second,
	})

	event1 := RawEvent{Type: EventToolStart, Data: map[string]interface{}{"tool_name": "FileRead", "file": "a.go"}}
	event2 := RawEvent{Type: EventToolStart, Data: map[string]interface{}{"tool_name": "FileRead", "file": "b.go"}}

	g.Add(event1, "tools")
	g.Add(event2, "tools")

	events := g.Flush("tools")
	if len(events) != 2 {
		t.Errorf("Flush returned %d events, want 2", len(events))
	}

	// Second flush should return nil.
	events2 := g.Flush("tools")
	if events2 != nil {
		t.Error("Second Flush should return nil")
	}
}

func TestGrouperShouldFlushByMaxItems(t *testing.T) {
	g := NewGrouper(GroupConfig{
		Window:    10 * time.Second,
		MaxItems:  3,
		FlushIdle: 10 * time.Second,
	})

	for i := 0; i < 3; i++ {
		event := RawEvent{Type: EventToolStart, Data: map[string]interface{}{"tool_name": "FileRead"}}
		g.Add(event, "tools")
	}

	if !g.ShouldFlush("tools") {
		t.Error("ShouldFlush should return true at MaxItems")
	}
}

func TestGrouperShouldFlushByTimeWindow(t *testing.T) {
	now := time.Now()
	g := NewGrouper(GroupConfig{
		Window:    3 * time.Second,
		MaxItems:  100,
		FlushIdle: 30 * time.Second,
	})
	g.now = func() time.Time { return now }

	event := RawEvent{Type: EventToolStart, Data: map[string]interface{}{"tool_name": "FileRead"}}
	g.Add(event, "tools")

	// Advance time past window.
	now = now.Add(4 * time.Second)
	if !g.ShouldFlush("tools") {
		t.Error("ShouldFlush should return true after Window")
	}
}

func TestGrouperShouldFlushByIdle(t *testing.T) {
	now := time.Now()
	g := NewGrouper(GroupConfig{
		Window:    30 * time.Second,
		MaxItems:  100,
		FlushIdle: 2 * time.Second,
	})
	g.now = func() time.Time { return now }

	event := RawEvent{Type: EventToolStart, Data: map[string]interface{}{"tool_name": "FileRead"}}
	g.Add(event, "tools")

	// Advance time past idle.
	now = now.Add(3 * time.Second)
	if !g.ShouldFlush("tools") {
		t.Error("ShouldFlush should return true after FlushIdle")
	}
}

func TestGrouperShouldNotFlushBeforeWindow(t *testing.T) {
	now := time.Now()
	g := NewGrouper(GroupConfig{
		Window:    5 * time.Second,
		MaxItems:  100,
		FlushIdle: 30 * time.Second,
	})
	g.now = func() time.Time { return now }

	event := RawEvent{Type: EventToolStart, Data: map[string]interface{}{"tool_name": "FileRead"}}
	g.Add(event, "tools")

	// Advance time but not past window.
	now = now.Add(2 * time.Second)
	if g.ShouldFlush("tools") {
		t.Error("ShouldFlush should return false before Window")
	}
}

func TestGrouperShouldNotFlushEmptyGroup(t *testing.T) {
	g := NewGrouper(DefaultGroupConfig())
	if g.ShouldFlush("nonexistent") {
		t.Error("ShouldFlush should return false for empty group")
	}
}

func TestGrouperFlushAll(t *testing.T) {
	g := NewGrouper(DefaultGroupConfig())

	g.Add(RawEvent{Type: EventToolStart}, "tools")
	g.Add(RawEvent{Type: EventToolStart}, "tools")
	g.Add(RawEvent{Type: EventTaskDiff}, "task_diff")

	all := g.FlushAll()
	if len(all) != 2 {
		t.Errorf("FlushAll returned %d groups, want 2", len(all))
	}
	if len(all["tools"]) != 2 {
		t.Errorf("tools group has %d events, want 2", len(all["tools"]))
	}
	if len(all["task_diff"]) != 1 {
		t.Errorf("task_diff group has %d events, want 1", len(all["task_diff"]))
	}
}

func TestGrouperPendingCount(t *testing.T) {
	g := NewGrouper(DefaultGroupConfig())

	g.Add(RawEvent{Type: EventToolStart}, "tools")
	g.Add(RawEvent{Type: EventToolStart}, "tools")
	g.Add(RawEvent{Type: EventTaskDiff}, "task_diff")

	if got := g.PendingCount(); got != 3 {
		t.Errorf("PendingCount = %d, want 3", got)
	}

	g.Flush("tools")
	if got := g.PendingCount(); got != 1 {
		t.Errorf("PendingCount after Flush = %d, want 1", got)
	}
}

func TestGroupSummary(t *testing.T) {
	events := []RawEvent{
		{Data: map[string]interface{}{"tool_name": "FileRead"}},
		{Data: map[string]interface{}{"tool_name": "FileRead"}},
		{Data: map[string]interface{}{"tool_name": "FileWrite"}},
	}
	summary := GroupSummary(events, "tools")
	if summary == "" {
		t.Error("GroupSummary should not be empty")
	}
}

func TestGroupSummaryEmpty(t *testing.T) {
	summary := GroupSummary(nil, "tools")
	if summary != "" {
		t.Errorf("GroupSummary for nil events = %q, want empty", summary)
	}
}

func TestGroupSummaryTaskDiff(t *testing.T) {
	events := []RawEvent{
		{Data: map[string]interface{}{"file": "a.go", "additions": 10, "deletions": 5}},
		{Data: map[string]interface{}{"file": "b.go", "additions": 20, "deletions": 0}},
	}
	summary := GroupSummary(events, "task_diff")
	if summary == "" {
		t.Error("GroupSummary should not be empty")
	}
}

func TestGroupSummaryShip(t *testing.T) {
	events := []RawEvent{
		{Data: map[string]interface{}{"entry_count": 5}},
	}
	summary := GroupSummary(events, "ship")
	if summary == "" {
		t.Error("GroupSummary should not be empty")
	}
}

func TestCollectFiles(t *testing.T) {
	events := []RawEvent{
		{Data: map[string]interface{}{"file": "a.go"}},
		{Data: map[string]interface{}{"file_path": "b.go"}},
		{Data: map[string]interface{}{"path": "c.go"}},
		{Data: map[string]interface{}{"file": "a.go"}}, // duplicate
	}
	files := CollectFiles(events)
	if len(files) != 3 {
		t.Errorf("CollectFiles returned %d files, want 3", len(files))
	}
}

func TestCollectFilesEmpty(t *testing.T) {
	files := CollectFiles(nil)
	if files != nil {
		t.Errorf("CollectFiles for nil events = %v, want nil", files)
	}
}
