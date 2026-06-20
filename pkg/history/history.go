package history

import (
	"encoding/json"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"
)

// FrecentHistory tracks prompt history by frecency (frequency + recency).
// Entries are persisted as JSON in the user's session directory.
type FrecentHistory struct {
	mu        sync.RWMutex
	entries   []FrecentEntry
	textIndex map[string]int // text → index in entries for O(1) lookup
	filePath  string
}

// FrecentEntry is a single history entry with scoring metadata.
type FrecentEntry struct {
	Text     string    `json:"text"`
	Score    float64   `json:"score"`
	LastUsed time.Time `json:"last_used"`
	UseCount int       `json:"use_count"`
}

// NewFrecentHistory creates a FrecentHistory backed by the given file path.
// If the file exists it is loaded immediately; otherwise an empty history is created.
func NewFrecentHistory(filePath string) *FrecentHistory {
	fh := &FrecentHistory{filePath: filePath}
	if err := fh.load(); err != nil && !os.IsNotExist(err) {
		// Log corrupt history file but continue with empty history
		slog.Warn("failed to load history, starting fresh", "error", err)
	}
	fh.rebuildIndex()
	return fh
}

func (fh *FrecentHistory) rebuildIndex() {
	fh.textIndex = make(map[string]int, len(fh.entries))
	for i, e := range fh.entries {
		fh.textIndex[e.Text] = i
	}
}

// Upsert adds or updates an entry in the history and persists to disk.
func (fh *FrecentHistory) Upsert(text string) {
	fh.mu.Lock()
	defer fh.mu.Unlock()
	text = strings.TrimSpace(text)
	if text == "" {
		return
	}
	now := time.Now()
	if idx, ok := fh.textIndex[text]; ok {
		fh.entries[idx].UseCount++
		fh.entries[idx].LastUsed = now
		fh.entries[idx].Score = fh.computeScore(fh.entries[idx])
		if err := fh.Save(); err != nil {
			slog.Warn("failed to save history after upsert", "error", err)
		}
		return
	}
	entry := FrecentEntry{
		Text:     text,
		UseCount: 1,
		LastUsed: now,
	}
	entry.Score = fh.computeScore(entry)
	fh.entries = append(fh.entries, entry)
	fh.textIndex[text] = len(fh.entries) - 1
	// Keep at most 500 entries
	if len(fh.entries) > 500 {
		fh.sort()
		fh.rebuildIndex()
		fh.entries = fh.entries[:500]
		fh.rebuildIndex()
	}
	if err := fh.Save(); err != nil {
		slog.Warn("failed to save history after upsert", "error", err)
	}
}

// Search returns entries matching the query, sorted by most recently used first.
func (fh *FrecentHistory) Search(query string, limit int) []FrecentEntry {
	fh.mu.RLock()
	defer fh.mu.RUnlock()
	query = strings.ToLower(strings.TrimSpace(query))
	var results []FrecentEntry
	for _, e := range fh.entries {
		if strings.Contains(strings.ToLower(e.Text), query) {
			results = append(results, e)
		}
	}
	sort.Slice(results, func(i, j int) bool {
		return results[i].LastUsed.After(results[j].LastUsed)
	})
	if limit > 0 && len(results) > limit {
		results = results[:limit]
	}
	return results
}

// Save persists the history to disk.
func (fh *FrecentHistory) Save() error {
	if fh.filePath == "" {
		return nil
	}
	if err := os.MkdirAll(filepath.Dir(fh.filePath), 0o700); err != nil {
		return fmt.Errorf("create history directory: %w", err)
	}
	fh.sort()
	data, err := json.Marshal(fh.entries)
	if err != nil {
		return err
	}
	return os.WriteFile(fh.filePath, data, 0o600)
}

func (fh *FrecentHistory) load() error {
	data, err := os.ReadFile(fh.filePath)
	if err != nil {
		return err
	}
	return json.Unmarshal(data, &fh.entries)
}

func (fh *FrecentHistory) sort() {
	sort.Slice(fh.entries, func(i, j int) bool {
		return fh.entries[i].Score > fh.entries[j].Score
	})
}

func (fh *FrecentHistory) computeScore(e FrecentEntry) float64 {
	ageHours := time.Since(e.LastUsed).Hours()
	// Combine recency (exponential decay) and frequency
	recency := 1.0 / (1.0 + ageHours/24.0)
	return recency*float64(e.UseCount)*10 + recency
}
