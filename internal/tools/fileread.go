package tools

import (
	"context"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/types"
	m31errors "github.com/eshanized/M31A/internal/errors"
)

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

func (t *FileRead) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	pathRaw, ok := input.Params["path"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: path")
	}
	path, ok := pathRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter path must be a string")
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
		return types.ToolResult{}, fmt.Errorf("cannot resolve path: %w", err)
	}

	// Resolve symlinks
	resolved, err := filepath.EvalSymlinks(absPath)
	if err != nil {
		if os.IsNotExist(err) {
			return types.ToolResult{}, fmt.Errorf("file not found: %s", path)
		}
		return types.ToolResult{}, fmt.Errorf("cannot resolve path: %w", err)
	}

	// Verify resolved path is within workDir (with separator guard)
	workDirPrefix := t.workDir
	if !strings.HasSuffix(workDirPrefix, string(filepath.Separator)) {
		workDirPrefix += string(filepath.Separator)
	}
	if resolved != t.workDir && !strings.HasPrefix(resolved, workDirPrefix) {
		return types.ToolResult{}, fmt.Errorf("path resolves outside working directory")
	}

	// Check if it's a directory
	fi, err := os.Stat(resolved)
	if err != nil {
		if os.IsNotExist(err) {
			return types.ToolResult{}, fmt.Errorf("file not found: %s", path)
		}
		return types.ToolResult{}, m31errors.ErrPermissionDenied
	}
	if fi.IsDir() {
		return types.ToolResult{}, fmt.Errorf("path is a directory, not a file: %s", path)
	}

	// Check file size
	fileSize := fi.Size()
	if fileSize > int64(limit) {
		return types.ToolResult{}, m31errors.ErrFileTooLarge
	}

	// Open and read
	f, err := os.Open(resolved)
	if err != nil {
		return types.ToolResult{}, m31errors.ErrPermissionDenied
	}
	defer f.Close()

	// Read first 512 bytes for binary detection
	header := make([]byte, 512)
	n, _ := f.Read(header)
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
