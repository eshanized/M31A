package tui

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

// MentionEntry represents a file or directory that can be @-mentioned in the chat.
type MentionEntry struct {
	Path        string // relative path from cwd
	IsDir       bool
	DisplayName string // last path component (filename or dir name)
	Size        int64  // file size in bytes (0 for directories)
	LineCount   int    // approximate line count (0 for directories)
}

// MentionContext holds a resolved @-mention path and the file's content.
type MentionContext struct {
	Path    string
	Content string
}

// MentionCompleter scans the working directory and provides fuzzy-filtered completions
// for @-mention autocomplete in the REPL textarea.
type MentionCompleter struct {
	cwd      string
	entries  []MentionEntry
	scanned  bool
	lastScan time.Time
}

// mentionCacheTTL is how long the completer cache is considered fresh.
const mentionCacheTTL = 30 * time.Second

// NewMentionCompleter creates a new completer rooted at the given directory.
func NewMentionCompleter(cwd string) *MentionCompleter {
	return &MentionCompleter{cwd: cwd}
}

// mentionSkipDirs are directories excluded from @-mention scanning.
var mentionSkipDirs = map[string]bool{
	".git": true, ".svn": true, ".hg": true,
	"node_modules": true, "vendor": true,
	"__pycache__": true, ".cache": true, ".pytest_cache": true,
	"dist": true, "build": true, "out": true, "target": true,
	".idea": true, ".vscode": true, ".next": true, ".nuxt": true,
}

const maxMentionEntries = 500

// Scan (re)scans the working directory, populating entries up to maxMentionEntries.
func (c *MentionCompleter) Scan() {
	c.entries = c.entries[:0]
	if c.cwd == "" {
		c.scanned = true
		return
	}
	_ = filepath.Walk(c.cwd, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return nil
		}
		if len(c.entries) >= maxMentionEntries {
			return filepath.SkipAll
		}
		rel, relErr := filepath.Rel(c.cwd, path)
		if relErr != nil || rel == "." {
			return nil
		}
		base := filepath.Base(rel)
		// Skip hidden entries
		if strings.HasPrefix(base, ".") {
			if info.IsDir() {
				return filepath.SkipDir
			}
			return nil
		}
		// Skip known noisy directories
		if info.IsDir() && mentionSkipDirs[base] {
			return filepath.SkipDir
		}
		entry := MentionEntry{
			Path:        rel,
			IsDir:       info.IsDir(),
			DisplayName: base,
			Size:        info.Size(),
		}
		if !info.IsDir() && info.Size() < 100_000 {
			if data, err := os.ReadFile(path); err == nil {
				entry.LineCount = strings.Count(string(data), "\n") + 1
			}
		}
		c.entries = append(c.entries, entry)
		return nil
	})
	c.scanned = true
	c.lastScan = time.Now()
}

// Filter returns up to 8 entries whose path/name matches query (fuzzy prefix + contains).
// If query is empty, returns the first 8 top-level entries.
func (c *MentionCompleter) Filter(query string) []MentionEntry {
	if !c.scanned || time.Since(c.lastScan) > mentionCacheTTL {
		c.Scan()
	}
	if query == "" {
		var top []MentionEntry
		for _, e := range c.entries {
			// Top-level entries have no separator in their path
			if !strings.Contains(e.Path, string(filepath.Separator)) {
				top = append(top, e)
			}
			if len(top) >= 8 {
				break
			}
		}
		return top
	}

	q := strings.ToLower(query)
	type scored struct {
		entry MentionEntry
		score int
	}
	var matches []scored
	for _, e := range c.entries {
		pLow := strings.ToLower(e.Path)
		bLow := strings.ToLower(e.DisplayName)
		switch {
		case strings.HasPrefix(bLow, q):
			matches = append(matches, scored{e, 3})
		case strings.HasPrefix(pLow, q):
			matches = append(matches, scored{e, 2})
		case strings.Contains(pLow, q):
			matches = append(matches, scored{e, 1})
		}
	}
	sort.SliceStable(matches, func(i, j int) bool {
		return matches[i].score > matches[j].score
	})
	result := make([]MentionEntry, 0, 8)
	for _, m := range matches {
		result = append(result, m.entry)
		if len(result) >= 8 {
			break
		}
	}
	return result
}

// ResolveMentions scans message for @path tokens and reads their file contents.
// Returns one MentionContext per successfully read file (directories are skipped).
// File contents are capped at 8 000 bytes to avoid flooding the context window.
func ResolveMentions(cwd, message string) []MentionContext {
	const maxBytes = 8000
	var out []MentionContext
	seen := make(map[string]bool)

	for _, word := range strings.Fields(message) {
		if !strings.HasPrefix(word, "@") {
			continue
		}
		rel := strings.TrimPrefix(word, "@")
		rel = strings.TrimRight(rel, ".,;:!?\"'`")
		if rel == "" || seen[rel] {
			continue
		}
		seen[rel] = true

		abs := rel
		if cwd != "" && !filepath.IsAbs(rel) {
			abs = filepath.Join(cwd, rel)
		}
		info, err := os.Stat(abs)
		if err != nil || info.IsDir() {
			continue
		}
		data, err := os.ReadFile(abs)
		if err != nil {
			continue
		}
		content := string(data)
		if len(content) > maxBytes {
			content = content[:maxBytes] + "\n... (truncated — file too large)"
		}
		out = append(out, MentionContext{Path: rel, Content: content})
	}
	return out
}

// detectProjectLanguage inspects top-level files in cwd and returns the dominant
// programming language, or an empty string if none is detected.
func detectProjectLanguage(cwd string) string {
	if cwd == "" {
		return ""
	}
	entries, err := os.ReadDir(cwd)
	if err != nil {
		return ""
	}
	counts := make(map[string]int)
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		ext := strings.ToLower(filepath.Ext(e.Name()))
		switch ext {
		case ".go":
			counts["Go"]++
		case ".ts", ".tsx":
			counts["TypeScript"]++
		case ".js", ".jsx", ".mjs":
			counts["JavaScript"]++
		case ".py":
			counts["Python"]++
		case ".rs":
			counts["Rust"]++
		case ".java":
			counts["Java"]++
		case ".kt", ".kts":
			counts["Kotlin"]++
		case ".rb":
			counts["Ruby"]++
		case ".c", ".h":
			counts["C"]++
		case ".cpp", ".hpp", ".cc", ".cxx":
			counts["C++"]++
		case ".cs":
			counts["C#"]++
		case ".swift":
			counts["Swift"]++
		case ".zig":
			counts["Zig"]++
		}
	}
	best, bestCount := "", 0
	for lang, count := range counts {
		if count > bestCount {
			best, bestCount = lang, count
		}
	}
	return best
}
