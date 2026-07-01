package narrative

import (
	"fmt"
	"sort"
	"sync"
	"time"
)

// GroupConfig controls how events are batched together.
type GroupConfig struct {
	Window    time.Duration // maximum time between first and last event in a group
	MaxItems  int           // maximum events before flush
	FlushIdle time.Duration // flush after this much inactivity
}

// DefaultGroupConfig returns sensible defaults.
func DefaultGroupConfig() GroupConfig {
	return GroupConfig{
		Window:    3 * time.Second,
		MaxItems:  10,
		FlushIdle: 2 * time.Second,
	}
}

// GroupedEvent is a raw event that has been assigned to a group.
type GroupedEvent struct {
	Event    RawEvent
	GroupKey string
}

// pendingGroup tracks events waiting to be flushed.
type pendingGroup struct {
	key       string
	events    []RawEvent
	firstTime time.Time
	lastTime  time.Time
}

// Grouper batches related events within time windows. It is safe for
// concurrent use when the same Grouper instance is shared.
type Grouper struct {
	mu      sync.Mutex
	config  GroupConfig
	pending map[string]*pendingGroup
	now     func() time.Time // injectable for testing
}

// NewGrouper creates a Grouper with the given configuration.
func NewGrouper(config GroupConfig) *Grouper {
	return &Grouper{
		config:  config,
		pending: make(map[string]*pendingGroup),
		now:     time.Now,
	}
}

// Add event to a group. Returns the group key. The event is not emitted
// until Flush or FlushAll is called.
func (g *Grouper) Add(event RawEvent, groupKey string) {
	g.mu.Lock()
	defer g.mu.Unlock()

	now := g.now()
	pg, ok := g.pending[groupKey]
	if !ok {
		pg = &pendingGroup{
			key:       groupKey,
			firstTime: now,
		}
		g.pending[groupKey] = pg
	}
	pg.events = append(pg.events, event)
	pg.lastTime = now
}

// ShouldFlush returns true if the group should be flushed based on time
// window or max items.
func (g *Grouper) ShouldFlush(groupKey string) bool {
	g.mu.Lock()
	defer g.mu.Unlock()

	pg, ok := g.pending[groupKey]
	if !ok {
		return false
	}
	now := g.now()

	// Flush if too many items.
	if len(pg.events) >= g.config.MaxItems {
		return true
	}

	// Flush if window expired.
	if now.Sub(pg.firstTime) >= g.config.Window {
		return true
	}

	// Flush if idle too long.
	if now.Sub(pg.lastTime) >= g.config.FlushIdle {
		return true
	}

	return false
}

// Flush returns all events in the group and removes them from pending.
// Returns nil if the group is empty.
func (g *Grouper) Flush(groupKey string) []RawEvent {
	g.mu.Lock()
	defer g.mu.Unlock()

	pg, ok := g.pending[groupKey]
	if !ok || len(pg.events) == 0 {
		return nil
	}

	events := pg.events
	delete(g.pending, groupKey)
	return events
}

// FlushAll returns all pending events across all groups, grouped by key.
func (g *Grouper) FlushAll() map[string][]RawEvent {
	g.mu.Lock()
	defer g.mu.Unlock()

	result := make(map[string][]RawEvent, len(g.pending))
	for key, pg := range g.pending {
		if len(pg.events) > 0 {
			result[key] = pg.events
		}
	}
	g.pending = make(map[string]*pendingGroup)
	return result
}

// PendingCount returns the number of events waiting in all groups.
func (g *Grouper) PendingCount() int {
	g.mu.Lock()
	defer g.mu.Unlock()

	count := 0
	for _, pg := range g.pending {
		count += len(pg.events)
	}
	return count
}

// GroupSummary produces a human-readable summary of grouped file-read events.
func GroupSummary(events []RawEvent, groupKey string) string {
	if len(events) == 0 {
		return ""
	}

	switch groupKey {
	case "tools":
		return toolGroupSummary(events)
	case "task_diff":
		return diffGroupSummary(events)
	case "ship":
		return shipGroupSummary(events)
	default:
		return fmt.Sprintf("%d events", len(events))
	}
}

func toolGroupSummary(events []RawEvent) string {
	// Classify tools by verb.
	reads := 0
	writes := 0
	searches := 0
	commands := 0
	other := 0

	for _, e := range events {
		toolName := e.GetString("tool_name")
		switch toolName {
		case "FileRead":
			reads++
		case "FileWrite", "Edit":
			writes++
		case "Grep", "Glob":
			searches++
		case "Bash":
			commands++
		default:
			other++
		}
	}

	parts := make([]string, 0, 4)
	if reads > 0 {
		parts = append(parts, fmt.Sprintf("%d file reads", reads))
	}
	if writes > 0 {
		parts = append(parts, fmt.Sprintf("%d writes", writes))
	}
	if searches > 0 {
		parts = append(parts, fmt.Sprintf("%d searches", searches))
	}
	if commands > 0 {
		parts = append(parts, fmt.Sprintf("%d commands", commands))
	}
	if other > 0 {
		parts = append(parts, fmt.Sprintf("%d other", other))
	}

	if len(parts) == 0 {
		return fmt.Sprintf("%d tool calls", len(events))
	}
	return joinParts(parts)
}

func diffGroupSummary(events []RawEvent) string {
	totalAdd := 0
	totalDel := 0
	files := make(map[string]bool)

	for _, e := range events {
		file := e.GetString("file")
		if file != "" {
			files[file] = true
		}
		totalAdd += e.GetInt("additions")
		totalDel += e.GetInt("deletions")
	}

	if len(files) > 0 {
		return fmt.Sprintf("%d files changed (+%d/-%d)", len(files), totalAdd, totalDel)
	}
	return fmt.Sprintf("+%d/-%d lines", totalAdd, totalDel)
}

func shipGroupSummary(events []RawEvent) string {
	entries := 0
	for _, e := range events {
		entries += e.GetInt("entry_count")
	}
	if entries > 0 {
		return fmt.Sprintf("Changelog: %d entries", entries)
	}
	return fmt.Sprintf("%d ship events", len(events))
}

func joinParts(parts []string) string {
	if len(parts) == 1 {
		return parts[0]
	}
	if len(parts) == 2 {
		return parts[0] + " and " + parts[1]
	}
	sort.Strings(parts)
	result := ""
	for i, p := range parts {
		if i > 0 {
			if i == len(parts)-1 {
				result += ", and "
			} else {
				result += ", "
			}
		}
		result += p
	}
	return result
}

// CollectFiles extracts unique file paths from a group of events.
func CollectFiles(events []RawEvent) []string {
	seen := make(map[string]bool)
	var files []string
	for _, e := range events {
		for _, key := range []string{"file", "file_path", "path"} {
			if f := e.GetString(key); f != "" && !seen[f] {
				seen[f] = true
				files = append(files, f)
			}
		}
	}
	return files
}
