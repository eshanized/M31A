package tui

import (
	"crypto/rand"
	"encoding/hex"
	"encoding/json"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

// HistoryEntry represents a single prompt history entry with frecency tracking.
type HistoryEntry struct {
	Text      string    `json:"text"`
	LastUsed  time.Time `json:"last_used"`
	Frequency int       `json:"frequency"`
	FirstUsed time.Time `json:"first_used"`
}

// HistoryData is the JSON serialization wrapper for history entries.
type HistoryData struct {
	Entries []HistoryEntry `json:"entries"`
}

// FrecentHistory manages persistent prompt history with frecency scoring.
// Frecency = frequency / (hours_since_last_use + 1)
type FrecentHistory struct {
	entries  []HistoryEntry
	maxSize  int
	filePath string
}

// NewFrecentHistory creates a new FrecentHistory with the given file path and max size.
func NewFrecentHistory(filePath string, maxSize int) *FrecentHistory {
	return &FrecentHistory{
		entries:  []HistoryEntry{},
		maxSize:  maxSize,
		filePath: filePath,
	}
}

// Load reads history from disk. If the file doesn't exist, starts with an empty history.
func (h *FrecentHistory) Load() error {
	data, err := os.ReadFile(h.filePath)
	if err != nil {
		if os.IsNotExist(err) {
			h.entries = []HistoryEntry{}
			return nil
		}
		return err
	}

	var hd HistoryData
	if err := json.Unmarshal(data, &hd); err != nil {
		return err
	}

	if hd.Entries == nil {
		h.entries = []HistoryEntry{}
	} else {
		h.entries = hd.Entries
	}
	return nil
}

// Save atomically writes history to disk. Before saving, enforces maxSize by evicting
// lowest-frecency entries.
func (h *FrecentHistory) Save() error {
	// Evict low-frecency entries if over limit
	if len(h.entries) > h.maxSize {
		h.sortByFrecency(false, h.entries) // ascending — lowest first
		h.entries = h.entries[len(h.entries)-h.maxSize:]
	}

	data := HistoryData{Entries: h.entries}
	payload, err := json.MarshalIndent(data, "", "  ")
	if err != nil {
		return err
	}

	return h.atomicWrite(h.filePath, payload)
}

// Upsert inserts or updates a prompt entry. If the text already exists, increments
// frequency and updates last_used. If new, appends with frequency=1.
func (h *FrecentHistory) Upsert(text string) {
	text = strings.TrimSpace(text)
	if text == "" {
		return
	}

	for i, entry := range h.entries {
		if entry.Text == text {
			h.entries[i].Frequency++
			h.entries[i].LastUsed = time.Now()
			return
		}
	}

	h.entries = append(h.entries, HistoryEntry{
		Text:      text,
		Frequency: 1,
		FirstUsed: time.Now(),
		LastUsed:  time.Now(),
	})
}

// Search returns up to `limit` entries matching the prefix, sorted by frecency descending.
// If prefix is empty, returns top entries overall.
func (h *FrecentHistory) Search(prefix string, limit int) []HistoryEntry {
	var filtered []HistoryEntry

	if prefix == "" {
		filtered = make([]HistoryEntry, len(h.entries))
		copy(filtered, h.entries)
	} else {
		for _, entry := range h.entries {
			if strings.HasPrefix(entry.Text, prefix) || strings.Contains(entry.Text, prefix) {
				filtered = append(filtered, entry)
			}
		}
	}

	h.sortByFrecency(true, filtered)

	if len(filtered) > limit {
		filtered = filtered[:limit]
	}

	return filtered
}

// Frecency computes the frecency score for an entry:
// score = frequency / (hours_since_last_use + 1)
func (h *FrecentHistory) Frecency(entry HistoryEntry) float64 {
	hoursSinceLastUse := time.Since(entry.LastUsed).Hours()
	return float64(entry.Frequency) / (hoursSinceLastUse + 1.0)
}

// All returns all entries sorted by frecency descending.
func (h *FrecentHistory) All() []HistoryEntry {
	result := make([]HistoryEntry, len(h.entries))
	copy(result, h.entries)
	h.sortByFrecency(true, result)
	return result
}

// Size returns the number of entries.
func (h *FrecentHistory) Size() int {
	return len(h.entries)
}

// Clear removes all entries.
func (h *FrecentHistory) Clear() {
	h.entries = []HistoryEntry{}
}

// frecency is the unexported helper for computing frecency scores.
func (h *FrecentHistory) frecency(entry HistoryEntry) float64 {
	return h.Frecency(entry)
}

// sortByFrecency sorts entries by frecency score. If descending is true, highest
// scores come first.
func (h *FrecentHistory) sortByFrecency(descending bool, entries []HistoryEntry) {
	sort.SliceStable(entries, func(i, j int) bool {
		if descending {
			return h.frecency(entries[i]) > h.frecency(entries[j])
		}
		return h.frecency(entries[i]) < h.frecency(entries[j])
	})
}

// atomicWrite atomically writes data to path using a temp file + rename pattern.
func (h *FrecentHistory) atomicWrite(path string, data []byte) (err error) {
	dir := filepath.Dir(path)

	// Generate random temp name in the same directory (cross-device safety)
	randBytes := make([]byte, 8)
	if _, err := rand.Read(randBytes); err != nil {
		return err
	}
	tmpPath := filepath.Join(dir, ".m31a_tmp_"+hex.EncodeToString(randBytes))

	// Clean up temp file on any error
	cleanup := true
	defer func() {
		if cleanup {
			os.Remove(tmpPath) // best-effort cleanup
		}
	}()

	tmpFile, err := os.OpenFile(tmpPath, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0644)
	if err != nil {
		return err
	}

	if _, err := tmpFile.Write(data); err != nil {
		tmpFile.Close()
		return err
	}
	if err := tmpFile.Sync(); err != nil {
		tmpFile.Close()
		return err
	}
	if err := tmpFile.Close(); err != nil {
		return err
	}

	// Atomic rename
	if err := os.Rename(tmpPath, path); err != nil {
		return err
	}

	cleanup = false
	return nil
}
