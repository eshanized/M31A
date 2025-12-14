package tools

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

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
	return types.RiskSafe
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

	// Security: both paths must be within workDir
	srcRel, err := filepath.Rel(t.workDir, srcAbs)
	if err != nil || len(srcRel) > 1 && srcRel[:2] == ".." {
		return types.ToolResult{}, fmt.Errorf("source path is outside working directory")
	}
	dstRel, err := filepath.Rel(t.workDir, dstAbs)
	if err != nil || len(dstRel) > 1 && dstRel[:2] == ".." {
		return types.ToolResult{}, fmt.Errorf("destination path is outside working directory")
	}

	if _, err := os.Stat(srcAbs); err != nil {
		return types.ToolResult{}, fmt.Errorf("source file not found: %s", srcAbs)
	}

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
