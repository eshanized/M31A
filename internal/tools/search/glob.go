package search

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/bmatcuk/doublestar/v4"
	"github.com/eshanized/M31A/internal/core/types"
)

// Compile-time interface check
var _ types.Tool = (*Glob)(nil)

const (
	MaxGlobResults = 1000
)

type Glob struct {
	workDir string
}

// NewGlob creates a new Glob tool instance.
func NewGlob(workDir string) *Glob {
	return &Glob{workDir: workDir}
}

func (t *Glob) Name() string {
	return "Glob"
}

func (t *Glob) Description() string {
	return "List files matching a glob pattern. Supports ** for recursive matching."
}

func (t *Glob) RiskLevel() types.RiskLevel {
	return types.RiskSafe
}

// ParameterSchema returns the JSON Schema for Glob tool parameters.
func (t *Glob) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"pattern": {
				"type": "string",
				"description": "Glob pattern to match files (supports ** for recursive)"
			},
			"path": {
				"type": "string",
				"description": "Directory to search in (defaults to working directory)"
			},
			"type": {
				"type": "string",
				"description": "Filter by type: 'file' (regular files only), 'dir' (directories only), or 'all' (default)",
				"enum": ["file", "dir", "all"]
			}
		},
		"required": ["pattern"]
	}`
}

func (t *Glob) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
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

	// Parse type filter
	typeFilter := "all"
	if typeRaw, ok := input.Params["type"]; ok {
		if typeStr, ok := typeRaw.(string); ok {
			typeFilter = typeStr
		}
	}

	// Check if .gitignore exists and rg is available for gitignore-aware listing
	_, rgErr := exec.LookPath("rg")
	hasRG := rgErr == nil
	gitignorePath := filepath.Join(t.workDir, ".gitignore")
	useRG := hasRG
	if _, err := os.Stat(gitignorePath); os.IsNotExist(err) {
		useRG = false
	}

	var matches []string
	var err error
	if useRG {
		matches, err = t.globWithRG(ctx, pattern)
		if err != nil {
			// rg failed at runtime — fall back to pure-Go doublestar
			matches, err = t.globWithDoublestar(pattern)
			if err != nil {
				return types.ToolResult{}, fmt.Errorf("invalid glob pattern: %w", err)
			}
		}
	} else {
		matches, err = t.globWithDoublestar(pattern)
		if err != nil {
			return types.ToolResult{}, fmt.Errorf("invalid glob pattern: %w", err)
		}
	}

	// Apply type filter and cache FileInfo for display formatting
	infoCache := make(map[string]os.FileInfo, len(matches))
	if typeFilter != "all" {
		var filtered []string
		for _, m := range matches {
			fullPath := m
			if !filepath.IsAbs(m) {
				fullPath = filepath.Join(t.workDir, m)
			}
			fi, err := os.Stat(fullPath)
			if err != nil {
				continue
			}
			infoCache[m] = fi
			switch typeFilter {
			case "file":
				if !fi.IsDir() {
					filtered = append(filtered, m)
				}
			case "dir":
				if fi.IsDir() {
					filtered = append(filtered, m)
				}
			}
		}
		matches = filtered
	}

	maxResults := MaxGlobResults
	truncated := false
	origCount := len(matches)
	if len(matches) > maxResults {
		matches = matches[:maxResults]
		truncated = true
	}

	sort.Strings(matches)

	if len(matches) == 0 {
		return types.ToolResult{Output: "No files matched pattern"}, nil
	}

	var b strings.Builder
	b.Grow(64 * len(matches)) // pre-allocate for ~64 bytes per line
	fmt.Fprintf(&b, "%-50s %10s %s\n", "path", "size", "modified")
	for _, m := range matches {
		fi, ok := infoCache[m]
		if !ok {
			fullPath := m
			if !filepath.IsAbs(m) {
				fullPath = filepath.Join(t.workDir, m)
			}
			var err error
			fi, err = os.Stat(fullPath)
			if err != nil {
				continue
			}
		}
		fmt.Fprintf(&b, "%-50s %10d %s\n", m, fi.Size(), fi.ModTime().Format(types.DateTimeFormat))
	}
	if truncated {
		fmt.Fprintf(&b, "[... %d more files]", origCount-maxResults)
	}

	return types.ToolResult{
		Output:     b.String(),
		DurationMs: time.Since(start).Milliseconds(),
		Truncated:  truncated,
	}, nil
}

func (t *Glob) globWithDoublestar(pattern string) ([]string, error) {
	matches, err := doublestar.Glob(os.DirFS(t.workDir), pattern)
	if err != nil {
		return nil, err
	}
	// Return paths relative to workDir for consistency
	return matches, nil
}

func (t *Glob) globWithRG(ctx context.Context, pattern string) ([]string, error) {
	cmd := exec.CommandContext(ctx, "rg", "--files", "--sort", "path", "--glob", pattern)
	cmd.Dir = t.workDir
	out, err := cmd.Output()
	if err != nil {
		return nil, err
	}
	lines := strings.Split(strings.TrimSpace(string(out)), "\n")
	if len(lines) == 0 || (len(lines) == 1 && lines[0] == "") {
		return nil, nil
	}
	// rg returns paths relative to working directory already
	return lines, nil
}
