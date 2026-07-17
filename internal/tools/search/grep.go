package search

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"sync"
	"time"

	"github.com/bmatcuk/doublestar/v4"
	"github.com/eshanized/M31A/internal/types"
)

// Compile-time interface check
var _ types.Tool = (*Grep)(nil)

// Constants from original constants.go
const (
	MaxGrepPatternLength  = 1024
	DefaultMaxGrepResults = types.DefaultMaxGrepResults
)

type Grep struct {
	workDir string
	hasRg   bool
}

// NewGrep creates a new Grep tool instance.
func NewGrep(workDir string) *Grep {
	_, err := exec.LookPath("rg")
	return &Grep{
		workDir: workDir,
		hasRg:   err == nil,
	}
}

func (t *Grep) Name() string {
	return "Grep"
}

func (t *Grep) Description() string {
	return "Search file contents using regex patterns. Uses ripgrep when available, falls back to pure-Go."
}

func (t *Grep) RiskLevel() types.RiskLevel {
	return types.RiskSafe
}

// ParameterSchema returns the JSON Schema for Grep tool parameters.
func (t *Grep) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"pattern": {
				"type": "string",
				"description": "Regex pattern to search for in file contents"
			},
			"path": {
				"type": "string",
				"description": "File or directory to search (defaults to working directory)"
			},
			"include": {
				"type": "string",
				"description": "Glob pattern to filter files (e.g. '*.go', '**/*.ts')"
			},
			"max_results": {
				"type": "integer",
				"description": "Maximum number of matches to return (default 100, min 1)"
			},
			"context_lines": {
				"type": "integer",
				"description": "Number of context lines to show before and after each match (default 0)",
				"minimum": 0,
				"maximum": 10
			},
			"fixed_string": {
				"type": "boolean",
				"description": "Treat pattern as a literal string, not a regex (default false)"
			},
			"skip_comments": {
				"type": "boolean",
				"description": "Skip lines that are comments (//, #, /*, *, --, ;) (default false)"
			}
		},
		"required": ["pattern"]
	}`
}

func (t *Grep) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	if err := ctx.Err(); err != nil {
		return types.ToolResult{}, err
	}

	patternRaw, ok := input.Params["pattern"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: pattern")
	}
	pattern, ok := patternRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter pattern must be a string")
	}
	if len(pattern) > MaxGrepPatternLength {
		return types.ToolResult{}, types.NewToolError(
			fmt.Errorf("regex pattern too long (max %d chars, got %d)", MaxGrepPatternLength, len(pattern)),
			"Simplify the pattern or use fixed_string=true for literal string searches.",
		)
	}

	// Fixed-string mode: escape regex special characters
	fixedString := false
	if fsRaw, ok := input.Params["fixed_string"]; ok {
		if fsBool, ok := fsRaw.(bool); ok {
			fixedString = fsBool
		}
	}
	if fixedString {
		pattern = regexp.QuoteMeta(pattern)
	}

	searchPath := t.workDir
	if pathRaw, ok := input.Params["path"]; ok {
		if pathStr, ok := pathRaw.(string); ok {
			joined := pathStr
			if !filepath.IsAbs(pathStr) {
				joined = filepath.Join(t.workDir, pathStr)
			}
			resolved, err := filepath.EvalSymlinks(joined)
			if err != nil {
				return types.ToolResult{}, fmt.Errorf("cannot resolve path: %w", err)
			}
			// Containment check: ensure resolved path is within workDir
			absWork, err := filepath.Abs(t.workDir)
			if err != nil {
				return types.ToolResult{}, fmt.Errorf("cannot resolve workdir: %w", err)
			}
			absResolved, err := filepath.Abs(resolved)
			if err != nil {
				return types.ToolResult{}, fmt.Errorf("cannot resolve path: %w", err)
			}
			if !strings.HasPrefix(absResolved+string(os.PathSeparator), absWork+string(os.PathSeparator)) && absResolved != absWork {
				return types.ToolResult{}, fmt.Errorf("path escapes work directory")
			}
			searchPath = resolved
		}
	}

	var globFilter string
	if globRaw, ok := input.Params["include"]; ok {
		if globStr, ok := globRaw.(string); ok {
			globFilter = globStr
		}
	}

	maxResults := DefaultMaxGrepResults
	if maxRaw, ok := input.Params["max_results"]; ok {
		if maxFloat, ok := maxRaw.(float64); ok {
			maxResults = int(maxFloat)
		}
	}

	contextLines := 0
	if ctxRaw, ok := input.Params["context_lines"]; ok {
		if ctxFloat, ok := ctxRaw.(float64); ok {
			contextLines = int(ctxFloat)
			if contextLines > 10 {
				contextLines = 10
			}
		}
	}

	skipComments := false
	if scRaw, ok := input.Params["skip_comments"]; ok {
		if scBool, ok := scRaw.(bool); ok {
			skipComments = scBool
		}
	}

	var result types.ToolResult
	var err error
	if t.hasRg {
		result, err = t.grepWithRG(ctx, pattern, searchPath, globFilter, maxResults, contextLines, skipComments)
	} else {
		result, err = t.grepPureGo(ctx, pattern, searchPath, globFilter, maxResults, contextLines, skipComments)
	}

	result.DurationMs = time.Since(start).Milliseconds()
	return result, err
}

type rgMatch struct {
	Type string          `json:"type"`
	Data json.RawMessage `json:"data"`
}

type rgDataMatch struct {
	Path struct {
		Text string `json:"text"`
	} `json:"path"`
	Lines struct {
		Text string `json:"text"`
	} `json:"lines"`
	LineNumber int `json:"line_number"`
}

func (t *Grep) grepWithRG(ctx context.Context, pattern, searchPath, glob string, maxResults, contextLines int, skipComments bool) (types.ToolResult, error) {
	args := []string{"--json", "--no-heading", "--line-number", "--max-count", fmt.Sprintf("%d", maxResults)}
	if contextLines > 0 {
		args = append(args, "--context", fmt.Sprintf("%d", contextLines))
	}
	if glob != "" {
		args = append(args, "--glob", glob)
	}
	args = append(args, pattern)
	args = append(args, searchPath)

	cmd := exec.CommandContext(ctx, "rg", args...)
	cmd.Dir = t.workDir
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("rg stdout pipe failed: %w", err)
	}
	var stderr bytes.Buffer
	cmd.Stderr = &stderr
	if err := cmd.Start(); err != nil {
		return types.ToolResult{}, fmt.Errorf("rg start failed: %w", err)
	}

	var results []string
	count := 0
	truncated := false
	seenMatch := make(map[string]bool) // track match lines to avoid duplicates from context
	scanner := bufio.NewScanner(stdout)
	for scanner.Scan() {
		line := scanner.Text()
		var match rgMatch
		if err := json.Unmarshal([]byte(line), &match); err != nil {
			continue
		}
		if match.Type == "match" {
			if count >= maxResults {
				if cmd.Process != nil {
					_ = cmd.Process.Kill()
				}
				truncated = true
				break
			}
			var data rgDataMatch
			if err := json.Unmarshal(match.Data, &data); err != nil {
				continue
			}
			content := strings.TrimRight(data.Lines.Text, "\n\r")
			if skipComments && isCommentLine(content) {
				continue
			}
			results = append(results, fmt.Sprintf("%s:%d: %s", data.Path.Text, data.LineNumber, content))
			count++
			seenMatch[fmt.Sprintf("%s:%d", data.Path.Text, data.LineNumber)] = true
		} else if match.Type == "context" && contextLines > 0 {
			// Context lines are useful for understanding surrounding code
			var data rgDataMatch
			if err := json.Unmarshal(match.Data, &data); err != nil {
				continue
			}
			// Add context lines with a separator marker
			content := strings.TrimRight(data.Lines.Text, "\n\r")
			results = append(results, fmt.Sprintf("%s:%d-%s", data.Path.Text, data.LineNumber, content))
		}
	}

	if err := cmd.Wait(); err != nil {
		// rg exits with code 1 when no matches found
		if exitErr, ok := err.(*exec.ExitError); ok {
			if exitErr.ExitCode() == 1 {
				if len(results) == 0 {
					return types.ToolResult{Output: "No results found for pattern"}, nil
				}
			}
			// rg may be killed when we hit maxResults — that's expected
			if exitErr.ExitCode() == -1 || strings.Contains(err.Error(), "killed") {
				// Process was killed, results are valid
			} else {
				stderrStr := strings.TrimSpace(stderr.String())
				if stderrStr != "" {
					return types.ToolResult{}, fmt.Errorf("rg execution failed: %w\nstderr: %s", err, stderrStr)
				}
				return types.ToolResult{}, fmt.Errorf("rg execution failed: %w", err)
			}
		} else {
			stderrStr := strings.TrimSpace(stderr.String())
			if stderrStr != "" {
				return types.ToolResult{}, fmt.Errorf("rg execution failed: %w\nstderr: %s", err, stderrStr)
			}
			return types.ToolResult{}, fmt.Errorf("rg execution failed: %w", err)
		}
	}

	if len(results) == 0 {
		return types.ToolResult{Output: "No results found for pattern"}, nil
	}

	output := strings.Join(results, "\n")
	if truncated {
		output += fmt.Sprintf("\n[... more matches (limit: %d)]", maxResults)
	}

	return types.ToolResult{Output: output, Truncated: truncated}, nil
}

func (t *Grep) grepPureGo(ctx context.Context, pattern, searchPath, glob string, maxResults, contextLines int, skipComments bool) (types.ToolResult, error) {
	if err := checkRedos(pattern); err != nil {
		return types.ToolResult{}, err
	}
	re, err := regexp.Compile(pattern)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("invalid regex: %w", err)
	}

	gitignorePatterns := loadGitignore(t.workDir)

	var results []string
	truncated := false
	err = filepath.Walk(searchPath, func(path string, fi os.FileInfo, err error) error {
		if err != nil {
			return nil // skip inaccessible files
		}
		// Check context cancellation (H2 fix)
		if ctx.Err() != nil {
			return ctx.Err()
		}
		if fi.IsDir() {
			// Skip hidden directories and common non-code dirs
			if strings.HasPrefix(fi.Name(), ".") && fi.Name() != "." {
				return filepath.SkipDir
			}
			return nil
		}

		// Check glob filter
		if glob != "" {
			relPath, _ := filepath.Rel(t.workDir, path)
			match, matchErr := doublestar.Match(glob, relPath)
			if matchErr != nil || !match {
				return nil
			}
		}

		// Check gitignore
		if MatchesGitignore(path, gitignorePatterns, t.workDir) {
			return nil
		}

		// Open file once for binary detection and scanning
		f, err := os.Open(path)
		if err != nil {
			return nil
		}
		defer f.Close() //nolint:errcheck

		// Read first 512 bytes for binary detection
		header := make([]byte, 512)
		n, _ := f.Read(header)
		isBinary := false
		if n > 0 {
			for _, b := range header[:n] {
				if b == 0 {
					isBinary = true
					break
				}
			}
		}
		if isBinary {
			return nil
		}

		// Reset reader to beginning for scanning
		if _, err := f.Seek(0, 0); err != nil {
			slog.Debug("grep: seek failed, skipping file", "path", path, "error", err)
			return nil
		}

		scanner := bufio.NewScanner(f)

		if contextLines == 0 {
			// Stream mode: O(1) memory when no context needed
			lineNum := 0
			for scanner.Scan() {
				lineNum++
				line := scanner.Text()
				if skipComments && isCommentLine(line) {
					continue
				}
				if re.MatchString(line) {
					if len(results) >= maxResults {
						truncated = true
						return filepath.SkipAll
					}
					relPath, _ := filepath.Rel(t.workDir, path)
					results = append(results, fmt.Sprintf("%s:%d: %s", relPath, lineNum, line))
				}
			}
		} else {
			// Ring buffer mode: O(contextLines) memory instead of O(fileSize)
			windowSize := 2*contextLines + 1
			ring := make([]string, windowSize)
			ringLine := make([]int, windowSize)
			rIdx := 0
			rCount := 0
			lineNum := 0
			lastMatchEnd := -1

			for scanner.Scan() {
				lineNum++
				line := scanner.Text()
				ring[rIdx] = line
				ringLine[rIdx] = lineNum
				rIdx = (rIdx + 1) % windowSize
				if rCount < windowSize {
					rCount++
				}

				if skipComments && isCommentLine(line) {
					continue
				}
				if re.MatchString(line) {
					if len(results) >= maxResults {
						truncated = true
						return filepath.SkipAll
					}
					relPath, _ := filepath.Rel(t.workDir, path)

					ctxStart := lineNum - contextLines
					if ctxStart < 1 {
						ctxStart = 1
					}
					if lastMatchEnd >= 0 && ctxStart <= lastMatchEnd {
						ctxStart = lastMatchEnd + 1
					}

					for ln := ctxStart; ln < lineNum; ln++ {
						if len(results) >= maxResults {
							truncated = true
							break
						}
						bufIdx := (rIdx - 1 - (lineNum - ln) + windowSize) % windowSize
						if ringLine[bufIdx] == ln {
							results = append(results, fmt.Sprintf("%s:%d %s", relPath, ln, ring[bufIdx]))
						}
					}

					if len(results) < maxResults {
						results = append(results, fmt.Sprintf("%s:%d: %s", relPath, lineNum, line))
					}
					lastMatchEnd = lineNum + contextLines
				}
			}
		}
		if err := scanner.Err(); err != nil {
			slog.Debug("grep: scanner error", "path", path, "error", err)
		}
		return nil
	})
	if err != nil {
		return types.ToolResult{}, err
	}

	if len(results) == 0 {
		return types.ToolResult{Output: "No results found for pattern"}, nil
	}

	output := strings.Join(results, "\n")
	if truncated {
		output += fmt.Sprintf("\n[... more matches (limit: %d)]", maxResults)
	}

	return types.ToolResult{Output: output, Truncated: truncated}, nil
}

// gitignoreCache caches parsed gitignore patterns with mtime-based invalidation.
// Uses a bounded LRU cache with maximum 1024 directories to prevent unbounded memory growth.
var gitignoreCacheInstance = newGitignoreCache()

const maxGitignoreCacheSize = 1024

type gitignoreCache struct {
	mu    sync.RWMutex
	cache map[string]*gitignoreCacheEntry
	order []string // LRU order: oldest first
}

type gitignoreCacheEntry struct {
	mu       sync.Mutex
	mtime    time.Time
	patterns []string
}

func newGitignoreCache() *gitignoreCache {
	return &gitignoreCache{
		cache: make(map[string]*gitignoreCacheEntry),
		order: make([]string, 0),
	}
}

func (c *gitignoreCache) getOrCreate(dir string) *gitignoreCacheEntry {
	c.mu.Lock()
	defer c.mu.Unlock()

	if entry, ok := c.cache[dir]; ok {
		// Move to end of LRU order (most recently used)
		for i, key := range c.order {
			if key == dir {
				c.order = append(c.order[:i], c.order[i+1:]...)
				break
			}
		}
		c.order = append(c.order, dir)
		return entry
	}

	// Evict oldest if at capacity
	if len(c.cache) >= maxGitignoreCacheSize {
		oldest := c.order[0]
		c.order = c.order[1:]
		delete(c.cache, oldest)
	}

	entry := &gitignoreCacheEntry{}
	c.cache[dir] = entry
	c.order = append(c.order, dir)
	return entry
}

// LoadGitignoreCached returns cached gitignore patterns, re-reading from disk
// only if the .gitignore file has been modified since the last read.
func LoadGitignoreCached(dir string) []string {
	path := filepath.Join(dir, ".gitignore")
	info, err := os.Stat(path)
	if err != nil {
		return nil
	}

	// Get or create per-directory cache entry via bounded LRU cache
	entry := gitignoreCacheInstance.getOrCreate(dir)
	entry.mu.Lock()
	defer entry.mu.Unlock()

	if !info.ModTime().After(entry.mtime) {
		return entry.patterns
	}

	data, err := os.ReadFile(path)
	if err != nil {
		entry.mtime = info.ModTime()
		entry.patterns = nil
		return nil
	}

	var patterns []string
	for _, line := range strings.Split(string(data), "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		patterns = append(patterns, line)
	}

	entry.mtime = info.ModTime()
	entry.patterns = patterns
	return patterns
}

func loadGitignore(dir string) []string {
	return LoadGitignoreCached(dir)
}

func MatchesGitignore(path string, patterns []string, workDir string) bool {
	// Convert to relative path for matching
	relPath, err := filepath.Rel(workDir, path)
	if err != nil {
		relPath = path
	}
	for _, p := range patterns {
		match, _ := doublestar.Match(p, relPath)
		if match {
			return true
		}
		// Also match against just the filename for simple patterns
		if !strings.Contains(p, "/") {
			match, _ = doublestar.Match(p, filepath.Base(relPath))
			if match {
				return true
			}
		}
	}
	return false
}

// redosDetector matches patterns containing nested quantifiers or adjacent
// quantifiers that are likely to cause catastrophic backtracking.
var redosDetector = regexp.MustCompile(`\([^)]*[+*][^)]*\)[+*{]`)

// adjacentQuantifierDetector matches adjacent quantifier characters (e.g., a++,
// a*+, a+*) which can cause catastrophic backtracking.
var adjacentQuantifierDetector = regexp.MustCompile(`[+*][+*]`)

// isCommentLine returns true if the line is a comment in common languages.
// Supports: //, #, /*, *, --, ; at the start of a line (after optional whitespace).
func isCommentLine(line string) bool {
	trimmed := strings.TrimSpace(line)
	if trimmed == "" {
		return false
	}
	// Single-line comments
	if strings.HasPrefix(trimmed, "//") || strings.HasPrefix(trimmed, "#") ||
		strings.HasPrefix(trimmed, "--") || strings.HasPrefix(trimmed, ";") {
		return true
	}
	// Block comment lines: /* ... */ or * ... (continuation)
	if strings.HasPrefix(trimmed, "/*") || strings.HasPrefix(trimmed, "*") {
		return true
	}
	return false
}

// checkRedos rejects regex patterns that contain nested quantifiers likely
// to cause catastrophic backtracking. This protects the pure-Go grep from
// hanging on malicious or poorly-crafted patterns.
func checkRedos(pattern string) error {
	if redosDetector.MatchString(pattern) {
		return fmt.Errorf("regex rejected: pattern contains nested quantifiers that may cause catastrophic backtracking")
	}
	if adjacentQuantifierDetector.MatchString(pattern) {
		return fmt.Errorf("regex rejected: pattern contains adjacent quantifiers that may cause catastrophic backtracking")
	}
	return nil
}