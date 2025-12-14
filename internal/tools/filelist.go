package tools

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

type FileList struct {
	workDir string
}

func NewFileList(workDir string) *FileList {
	return &FileList{workDir: workDir}
}

func (t *FileList) Name() string {
	return "FileList"
}

func (t *FileList) Description() string {
	return "List directory contents as a tree with sizes. Respects skip directories."
}

func (t *FileList) RiskLevel() types.RiskLevel {
	return types.RiskSafe
}

func (t *FileList) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"path": {
				"type": "string",
				"description": "Directory path to list (defaults to working directory)"
			},
			"depth": {
				"type": "integer",
				"description": "Maximum depth to traverse (default 3, max 6)",
				"minimum": 1,
				"maximum": 6
			}
		}
	}`
}

func (t *FileList) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	targetDir := t.workDir
	if pathRaw, ok := input.Params["path"]; ok {
		if p, ok := pathRaw.(string); ok && p != "" {
			if filepath.IsAbs(p) {
				targetDir = p
			} else {
				targetDir = filepath.Join(t.workDir, p)
			}
		}
	}

	maxDepth := 3
	if depthRaw, ok := input.Params["depth"]; ok {
		if d, ok := depthRaw.(float64); ok && d > 0 {
			maxDepth = int(d)
			if maxDepth > 6 {
				maxDepth = 6
			}
		}
	}

	skipDirs := types.SkipDirsMap()

	var sb strings.Builder
	count := 0
	maxEntries := 500

	var walk func(dir string, prefix string, depth int)
	walk = func(dir string, prefix string, depth int) {
		if depth > maxDepth || count >= maxEntries {
			return
		}
		entries, err := os.ReadDir(dir)
		if err != nil {
			return
		}
		for i, entry := range entries {
			if count >= maxEntries {
				sb.WriteString(prefix + "... (truncated at " + fmt.Sprintf("%d", maxEntries) + " entries)\n")
				return
			}
			name := entry.Name()
			if skipDirs[name] {
				continue
			}
			isLast := i == len(entries)-1
			connector := "├── "
			if isLast {
				connector = "└── "
			}

			info, err := entry.Info()
			if err != nil {
				continue
			}

			sizeStr := ""
			if !entry.IsDir() {
				sizeStr = fmt.Sprintf(" (%s)", humanSize(info.Size()))
			}

			sb.WriteString(prefix + connector + name + sizeStr + "\n")
			count++

			if entry.IsDir() {
				childPrefix := prefix + "│   "
				if isLast {
					childPrefix = prefix + "    "
				}
				walk(filepath.Join(dir, name), childPrefix, depth+1)
			}
		}
	}

	sb.WriteString(filepath.Base(targetDir) + "/\n")
	walk(targetDir, "", 1)

	if count >= maxEntries {
		sb.WriteString(fmt.Sprintf("\n(listing truncated at %d entries)\n", maxEntries))
	}

	return types.ToolResult{
		Output:     sb.String(),
		DurationMs: time.Since(start).Milliseconds(),
	}, nil
}

func humanSize(b int64) string {
	const unit = 1024
	if b < unit {
		return fmt.Sprintf("%d B", b)
	}
	div, exp := int64(unit), 0
	for n := b / unit; n >= unit; n /= unit {
		div *= unit
		exp++
	}
	return fmt.Sprintf("%.1f %cB", float64(b)/float64(div), "KMGTPE"[exp])
}
