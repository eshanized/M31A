package tools

import (
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"time"
)

// OutputStore bounds tool output to prevent single tool calls from consuming
// the entire context window. When output exceeds configured limits, the full
// output is saved to a managed directory and a head+tail preview is returned.
type OutputStore struct {
	mu       sync.Mutex
	baseDir  string
	maxLines int
	maxBytes int
	counter  atomic.Int64
}

// NewOutputStore creates an OutputStore with the given limits.
// baseDir is the directory where truncated outputs are saved (e.g., ~/.m31a/tool-output/).
func NewOutputStore(baseDir string, maxLines, maxBytes int) *OutputStore {
	if maxLines <= 0 {
		maxLines = DefaultOutputMaxLines
	}
	if maxBytes <= 0 {
		maxBytes = DefaultOutputMaxBytes
	}
	return &OutputStore{
		baseDir:  baseDir,
		maxLines: maxLines,
		maxBytes: maxBytes,
	}
}

// Bound checks if the output exceeds configured limits. If so, it saves the
// full output to a file and returns a head+tail preview with a truncation
// marker. Returns (boundedOutput, savedPath, wasTruncated).
func (s *OutputStore) Bound(output string) (string, string, bool) {
	if output == "" {
		return output, "", false
	}

	lineCount := strings.Count(output, "\n") + 1
	byteCount := len(output)

	if lineCount <= s.maxLines && byteCount <= s.maxBytes {
		return output, "", false
	}

	savedPath, err := s.saveFull(output)
	if err != nil {
		slog.Warn("failed to save truncated tool output", "error", err)
		return s.truncateInPlace(output), "", true
	}

	preview := s.headTailPreview(output)
	marker := fmt.Sprintf(
		"\n\n... output truncated (%d lines, %d bytes); full content saved to %s ...\n"+
			"Hint: use FileRead with offset/limit to inspect the full output, or use Grep to search within it.\n",
		lineCount, byteCount, savedPath,
	)

	return preview + marker, savedPath, true
}

// saveFull writes the full output to the managed directory and returns the path.
func (s *OutputStore) saveFull(output string) (string, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	if err := os.MkdirAll(s.baseDir, DirPermission); err != nil {
		return "", fmt.Errorf("create output store dir: %w", err)
	}

	id := s.counter.Add(1)
	filename := fmt.Sprintf("%d_%d.txt", time.Now().UnixMilli(), id)
	path := filepath.Join(s.baseDir, filename)

	if err := os.WriteFile(path, []byte(output), FilePermission); err != nil {
		return "", fmt.Errorf("write tool output: %w", err)
	}

	return path, nil
}

// headTailPreview returns a preview containing the first 60% and last 40%
// of the allowed line count, with a separator in between.
func (s *OutputStore) headTailPreview(output string) string {
	lines := strings.Split(output, "\n")
	totalLines := len(lines)

	headCount := int(float64(s.maxLines) * 0.6)
	tailCount := s.maxLines - headCount

	if totalLines <= headCount+tailCount {
		return output
	}

	var sb strings.Builder
	for i := 0; i < headCount && i < totalLines; i++ {
		sb.WriteString(lines[i])
		sb.WriteByte('\n')
	}

	sb.WriteString(fmt.Sprintf("\n... %d lines omitted ...\n\n", totalLines-headCount-tailCount))

	start := totalLines - tailCount
	if start < headCount {
		start = headCount
	}
	for i := start; i < totalLines; i++ {
		sb.WriteString(lines[i])
		if i < totalLines-1 {
			sb.WriteByte('\n')
		}
	}

	return sb.String()
}

// truncateInPlace truncates output without saving to disk (fallback).
func (s *OutputStore) truncateInPlace(output string) string {
	lines := strings.Split(output, "\n")
	if len(lines) <= s.maxLines {
		if len(output) > s.maxBytes {
			return output[:s.maxBytes] + "\n...[truncated by byte limit]"
		}
		return output
	}
	return s.headTailPreview(output) + "\n...[truncated; output store unavailable]"
}

// Cleanup removes output files older than the given retention period.
func (s *OutputStore) Cleanup(retention time.Duration) (int, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	entries, err := os.ReadDir(s.baseDir)
	if err != nil {
		if os.IsNotExist(err) {
			return 0, nil
		}
		return 0, fmt.Errorf("read output store dir: %w", err)
	}

	cutoff := time.Now().Add(-retention)
	removed := 0

	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		info, err := entry.Info()
		if err != nil {
			continue
		}
		if info.ModTime().Before(cutoff) {
			path := filepath.Join(s.baseDir, entry.Name())
			if err := os.Remove(path); err == nil {
				removed++
			}
		}
	}

	return removed, nil
}
