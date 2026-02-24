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
		return types.ToolResult{}, fmt.Errorf("%w: missing parameter: path", m31errors.ErrToolExecution)
	}
	path, ok := pathRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: parameter path must be a string", m31errors.ErrToolExecution)
	}

	contentRaw, ok := input.Params["content"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: missing parameter: content", m31errors.ErrToolExecution)
	}
	content, ok := contentRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: parameter content must be a string", m31errors.ErrToolExecution)
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
		return types.ToolResult{}, fmt.Errorf("%w: cannot resolve path: %v", m31errors.ErrToolExecution, err)
	}

	// Path safety: verify resolved path is within workDir
	// If file exists, resolve its symlinks; otherwise resolve the parent directory
	resolved := targetPath
	if _, err := os.Stat(targetPath); err == nil {
		// File exists — resolve symlinks
		resolved, err = filepath.EvalSymlinks(targetPath)
		if err != nil {
			return types.ToolResult{}, fmt.Errorf("%w: cannot resolve path: %v", m31errors.ErrToolExecution, err)
		}
	} else if !os.IsNotExist(err) {
		return types.ToolResult{}, fmt.Errorf("%w: cannot stat path: %v", m31errors.ErrToolExecution, err)
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
		return types.ToolResult{}, fmt.Errorf("%w: path resolves outside working directory", m31errors.ErrToolExecution)
	}
	targetPath = resolved

	// Backup existing file
	if _, err := os.Stat(targetPath); err == nil {
		relPath, _ := filepath.Rel(t.workDir, targetPath)
		sanitized := strings.ReplaceAll(relPath, string(filepath.Separator), "_")
		randBytes := make([]byte, 4)
		if _, err := rand.Read(randBytes); err != nil {
			return types.ToolResult{}, fmt.Errorf("%w: cannot generate backup name: %v", m31errors.ErrToolExecution, err)
		}
		backupName := fmt.Sprintf("%s.%s.%s", sanitized, time.Now().Format("20060102T150405.000"), hex.EncodeToString(randBytes))
		backupPath := filepath.Join(t.backupDir, backupName)

		if err := os.MkdirAll(t.backupDir, DirPermission); err != nil {
			return types.ToolResult{}, fmt.Errorf("%w: cannot create backup directory: %v", m31errors.ErrToolExecution, err)
		}

		existingContent, err := os.ReadFile(targetPath)
		if err != nil {
			return types.ToolResult{}, fmt.Errorf("%w: cannot read original for backup: %v", m31errors.ErrToolExecution, err)
		}

		// Prune old backups for this file BEFORE writing the new one to prevent
		// the new backup from being accidentally pruned on fast disks.
		t.pruneBackups(sanitized)

		if err := os.WriteFile(backupPath, existingContent, FilePermission); err != nil {
			return types.ToolResult{}, fmt.Errorf("%w: cannot write backup: %v", m31errors.ErrToolExecution, err)
		}
	}

	// Create parent directories
	if createDirs {
		if err := os.MkdirAll(filepath.Dir(targetPath), DirPermission); err != nil {
			return types.ToolResult{}, fmt.Errorf("%w: cannot create directories: %v", m31errors.ErrToolExecution, err)
		}
	}

	// Temp file creation
	randBytes := make([]byte, 8)
	if _, err := rand.Read(randBytes); err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: cannot generate temp name: %v", m31errors.ErrToolExecution, err)
	}
	tmpPath := filepath.Join(filepath.Dir(targetPath), ".m31a_tmp_"+hex.EncodeToString(randBytes))

	tmpFile, err := os.OpenFile(tmpPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, FilePermission)
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
		return types.ToolResult{}, fmt.Errorf("%w: write failed: %v", m31errors.ErrToolExecution, err)
	}
	if err := tmpFile.Sync(); err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: fsync failed: %v", m31errors.ErrToolExecution, err)
	}
	if err := tmpFile.Close(); err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: close failed: %v", m31errors.ErrToolExecution, err)
	}

	// Atomic rename
	if err := os.Rename(tmpPath, targetPath); err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: rename failed: %v", m31errors.ErrToolExecution, err)
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
//
// Called BEFORE writing the new backup so that the just-written backup is
// never accidentally pruned by lexicographic ordering on fast disks.
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

	// Leave room for one new backup: prune when at or above the limit.
	if len(matches) < MaxBackupsPerFile {
		return
	}

	// Sort lexicographically — timestamp in the name ensures chronological order
	sort.Strings(matches)

	// Delete oldest entries (lowest sort order) to keep room for the new backup
	toDelete := matches[:len(matches)-MaxBackupsPerFile+1]
	for _, name := range toDelete {
		path := filepath.Join(t.backupDir, name)
		if err := os.Remove(path); err != nil {
			slog.Warn("failed to prune old backup", "path", path, "error", err)
		}
	}
}
