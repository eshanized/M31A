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

// Compile-time interface check
var _ types.Tool = (*FileMove)(nil)

type FileMove struct {
	workDir string
}

func NewFileMove(workDir string) *FileMove {
	return &FileMove{workDir: workDir}
}

func (t *FileMove) Name() string {
	return "FileMove"
}

func (t *FileMove) Description() string {
	return "Move or rename a file. Both source and destination must be within the working directory."
}

func (t *FileMove) RiskLevel() types.RiskLevel {
	return types.RiskDangerous
}

func (t *FileMove) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"source": {
				"type": "string",
				"description": "Source file path"
			},
			"destination": {
				"type": "string",
				"description": "Destination file path"
			}
		},
		"required": ["source", "destination"]
	}`
}

func (t *FileMove) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	if err := ctx.Err(); err != nil {
		return types.ToolResult{}, fmt.Errorf("context cancelled: %w", err)
	}

	srcRaw, ok := input.Params["source"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: source")
	}
	src, ok := srcRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter source must be a string")
	}

	dstRaw, ok := input.Params["destination"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: destination")
	}
	dst, ok := dstRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter destination must be a string")
	}

	srcAbs := src
	if !filepath.IsAbs(src) {
		srcAbs = filepath.Join(t.workDir, src)
	}
	dstAbs := dst
	if !filepath.IsAbs(dst) {
		dstAbs = filepath.Join(t.workDir, dst)
	}

	// Security: resolve symlinks and verify both paths are within workDir
	workDirPrefix := t.workDir
	if !strings.HasSuffix(workDirPrefix, string(filepath.Separator)) {
		workDirPrefix += string(filepath.Separator)
	}

	// Source must exist and resolve within workDir
	srcResolved, err := filepath.EvalSymlinks(srcAbs)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("source file not found: %s", src)
	}
	if err := ContainedInWorkDir(srcResolved, t.workDir); err != nil {
		return types.ToolResult{}, fmt.Errorf("source %w", err)
	}
	srcAbs = srcResolved

	// Destination: resolve parent directory through symlinks if file doesn't exist
	dstResolved := dstAbs
	if _, err := os.Stat(dstAbs); err == nil {
		dstResolved, err = filepath.EvalSymlinks(dstAbs)
		if err != nil {
			return types.ToolResult{}, fmt.Errorf("cannot resolve destination: %w", err)
		}
	} else {
		parentDir := filepath.Dir(dstAbs)
		if resolvedParent, err := filepath.EvalSymlinks(parentDir); err == nil {
			dstResolved = filepath.Join(resolvedParent, filepath.Base(dstAbs))
		}
	}
	if err := ContainedInWorkDir(dstResolved, t.workDir); err != nil {
		return types.ToolResult{}, fmt.Errorf("destination %w", err)
	}
	dstAbs = dstResolved

	// Ensure destination directory exists
	dstDir := filepath.Dir(dstAbs)
	if err := os.MkdirAll(dstDir, types.DirPermission); err != nil {
		return types.ToolResult{}, fmt.Errorf("cannot create destination directory: %w", err)
	}

	if err := os.Rename(srcAbs, dstAbs); err != nil {
		return types.ToolResult{}, fmt.Errorf("cannot move file: %w", err)
	}

	return types.ToolResult{
		Output:     fmt.Sprintf("Moved: %s → %s", src, dst),
		DurationMs: time.Since(start).Milliseconds(),
	}, nil
}
