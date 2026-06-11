package history

import (
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestFrecentHistoryNew(t *testing.T) {
	fh := NewFrecentHistory("")
	if fh == nil {
		t.Error("nil result")
	}
}

func TestFrecentHistoryNewWithFile(t *testing.T) {
	fp := filepath.Join(t.TempDir(), "history.json")
	if fh := NewFrecentHistory(fp); fh == nil {
		t.Error("nil with file")
	}
}

func TestFrecentHistoryUpsert(t *testing.T) {
	fh := NewFrecentHistory("")
	fh.Upsert("hello")
	fh.Upsert("world")
	fh.Upsert("hello")
	if len(fh.entries) != 2 {
		t.Errorf("entries=%d, want 2", len(fh.entries))
	}
	for _, e := range fh.entries {
		if e.Text == "hello" && e.UseCount != 2 {
			t.Errorf("hello UseCount=%d", e.UseCount)
		}
	}
}

func TestFrecentHistoryUpsertEmpty(t *testing.T) {
	fh := NewFrecentHistory("")
	fh.Upsert("")
	fh.Upsert("  ")
	if len(fh.entries) != 0 {
		t.Error("empty upserts should not add entries")
	}
}

func TestFrecentHistorySearch(t *testing.T) {
	fh := NewFrecentHistory("")
	fh.Upsert("build website")
	fh.Upsert("fix login bug")
	fh.Upsert("add dark mode")
	if r := fh.Search("build", 10); len(r) != 1 {
		t.Errorf("Search('build')=%d, want 1", len(r))
	}
	if r := fh.Search("", 10); len(r) != 3 {
		t.Errorf("Search('')=%d, want 3", len(r))
	}
	if r := fh.Search("xyz", 10); len(r) != 0 {
		t.Errorf("Search('xyz')=%d, want 0", len(r))
	}
}

func TestFrecentHistorySearchLimit(t *testing.T) {
	fh := NewFrecentHistory("")
	for i := 0; i < 20; i++ {
		fh.Upsert("item")
	}
	if r := fh.Search("item", 5); len(r) > 5 {
		t.Errorf("limit=%d", len(r))
	}
}

func TestFrecentHistorySaveLoad(t *testing.T) {
	fp := filepath.Join(t.TempDir(), "history.json")
	fh := NewFrecentHistory("")
	fh.Upsert("task one")
	fh.Upsert("task two")
	fh.filePath = fp
	if err := fh.Save(); err != nil {
		t.Fatal(err)
	}
	fh2 := NewFrecentHistory(fp)
	if len(fh2.entries) != 2 {
		t.Errorf("loaded=%d, want 2", len(fh2.entries))
	}
}

func TestFrecentHistorySaveEmptyPath(t *testing.T) {
	fh := NewFrecentHistory("")
	fh.Upsert("task")
	if err := fh.Save(); err != nil {
		t.Error(err)
	}
}

func TestFrecentHistorySort(t *testing.T) {
	fh := NewFrecentHistory("")
	fh.Upsert("low")
	fh.Upsert("high")
	fh.Upsert("high")
	fh.sort()
	if fh.entries[0].Text != "high" {
		t.Errorf("first=%q, want high", fh.entries[0].Text)
	}
}

func TestFrecentHistoryMaxEntries(t *testing.T) {
	fh := NewFrecentHistory("")
	for i := 0; i < 501; i++ {
		fh.Upsert(strings.Repeat("a", 10) + string(rune('0'+i%10)))
	}
	if len(fh.entries) > 500 {
		t.Errorf("entries=%d, want max 500", len(fh.entries))
	}
}

func TestFrecentHistoryScore(t *testing.T) {
	fh := NewFrecentHistory("")
	fh.Upsert("recent")
	fh.entries = append(fh.entries, FrecentEntry{Text: "old", LastUsed: time.Now().Add(-24 * time.Hour), UseCount: 1})
	fh.entries[1].Score = fh.computeScore(fh.entries[1])
	results := fh.Search("", 10)
	if len(results) >= 2 && results[0].Text != "recent" {
		t.Errorf("first=%q, want recent", results[0].Text)
	}
}
