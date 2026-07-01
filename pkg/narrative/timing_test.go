package narrative

import (
	"testing"
	"time"
)

func TestTimingGuardShouldEmitFirstEvent(t *testing.T) {
	tg := NewTimingGuard(TimingConfig{
		DebounceWindow: 100 * time.Millisecond,
		MinDisplay:     500 * time.Millisecond,
		MaxAge:         30 * time.Second,
	})

	n := NarrativeObject{
		Type: NarrativeReadingFile,
		Text: "Reading main.go",
	}
	if !tg.ShouldEmit(n) {
		t.Error("First event should always be emitted")
	}
}

func TestTimingGuardShouldEmitAfterMinDisplay(t *testing.T) {
	now := time.Now()
	tg := NewTimingGuard(TimingConfig{
		DebounceWindow: 100 * time.Millisecond,
		MinDisplay:     500 * time.Millisecond,
		MaxAge:         30 * time.Second,
	})
	tg.now = func() time.Time { return now }

	n1 := NarrativeObject{Type: NarrativeReadingFile, Text: "Reading main.go"}
	tg.Activate(n1)

	// Advance time past min display.
	now = now.Add(600 * time.Millisecond)
	n2 := NarrativeObject{Type: NarrativeWritingFile, Text: "Writing config.go"}
	if !tg.ShouldEmit(n2) {
		t.Error("Should emit after MinDisplay")
	}
}

func TestTimingGuardShouldNotEmitBeforeMinDisplay(t *testing.T) {
	now := time.Now()
	tg := NewTimingGuard(TimingConfig{
		DebounceWindow: 100 * time.Millisecond,
		MinDisplay:     500 * time.Millisecond,
		MaxAge:         30 * time.Second,
	})
	tg.now = func() time.Time { return now }

	n1 := NarrativeObject{Type: NarrativeReadingFile, Text: "Reading main.go"}
	tg.Activate(n1)

	// Don't advance time.
	n2 := NarrativeObject{Type: NarrativeWritingFile, Text: "Writing config.go"}
	if tg.ShouldEmit(n2) {
		t.Error("Should not emit before MinDisplay")
	}
}

func TestTimingGuardDebounce(t *testing.T) {
	now := time.Now()
	tg := NewTimingGuard(TimingConfig{
		DebounceWindow: 200 * time.Millisecond,
		MinDisplay:     50 * time.Millisecond,
		MaxAge:         30 * time.Second,
	})
	tg.now = func() time.Time { return now }

	n := NarrativeObject{Type: NarrativeReadingFile, Text: "Reading main.go"}
	tg.Activate(n)

	// Advance time past min display but within debounce window.
	now = now.Add(100 * time.Millisecond)
	n2 := NarrativeObject{Type: NarrativeReadingFile, Text: "Reading main.go"}
	if tg.ShouldEmit(n2) {
		t.Error("Should not emit same text within debounce window")
	}

	// Different text should be emitted.
	n3 := NarrativeObject{Type: NarrativeReadingFile, Text: "Reading config.go"}
	if !tg.ShouldEmit(n3) {
		t.Error("Different text should be emitted")
	}
}

func TestTimingGuardDebounceExpired(t *testing.T) {
	now := time.Now()
	tg := NewTimingGuard(TimingConfig{
		DebounceWindow: 100 * time.Millisecond,
		MinDisplay:     100 * time.Millisecond,
		MaxAge:         30 * time.Second,
	})
	tg.now = func() time.Time { return now }

	n := NarrativeObject{Type: NarrativeReadingFile, Text: "Reading main.go"}
	tg.Activate(n)

	// Advance time past both min display and debounce window.
	now = now.Add(250 * time.Millisecond)
	n2 := NarrativeObject{Type: NarrativeReadingFile, Text: "Reading main.go"}
	if !tg.ShouldEmit(n2) {
		t.Error("Should emit after debounce window expires")
	}
}

func TestTimingGuardIsStale(t *testing.T) {
	now := time.Now()
	tg := NewTimingGuard(TimingConfig{
		DebounceWindow: 100 * time.Millisecond,
		MinDisplay:     100 * time.Millisecond,
		MaxAge:         5 * time.Second,
	})
	tg.now = func() time.Time { return now }

	n := NarrativeObject{Type: NarrativeReadingFile, Text: "Reading main.go"}
	tg.Activate(n)

	if tg.IsStale() {
		t.Error("Should not be stale immediately")
	}

	// Advance past max age.
	now = now.Add(6 * time.Second)
	if !tg.IsStale() {
		t.Error("Should be stale after MaxAge")
	}
}

func TestTimingGuardDeactivate(t *testing.T) {
	tg := NewTimingGuard(DefaultTimingConfig())

	n := NarrativeObject{Type: NarrativeReadingFile, Text: "Reading main.go"}
	tg.Activate(n)

	tg.Deactivate()
	if tg.ActiveNarrativeType() != "" {
		t.Error("ActiveNarrativeType should be empty after Deactivate")
	}
}

func TestTimingGuardTimeSinceEmit(t *testing.T) {
	now := time.Now()
	tg := NewTimingGuard(DefaultTimingConfig())
	tg.now = func() time.Time { return now }

	n := NarrativeObject{Type: NarrativeReadingFile, Text: "Reading main.go"}
	tg.Activate(n)

	now = now.Add(3 * time.Second)
	since := tg.TimeSinceEmit()
	if since != 3*time.Second {
		t.Errorf("TimeSinceEmit = %v, want 3s", since)
	}
}

func TestShouldReplaceAlways(t *testing.T) {
	active := NarrativeObject{Type: NarrativeReadingFile}
	incoming := NarrativeObject{Type: NarrativeWritingFile}
	if !ShouldReplace(active, incoming, ReplaceAlways) {
		t.Error("ReplaceAlways should always return true")
	}
}

func TestShouldReplaceOnError(t *testing.T) {
	active := NarrativeObject{Type: NarrativeReadingFile}
	incoming := NarrativeObject{Type: NarrativeError, Category: CategoryAlerting}
	if !ShouldReplace(active, incoming, ReplaceOnError) {
		t.Error("ReplaceOnError should return true for alerting")
	}

	incoming2 := NarrativeObject{Type: NarrativeWritingFile, Category: CategoryExecuting}
	if ShouldReplace(active, incoming2, ReplaceOnError) {
		t.Error("ReplaceOnError should return false for non-alerting")
	}
}

func TestShouldReplaceOnPhase(t *testing.T) {
	active := NarrativeObject{Type: NarrativeReadingFile}
	incoming := NarrativeObject{Type: NarrativeTaskComplete, Priority: PriorityPhase}
	if !ShouldReplace(active, incoming, ReplaceOnPhase) {
		t.Error("ReplaceOnPhase should return true for phase transitions")
	}

	incoming2 := NarrativeObject{Type: NarrativeReadingFile, Priority: PriorityTask}
	if ShouldReplace(active, incoming2, ReplaceOnPhase) {
		t.Error("ReplaceOnPhase should return false for task-level events")
	}
}

func TestShouldReplaceOnCompletion(t *testing.T) {
	active := NarrativeObject{Type: NarrativeStartingTask}
	incoming := NarrativeObject{Type: NarrativeTaskComplete}
	if !ShouldReplace(active, incoming, ReplaceOnCompletion) {
		t.Error("ReplaceOnCompletion should return true for task complete after task start")
	}

	active2 := NarrativeObject{Type: NarrativeReadingFile}
	incoming2 := NarrativeObject{Type: NarrativeTaskComplete}
	if ShouldReplace(active2, incoming2, ReplaceOnCompletion) {
		t.Error("ReplaceOnCompletion should return false when active is not task start")
	}
}

func TestDisplayOrder(t *testing.T) {
	narratives := []NarrativeObject{
		{Type: NarrativeReadingFile, Priority: PriorityTask, Timestamp: time.Now()},
		{Type: NarrativeProviderError, Priority: PriorityActionRequired, Timestamp: time.Now()},
		{Type: NarrativeTaskComplete, Priority: PriorityTask, Timestamp: time.Now().Add(-time.Second)},
	}

	sorted := DisplayOrder(narratives)
	if sorted[0].Type != NarrativeProviderError {
		t.Errorf("First should be ProviderError, got %q", sorted[0].Type)
	}
	// Same priority: earlier timestamp comes first
	if sorted[1].Type != NarrativeTaskComplete {
		t.Errorf("Second should be TaskComplete (earlier timestamp), got %q", sorted[1].Type)
	}
	if sorted[2].Type != NarrativeReadingFile {
		t.Errorf("Third should be ReadingFile, got %q", sorted[2].Type)
	}
}
