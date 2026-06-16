package tools

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// Compile-time interface check
var _ types.Tool = (*FileRead)(nil)

type FileRead struct {
	workDir string
}

func NewFileRead(workDir string) *FileRead {
	return &FileRead{workDir: workDir}
}

func (t *FileRead) Name() string {
	return "FileRead"
}

func (t *FileRead) Description() string {
	return "Read a file's contents with path safety checks. Returns text content or a binary file placeholder."
}

func (t *FileRead) RiskLevel() types.RiskLevel {
	return types.RiskSafe
}

// ParameterSchema returns the JSON Schema for FileRead tool parameters.
func (t *FileRead) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"path": {
				"type": "string",
				"description": "Path to the file to read (relative to working directory)"
			},
			"limit": {
				"type": "integer",
				"description": "Maximum number of bytes to read (default 5242880)",
				"minimum": 1
			}
		},
		"required": ["path"]
	}`
}

func (t *FileRead) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	if err := ctx.Err(); err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: %v", m31errors.ErrToolExecution, err)
	}

	pathRaw, ok := input.Params["path"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: missing parameter: path", m31errors.ErrToolExecution)
	}
	path, ok := pathRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: parameter path must be a string", m31errors.ErrToolExecution)
	}

	limit := types.MaxFileSize
	if limitRaw, ok := input.Params["limit"]; ok {
		if limitFloat, ok := limitRaw.(float64); ok {
			limit = int(limitFloat)
		}
	}

	// Resolve relative to workDir; absolute paths used as-is
	joined := path
	if !filepath.IsAbs(path) {
		joined = filepath.Join(t.workDir, path)
	}
	absPath, err := filepath.Abs(joined)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: cannot resolve path: %v", m31errors.ErrToolExecution, err)
	}

	// Resolve symlinks
	resolved, err := filepath.EvalSymlinks(absPath)
	if err != nil {
		if os.IsNotExist(err) {
			return types.ToolResult{}, fmt.Errorf("%w: file not found: %s", m31errors.ErrToolExecution, path)
		}
		return types.ToolResult{}, fmt.Errorf("%w: cannot resolve path: %v", m31errors.ErrToolExecution, err)
	}

	// Verify resolved path is within workDir (with separator guard)
	workDirPrefix := t.workDir
	if !strings.HasSuffix(workDirPrefix, string(filepath.Separator)) {
		workDirPrefix += string(filepath.Separator)
	}
	if resolved != t.workDir && !strings.HasPrefix(resolved, workDirPrefix) {
		return types.ToolResult{}, fmt.Errorf("%w: path resolves outside working directory", m31errors.ErrToolExecution)
	}

	// Check if it's a directory
	fi, err := os.Stat(resolved)
	if err != nil {
		if os.IsNotExist(err) {
			return types.ToolResult{}, fmt.Errorf("%w: file not found: %s", m31errors.ErrToolExecution, path)
		}
		return types.ToolResult{}, fmt.Errorf("%w: cannot access %s: %w", m31errors.ErrToolExecution, path, err)
	}
	if fi.IsDir() {
		return types.ToolResult{}, fmt.Errorf("%w: path is a directory, not a file: %s", m31errors.ErrToolExecution, path)
	}

	// Check file size
	fileSize := fi.Size()
	if fileSize > int64(limit) {
		return types.ToolResult{}, fmt.Errorf("%w: file %s exceeds size limit", m31errors.ErrFileTooLarge, path)
	}

	// Open and read
	f, err := os.Open(resolved)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: cannot access %s: %w", m31errors.ErrToolExecution, path, err)
	}
	defer f.Close() //nolint:errcheck

	// Read first 512 bytes for binary detection
	header := make([]byte, 512)
	n, readErr := f.Read(header)
	if readErr != nil && readErr != io.EOF {
		return types.ToolResult{}, fmt.Errorf("%w: read header: %v", m31errors.ErrToolExecution, readErr)
	}
	header = header[:n]

	// Check for null byte (binary detection)
	isBinary := false
	for _, b := range header {
		if b == 0 {
			isBinary = true
			break
		}
	}

	if isBinary {
		mimeType := http.DetectContentType(header)
		elapsed := time.Since(start).Milliseconds()
		return types.ToolResult{
			Output:     fmt.Sprintf("[binary file, %s, %d bytes]", mimeType, fileSize),
			DurationMs: elapsed,
		}, nil
	}

	// Read the rest of the file (up to limit)
	totalContent := make([]byte, len(header))
	copy(totalContent, header)

	remaining := limit - len(header)
	if remaining > 0 {
		buf := make([]byte, 64*1024)
		for remaining > 0 {
			readSize := len(buf)
			if readSize > remaining {
				readSize = remaining
			}
			n, err := f.Read(buf[:readSize])
			if n > 0 {
				totalContent = append(totalContent, buf[:n]...)
				remaining -= n
			}
			if err != nil {
				break
			}
		}
	}

	elapsed := time.Since(start).Milliseconds()
	return types.ToolResult{
		Output:     string(totalContent),
		DurationMs: elapsed,
	}, nil
}
