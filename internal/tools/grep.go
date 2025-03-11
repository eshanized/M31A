package tools

import (
	"bufio"
	"context"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"

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

func (t *Grep) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	patternRaw, ok := input.Params["pattern"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: pattern")
	}
	pattern, ok := patternRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter pattern must be a string")
	}

	searchPath := t.workDir
	if pathRaw, ok := input.Params["path"]; ok {
		if pathStr, ok := pathRaw.(string); ok {
			searchPath = pathStr
		}
	}

	var globFilter string
	if globRaw, ok := input.Params["glob"]; ok {
		if globStr, ok := globRaw.(string); ok {
			globFilter = globStr
		}
	}

	maxResults := 100
	if maxRaw, ok := input.Params["max_results"]; ok {
		if maxFloat, ok := maxRaw.(float64); ok {
			maxResults = int(maxFloat)
		}
	}

	if t.hasRg {
		return t.grepWithRG(pattern, searchPath, globFilter, maxResults)
	}
	return t.grepPureGo(pattern, searchPath, globFilter, maxResults)
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

func (t *Grep) grepWithRG(pattern, searchPath, glob string, maxResults int) (types.ToolResult, error) {
	args := []string{"--json", "--no-heading", "--line-number", "--max-count", fmt.Sprintf("%d", maxResults)}
	if glob != "" {
		args = append(args, "--glob", glob)
	}
	args = append(args, pattern)
	args = append(args, searchPath)

	cmd := exec.Command("rg", args...)
	cmd.Dir = t.workDir
	out, err := cmd.Output()
	if err != nil {
		// rg exits with code 1 when no matches found
		if exitErr, ok := err.(*exec.ExitError); ok && exitErr.ExitCode() == 1 {
			return types.ToolResult{Output: "No results found for pattern"}, nil
		}
		return types.ToolResult{}, fmt.Errorf("rg execution failed: %w", err)
	}

	lines := strings.Split(strings.TrimSpace(string(out)), "\n")
	var results []string
	count := 0
	for _, line := range lines {
		if count >= maxResults {
			break
		}
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

	if len(results) == 0 {
		return types.ToolResult{Output: "No results found for pattern"}, nil
	}

	return types.ToolResult{Output: strings.Join(results, "\n")}, nil
}

func (t *Grep) grepPureGo(pattern, searchPath, glob string, maxResults int) (types.ToolResult, error) {
	re, err := regexp.Compile(pattern)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("invalid regex: %w", err)
	}

	gitignorePatterns := loadGitignore(t.workDir)

	var results []string
	err = filepath.Walk(searchPath, func(path string, fi os.FileInfo, err error) error {
		if err != nil {
			return nil // skip inaccessible files
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
		if matchesGitignore(path, gitignorePatterns) {
			return nil
		}

		// Check for binary
		f, err := os.Open(path)
		if err != nil {
			return nil
		}
		defer f.Close()

		header := make([]byte, 512)
		n, _ := f.Read(header)
		if n > 0 {
			for _, b := range header[:n] {
				if b == 0 {
					return nil // skip binary
				}
			}
		}

		// Reset to beginning
		f.Seek(0, 0)

		scanner := bufio.NewScanner(f)
		lineNum := 0
		for scanner.Scan() {
			lineNum++
			if re.MatchString(scanner.Text()) {
				if len(results) >= maxResults {
					return filepath.SkipAll
				}
				relPath, _ := filepath.Rel(t.workDir, path)
				results = append(results, fmt.Sprintf("%s:%d: %s", relPath, lineNum, scanner.Text()))
			}
		}
		return nil
	})
	if err != nil {
		return types.ToolResult{}, err
	}

	if len(results) == 0 {
		return types.ToolResult{Output: "No results found for pattern"}, nil
	}

	return types.ToolResult{Output: strings.Join(results, "\n")}, nil
}

func loadGitignore(dir string) []string {
	data, err := os.ReadFile(filepath.Join(dir, ".gitignore"))
	if err != nil {
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
	return patterns
}

func matchesGitignore(path string, patterns []string) bool {
	for _, p := range patterns {
		match, _ := doublestar.Match(p, path)
		if match {
			return true
		}
	}
	return false
}
