package narrative

import (
	"testing"
	"time"
)

func TestEngineProcessEventHidden(t *testing.T) {
	e := NewEngine(DefaultEngineConfig())
	event := RawEvent{Type: EventPlanRefine} // Hidden by classifier
	result := e.ProcessEvent(event)
	if result != nil {
		t.Errorf("Hidden event should produce nil result, got %d", len(result))
	}
}

func TestEngineProcessEventNarrative(t *testing.T) {
	e := NewEngine(DefaultEngineConfig())
	event := RawEvent{
		Type:      EventTaskStart,
		Timestamp: time.Now(),
		Data:      map[string]interface{}{"description": "implement auth"},
	}
	result := e.ProcessEvent(event)
	if len(result) != 1 {
		t.Fatalf("Expected 1 narrative, got %d", len(result))
	}
	if result[0].Type != NarrativeStartingTask {
		t.Errorf("Type = %q, want %q", result[0].Type, NarrativeStartingTask)
	}
}

func TestEngineProcessEventGrouped(t *testing.T) {
	e := NewEngine(DefaultEngineConfig())
	event := RawEvent{
		Type:      EventToolStart,
		Timestamp: time.Now(),
		Data:      map[string]interface{}{"tool_name": "FileRead", "file": "main.go"},
	}
	result := e.ProcessEvent(event)
	// Should buffer, not emit yet.
	if result != nil {
		t.Errorf("Grouped event should buffer, got %d narratives", len(result))
	}
}

func TestEngineFlushGroups(t *testing.T) {
	e := NewEngine(DefaultEngineConfig())
	event := RawEvent{
		Type:      EventToolStart,
		Timestamp: time.Now(),
		Data:      map[string]interface{}{"tool_name": "FileRead", "file": "main.go"},
	}
	e.ProcessEvent(event)

	result := e.FlushGroups()
	if len(result) != 1 {
		t.Fatalf("FlushGroups should return 1 narrative, got %d", len(result))
	}
	if result[0].Text == "" {
		t.Error("Flushed narrative should have text")
	}
}

func TestEngineProcessEvents(t *testing.T) {
	e := NewEngine(DefaultEngineConfig())
	events := []RawEvent{
		{Type: EventPlanRefine}, // hidden by classifier
		{
			Type:      EventTaskStart,
			Timestamp: time.Now(),
			Data:      map[string]interface{}{"description": "test"},
		},
		{
			Type:      EventToolStart,
			Timestamp: time.Now(),
			Data:      map[string]interface{}{"tool_name": "FileRead", "file": "a.go"},
		},
	}

	result := e.ProcessEvents(events)
	if len(result) != 1 {
		t.Errorf("Expected 1 narrative (hidden ignored, grouped buffered), got %d", len(result))
	}
}

func TestEnginePendingNarratives(t *testing.T) {
	now := time.Now()
	config := DefaultEngineConfig()
	config.Grouper = NewGrouper(GroupConfig{
		Window:    1 * time.Millisecond,
		MaxItems:  10,
		FlushIdle: 1 * time.Millisecond,
	})
	config.Timing = NewTimingGuard(TimingConfig{
		DebounceWindow: 10 * time.Millisecond,
		MinDisplay:     10 * time.Millisecond,
		MaxAge:         30 * time.Second,
	})
	e := NewEngine(config)
	e.now = func() time.Time { return now }
	config.Grouper.now = func() time.Time { return now }

	event := RawEvent{
		Type:      EventToolStart,
		Timestamp: now,
		Data:      map[string]interface{}{"tool_name": "FileRead", "file": "a.go"},
	}
	e.ProcessEvent(event)

	// Advance time past flush window.
	now = now.Add(5 * time.Millisecond)
	config.Grouper.now = func() time.Time { return now }

	result := e.PendingNarratives()
	if len(result) != 1 {
		t.Errorf("PendingNarratives should return 1, got %d", len(result))
	}
}

func TestEngineReset(t *testing.T) {
	e := NewEngine(DefaultEngineConfig())
	event := RawEvent{
		Type:      EventToolStart,
		Timestamp: time.Now(),
		Data:      map[string]interface{}{"tool_name": "FileRead", "file": "a.go"},
	}
	e.ProcessEvent(event)

	e.Reset()

	if e.ActiveNarrative() != "" {
		t.Error("ActiveNarrative should be empty after Reset")
	}
}

func TestEngineActiveNarrative(t *testing.T) {
	e := NewEngine(DefaultEngineConfig())
	event := RawEvent{
		Type:      EventTaskStart,
		Timestamp: time.Now(),
		Data:      map[string]interface{}{"description": "test"},
	}
	e.ProcessEvent(event)

	if e.ActiveNarrative() != NarrativeStartingTask {
		t.Errorf("ActiveNarrative = %q, want %q", e.ActiveNarrative(), NarrativeStartingTask)
	}
}

func TestNarrativeForPhase(t *testing.T) {
	tests := []struct {
		phase string
		want  NarrativeType
	}{
		{"initialize", NarrativeReadingProject},
		{"discuss", NarrativeAskingQuestion},
		{"plan", NarrativePlanningWork},
		{"execute", NarrativeStartingTask},
		{"verify", NarrativeRunningTests},
		{"runtime", NarrativeRunningTests},
		{"ship", NarrativePreparingCommit},
		{"unknown", NarrativeReadingProject},
	}
	for _, tt := range tests {
		got := NarrativeForPhase(tt.phase)
		if got != tt.want {
			t.Errorf("NarrativeForPhase(%q) = %q, want %q", tt.phase, got, tt.want)
		}
	}
}

func TestNarrativeForTool(t *testing.T) {
	tests := []struct {
		tool string
		want NarrativeType
	}{
		{"FileRead", NarrativeReadingFile},
		{"FileWrite", NarrativeWritingFile},
		{"Edit", NarrativeEditingFile},
		{"Grep", NarrativeSearchingCode},
		{"Glob", NarrativeFindingFiles},
		{"Bash", NarrativeRunningCommand},
		{"WebFetch", NarrativeFetchingUrl},
		{"WebSearch", NarrativeSearchingWeb},
		{"CodeMap", NarrativeMappingCodebase},
		{"Agent", NarrativeAgentStart},
		{"Unknown", NarrativeRunningCommand},
	}
	for _, tt := range tests {
		got := NarrativeForTool(tt.tool)
		if got != tt.want {
			t.Errorf("NarrativeForTool(%q) = %q, want %q", tt.tool, got, tt.want)
		}
	}
}

func TestEngineConcurrency(t *testing.T) {
	e := NewEngine(DefaultEngineConfig())
	done := make(chan bool)

	for i := 0; i < 10; i++ {
		go func() {
			event := RawEvent{
				Type:      EventTaskStart,
				Timestamp: time.Now(),
				Data:      map[string]interface{}{"description": "concurrent test"},
			}
			_ = e.ProcessEvent(event)
			done <- true
		}()
	}

	for i := 0; i < 10; i++ {
		<-done
	}
}
