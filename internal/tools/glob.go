package tools

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"

	"github.com/bmatcuk/doublestar/v4"
	"github.com/eshanized/M31A/internal/types"
)

type Glob struct {
	workDir string
}

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

func (t *Glob) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	patternRaw, ok := input.Params["pattern"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: pattern")
	}
	pattern, ok := patternRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter pattern must be a string")
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
		matches, err = t.globWithRG(pattern)
		if err != nil {
			return types.ToolResult{}, fmt.Errorf("invalid glob pattern: %w", err)
		}
	} else {
		matches, err = t.globWithDoublestar(pattern)
		if err != nil {
			return types.ToolResult{}, fmt.Errorf("invalid glob pattern: %w", err)
		}
	}

	const maxResults = 1000
	truncated := false
	if len(matches) > maxResults {
		matches = matches[:maxResults]
		truncated = true
	}

	sort.Strings(matches)

	if len(matches) == 0 {
		return types.ToolResult{Output: "No files matched pattern"}, nil
	}

	var b strings.Builder
	fmt.Fprintf(&b, "%-50s %10s %s\n", "path", "size", "modified")
	for _, m := range matches {
		fullPath := filepath.Join(t.workDir, m)
		fi, err := os.Stat(fullPath)
		if err != nil {
			continue
		}
		fmt.Fprintf(&b, "%-50s %10d %s\n", m, fi.Size(), fi.ModTime().Format("2006-01-02 15:04"))
	}
	if truncated {
		fmt.Fprintf(&b, "[... %d more files]", len(matches)-maxResults)
	}

	return types.ToolResult{Output: b.String()}, nil
}

func (t *Glob) globWithDoublestar(pattern string) ([]string, error) {
	matches, err := doublestar.Glob(os.DirFS(t.workDir), pattern)
	if err != nil {
		return nil, err
	}
	// Return paths relative to workDir for consistency
	return matches, nil
}

func (t *Glob) globWithRG(pattern string) ([]string, error) {
	cmd := exec.Command("rg", "--files", "--glob", pattern)
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
