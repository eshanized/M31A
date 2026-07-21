package ledger

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
)

// ---------------------------------------------------------------------------
// splitKeywords tests
// ---------------------------------------------------------------------------

func TestSplitKeywords_Basic(t *testing.T) {
	kws := splitKeywords("Build a Go REST API")
	if len(kws) == 0 {
		t.Fatal("splitKeywords returned empty")
	}
	// "a" is a stop word
	for _, w := range kws {
		if w == "a" {
			t.Error("stop word 'a' should be filtered out")
		}
	}
	// Should contain "build", "go", "rest", "api"
	found := map[string]bool{}
	for _, w := range kws {
		found[w] = true
	}
	for _, expected := range []string{"build", "go", "rest", "api"} {
		if !found[expected] {
			t.Errorf("expected keyword %q not found in %v", expected, kws)
		}
	}
}

func TestSplitKeywords_AllStopWords(t *testing.T) {
	kws := splitKeywords("a an the and or but in on at to")
	if len(kws) != 0 {
		t.Errorf("expected empty keywords for all stop words, got %v", kws)
	}
}

func TestSplitKeywords_Empty(t *testing.T) {
	kws := splitKeywords("")
	if len(kws) != 0 {
		t.Errorf("expected empty keywords for empty string, got %v", kws)
	}
}

func TestSplitKeywords_Punctuation(t *testing.T) {
	kws := splitKeywords("Hello World. Test going?")
	found := map[string]bool{}
	for _, w := range kws {
		found[w] = true
	}
	// Punctuation should be stripped
	for _, word := range []string{"hello", "world", "test", "going"} {
		if !found[word] {
			t.Errorf("expected keyword %q, not found in %v", word, kws)
		}
	}
}

func TestSplitKeywords_CaseInsensitive(t *testing.T) {
	kws := splitKeywords("BUILD GO REST API")
	for _, w := range kws {
		if w != strings.ToLower(w) {
			t.Errorf("expected lowercase keyword, got %q", w)
		}
	}
}

// ---------------------------------------------------------------------------
// formatEntry tests
// ---------------------------------------------------------------------------

func TestFormatEntry(t *testing.T) {
	entry := LedgerEntry{
		SessionID:       "abc00001",
		Timestamp:       time.Date(2026, 5, 28, 10, 30, 0, 0, time.UTC),
		Model:           "model-a",
		ProjectType:     "go",
		TaskCount:       10,
		FailedTasks:     2,
		SkippedTasks:    1,
		CostEstimate:    0.42,
		DurationMinutes: 45,
		CommitCount:     3,
	}

	line := formatEntry(entry)

	if !strings.HasPrefix(line, "| ") {
		t.Error("formatEntry should start with '| '")
	}
	if !strings.HasSuffix(line, "|") {
		t.Error("formatEntry should end with '|'")
	}
	if !strings.Contains(line, "abc00001") {
		t.Error("formatEntry should contain session ID")
	}
	if !strings.Contains(line, "model-a") {
		t.Error("formatEntry should contain model")
	}
	if !strings.Contains(line, "go") {
		t.Error("formatEntry should contain project type")
	}
	if !strings.Contains(line, "0.42") {
		t.Error("formatEntry should contain cost estimate")
	}
}

func TestFormatEntry_ZeroValues(t *testing.T) {
	entry := LedgerEntry{
		SessionID:   "zero0001",
		Timestamp:   time.Time{},
		Model:       "",
		ProjectType: "",
	}

	line := formatEntry(entry)
	if !strings.Contains(line, "zero0001") {
		t.Error("formatEntry should contain session ID even with zero values")
	}
}

// ---------------------------------------------------------------------------
// topN tests
// ---------------------------------------------------------------------------

func TestTopN(t *testing.T) {
	counts := map[string]int{
		"go":     5,
		"node":   3,
		"python": 4,
		"rust":   1,
	}
	result := topN(counts, 2)
	if len(result) != 2 {
		t.Fatalf("topN(2) should return 2 results, got %d", len(result))
	}
	if result[0] != "go" {
		t.Errorf("topN first should be 'go', got %q", result[0])
	}
	if result[1] != "python" {
		t.Errorf("topN second should be 'python', got %q", result[1])
	}
}

func TestTopN_MoreThanAvailable(t *testing.T) {
	counts := map[string]int{"go": 1}
	result := topN(counts, 5)
	if len(result) != 1 {
		t.Errorf("topN should return 1 result, got %d", len(result))
	}
}

func TestTopN_EmptyMap(t *testing.T) {
	counts := map[string]int{}
	result := topN(counts, 5)
	if len(result) != 0 {
		t.Errorf("topN on empty map should return empty, got %v", result)
	}
}

func TestTopN_ZeroN(t *testing.T) {
	counts := map[string]int{"go": 1}
	result := topN(counts, 0)
	if len(result) != 0 {
		t.Errorf("topN(0) should return empty, got %v", result)
	}
}

// ---------------------------------------------------------------------------
// matchesAnyKeyword tests
// ---------------------------------------------------------------------------

func TestMatchesAnyKeyword_SubstringMatch(t *testing.T) {
	entryKws := []string{"api", "backend", "database"}
	queryKws := []string{"back"}

	if !matchesAnyKeyword(entryKws, queryKws) {
		t.Error("should match 'back' as substring of 'backend'")
	}
}

func TestMatchesAnyKeyword_CaseInsensitive(t *testing.T) {
	entryKws := []string{"API", "Backend"}
	queryKws := []string{"api"}

	if !matchesAnyKeyword(entryKws, queryKws) {
		t.Error("should match case-insensitively")
	}
}

func TestMatchesAnyKeyword_NoMatch(t *testing.T) {
	entryKws := []string{"api", "backend"}
	queryKws := []string{"frontend"}

	if matchesAnyKeyword(entryKws, queryKws) {
		t.Error("should not match when no keywords overlap")
	}
}

func TestMatchesAnyKeyword_EmptyQuery(t *testing.T) {
	entryKws := []string{"api", "backend"}
	if matchesAnyKeyword(entryKws, []string{}) {
		t.Error("should not match with empty query")
	}
}

func TestMatchesAnyKeyword_EmptyEntryKeywords(t *testing.T) {
	if matchesAnyKeyword([]string{}, []string{"api"}) {
		t.Error("should not match with empty entry keywords")
	}
}

func TestMatchesAnyKeyword_ExactMatch(t *testing.T) {
	entryKws := []string{"api"}
	if !matchesAnyKeyword(entryKws, []string{"api"}) {
		t.Error("should match exact keyword")
	}
}

// ---------------------------------------------------------------------------
// parseInt / parseFloat tests
// ---------------------------------------------------------------------------

func TestParseInt(t *testing.T) {
	n, err := parseInt("42")
	if err != nil {
		t.Fatalf("parseInt failed: %v", err)
	}
	if n != 42 {
		t.Errorf("expected 42, got %d", n)
	}
}

func TestParseInt_Zero(t *testing.T) {
	n, err := parseInt("0")
	if err != nil {
		t.Fatalf("parseInt failed: %v", err)
	}
	if n != 0 {
		t.Errorf("expected 0, got %d", n)
	}
}

func TestParseInt_Invalid(t *testing.T) {
	_, err := parseInt("abc")
	if err == nil {
		t.Error("parseInt should fail for non-numeric string")
	}
}

func TestParseInt_Float(t *testing.T) {
	// Sscanf with %d parses the integer part "1" from "1.5" without error
	n, err := parseInt("1.5")
	if err != nil {
		t.Errorf("parseInt('1.5') unexpected error: %v", err)
	}
	if n != 1 {
		t.Errorf("expected 1 (integer part), got %d", n)
	}
}

func TestParseFloat(t *testing.T) {
	f, err := parseFloat("3.14")
	if err != nil {
		t.Fatalf("parseFloat failed: %v", err)
	}
	if f < 3.13 || f > 3.15 {
		t.Errorf("expected ~3.14, got %f", f)
	}
}

func TestParseFloat_Invalid(t *testing.T) {
	_, err := parseFloat("abc")
	if err == nil {
		t.Error("parseFloat should fail for non-numeric string")
	}
}

func TestParseFloat_Zero(t *testing.T) {
	f, err := parseFloat("0")
	if err != nil {
		t.Fatalf("parseFloat failed: %v", err)
	}
	if f != 0 {
		t.Errorf("expected 0, got %f", f)
	}
}

// ---------------------------------------------------------------------------
// parseEntry tests
// ---------------------------------------------------------------------------

func TestParseEntry_Valid(t *testing.T) {
	line := "| abc00001 | 2026-05-28T10:30:00Z | model-a | go | 10 | 2 | 1 | 0.42 | 45 | 3 |"
	entry, err := parseEntry(line)
	if err != nil {
		t.Fatalf("parseEntry failed: %v", err)
	}
	if entry.SessionID != "abc00001" {
		t.Errorf("expected session ID abc00001, got %s", entry.SessionID)
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
	if entry.CommitCount != 3 {
		t.Errorf("expected commit count 3, got %d", entry.CommitCount)
	}
}

func TestParseEntry_TooFewFields(t *testing.T) {
	line := "| abc00001 | model-a | go |"
	_, err := parseEntry(line)
	if err == nil {
		t.Error("parseEntry should fail for line with too few fields")
	}
}

func TestParseEntry_InvalidTimestamp(t *testing.T) {
	line := "| abc00001 | not-a-timestamp | model-a | go | 10 | 2 | 1 | 0.42 | 45 | 3 |"
	_, err := parseEntry(line)
	if err == nil {
		t.Error("parseEntry should fail for invalid timestamp")
	}
}

func TestParseEntry_InvalidTaskCount(t *testing.T) {
	line := "| abc00001 | 2026-05-28T10:30:00Z | model-a | go | notanumber | 2 | 1 | 0.42 | 45 | 3 |"
	_, err := parseEntry(line)
	if err == nil {
		t.Error("parseEntry should fail for invalid task count")
	}
}

func TestParseEntry_InvalidCost(t *testing.T) {
	line := "| abc00001 | 2026-05-28T10:30:00Z | model-a | go | 10 | 2 | 1 | notacost | 45 | 3 |"
	_, err := parseEntry(line)
	if err == nil {
		t.Error("parseEntry should fail for invalid cost")
	}
}

func TestParseEntry_InvalidDuration(t *testing.T) {
	line := "| abc00001 | 2026-05-28T10:30:00Z | model-a | go | 10 | 2 | 1 | 0.42 | notaduration | 3 |"
	_, err := parseEntry(line)
	if err == nil {
		t.Error("parseEntry should fail for invalid duration")
	}
}

func TestParseEntry_InvalidCommitCount(t *testing.T) {
	line := "| abc00001 | 2026-05-28T10:30:00Z | model-a | go | 10 | 2 | 1 | 0.42 | 45 | notanumber |"
	_, err := parseEntry(line)
	if err == nil {
		t.Error("parseEntry should fail for invalid commit count")
	}
}

// ---------------------------------------------------------------------------
// EntriesFiltered combined tests
// ---------------------------------------------------------------------------

func TestEntriesFiltered_CombinedTypeAndKeywords(t *testing.T) {
	l, _ := setupLedger(t)

	now := time.Now()

	e1 := newTestEntry("abc00001", "go", 10, 1)
	e1.GoalKeywords = []string{"api", "backend"}
	e1.Timestamp = now

	e2 := newTestEntry("abc00002", "go", 5, 0)
	e2.GoalKeywords = []string{"frontend", "ui"}
	e2.Timestamp = now

	e3 := newTestEntry("abc00003", "node", 8, 2)
	e3.GoalKeywords = []string{"api", "auth"}
	e3.Timestamp = now

	l.Append(e1)
	l.Append(e2)
	l.Append(e3)

	// Filter by type "go" AND keyword "api" → only e1
	filtered := l.EntriesFiltered("go", []string{"api"}, 0)
	if len(filtered) != 1 {
		t.Fatalf("expected 1 entry, got %d", len(filtered))
	}
	if filtered[0].SessionID != "abc00001" {
		t.Errorf("expected abc00001, got %s", filtered[0].SessionID)
	}
}

func TestEntriesFiltered_NegativeMaxResults(t *testing.T) {
	l, _ := setupLedger(t)

	for i := 0; i < 10; i++ {
		e := newTestEntry(
			fmt.Sprintf("abc%05d", i),
			"go", 5, 0,
		)
		e.Timestamp = time.Now().Add(time.Duration(-i) * time.Hour)
		l.Append(e)
	}

	// Negative maxResults should default to 5
	filtered := l.EntriesFiltered("go", nil, -1)
	if len(filtered) != 5 {
		t.Fatalf("expected 5 entries with negative maxResults, got %d", len(filtered))
	}
}

func TestEntriesFiltered_MultipleKeywords(t *testing.T) {
	l, _ := setupLedger(t)

	e1 := newTestEntry("abc00001", "go", 10, 0)
	e1.GoalKeywords = []string{"api"}
	e2 := newTestEntry("abc00002", "go", 5, 0)
	e2.GoalKeywords = []string{"database"}
	e3 := newTestEntry("abc00003", "go", 8, 0)
	e3.GoalKeywords = []string{"frontend"}

	l.Append(e1)
	l.Append(e2)
	l.Append(e3)

	// Match any of the keywords
	filtered := l.EntriesFiltered("", []string{"api", "database"}, 0)
	if len(filtered) != 2 {
		t.Fatalf("expected 2 entries, got %d", len(filtered))
	}
}

// ---------------------------------------------------------------------------
// Reload edge cases
// ---------------------------------------------------------------------------

func TestReload_EmptyFile(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "LEDGER.md")

	l := New(path)
	l.Append(newTestEntry("abc00001", "go", 5, 0))

	// Verify entry exists
	if len(l.Entries()) != 1 {
		t.Fatal("expected 1 entry after append")
	}

	// Overwrite the file with empty content
	os.WriteFile(path, []byte(""), 0644)

	if err := l.Reload(); err != nil {
		t.Fatalf("Reload failed: %v", err)
	}
	// After reload from empty file, entries should be empty
	entries := l.Entries()
	if len(entries) != 0 {
		t.Errorf("expected 0 entries after reload from empty file, got %d", len(entries))
	}
}

func TestReload_MalformedLines(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "LEDGER.md")
	content := `# Cross-Session Learning Ledger

| Session ID | Timestamp | Model | Project Type | Tasks | Failed | Skipped | Cost | Duration | Commits |
|---|---|---|---|---|---|---|---|---|---|
| abc00001 | 2026-05-28T10:30:00Z | model-a | go | 10 | 1 | 0 | 0.42 | 45 | 3 |
this is not a table row
| abc00002 | not-a-date | model-b | node | 5 | 0 | 0 | 0.15 | 20 | 1 |
`
	os.WriteFile(path, []byte(content), 0644)

	l := New(path)
	entries := l.Entries()
	// Only the valid entry should be parsed
	if len(entries) != 1 {
		t.Errorf("expected 1 valid entry, got %d", len(entries))
	}
}

// ---------------------------------------------------------------------------
// NewEntry with goal keywords
// ---------------------------------------------------------------------------

func TestNewEntry_GoalKeywords(t *testing.T) {
	session := types.Session{
		ID:    "sess0003",
		Model: "m",
		Project: &types.ProjectState{
			Goal:        "Build a robust Go REST API with authentication",
			ProjectType: "go",
			Framework:   "gin",
		},
	}
	entry := NewEntry(session, 5, 0, 0, 2, 0.10)

	if len(entry.GoalKeywords) == 0 {
		t.Error("expected non-empty GoalKeywords")
	}
	// "a" and "with" are stop words, should be filtered
	for _, kw := range entry.GoalKeywords {
		if kw == "a" || kw == "with" {
			t.Errorf("stop word %q should be filtered from GoalKeywords", kw)
		}
	}
}

// ---------------------------------------------------------------------------
// Truncate edge cases
// ---------------------------------------------------------------------------

func TestTruncate_NegativeMax(t *testing.T) {
	l, _ := setupLedger(t)

	for i := 0; i < 10; i++ {
		e := newTestEntry(
			fmt.Sprintf("abc%05d", i),
			"go", 5, 0,
		)
		e.Timestamp = time.Now().Add(time.Duration(-i) * time.Minute)
		l.Append(e)
	}

	// Negative maxEntries defaults to 100
	if err := l.Truncate(-1); err != nil {
		t.Fatalf("Truncate(-1) failed: %v", err)
	}
	entries := l.Entries()
	if len(entries) != 10 {
		t.Errorf("expected 10 entries (under default limit of 100), got %d", len(entries))
	}
}

// ---------------------------------------------------------------------------
// Stats with no failures
// ---------------------------------------------------------------------------

func TestStats_NoFailures(t *testing.T) {
	l, _ := setupLedger(t)

	e1 := newTestEntry("abc00001", "go", 10, 0)
	e1.Framework = "gin"
	l.Append(e1)

	stats := l.Stats()
	if stats.TotalFailedTasks != 0 {
		t.Errorf("expected 0 failed tasks, got %d", stats.TotalFailedTasks)
	}
	if len(stats.TopFailures) != 0 {
		t.Errorf("expected empty TopFailures, got %v", stats.TopFailures)
	}
	if len(stats.TopFrameworks) != 1 {
		t.Errorf("expected 1 top framework, got %d", len(stats.TopFrameworks))
	}
}

func TestStats_NoFrameworks(t *testing.T) {
	l, _ := setupLedger(t)

	e1 := newTestEntry("abc00001", "go", 10, 0)
	l.Append(e1)

	stats := l.Stats()
	if len(stats.TopFrameworks) != 0 {
		t.Errorf("expected empty TopFrameworks, got %v", stats.TopFrameworks)
	}
}
