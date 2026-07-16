package narrative

import (
	"sync"
	"time"
)

// TimingConfig controls debounce, minimum display duration, and replacement rules.
type TimingConfig struct {
	DebounceWindow time.Duration // events within this window are debounced
	MinDisplay     time.Duration // minimum time a narrative must be visible
	MaxAge         time.Duration // narrative is stale after this time
}

// DefaultTimingConfig returns sensible defaults.
func DefaultTimingConfig() TimingConfig {
	return TimingConfig{
		DebounceWindow: 100 * time.Millisecond,
		MinDisplay:     500 * time.Millisecond,
		MaxAge:         30 * time.Second,
	}
}

// TimingGuard prevents rapid narrative changes by enforcing minimum
// display durations and debouncing rapid events. It is safe for concurrent use.
type TimingGuard struct {
	mu          sync.Mutex
	config      TimingConfig
	lastEmit    time.Time
	lastType    NarrativeType
	lastText    string
	activeStart time.Time
	activeType  NarrativeType
	now         func() time.Time // injectable for testing
}

// NewTimingGuard creates a TimingGuard with the given configuration.
func NewTimingGuard(config TimingConfig) *TimingGuard {
	return &TimingGuard{
		config: config,
		now:    time.Now,
	}
}

// ShouldEmit returns true if the new narrative should be displayed.
// It returns false if the new narrative would replace an active one
// that hasn't been visible for its minimum duration.
func (tg *TimingGuard) ShouldEmit(new NarrativeObject) bool {
	tg.mu.Lock()
	defer tg.mu.Unlock()

	now := tg.now()

	// If there's an active narrative, check if it's been visible long enough.
	if !tg.activeStart.IsZero() {
		elapsed := now.Sub(tg.activeStart)
		if elapsed < tg.config.MinDisplay {
			return false
		}
	}

	// Debounce: if the same text was emitted within the debounce window, skip.
	if new.Text == tg.lastText && now.Sub(tg.lastEmit) < tg.config.DebounceWindow {
		return false
	}

	return true
}

// Activate marks a narrative as currently displayed.
func (tg *TimingGuard) Activate(narrative NarrativeObject) {
	tg.mu.Lock()
	defer tg.mu.Unlock()

	tg.activeStart = tg.now()
	tg.activeType = narrative.Type
	tg.lastEmit = tg.activeStart
	tg.lastType = narrative.Type
	tg.lastText = narrative.Text
}

// Deactivate clears the active narrative.
func (tg *TimingGuard) Deactivate() {
	tg.mu.Lock()
	defer tg.mu.Unlock()

	tg.activeStart = time.Time{}
	tg.activeType = ""
}

// IsStale returns true if the active narrative has exceeded its max age.
func (tg *TimingGuard) IsStale() bool {
	tg.mu.Lock()
	defer tg.mu.Unlock()

	if tg.activeStart.IsZero() {
		return false
	}
	return tg.now().Sub(tg.activeStart) > tg.config.MaxAge
}

// ActiveNarrativeType returns the type of the currently active narrative.
func (tg *TimingGuard) ActiveNarrativeType() NarrativeType {
	tg.mu.Lock()
	defer tg.mu.Unlock()
	return tg.activeType
}

// TimeSinceEmit returns the time since the last narrative was emitted.
func (tg *TimingGuard) TimeSinceEmit() time.Duration {
	tg.mu.Lock()
	defer tg.mu.Unlock()
	return tg.now().Sub(tg.lastEmit)
}

// ReplacementPolicy determines whether a new narrative should replace
// the current active one.
type ReplacementPolicy int

const (
	// ReplaceAlways always allows replacement.
	ReplaceAlways ReplacementPolicy = iota
	// ReplaceOnError only allows replacement if the new narrative is an error.
	ReplaceOnError
	// ReplaceOnPhase only allows replacement on phase transitions.
	ReplaceOnPhase
	// ReplaceOnCompletion allows replacement when the current narrative completes.
	ReplaceOnCompletion
)

// ShouldReplace decides if a new narrative should replace the current active one.
func ShouldReplace(active NarrativeObject, incoming NarrativeObject, policy ReplacementPolicy) bool {
	switch policy {
	case ReplaceAlways:
		return true
	case ReplaceOnError:
		return incoming.Category == CategoryAlerting
	case ReplaceOnPhase:
		return incoming.Priority <= PriorityPhase
	case ReplaceOnCompletion:
		// Replace if the incoming narrative is a completion of the same task.
		return active.Type == NarrativeStartingTask && incoming.Type == NarrativeTaskComplete
	default:
		return false
	}
}

// DisplayOrder returns the priority ordering for sidebar rendering.
// Lower priority number = higher visual weight.
func DisplayOrder(narratives []NarrativeObject) []NarrativeObject {
	sorted := make([]NarrativeObject, len(narratives))
	copy(sorted, narratives)
	// Stable sort by priority, then by timestamp.
	for i := 1; i < len(sorted); i++ {
		for j := i; j > 0; j-- {
			if sorted[j].Priority < sorted[j-1].Priority ||
				(sorted[j].Priority == sorted[j-1].Priority && sorted[j].Timestamp.Before(sorted[j-1].Timestamp)) {
				sorted[j], sorted[j-1] = sorted[j-1], sorted[j]
			}
		}
	}
	return sorted
}
