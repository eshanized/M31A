package tools

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

// Compile-time interface check
var _ types.Tool = (*FileDelete)(nil)

type FileDelete struct {
	workDir   string
	backupDir string
}

func NewFileDelete(workDir, backupDir string) *FileDelete {
	return &FileDelete{workDir: workDir, backupDir: backupDir}
}

func (t *FileDelete) Name() string {
	return "FileDelete"
}

func (t *FileDelete) Description() string {
	return "Delete a file with optional backup before removal."
}

func (t *FileDelete) RiskLevel() types.RiskLevel {
	return types.RiskDangerous
}

func (t *FileDelete) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"path": {
				"type": "string",
				"description": "Path to the file to delete"
			},
			"permanent": {
				"type": "boolean",
				"description": "If true, skip backup and delete permanently (default false)"
			}
		},
		"required": ["path"]
	}`
}

func (t *FileDelete) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	pathRaw, ok := input.Params["path"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: path")
	}
	path, ok := pathRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter path must be a string")
	}

	absPath := path
	if !filepath.IsAbs(path) {
		absPath = filepath.Join(t.workDir, path)
	}

	// Resolve symlinks before containment check to prevent symlink bypass
	resolved, err := filepath.EvalSymlinks(absPath)
	if err != nil {
		if os.IsNotExist(err) {
			return types.ToolResult{}, fmt.Errorf("file not found: %s", path)
		}
		return types.ToolResult{}, fmt.Errorf("cannot resolve path: %w", err)
	}

	workDirPrefix := t.workDir
	if !strings.HasSuffix(workDirPrefix, string(filepath.Separator)) {
		workDirPrefix += string(filepath.Separator)
	}
	if resolved != t.workDir && !strings.HasPrefix(resolved, workDirPrefix) {
		return types.ToolResult{}, fmt.Errorf("path resolves outside working directory")
	}
	absPath = resolved

	info, err := os.Stat(absPath)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("file not found: %s", absPath)
	}
	if info.IsDir() {
		return types.ToolResult{}, fmt.Errorf("cannot delete directory, use Bash rm -r instead")
	}

	permanent := false
	if permRaw, ok := input.Params["permanent"]; ok {
		if p, ok := permRaw.(bool); ok {
			permanent = p
		}
	}

	if !permanent {
		// Backup before delete
		backupPrefix := filepath.Base(absPath) + ".deleted"
		t.pruneBackups(backupPrefix)
		backupPath := filepath.Join(t.backupDir, backupPrefix+"."+time.Now().Format("20060102150405"))
		if err := os.MkdirAll(t.backupDir, types.DirPermission); err != nil {
			return types.ToolResult{}, fmt.Errorf("cannot create backup directory: %w", err)
		}
		data, err := os.ReadFile(absPath)
		if err != nil {
			return types.ToolResult{}, fmt.Errorf("cannot read file for backup: %w", err)
		}
		if err := os.WriteFile(backupPath, data, info.Mode()); err != nil {
			return types.ToolResult{}, fmt.Errorf("cannot write backup: %w", err)
		}
	}

	if err := os.Remove(absPath); err != nil {
		return types.ToolResult{}, fmt.Errorf("cannot delete file: %w", err)
	}

	msg := fmt.Sprintf("Deleted: %s", absPath)
	if !permanent {
		msg += " (backup saved)"
	}

	return types.ToolResult{
		Output:     msg,
		DurationMs: time.Since(start).Milliseconds(),
	}, nil
}

// pruneBackups removes the oldest backups matching the given prefix when the
// count exceeds MaxBackupsPerFile.
func (t *FileDelete) pruneBackups(prefix string) {
	entries, err := os.ReadDir(t.backupDir)
	if err != nil {
		return
	}

	var matches []string
	for _, e := range entries {
		if !e.IsDir() && strings.HasPrefix(e.Name(), prefix+".") {
			matches = append(matches, e.Name())
		}
	}

	if len(matches) < MaxBackupsPerFile {
		return
	}

	sort.Strings(matches)

	toDelete := matches[:len(matches)-MaxBackupsPerFile+1]
	for _, name := range toDelete {
		path := filepath.Join(t.backupDir, name)
		if err := os.Remove(path); err != nil {
			slog.Warn("failed to prune old delete backup", "path", path, "error", err)
		}
	}
}
