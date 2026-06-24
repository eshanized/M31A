package ledger

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// LedgerEntry represents a single session record stored in the ledger.
type LedgerEntry struct {
	SessionID       string    `json:"session_id"`
	Timestamp       time.Time `json:"timestamp"`
	Model           string    `json:"model"`
	Provider        string    `json:"provider"`
	ProjectType     string    `json:"project_type"`
	GoalKeywords    []string  `json:"goal_keywords,omitempty"`
	Framework       string    `json:"framework,omitempty"`
	TaskCount       int       `json:"task_count"`
	FailedTasks     int       `json:"failed_tasks"`
	SkippedTasks    int       `json:"skipped_tasks"`
	CostEstimate    float64   `json:"cost_estimate"`
	DurationMinutes int       `json:"duration_minutes"`
	CommitCount     int       `json:"commit_count"`
}

// Ledger manages persistent session records in a markdown file.
type Ledger struct {
	mu      sync.RWMutex
	path    string
	entries []LedgerEntry

	// Stats cache with mtime-based invalidation.
	statsCache      LedgerStats
	statsCacheMtime time.Time
}

// LedgerStats holds aggregate statistics computed from all ledger entries.
type LedgerStats struct {
	TotalSessions      int            `json:"total_sessions"`
	AvgTaskCount       float64        `json:"avg_task_count"`
	AvgCost            float64        `json:"avg_cost"`
	AvgDurationMinutes float64        `json:"avg_duration_minutes"`
	TotalFailedTasks   int            `json:"total_failed_tasks"`
	TopFailures        []string       `json:"top_failures,omitempty"`
	TopFrameworks      []string       `json:"top_frameworks,omitempty"`
	ByProjectType      map[string]int `json:"by_project_type"`
}

// New creates a Ledger for the given file path. If the file already exists,
// it parses existing entries into memory. The directory is created if needed.
func New(filePath string) *Ledger {
	if err := os.MkdirAll(filepath.Dir(filePath), types.DirPermission); err != nil {
		return &Ledger{path: filePath}
	}

	l := &Ledger{
		path:    filePath,
		entries: make([]LedgerEntry, 0),
	}

	if _, err := os.Stat(filePath); err == nil {
		_ = l.parseFile()
	}

	return l
}

// stopWords are filtered from goal keyword extraction.
var stopWords = map[string]bool{
	"a": true, "an": true, "the": true, "and": true, "or": true,
	"but": true, "in": true, "on": true, "at": true, "to": true,
	"for": true, "of": true, "with": true, "is": true, "are": true,
	"it": true, "as": true, "by": true, "be": true, "this": true,
	"that": true, "from": true, "was": true, "were": true, "been": true,
}

// splitKeywords splits a goal string into individual words, filtering stop words.
func splitKeywords(goal string) []string {
	words := strings.Fields(goal)
	var keywords []string
	for _, w := range words {
		w = strings.Trim(strings.ToLower(w), ".,!?;:'\"")
		if w != "" && !stopWords[w] {
			keywords = append(keywords, w)
		}
	}
	return keywords
}

// NewEntry constructs a LedgerEntry from a Session and summary data.
func NewEntry(session types.Session, taskCount, failedTasks, skippedTasks, commitCount int, costEstimate float64) LedgerEntry {
	entry := LedgerEntry{
		SessionID:       session.ID,
		Timestamp:       time.Now(),
		Model:           session.Model,
		Provider:        session.Provider,
		TaskCount:       taskCount,
		FailedTasks:     failedTasks,
		SkippedTasks:    skippedTasks,
		CostEstimate:    costEstimate,
		DurationMinutes: int(time.Since(session.StartedAt).Minutes()),
		CommitCount:     commitCount,
	}
	if session.Project != nil {
		entry.ProjectType = session.Project.ProjectType
		entry.Framework = session.Project.Framework
		entry.GoalKeywords = splitKeywords(session.Project.Goal)
	}
	return entry
}

// Append adds an entry to the ledger, deduplicating by SessionID.
// If the file doesn't exist, it creates it with a markdown table header.
// Returns an error wrapping ErrTaskFailed if an entry for the same SessionID
// already exists (H-18 idempotency guard). Uses true append-only writes (H9 fix)
// instead of rewriting the entire file on every append.
func (l *Ledger) Append(entry LedgerEntry) error {
	l.mu.Lock()
	defer l.mu.Unlock()

	// Dedup by SessionID — reject duplicates with typed error.
	for _, e := range l.entries {
		if e.SessionID == entry.SessionID {
			return fmt.Errorf("entry already exists for session %s: %w", entry.SessionID, m31errors.ErrTaskFailed)
		}
	}

	l.entries = append(l.entries, entry)

	// Append-only write (H9 fix): if file exists, append the new row;
	// otherwise create the full file with header.
	if _, err := os.Stat(l.path); err == nil {
		return l.appendEntry(entry)
	}
	return l.rewriteFile()
}

// appendEntry appends a single entry to the ledger file without rewriting.
// The file must already exist with the header.
func (l *Ledger) appendEntry(entry LedgerEntry) error {
	f, err := os.OpenFile(l.path, os.O_WRONLY|os.O_APPEND, types.FilePermission)
	if err != nil {
		return fmt.Errorf("append entry (open): %w", err)
	}
	defer f.Close() //nolint:errcheck

	w := bufio.NewWriter(f)
	if _, err := fmt.Fprintln(w, formatEntry(entry)); err != nil {
		return fmt.Errorf("append entry: %w", err)
	}
	if err := w.Flush(); err != nil {
		return fmt.Errorf("flush append: %w", err)
	}
	return nil
}

// formatEntry serializes a LedgerEntry to a markdown table row.
func formatEntry(entry LedgerEntry) string {
	timestamp := entry.Timestamp.Format(time.RFC3339)
	return fmt.Sprintf(
		"| %s | %s | %s | %s | %d | %d | %d | %.2f | %d | %d |",
		entry.SessionID,
		timestamp,
		entry.Model,
		entry.ProjectType,
		entry.TaskCount,
		entry.FailedTasks,
		entry.SkippedTasks,
		entry.CostEstimate,
		entry.DurationMinutes,
		entry.CommitCount,
	)
}

// Entries returns a copy of all entries sorted newest-first by Timestamp.
func (l *Ledger) Entries() []LedgerEntry {
	l.mu.RLock()
	defer l.mu.RUnlock()

	result := make([]LedgerEntry, len(l.entries))
	copy(result, l.entries)
	sort.Slice(result, func(i, j int) bool {
		return result[i].Timestamp.After(result[j].Timestamp)
	})
	return result
}

// EntriesFiltered returns entries matching the given filters, sorted newest-first.
//
//   - projectType: exact match, case-insensitive. If empty, skip this filter.
//   - keywords: an entry passes if ANY keyword matches in GoalKeywords
//     (substring match, case-insensitive).
//   - maxResults: maximum entries to return. If <= 0, defaults to 5.
func (l *Ledger) EntriesFiltered(projectType string, keywords []string, maxResults int) []LedgerEntry {
	l.mu.RLock()
	defer l.mu.RUnlock()

	if maxResults <= 0 {
		maxResults = 5
	}

	var filtered []LedgerEntry
	for _, e := range l.entries {
		// Filter by project type
		if projectType != "" && !strings.EqualFold(e.ProjectType, projectType) {
			continue
		}

		// Filter by keywords
		if len(keywords) > 0 {
			if !matchesAnyKeyword(e.GoalKeywords, keywords) {
				continue
			}
		}

		filtered = append(filtered, e)
	}

	// Sort newest-first
	sort.Slice(filtered, func(i, j int) bool {
		return filtered[i].Timestamp.After(filtered[j].Timestamp)
	})

	// Limit
	if len(filtered) > maxResults {
		filtered = filtered[:maxResults]
	}

	return filtered
}

// matchesAnyKeyword checks if any of the entry's GoalKeywords contains
// any of the query keywords (substring match, case-insensitive).
func matchesAnyKeyword(entryKeywords, queryKeywords []string) bool {
	for _, qk := range queryKeywords {
		qkLower := strings.ToLower(qk)
		for _, ek := range entryKeywords {
			if strings.Contains(strings.ToLower(ek), qkLower) {
				return true
			}
		}
	}
	return false
}

// Stats computes aggregate statistics from all entries.
// Returns zero-valued stats for an empty ledger (never panics).
// Uses mtime-based caching: if LEDGER.md hasn't been modified since
// the last call, returns the cached result (M-16).
func (l *Ledger) Stats() LedgerStats {
	l.mu.Lock()
	defer l.mu.Unlock()

	// Check mtime-based cache
	if info, err := os.Stat(l.path); err == nil {
		if l.statsCacheMtime.Equal(info.ModTime()) && l.statsCache.TotalSessions == len(l.entries) {
			return l.statsCache
		}
	}

	stats := LedgerStats{
		TotalSessions: len(l.entries),
		ByProjectType: make(map[string]int),
	}

	if len(l.entries) == 0 {
		return stats
	}

	var totalTasks, totalDuration int
	var totalCost float64
	failureCounts := make(map[string]int)
	frameworkCounts := make(map[string]int)

	for _, e := range l.entries {
		totalTasks += e.TaskCount
		totalDuration += e.DurationMinutes
		totalCost += e.CostEstimate
		stats.TotalFailedTasks += e.FailedTasks
		stats.ByProjectType[e.ProjectType]++

		if e.FailedTasks > 0 && e.ProjectType != "" {
			failureCounts[e.ProjectType]++
		}
		if e.Framework != "" {
			frameworkCounts[e.Framework]++
		}
	}

	count := float64(len(l.entries))
	stats.AvgTaskCount = float64(totalTasks) / count
	stats.AvgCost = totalCost / count
	stats.AvgDurationMinutes = float64(totalDuration) / count

	// Top failures (by project type with most failed tasks)
	stats.TopFailures = topN(failureCounts, 5)

	// Top frameworks
	stats.TopFrameworks = topN(frameworkCounts, 5)

	// Update stats cache
	if info, err := os.Stat(l.path); err == nil {
		l.statsCache = stats
		l.statsCacheMtime = info.ModTime()
	}

	return stats
}

// topN returns the top N keys from a frequency map, sorted by count descending.
func topN(counts map[string]int, n int) []string {
	type kv struct {
		key   string
		count int
	}
	var sorted []kv
	for k, v := range counts {
		sorted = append(sorted, kv{k, v})
	}
	sort.Slice(sorted, func(i, j int) bool {
		return sorted[i].count > sorted[j].count
	})
	if len(sorted) > n {
		sorted = sorted[:n]
	}
	result := make([]string, len(sorted))
	for i, kv := range sorted {
		result[i] = kv.key
	}
	return result
}

// Truncate removes the oldest entries, keeping only the newest maxEntries.
// If maxEntries <= 0, defaults to 100. If the current count is already
// within the limit, no action is taken.
func (l *Ledger) Truncate(maxEntries int) error {
	l.mu.Lock()
	defer l.mu.Unlock()

	if maxEntries <= 0 {
		maxEntries = 100
	}

	if len(l.entries) <= maxEntries {
		return nil
	}

	// Sort newest-first and keep top maxEntries
	sorted := make([]LedgerEntry, len(l.entries))
	copy(sorted, l.entries)
	sort.Slice(sorted, func(i, j int) bool {
		return sorted[i].Timestamp.After(sorted[j].Timestamp)
	})
	l.entries = sorted[:maxEntries]

	// Rewrite the entire file
	return l.rewriteFile()
}

// rewriteFile writes all in-memory entries to the ledger file atomically.
func (l *Ledger) rewriteFile() error {
	tmpPath := l.path + ".tmp"
	f, err := os.Create(tmpPath)
	if err != nil {
		return fmt.Errorf("create temp file: %w", err)
	}

	writeLines := func() error {
		w := bufio.NewWriter(f)
		if _, err := fmt.Fprintln(w, "# Cross-Session Learning Ledger"); err != nil {
			return err
		}
		if _, err := fmt.Fprintln(w, ""); err != nil {
			return err
		}
		if _, err := fmt.Fprintln(w, "| Session ID | Timestamp | Model | Project Type | Tasks | Failed | Skipped | Cost | Duration | Commits |"); err != nil {
			return err
		}
		if _, err := fmt.Fprintln(w, "|---|---|---|---|---|---|---|---|---|---|"); err != nil {
			return err
		}
		for _, entry := range l.entries {
			if _, err := fmt.Fprintln(w, formatEntry(entry)); err != nil {
				return err
			}
		}
		return w.Flush()
	}

	if err := writeLines(); err != nil {
		_ = f.Close()
		_ = os.Remove(tmpPath)
		return fmt.Errorf("write ledger: %w", err)
	}

	if err := f.Sync(); err != nil {
		_ = f.Close()
		_ = os.Remove(tmpPath)
		return fmt.Errorf("sync ledger: %w", err)
	}
	if err := f.Close(); err != nil {
		_ = os.Remove(tmpPath)
		return fmt.Errorf("close ledger: %w", err)
	}

	if err := os.Rename(tmpPath, l.path); err != nil {
		_ = os.Remove(tmpPath)
		return fmt.Errorf("rename ledger: %w", err)
	}

	return nil
}

// parseFile reads the ledger file and populates the entries slice.
func (l *Ledger) parseFile() error {
	l.entries = make([]LedgerEntry, 0)

	f, err := os.Open(l.path)
	if err != nil {
		return fmt.Errorf("open ledger for parse: %w", err)
	}
	defer f.Close() //nolint:errcheck

	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" || strings.HasPrefix(line, "#") || strings.HasPrefix(line, "|--") {
			continue
		}
		if strings.HasPrefix(line, "|") && strings.Contains(line, "Session ID") {
			continue // skip header row
		}
		if !strings.HasPrefix(line, "|") {
			continue
		}

		entry, err := parseEntry(line)
		if err != nil {
			// Skip malformed lines gracefully
			continue
		}
		l.entries = append(l.entries, entry)
	}

	return scanner.Err()
}

// Reload re-reads the ledger file from disk, re-populating the entries slice.
// Useful if the file was modified externally.
func (l *Ledger) Reload() error {
	l.mu.Lock()
	defer l.mu.Unlock()
	// Invalidate stats cache on reload
	l.statsCacheMtime = time.Time{}
	return l.parseFile()
}

// Path returns the ledger file path.
func (l *Ledger) Path() string {
	return l.path
}

// Clear deletes the ledger file from disk and clears in-memory entries.
func (l *Ledger) Clear() error {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.entries = l.entries[:0]
	l.statsCache = LedgerStats{}
	l.statsCacheMtime = time.Time{}
	if l.path != "" {
		if err := os.Remove(l.path); err != nil && !os.IsNotExist(err) {
			return fmt.Errorf("remove ledger file: %w", err)
		}
	}
	return nil
}

// parseEntry parses a single markdown table row into a LedgerEntry.
// Expected format: | SessionID | Timestamp | Model | ProjectType | TaskCount | FailedTasks | Cost | Duration |
func parseEntry(line string) (LedgerEntry, error) {
	line = strings.TrimSpace(line)
	// Remove leading and trailing pipes
	line = strings.TrimPrefix(line, "|")
	line = strings.TrimSuffix(line, "|")

	parts := strings.Split(line, "|")
	// After stripping leading/trailing pipes, we collect non-empty fields.
	var fields []string
	for _, p := range parts {
		f := strings.TrimSpace(p)
		if f != "" {
			fields = append(fields, f)
		}
	}

	if len(fields) < 10 {
		return LedgerEntry{}, fmt.Errorf("expected 10 fields, got %d: %s", len(fields), line)
	}

	timestamp, err := time.Parse(time.RFC3339, fields[1])
	if err != nil {
		return LedgerEntry{}, fmt.Errorf("parse timestamp %q: %w", fields[1], err)
	}

	taskCount, err := parseInt(fields[4])
	if err != nil {
		return LedgerEntry{}, fmt.Errorf("parse taskCount %q: %w", fields[4], err)
	}
	failedTasks, err := parseInt(fields[5])
	if err != nil {
		return LedgerEntry{}, fmt.Errorf("parse failedTasks %q: %w", fields[5], err)
	}
	skippedTasks, err := parseInt(fields[6])
	if err != nil {
		return LedgerEntry{}, fmt.Errorf("parse skippedTasks %q: %w", fields[6], err)
	}
	cost, err := parseFloat(fields[7])
	if err != nil {
		return LedgerEntry{}, fmt.Errorf("parse cost %q: %w", fields[7], err)
	}
	duration, err := parseInt(fields[8])
	if err != nil {
		return LedgerEntry{}, fmt.Errorf("parse duration %q: %w", fields[8], err)
	}
	commitCount, err := parseInt(fields[9])
	if err != nil {
		return LedgerEntry{}, fmt.Errorf("parse commitCount %q: %w", fields[9], err)
	}

	return LedgerEntry{
		SessionID:       fields[0],
		Timestamp:       timestamp,
		Model:           fields[2],
		ProjectType:     fields[3],
		TaskCount:       taskCount,
		FailedTasks:     failedTasks,
		SkippedTasks:    skippedTasks,
		CostEstimate:    cost,
		DurationMinutes: duration,
		CommitCount:     commitCount,
	}, nil
}

// parseInt parses an integer from a string, returning an error on failure.
func parseInt(s string) (int, error) {
	var n int
	if _, err := fmt.Sscanf(s, "%d", &n); err != nil {
		return 0, err
	}
	return n, nil
}

// parseFloat parses a float64 from a string, returning an error on failure.
func parseFloat(s string) (float64, error) {
	var f float64
	if _, err := fmt.Sscanf(s, "%f", &f); err != nil {
		return 0, err
	}
	return f, nil
}
