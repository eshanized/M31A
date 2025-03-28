// Package ledger provides cross-session learning ledger storage.
// After each Ship phase, session outcomes are appended to the ledger
// file at ~/.m31a/LEDGER.md for context injection in future sessions.
package ledger

import (
	"fmt"
	"sort"
	"time"
)

// Entry represents a single completed session in the ledger.
type Entry struct {
	SessionID   string
	Model       string
	Provider    string
	ProjectType string
	TaskCount   int
	TotalCost   float64
	Duration    string
	StartedAt   time.Time
}

// Ledger manages the cross-session learning history.
type Ledger struct {
	entries []Entry
}

// New creates a new Ledger with empty entries.
func New() *Ledger {
	return &Ledger{
		entries: make([]Entry, 0),
	}
}

// Append adds an entry to the in-memory ledger.
// V1 stub: stores in memory only. Full implementation persists to
// ~/.m31a/LEDGER.md and enforces max_entries pruning.
func (l *Ledger) Append(entry Entry) {
	l.entries = append(l.entries, entry)
}

// Entries returns all ledger entries, most recent first.
func (l *Ledger) Entries() ([]Entry, error) {
	if len(l.entries) == 0 {
		return nil, nil
	}
	sorted := make([]Entry, len(l.entries))
	copy(sorted, l.entries)
	sort.Slice(sorted, func(i, j int) bool {
		return sorted[i].StartedAt.After(sorted[j].StartedAt)
	})
	return sorted, nil
}

// EntriesFiltered returns entries filtered by project type and limited to
// the specified count. Tags filtering is reserved for V1.1.
func (l *Ledger) EntriesFiltered(projectType string, tags []string, limit int) ([]Entry, error) {
	all, err := l.Entries()
	if err != nil {
		return nil, err
	}

	var filtered []Entry
	for _, e := range all {
		if projectType != "" && e.ProjectType != projectType {
			continue
		}
		filtered = append(filtered, e)
	}

	if limit > 0 && len(filtered) > limit {
		filtered = filtered[:limit]
	}

	return filtered, nil
}

// Stats returns aggregate statistics over all ledger entries.
// V1 stub: returns a formatted message when no entries exist.
func (l *Ledger) Stats() string {
	if len(l.entries) == 0 {
		return "Ledger stats not available (no completed sessions)"
	}

	var totalCost float64
	taskCounts := make([]int, 0, len(l.entries))
	for _, e := range l.entries {
		totalCost += e.TotalCost
		taskCounts = append(taskCounts, e.TaskCount)
	}

	avgTasks := 0
	if len(taskCounts) > 0 {
		sum := 0
		for _, c := range taskCounts {
			sum += c
		}
		avgTasks = sum / len(taskCounts)
	}

	return fmt.Sprintf(
		"Sessions: %d | Avg tasks: %d | Total cost: $%.4f | Avg cost/session: $%.4f",
		len(l.entries), avgTasks, totalCost, totalCost/float64(len(l.entries)),
	)
}
