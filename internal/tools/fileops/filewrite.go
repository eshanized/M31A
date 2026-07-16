package fileops

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// Compile-time interface check
var _ types.Tool = (*FileWrite)(nil)

type FileWrite struct {
	workDir   string
	backupDir string
}

// NewFileWrite creates a new FileWrite tool instance.
func NewFileWrite(workDir, backupDir string) *FileWrite {
	return &FileWrite{workDir: workDir, backupDir: backupDir}
}

func (t *FileWrite) Name() string {
	return "FileWrite"
}

func (t *FileWrite) Description() string {
	return "Write content to a file atomically with backup and path safety. Supports full overwrite (default) or append mode."
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
			},
			"create_dirs": {
				"type": "boolean",
				"description": "Create parent directories if they don't exist (default true)"
			},
			"append": {
				"type": "boolean",
				"description": "Append content to end of file instead of overwriting (default false)"
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

	appendMode := false
	if aRaw, ok := input.Params["append"]; ok {
		if aBool, ok := aRaw.(bool); ok {
			appendMode = aBool
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
	targetPath, err := ResolveAndContainPath(path, t.workDir)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: %w", m31errors.ErrToolExecution, err)
	}

	// Backup existing file (before append or overwrite)
	if _, backupStatErr := os.Stat(targetPath); backupStatErr == nil {
		relPath, _ := filepath.Rel(t.workDir, targetPath)
		sanitized := strings.ReplaceAll(relPath, string(filepath.Separator), "_")
		randBytes := make([]byte, 4)
		if _, backupRandErr := rand.Read(randBytes); backupRandErr != nil {
			return types.ToolResult{}, fmt.Errorf("%w: cannot generate backup name: %w", m31errors.ErrToolExecution, backupRandErr)
		}
		backupName := fmt.Sprintf("%s.%s.%s", sanitized, time.Now().Format("20060102T150405.000"), hex.EncodeToString(randBytes))
		backupPath := filepath.Join(t.backupDir, backupName)

		if backupDirErr := os.MkdirAll(t.backupDir, DirPermission); backupDirErr != nil {
			return types.ToolResult{}, fmt.Errorf("%w: cannot create backup directory: %w", m31errors.ErrToolExecution, backupDirErr)
		}

		existingContent, readErr := os.ReadFile(targetPath)
		if readErr != nil {
			return types.ToolResult{}, fmt.Errorf("%w: cannot read original for backup: %w", m31errors.ErrToolExecution, readErr)
		}

		// Prune old backups for this file BEFORE writing the new one to prevent
		// the new backup from being accidentally pruned on fast disks.
		t.pruneBackups(sanitized)

		if writeErr := os.WriteFile(backupPath, existingContent, FilePermission); writeErr != nil {
			return types.ToolResult{}, fmt.Errorf("%w: cannot write backup: %w", m31errors.ErrToolExecution, writeErr)
		}
	}

	// Create parent directories
	if createDirs {
		if dirErr := os.MkdirAll(filepath.Dir(targetPath), DirPermission); dirErr != nil {
			return types.ToolResult{}, fmt.Errorf("%w: cannot create directories: %w", m31errors.ErrToolExecution, dirErr)
		}
	}

	// For append mode: read existing content and prepend
	if appendMode {
		existingContent, readErr := os.ReadFile(targetPath)
		if readErr != nil && !os.IsNotExist(readErr) {
			return types.ToolResult{}, fmt.Errorf("%w: cannot read file for append: %w", m31errors.ErrToolExecution, readErr)
		}
		if readErr == nil {
			contentBytes = append(existingContent, contentBytes...)
		}
	}

	// Temp file creation
	randBytes := make([]byte, 8)
	if _, randErr := rand.Read(randBytes); randErr != nil {
		return types.ToolResult{}, fmt.Errorf("%w: cannot generate temp name: %w", m31errors.ErrToolExecution, randErr)
	}
	tmpPath := filepath.Join(filepath.Dir(targetPath), ".m31a_tmp_"+hex.EncodeToString(randBytes))

	tmpFile, err := os.OpenFile(tmpPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, FilePermission)
	if err != nil {
		return types.ToolResult{}, m31errors.ErrPermissionDenied
	}

	cleanup := true
	defer func() {
		if cleanup {
			_ = os.Remove(tmpPath)
		}
	}()

	if _, err := tmpFile.Write(contentBytes); err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: write failed: %w", m31errors.ErrToolExecution, err)
	}
	if err := tmpFile.Sync(); err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: fsync failed: %w", m31errors.ErrToolExecution, err)
	}
	if err := tmpFile.Close(); err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: close failed: %w", m31errors.ErrToolExecution, err)
	}

	// Atomic rename
	if err := os.Rename(tmpPath, targetPath); err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: rename failed: %w", m31errors.ErrToolExecution, err)
	}

	cleanup = false

	elapsed := time.Since(start).Milliseconds()

	action := "Wrote"
	if appendMode {
		action = "Appended to"
	}

	return types.ToolResult{
		Output:     fmt.Sprintf("%s %d bytes to %s", action, len(contentBytes), path),
		DurationMs: elapsed,
	}, nil
}

// pruneBackups removes the oldest backups for a given file prefix when the
// count exceeds MaxBackupsPerFile. Called BEFORE writing the new backup so
// that the just-written backup is never accidentally pruned.
func (t *FileWrite) pruneBackups(sanitizedPrefix string) {
	pruneBackupsByPrefix(t.backupDir, sanitizedPrefix, MaxBackupsPerFile)
}