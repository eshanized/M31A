package tools

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/pkg/types"
)

// Compile-time interface check
var _ types.Tool = (*FileRead)(nil)

type FileRead struct {
	workDir string
}

// NewFileRead creates a new FileRead tool instance.
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
				"description": "Maximum number of bytes to read (default 5242880). Ignored when offset is set.",
				"minimum": 1
			},
			"offset": {
				"type": "integer",
				"description": "Line number to start reading from (1-indexed). When set, limit becomes max lines to return.",
				"minimum": 1
			},
			"max_lines": {
				"type": "integer",
				"description": "Maximum number of lines to return when offset is set (default 2000).",
				"minimum": 1
			}
		},
		"required": ["path"]
	}`
}

func (t *FileRead) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	if err := ctx.Err(); err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: %w", m31errors.ErrToolExecution, err)
	}

	pathRaw, ok := input.Params["path"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: missing parameter: path", m31errors.ErrToolExecution)
	}
	path, ok := pathRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: parameter path must be a string", m31errors.ErrToolExecution)
	}

	// Parse offset (line number to start from, 1-indexed)
	offset := 0
	if offsetRaw, ok := input.Params["offset"]; ok {
		if offsetFloat, ok := offsetRaw.(float64); ok {
			offset = int(offsetFloat)
		}
	}

	// Parse max_lines (max lines to return when offset is set)
	maxLines := 2000
	if maxLinesRaw, ok := input.Params["max_lines"]; ok {
		if maxLinesFloat, ok := maxLinesRaw.(float64); ok {
			maxLines = int(maxLinesFloat)
		}
	}

	// Parse byte limit (only used when offset is not set)
	limit := types.MaxFileSize
	if offset == 0 {
		if limitRaw, ok := input.Params["limit"]; ok {
			if limitFloat, ok := limitRaw.(float64); ok {
				limit = int(limitFloat)
			}
		}
	}

	// Resolve relative to workDir; absolute paths used as-is
	resolved, err := ResolveAndContainPathExists(path, t.workDir)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: %w", m31errors.ErrToolExecution, err)
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

	// Check file size (skip for offset mode — we read line-by-line)
	fileSize := fi.Size()
	if offset == 0 && fileSize > int64(limit) {
		return types.ToolResult{}, types.NewToolError(
			fmt.Errorf("%w: file %s exceeds size limit (%d bytes)", m31errors.ErrFileTooLarge, path, fileSize),
			fmt.Sprintf("Use offset and max_lines parameters to read specific line ranges, or increase the limit parameter (current: %d bytes).", limit),
		)
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
		return types.ToolResult{}, fmt.Errorf("%w: read header: %w", m31errors.ErrToolExecution, readErr)
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

	// Line-level reading mode (offset is set)
	if offset > 0 {
		return t.readLineRange(f, path, offset, maxLines, start)
	}

	// Byte-level reading mode (original behavior)
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

// readLineRange reads lines from a file starting at offset (1-indexed) up to maxLines.
// It returns the selected lines with line numbers prefixed.
func (t *FileRead) readLineRange(f *os.File, path string, offset, maxLines int, start time.Time) (types.ToolResult, error) {
	scanner := bufio.NewScanner(f)
	lineNum := 0
	var sb strings.Builder
	linesShown := 0

	for scanner.Scan() {
		lineNum++
		if lineNum < offset {
			continue
		}
		if linesShown >= maxLines {
			break
		}
		fmt.Fprintf(&sb, "%d: %s\n", lineNum, scanner.Text())
		linesShown++
	}

	if err := scanner.Err(); err != nil {
		return types.ToolResult{}, fmt.Errorf("%w: read error: %w", m31errors.ErrToolExecution, err)
	}

	elapsed := time.Since(start).Milliseconds()

	if linesShown == 0 {
		totalLines := lineNum
		return types.ToolResult{
			Output:     fmt.Sprintf("No lines found at offset %d (file has %d lines)", offset, totalLines),
			DurationMs: elapsed,
		}, nil
	}

	output := sb.String()
	truncated := linesShown >= maxLines
	if truncated {
		output += fmt.Sprintf("\n[... showing %d lines from line %d (limit: %d lines)]", linesShown, offset, maxLines)
	}

	return types.ToolResult{
		Output:     output,
		DurationMs: elapsed,
		Truncated:  truncated,
	}, nil
}
