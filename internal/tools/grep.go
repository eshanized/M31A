package tools

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

type Grep struct {
	workDir string
	hasRg   bool
}

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
		return types.ToolResult{}, fmt.Errorf("regex pattern too long (max %d chars)", MaxGrepPatternLength)
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

	var result types.ToolResult
	var err error
	if t.hasRg {
		result, err = t.grepWithRG(ctx, pattern, searchPath, globFilter, maxResults)
	} else {
		result, err = t.grepPureGo(ctx, pattern, searchPath, globFilter, maxResults)
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

func (t *Grep) grepWithRG(ctx context.Context, pattern, searchPath, glob string, maxResults int) (types.ToolResult, error) {
	args := []string{"--json", "--no-heading", "--line-number", "--max-count", fmt.Sprintf("%d", maxResults)}
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
	scanner := bufio.NewScanner(stdout)
	for scanner.Scan() {
		if count >= maxResults {
			if cmd.Process != nil {
				_ = cmd.Process.Kill()
			}
			truncated = true
			break
		}
		line := scanner.Text()
		var match rgMatch
		if err := json.Unmarshal([]byte(line), &match); err != nil {
			continue
		}
		if match.Type != "match" {
			continue
		}
		var data rgDataMatch
		if err := json.Unmarshal(match.Data, &data); err != nil {
			continue
		}
		content := strings.TrimRight(data.Lines.Text, "\n\r")
		results = append(results, fmt.Sprintf("%s:%d: %s", data.Path.Text, data.LineNumber, content))
		count++
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

func (t *Grep) grepPureGo(ctx context.Context, pattern, searchPath, glob string, maxResults int) (types.ToolResult, error) {
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
			match, err := doublestar.Match(glob, relPath)
			if err != nil || !match {
				return nil
			}
		}

		// Check gitignore
		if matchesGitignore(path, gitignorePatterns, t.workDir) {
			return nil
		}

		// Open file once for binary detection and scanning
		f, err := os.Open(path)
		if err != nil {
			return nil
		}
		defer f.Close()

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
		lineNum := 0
		for scanner.Scan() {
			lineNum++
			if re.MatchString(scanner.Text()) {
				if len(results) >= maxResults {
					truncated = true
					return filepath.SkipAll
				}
				relPath, _ := filepath.Rel(t.workDir, path)
				results = append(results, fmt.Sprintf("%s:%d: %s", relPath, lineNum, scanner.Text()))
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
type gitignoreCache struct {
	mu       sync.Mutex
	dir      string
	mtime    time.Time
	patterns []string
}

var globalGitignoreCache gitignoreCache

// loadGitignoreCached returns cached gitignore patterns, re-reading from disk
// only if the .gitignore file has been modified since the last read.
func loadGitignoreCached(dir string) []string {
	path := filepath.Join(dir, ".gitignore")
	info, err := os.Stat(path)
	if err != nil {
		return nil
	}

	globalGitignoreCache.mu.Lock()
	defer globalGitignoreCache.mu.Unlock()

	if globalGitignoreCache.dir == dir && !info.ModTime().After(globalGitignoreCache.mtime) {
		return globalGitignoreCache.patterns
	}

	data, err := os.ReadFile(path)
	if err != nil {
		globalGitignoreCache.dir = dir
		globalGitignoreCache.mtime = info.ModTime()
		globalGitignoreCache.patterns = nil
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

	globalGitignoreCache.dir = dir
	globalGitignoreCache.mtime = info.ModTime()
	globalGitignoreCache.patterns = patterns
	return patterns
}

func loadGitignore(dir string) []string {
	return loadGitignoreCached(dir)
}

func matchesGitignore(path string, patterns []string, workDir string) bool {
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
