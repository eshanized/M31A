package tools

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

type FileWrite struct {
	workDir   string
	backupDir string
}

// MaxBackupsPerFile is the maximum number of backups to keep per file.
// Defined in constants.go as MaxBackupsPerFile.

func NewFileWrite(workDir, backupDir string) *FileWrite {
	return &FileWrite{workDir: workDir, backupDir: backupDir}
}

func (t *FileWrite) Name() string {
	return "FileWrite"
}

func (t *FileWrite) Description() string {
	return "Write content to a file atomically with backup and path safety."
}

func (t *FileWrite) RiskLevel() types.RiskLevel {
	return types.RiskDestructive
}

// ParameterSchema returns the JSON Schema for FileWrite tool parameters.
func (t *FileWrite) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"path": {
				"type": "string",
				"description": "Path to the file to write (relative to working directory)"
			},
			"content": {
				"type": "string",
				"description": "The content to write to the file"
			}
		},
		"required": ["path", "content"]
	}`
}

func (t *FileWrite) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	pathRaw, ok := input.Params["path"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: path")
	}
	path, ok := pathRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter path must be a string")
	}

	contentRaw, ok := input.Params["content"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: content")
	}
	content, ok := contentRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter content must be a string")
	}

	createDirs := true
	if cdRaw, ok := input.Params["create_dirs"]; ok {
		if cdBool, ok := cdRaw.(bool); ok {
			createDirs = cdBool
		}
	}

	// Binary content check
	contentBytes := []byte(content)
	for _, b := range contentBytes {
		if b == 0 {
			return types.ToolResult{}, m31errors.ErrNoBinaryContent
		}
	}

	// Resolve target path relative to workDir
	joined := path
	if !filepath.IsAbs(path) {
		joined = filepath.Join(t.workDir, path)
	}
	targetPath, err := filepath.Abs(joined)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("cannot resolve path: %w", err)
	}

	// Path safety: verify resolved path is within workDir
	// If file exists, resolve its symlinks; otherwise resolve the parent directory
	resolved := targetPath
	if _, err := os.Stat(targetPath); err == nil {
		// File exists — resolve symlinks
		resolved, err = filepath.EvalSymlinks(targetPath)
		if err != nil {
			return types.ToolResult{}, fmt.Errorf("cannot resolve path: %w", err)
		}
	} else if !os.IsNotExist(err) {
		return types.ToolResult{}, fmt.Errorf("cannot stat path: %w", err)
	} else {
		// File doesn't exist — resolve parent directory through symlinks
		parentDir := filepath.Dir(targetPath)
		if resolvedParent, err := filepath.EvalSymlinks(parentDir); err == nil {
			resolved = filepath.Join(resolvedParent, filepath.Base(targetPath))
		}
		// If parent also doesn't exist, we'll create it; use targetPath as-is
	}

	workDirPrefix := t.workDir
	if !strings.HasSuffix(workDirPrefix, string(filepath.Separator)) {
		workDirPrefix += string(filepath.Separator)
	}
	if resolved != t.workDir && !strings.HasPrefix(resolved, workDirPrefix) {
		return types.ToolResult{}, fmt.Errorf("path resolves outside working directory")
	}
	targetPath = resolved

	// Backup existing file
	if _, err := os.Stat(targetPath); err == nil {
		relPath, _ := filepath.Rel(t.workDir, targetPath)
		sanitized := strings.ReplaceAll(relPath, string(filepath.Separator), "_")
		randBytes := make([]byte, 4)
		if _, err := rand.Read(randBytes); err != nil {
			return types.ToolResult{}, fmt.Errorf("cannot generate backup name: %w", err)
		}
		backupName := fmt.Sprintf("%s.%s.%s", sanitized, time.Now().Format("20060102T150405.000"), hex.EncodeToString(randBytes))
		backupPath := filepath.Join(t.backupDir, backupName)

		if err := os.MkdirAll(t.backupDir, DirPermission); err != nil {
			return types.ToolResult{}, fmt.Errorf("cannot create backup directory: %w", err)
		}

		existingContent, err := os.ReadFile(targetPath)
		if err != nil {
			return types.ToolResult{}, fmt.Errorf("cannot read original for backup: %w", err)
		}
		if err := os.WriteFile(backupPath, existingContent, FilePermission); err != nil {
			return types.ToolResult{}, fmt.Errorf("cannot write backup: %w", err)
		}

		// Prune old backups for this file to prevent unbounded accumulation
		t.pruneBackups(sanitized)
	}

	// Create parent directories
	if createDirs {
		if err := os.MkdirAll(filepath.Dir(targetPath), 0755); err != nil {
			return types.ToolResult{}, fmt.Errorf("cannot create directories: %w", err)
		}
	}

	// Temp file creation
	randBytes := make([]byte, 8)
	if _, err := rand.Read(randBytes); err != nil {
		return types.ToolResult{}, fmt.Errorf("cannot generate temp name: %w", err)
	}
	tmpPath := filepath.Join(filepath.Dir(targetPath), ".m31a_tmp_"+hex.EncodeToString(randBytes))

	tmpFile, err := os.Create(tmpPath)
	if err != nil {
		return types.ToolResult{}, m31errors.ErrPermissionDenied
	}

	cleanup := true
	defer func() {
		if cleanup {
			os.Remove(tmpPath)
		}
	}()

	if _, err := tmpFile.Write(contentBytes); err != nil {
		return types.ToolResult{}, fmt.Errorf("write failed: %w", err)
	}
	if err := tmpFile.Sync(); err != nil {
		return types.ToolResult{}, fmt.Errorf("fsync failed: %w", err)
	}
	if err := tmpFile.Close(); err != nil {
		return types.ToolResult{}, fmt.Errorf("close failed: %w", err)
	}

	// Atomic rename
	if err := os.Rename(tmpPath, targetPath); err != nil {
		return types.ToolResult{}, fmt.Errorf("rename failed: %w", err)
	}

	cleanup = false

	elapsed := time.Since(start).Milliseconds()

	return types.ToolResult{
		Output:     fmt.Sprintf("Wrote %d bytes to %s", len(contentBytes), path),
		DurationMs: elapsed,
	}, nil
}

// pruneBackups removes the oldest backups for a given file prefix when the
// count exceeds MaxBackupsPerFile. Backups are sorted lexicographically
// (timestamp in the name ensures chronological order). Logs but does not
// fail on removal errors.
func (t *FileWrite) pruneBackups(sanitizedPrefix string) {
	entries, err := os.ReadDir(t.backupDir)
	if err != nil {
		slog.Warn("cannot read backup directory for pruning", "dir", t.backupDir, "error", err)
		return
	}

	// Filter to backups matching this file's prefix
	var matches []string
	for _, e := range entries {
		if !e.IsDir() && strings.HasPrefix(e.Name(), sanitizedPrefix+".") {
			matches = append(matches, e.Name())
		}
	}

	if len(matches) <= MaxBackupsPerFile {
		return
	}

	// Sort lexicographically — timestamp in the name ensures chronological order
	sort.Strings(matches)

	// Delete oldest entries (lowest sort order) to keep exactly MaxBackupsPerFile
	toDelete := matches[:len(matches)-MaxBackupsPerFile]
	for _, name := range toDelete {
		path := filepath.Join(t.backupDir, name)
		if err := os.Remove(path); err != nil {
			slog.Warn("failed to prune old backup", "path", path, "error", err)
		}
	}
}
