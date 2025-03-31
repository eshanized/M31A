package ledger

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

// newTestEntry creates a LedgerEntry with specified fields for testing.
func newTestEntry(sessionID string, projectType string, taskCount, failed int) LedgerEntry {
	return LedgerEntry{
		SessionID:       sessionID,
		Timestamp:       time.Now(),
		Model:           "test-model",
		ProjectType:     projectType,
		TaskCount:       taskCount,
		FailedTasks:     failed,
		CostEstimate:    0.50,
		DurationMinutes: 30,
	}
}

// newTestSession creates a Session with specified fields for testing.
func newTestSession(id string, projectType, framework string) types.Session {
	return types.Session{
		ID:       id,
		Model:    "test-model",
		Provider: "test-provider",
		Project: &types.ProjectState{
			Goal:        "build " + projectType + " " + framework + " application",
			ProjectType: projectType,
			Framework:   framework,
		},
	}
}

// setupLedger creates a Ledger backed by a temp file.
func setupLedger(t *testing.T) (*Ledger, string) {
	t.Helper()
	dir := t.TempDir()
	path := filepath.Join(dir, "LEDGER.md")
	l := New(path)
	return l, path
}

// TestNew_FileNotExist verifies New() creates the file when it doesn't exist.
func TestNew_FileNotExist(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "LEDGER.md")
	l := New(path)

	if l == nil {
		t.Fatal("New() returned nil")
	}
	if l.path != path {
		t.Errorf("expected path %q, got %q", path, l.path)
	}

	// Verify file was NOT created yet (file created on first append)
	_, err := os.Stat(path)
	if !os.IsNotExist(err) {
		t.Errorf("expected file to not exist yet, got err=%v", err)
	}
}

// TestNew_ParseExisting verifies New() parses existing file content.
func TestNew_ParseExisting(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "LEDGER.md")

	// Write pre-formatted content
	content := `# Cross-Session Learning Ledger

| Session ID | Timestamp | Model | Project Type | Tasks | Failed | Cost | Duration |
|---|---|---|---|---|---|---|---|
| abc00001 | 2026-05-28T10:30:00Z | model-a | go | 10 | 1 | 0.42 | 45 |
| abc00002 | 2026-05-28T11:00:00Z | model-b | node | 5 | 0 | 0.15 | 20 |
| abc00003 | 2026-05-28T12:00:00Z | model-c | python | 8 | 2 | 0.35 | 30 |
`
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		t.Fatal(err)
	}

	l := New(path)
	entries := l.Entries()

	if len(entries) != 3 {
		t.Fatalf("expected 3 entries, got %d", len(entries))
	}

	// Verify entries parsed correctly (sorted newest-first)
	if entries[0].SessionID != "abc00003" {
		t.Errorf("expected newest first abc00003, got %s", entries[0].SessionID)
	}
	if entries[0].TaskCount != 8 {
		t.Errorf("expected task count 8, got %d", entries[0].TaskCount)
	}
	if entries[0].FailedTasks != 2 {
		t.Errorf("expected failed tasks 2, got %d", entries[0].FailedTasks)
	}
	if entries[0].CostEstimate != 0.35 {
		t.Errorf("expected cost 0.35, got %f", entries[0].CostEstimate)
	}
	if entries[0].DurationMinutes != 30 {
		t.Errorf("expected duration 30, got %d", entries[0].DurationMinutes)
	}
}

// TestAppend verifies entries are appended correctly.
func TestAppend(t *testing.T) {
	l, _ := setupLedger(t)

	e1 := newTestEntry("abc00001", "go", 10, 1)
	e2 := newTestEntry("abc00002", "node", 5, 0)

	if err := l.Append(e1); err != nil {
		t.Fatalf("Append(e1) failed: %v", err)
	}
	if err := l.Append(e2); err != nil {
		t.Fatalf("Append(e2) failed: %v", err)
	}

	entries := l.Entries()
	if len(entries) != 2 {
		t.Fatalf("expected 2 entries, got %d", len(entries))
	}
}

// TestAppend_Dedup verifies that appending the same SessionID twice skips.
func TestAppend_Dedup(t *testing.T) {
	l, _ := setupLedger(t)

	e1 := newTestEntry("abc00001", "go", 10, 1)
	e2 := newTestEntry("abc00001", "node", 5, 0) // same ID, different data

	if err := l.Append(e1); err != nil {
		t.Fatalf("Append(e1) failed: %v", err)
	}
	if err := l.Append(e2); err != nil {
		t.Fatalf("Append(e2) failed: %v", err)
	}

	entries := l.Entries()
	if len(entries) != 1 {
		t.Fatalf("expected 1 entry after dedup, got %d", len(entries))
	}
	// Should keep the first entry (go, not node)
	if entries[0].ProjectType != "go" {
		t.Errorf("expected project type 'go' (first entry), got %q", entries[0].ProjectType)
	}
}

// TestAppend_Persistence verifies entries survive ledger reopen.
func TestAppend_Persistence(t *testing.T) {
	l, path := setupLedger(t)

	entries := []LedgerEntry{
		newTestEntry("abc00001", "go", 10, 1),
		newTestEntry("abc00002", "node", 5, 0),
		newTestEntry("abc00003", "python", 8, 2),
	}

	for _, e := range entries {
		if err := l.Append(e); err != nil {
			t.Fatalf("Append(%s) failed: %v", e.SessionID, err)
		}
	}

	// Reopen ledger
	l2 := New(path)
	reloaded := l2.Entries()
	if len(reloaded) != 3 {
		t.Fatalf("expected 3 entries after reopen, got %d", len(reloaded))
	}
}

// TestEntries_Order verifies entries are sorted newest-first by timestamp.
func TestEntries_Order(t *testing.T) {
	l, _ := setupLedger(t)

	now := time.Now()
	past := now.Add(-2 * time.Hour)
	future := now.Add(2 * time.Hour)

	e1 := newTestEntry("abc00001", "go", 10, 1)
	e1.Timestamp = past

	e2 := newTestEntry("abc00002", "node", 5, 0)
	e2.Timestamp = now

	e3 := newTestEntry("abc00003", "python", 8, 2)
	e3.Timestamp = future

	// Append in wrong order
	if err := l.Append(e1); err != nil {
		t.Fatal(err)
	}
	if err := l.Append(e3); err != nil {
		t.Fatal(err)
	}
	if err := l.Append(e2); err != nil {
		t.Fatal(err)
	}

	entries := l.Entries()
	if len(entries) != 3 {
		t.Fatalf("expected 3 entries, got %d", len(entries))
	}

	// Verify newest-first
	expected := []string{"abc00003", "abc00002", "abc00001"}
	for i, e := range entries {
		if e.SessionID != expected[i] {
			t.Errorf("position %d: expected %s, got %s", i, expected[i], e.SessionID)
		}
	}
}

// TestEntriesFiltered_ByType verifies filtering by project type.
func TestEntriesFiltered_ByType(t *testing.T) {
	l, _ := setupLedger(t)

	entries := []LedgerEntry{
		newTestEntry("abc00001", "go", 10, 1),
		newTestEntry("abc00002", "node", 5, 0),
		newTestEntry("abc00003", "go", 8, 2),
		newTestEntry("abc00004", "python", 3, 0),
	}

	for _, e := range entries {
		l.Append(e)
	}

	filtered := l.EntriesFiltered("go", nil, 0)
	if len(filtered) != 2 {
		t.Fatalf("expected 2 go entries, got %d", len(filtered))
	}
	for _, e := range filtered {
		if e.ProjectType != "go" {
			t.Errorf("expected project type 'go', got %q", e.ProjectType)
		}
	}
}

// TestEntriesFiltered_ByTypeCaseInsensitive verifies case-insensitive filter.
func TestEntriesFiltered_ByTypeCaseInsensitive(t *testing.T) {
	l, _ := setupLedger(t)

	entries := []LedgerEntry{
		newTestEntry("abc00001", "Go", 10, 1),
		newTestEntry("abc00002", "GO", 5, 0),
		newTestEntry("abc00003", "node", 8, 2),
	}

	for _, e := range entries {
		l.Append(e)
	}

	filtered := l.EntriesFiltered("go", nil, 0)
	if len(filtered) != 2 {
		t.Fatalf("expected 2 Go/GO entries, got %d", len(filtered))
	}
}

// TestEntriesFiltered_ByKeywords verifies filtering by keyword substring match.
func TestEntriesFiltered_ByKeywords(t *testing.T) {
	l, _ := setupLedger(t)

	now := time.Now()

	// Create entries with GoalKeywords
	e1 := newTestEntry("abc00001", "go", 10, 1)
	e1.GoalKeywords = []string{"api", "backend", "database"}
	e1.Timestamp = now

	e2 := newTestEntry("abc00002", "node", 5, 0)
	e2.GoalKeywords = []string{"frontend", "ui", "react"}
	e2.Timestamp = now

	e3 := newTestEntry("abc00003", "python", 8, 2)
	e3.GoalKeywords = []string{"api", "auth", "security"}
	e3.Timestamp = now

	l.Append(e1)
	l.Append(e2)
	l.Append(e3)

	// Filter by "api" keyword (should match e1 and e3)
	filtered := l.EntriesFiltered("", []string{"api"}, 0)
	if len(filtered) != 2 {
		t.Fatalf("expected 2 entries matching 'api', got %d", len(filtered))
	}
}

// TestEntriesFiltered_NoMatch verifies empty result for non-matching filter.
func TestEntriesFiltered_NoMatch(t *testing.T) {
	l, _ := setupLedger(t)

	entries := []LedgerEntry{
		newTestEntry("abc00001", "go", 10, 1),
		newTestEntry("abc00002", "node", 5, 0),
	}

	for _, e := range entries {
		l.Append(e)
	}

	filtered := l.EntriesFiltered("rust", nil, 0)
	if len(filtered) != 0 {
		t.Fatalf("expected 0 entries for 'rust', got %d", len(filtered))
	}
}

// TestEntriesFiltered_MaxResults verifies maxResults limit.
func TestEntriesFiltered_MaxResults(t *testing.T) {
	l, _ := setupLedger(t)

	for i := 0; i < 10; i++ {
		e := newTestEntry(
			sprintf("abc%05d", i),
			"go", 5, 0,
		)
		e.Timestamp = time.Now().Add(time.Duration(-i) * time.Hour)
		l.Append(e)
	}

	filtered := l.EntriesFiltered("go", nil, 3)
	if len(filtered) > 3 {
		t.Fatalf("expected at most 3 entries, got %d", len(filtered))
	}
}

// TestEntriesFiltered_DefaultMaxResults verifies default maxResults=5.
func TestEntriesFiltered_DefaultMaxResults(t *testing.T) {
	l, _ := setupLedger(t)

	for i := 0; i < 10; i++ {
		e := newTestEntry(
			sprintf("abc%05d", i),
			"go", 5, 0,
		)
		e.Timestamp = time.Now().Add(time.Duration(-i) * time.Hour)
		l.Append(e)
	}

	// maxResults=0 should default to 5
	filtered := l.EntriesFiltered("go", nil, 0)
	if len(filtered) != 5 {
		t.Fatalf("expected 5 entries (default max), got %d", len(filtered))
	}
}

// TestStats_Empty verifies Stats() returns zero-valued struct for empty ledger.
func TestStats_Empty(t *testing.T) {
	l, _ := setupLedger(t)

	stats := l.Stats()

	if stats.TotalSessions != 0 {
		t.Errorf("expected 0 sessions, got %d", stats.TotalSessions)
	}
	if stats.AvgTaskCount != 0 {
		t.Errorf("expected 0 avg tasks, got %f", stats.AvgTaskCount)
	}
	if stats.AvgCost != 0 {
		t.Errorf("expected 0 avg cost, got %f", stats.AvgCost)
	}
	if stats.AvgDurationMinutes != 0 {
		t.Errorf("expected 0 avg duration, got %f", stats.AvgDurationMinutes)
	}
	if stats.TotalFailedTasks != 0 {
		t.Errorf("expected 0 failed tasks, got %d", stats.TotalFailedTasks)
	}
	if stats.ByProjectType == nil {
		t.Error("expected non-nil ByProjectType map")
	}
}

// TestStats_Computed verifies Stats() computes correct aggregates.
func TestStats_Computed(t *testing.T) {
	l, _ := setupLedger(t)

	now := time.Now()

	entries := []LedgerEntry{
		{SessionID: "abc00001", Timestamp: now, Model: "m1", ProjectType: "go", Framework: "gin", TaskCount: 10, FailedTasks: 2, CostEstimate: 0.50, DurationMinutes: 30},
		{SessionID: "abc00002", Timestamp: now, Model: "m2", ProjectType: "node", Framework: "express", TaskCount: 5, FailedTasks: 0, CostEstimate: 0.20, DurationMinutes: 15},
		{SessionID: "abc00003", Timestamp: now, Model: "m3", ProjectType: "python", Framework: "django", TaskCount: 8, FailedTasks: 1, CostEstimate: 0.35, DurationMinutes: 25},
		{SessionID: "abc00004", Timestamp: now, Model: "m1", ProjectType: "go", Framework: "gin", TaskCount: 15, FailedTasks: 3, CostEstimate: 0.80, DurationMinutes: 60},
		{SessionID: "abc00005", Timestamp: now, Model: "m4", ProjectType: "rust", Framework: "actix", TaskCount: 3, FailedTasks: 0, CostEstimate: 0.10, DurationMinutes: 10},
	}

	for _, e := range entries {
		if err := l.Append(e); err != nil {
			t.Fatalf("Append(%s) failed: %v", e.SessionID, err)
		}
	}

	stats := l.Stats()

	if stats.TotalSessions != 5 {
		t.Errorf("expected 5 sessions, got %d", stats.TotalSessions)
	}

	expectedAvgTasks := (10.0 + 5 + 8 + 15 + 3) / 5
	if stats.AvgTaskCount != expectedAvgTasks {
		t.Errorf("expected avg tasks %.2f, got %.2f", expectedAvgTasks, stats.AvgTaskCount)
	}

	expectedTotalFailed := 2 + 0 + 1 + 3 + 0
	if stats.TotalFailedTasks != expectedTotalFailed {
		t.Errorf("expected total failed %d, got %d", expectedTotalFailed, stats.TotalFailedTasks)
	}

	expectedAvgCost := (0.50 + 0.20 + 0.35 + 0.80 + 0.10) / 5
	if stats.AvgCost != expectedAvgCost {
		t.Errorf("expected avg cost %.2f, got %.2f", expectedAvgCost, stats.AvgCost)
	}

	expectedAvgDuration := (30.0 + 15 + 25 + 60 + 10) / 5
	if stats.AvgDurationMinutes != expectedAvgDuration {
		t.Errorf("expected avg duration %.1f, got %.1f", expectedAvgDuration, stats.AvgDurationMinutes)
	}

	// ByProjectType
	if stats.ByProjectType["go"] != 2 {
		t.Errorf("expected 2 go entries, got %d", stats.ByProjectType["go"])
	}
	if stats.ByProjectType["node"] != 1 {
		t.Errorf("expected 1 node entry, got %d", stats.ByProjectType["node"])
	}

	// TopFailures should include "go" (has entries with failed tasks) and "python"
	if len(stats.TopFailures) == 0 {
		t.Error("expected non-empty TopFailures")
	}

	// TopFrameworks should include "gin" (most common)
	if len(stats.TopFrameworks) == 0 {
		t.Error("expected non-empty TopFrameworks")
	}
	if stats.TopFrameworks[0] != "gin" {
		t.Errorf("expected top framework 'gin', got %q", stats.TopFrameworks[0])
	}
}

// TestTruncate_MoreThanLimit verifies truncation removes oldest entries.
func TestTruncate_MoreThanLimit(t *testing.T) {
	l, _ := setupLedger(t)

	// Append 150 entries
	for i := 0; i < 150; i++ {
		e := newTestEntry(
			sprintf("abc%05d", i),
			"go", 5, 0,
		)
		e.Timestamp = time.Now().Add(time.Duration(-i) * time.Minute)
		l.Append(e)
	}

	if err := l.Truncate(100); err != nil {
		t.Fatalf("Truncate failed: %v", err)
	}

	entries := l.Entries()
	if len(entries) != 100 {
		t.Fatalf("expected 100 entries after truncate, got %d", len(entries))
	}

	// Verify newest entries are kept (abc00000 should be the newest, abc00099 the 100th)
	hasNewest := false
	has100th := false
	for _, e := range entries {
		if e.SessionID == "abc00000" {
			hasNewest = true
		}
		if e.SessionID == "abc00099" {
			has100th = true
		}
	}
	if !hasNewest {
		t.Error("expected newest entry (abc00000) to be kept after truncate")
	}
	if !has100th {
		t.Error("expected 100th-newest entry (abc00099) to be kept after truncate")
	}

	// Verify oldest entries are removed (abc00100 should be the first removed)
	for _, e := range entries {
		if e.SessionID == "abc00100" {
			t.Error("expected oldest entry (abc00100) to be removed after truncate")
			break
		}
	}
}

// TestTruncate_LessThanLimit verifies no truncation when under limit.
func TestTruncate_LessThanLimit(t *testing.T) {
	l, _ := setupLedger(t)

	for i := 0; i < 50; i++ {
		e := newTestEntry(
			sprintf("abc%05d", i),
			"go", 5, 0,
		)
		l.Append(e)
	}

	if err := l.Truncate(100); err != nil {
		t.Fatalf("Truncate failed: %v", err)
	}

	entries := l.Entries()
	if len(entries) != 50 {
		t.Fatalf("expected all 50 entries to remain, got %d", len(entries))
	}
}

// TestTruncate_DefaultLimit verifies truncate with maxEntries=0 defaults to 100.
func TestTruncate_DefaultLimit(t *testing.T) {
	l, _ := setupLedger(t)

	for i := 0; i < 150; i++ {
		e := newTestEntry(
			sprintf("abc%05d", i),
			"go", 5, 0,
		)
		e.Timestamp = time.Now().Add(time.Duration(-i) * time.Minute)
		l.Append(e)
	}

	// maxEntries=0 should default to 100
	if err := l.Truncate(0); err != nil {
		t.Fatalf("Truncate(0) failed: %v", err)
	}

	entries := l.Entries()
	if len(entries) != 100 {
		t.Fatalf("expected 100 entries after truncate(0), got %d", len(entries))
	}
}

// TestNewEntry verifies NewEntry creates correct LedgerEntry from Session.
func TestNewEntry(t *testing.T) {
	session := newTestSession("sess0001", "go", "gin")
	entry := NewEntry(session, 10, 2, 1, 5, 0.42)

	if entry.SessionID != "sess0001" {
		t.Errorf("expected session ID 'sess0001', got %q", entry.SessionID)
	}
	if entry.Model != "test-model" {
		t.Errorf("expected model 'test-model', got %q", entry.Model)
	}
	if entry.ProjectType != "go" {
		t.Errorf("expected project type 'go', got %q", entry.ProjectType)
	}
	if entry.Framework != "gin" {
		t.Errorf("expected framework 'gin', got %q", entry.Framework)
	}
	if entry.TaskCount != 10 {
		t.Errorf("expected task count 10, got %d", entry.TaskCount)
	}
	if entry.FailedTasks != 2 {
		t.Errorf("expected failed tasks 2, got %d", entry.FailedTasks)
	}
	if entry.SkippedTasks != 1 {
		t.Errorf("expected skipped tasks 1, got %d", entry.SkippedTasks)
	}
	if entry.CommitCount != 5 {
		t.Errorf("expected commit count 5, got %d", entry.CommitCount)
	}
	if entry.CostEstimate != 0.42 {
		t.Errorf("expected cost 0.42, got %f", entry.CostEstimate)
	}
	if entry.Timestamp.IsZero() {
		t.Error("expected non-zero timestamp")
	}
	if entry.DurationMinutes < 0 {
		t.Error("expected non-negative duration")
	}

	// GoalKeywords should be extracted from goal
	if len(entry.GoalKeywords) == 0 {
		t.Error("expected non-empty GoalKeywords")
	}
}

// TestNewEntry_NilProject verifies NewEntry handles nil project.
func TestNewEntry_NilProject(t *testing.T) {
	session := types.Session{
		ID:    "sess0002",
		Model: "test-model",
	}
	entry := NewEntry(session, 5, 0, 0, 2, 0.15)

	if entry.ProjectType != "" {
		t.Errorf("expected empty project type, got %q", entry.ProjectType)
	}
	if entry.Framework != "" {
		t.Errorf("expected empty framework, got %q", entry.Framework)
	}
	if len(entry.GoalKeywords) != 0 {
		t.Errorf("expected empty goal keywords, got %v", entry.GoalKeywords)
	}
}

// TestReload verifies Reload() re-reads the file from disk.
func TestReload(t *testing.T) {
	l, path := setupLedger(t)

	e1 := newTestEntry("abc00001", "go", 10, 1)
	l.Append(e1)

	// Write an additional entry directly to the file (simulating external modification)
	e2 := newTestEntry("abc00002", "node", 5, 0)
	l.Append(e2)

	// Reload should pick up both
	if err := l.Reload(); err != nil {
		t.Fatalf("Reload failed: %v", err)
	}

	entries := l.Entries()
	if len(entries) != 2 {
		t.Fatalf("expected 2 entries after reload, got %d", len(entries))
	}

	// Simulate external modification: manually append to file
	f, err := os.OpenFile(path, os.O_APPEND|os.O_WRONLY, 0644)
	if err != nil {
		t.Fatal(err)
	}
	fmtLine := formatEntry(newTestEntry("abc00003", "python", 8, 2))
	if _, err := fmt.Fprintln(f, fmtLine); err != nil {
		t.Fatal(err)
	}
	f.Close()

	// Reload should pick up the external entry
	if err := l.Reload(); err != nil {
		t.Fatalf("Reload after external write failed: %v", err)
	}

	entries = l.Entries()
	if len(entries) != 3 {
		t.Fatalf("expected 3 entries after reload, got %d", len(entries))
	}
}

// sprint is a helper for string formatting (avoids fmt.Sprintf import in test entries).
func sprintf(format string, args ...interface{}) string {
	// Use fmt.Sprintf via a direct call
	return fmt.Sprintf(format, args...)
}
