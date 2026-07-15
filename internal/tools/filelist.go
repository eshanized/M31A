package tools

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

// Compile-time interface check
var _ types.Tool = (*FileList)(nil)

type FileList struct {
	workDir string
}

// NewFileList creates a new FileList tool instance.
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
			},
			"sort": {
				"type": "string",
				"description": "Sort entries by: 'name' (default), 'size', or 'modified'",
				"enum": ["name", "size", "modified"]
			}
		}
	}`
}

func (t *FileList) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	if err := ctx.Err(); err != nil {
		return types.ToolResult{}, fmt.Errorf("context cancelled: %w", err)
	}

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

	// Security: verify target directory is within workDir
	resolvedTarget, err := ResolveAndContainPathExists(targetDir, t.workDir)
	if err != nil {
		return types.ToolResult{}, err
	}
	targetDir = resolvedTarget

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

	// Parse sort parameter
	sortBy := "name"
	if sortRaw, ok := input.Params["sort"]; ok {
		if sortStr, ok := sortRaw.(string); ok {
			sortBy = sortStr
		}
	}

	var sb strings.Builder
	count := 0
	maxEntries := 500

	var walk func(dir string, prefix string, depth int)
	walk = func(dir string, prefix string, depth int) {
		if depth > maxDepth || count >= maxEntries {
			return
		}
		if ctx.Err() != nil {
			return
		}
		entries, err := os.ReadDir(dir)
		if err != nil {
			return
		}
		// Sort entries based on sort parameter
		sortEntries(entries, sortBy)
		for i, entry := range entries {
			if count >= maxEntries {
				fmt.Fprintf(&sb, "%s... (truncated at %d entries)\n", prefix, maxEntries)
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
	units := "KMGTPE"
	if exp >= len(units) {
		exp = len(units) - 1
		div = int64(unit)
		for i := 1; i <= exp; i++ {
			div *= unit
		}
	}
	return fmt.Sprintf("%.1f %cB", float64(b)/float64(div), units[exp])
}

// sortEntries sorts directory entries by the given criteria.
// Directories always come first, then sorted by the specified field.
func sortEntries(entries []os.DirEntry, sortBy string) {
	type entryInfo struct {
		entry os.DirEntry
		info  os.FileInfo
	}
	infos := make([]entryInfo, len(entries))
	for i, e := range entries {
		info, _ := e.Info()
		infos[i] = entryInfo{entry: e, info: info}
	}
	sort.SliceStable(infos, func(i, j int) bool {
		// Directories always come first
		if infos[i].entry.IsDir() != infos[j].entry.IsDir() {
			return infos[i].entry.IsDir()
		}

		switch sortBy {
		case "size":
			if infos[i].info != nil && infos[j].info != nil {
				return infos[i].info.Size() > infos[j].info.Size() // largest first
			}
			return infos[i].entry.Name() < infos[j].entry.Name()
		case "modified":
			if infos[i].info != nil && infos[j].info != nil {
				return infos[i].info.ModTime().After(infos[j].info.ModTime()) // newest first
			}
			return infos[i].entry.Name() < infos[j].entry.Name()
		default: // "name"
			return infos[i].entry.Name() < infos[j].entry.Name()
		}
	})
	for i, ei := range infos {
		entries[i] = ei.entry
	}
}
