package tools

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

type Edit struct {
	workDir   string
	backupDir string
}

func NewEdit(workDir, backupDir string) *Edit {
	return &Edit{workDir: workDir, backupDir: backupDir}
}

func (t *Edit) Name() string {
	return "Edit"
}

func (t *Edit) Description() string {
	return "Make targeted edits to a file using line-range replacement or smart string matching."
}

func (t *Edit) RiskLevel() types.RiskLevel {
	return types.RiskDangerous
}

func (t *Edit) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	start := time.Now()

	if err := ctx.Err(); err != nil {
		return types.ToolResult{}, err
	}

	pathRaw, ok := input.Params["path"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: path")
	}
	path, ok := pathRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter path must be a string")
	}

	newStringRaw, ok := input.Params["new_string"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("missing parameter: new_string")
	}
	newString, ok := newStringRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("parameter new_string must be a string")
	}

	// Check for binary content
	for _, b := range []byte(newString) {
		if b == 0 {
			return types.ToolResult{}, m31errors.ErrNoBinaryContent
		}
	}

	// Resolve target path
	targetPath, err := t.resolvePath(path)
	if err != nil {
		return types.ToolResult{}, err
	}

	// Read existing content
	oldContent, err := os.ReadFile(targetPath)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("cannot read file: %w", err)
	}
	content := string(oldContent)

	// Detect line ending style
	lineEnding := detectLineEnding(content)

	// Normalize line endings for processing
	normalizedContent := strings.ReplaceAll(content, "\r\n", "\n")
	normalizedNewString := strings.ReplaceAll(newString, "\r\n", "\n")

	// Apply replacement strategy
	var newContent string
	var strategy string
	var matchErr error

	// Strategy 1: Line-range replacement
	startLineRaw, hasStartLine := input.Params["start_line"]
	endLineRaw, hasEndLine := input.Params["end_line"]

	if hasStartLine && hasEndLine {
		startLine, ok := toInt(startLineRaw)
		if !ok {
			return types.ToolResult{}, fmt.Errorf("parameter start_line must be an integer")
		}
		endLine, ok := toInt(endLineRaw)
		if !ok {
			return types.ToolResult{}, fmt.Errorf("parameter end_line must be an integer")
		}
		newContent, matchErr = replaceByLineRange(normalizedContent, startLine, endLine, normalizedNewString)
		strategy = "line-range"
	} else {
		// Strategy 2-5: Cascading string matching
		oldStringRaw, ok := input.Params["old_string"]
		if !ok {
			return types.ToolResult{}, fmt.Errorf("must provide either start_line+end_line or old_string")
		}
		oldString, ok := oldStringRaw.(string)
		if !ok {
			return types.ToolResult{}, fmt.Errorf("parameter old_string must be a string")
		}
		normalizedOldString := strings.ReplaceAll(oldString, "\r\n", "\n")

		newContent, strategy, matchErr = cascadingReplace(normalizedContent, normalizedOldString, normalizedNewString)
	}

	if matchErr != nil {
		return types.ToolResult{
			Output:     matchErr.Error(),
			DurationMs: time.Since(start).Milliseconds(),
		}, nil
	}

	// Restore original line endings
	newContent = strings.ReplaceAll(newContent, "\n", lineEnding)

	// Write atomically
	if err := t.atomicWrite(targetPath, newContent, oldContent); err != nil {
		return types.ToolResult{}, err
	}

	// Generate diff summary
	diffSummary := generateDiffSummary(path, content, newContent)

	elapsed := time.Since(start).Milliseconds()
	return types.ToolResult{
		Output:     fmt.Sprintf("Edited %s using %s strategy\n\n%s", path, strategy, diffSummary),
		DurationMs: elapsed,
	}, nil
}

func (t *Edit) resolvePath(path string) (string, error) {
	joined := path
	if !filepath.IsAbs(path) {
		joined = filepath.Join(t.workDir, path)
	}
	targetPath, err := filepath.Abs(joined)
	if err != nil {
		return "", fmt.Errorf("cannot resolve path: %w", err)
	}

	resolved := targetPath
	if _, err := os.Stat(targetPath); err == nil {
		resolved, err = filepath.EvalSymlinks(targetPath)
		if err != nil {
			return "", fmt.Errorf("cannot resolve symlinks: %w", err)
		}
	} else if !os.IsNotExist(err) {
		return "", fmt.Errorf("cannot stat path: %w", err)
	}

	workDirPrefix := t.workDir
	if !strings.HasSuffix(workDirPrefix, string(filepath.Separator)) {
		workDirPrefix += string(filepath.Separator)
	}
	if resolved != t.workDir && !strings.HasPrefix(resolved, workDirPrefix) {
		return "", fmt.Errorf("path resolves outside working directory")
	}

	return resolved, nil
}

func (t *Edit) atomicWrite(targetPath, newContent string, oldContent []byte) error {
	// Backup existing file
	if _, err := os.Stat(targetPath); err == nil {
		relPath, _ := filepath.Rel(t.workDir, targetPath)
		sanitized := strings.ReplaceAll(relPath, string(filepath.Separator), "_")
		backupPath := filepath.Join(t.backupDir, fmt.Sprintf("%s.%d.bak", sanitized, time.Now().Unix()))

		if err := os.MkdirAll(t.backupDir, DirPermission); err != nil {
			return fmt.Errorf("cannot create backup directory: %w", err)
		}
		if err := os.WriteFile(backupPath, oldContent, FilePermission); err != nil {
			return fmt.Errorf("cannot write backup: %w", err)
		}
	}

	// Create parent directories if needed
	if err := os.MkdirAll(filepath.Dir(targetPath), DirPermission); err != nil {
		return fmt.Errorf("cannot create directories: %w", err)
	}

	// Write to temp file then rename
	randBytes := make([]byte, 8)
	if _, err := rand.Read(randBytes); err != nil {
		return fmt.Errorf("cannot generate temp name: %w", err)
	}
	tmpPath := filepath.Join(filepath.Dir(targetPath), ".m31a_tmp_"+hex.EncodeToString(randBytes))
	tmpFile, err := os.Create(tmpPath)
	if err != nil {
		return fmt.Errorf("create temp file failed: %w", err)
	}
	if _, err := tmpFile.WriteString(newContent); err != nil {
		tmpFile.Close()
		os.Remove(tmpPath)
		return fmt.Errorf("write failed: %w", err)
	}
	if err := tmpFile.Sync(); err != nil {
		tmpFile.Close()
		os.Remove(tmpPath)
		return fmt.Errorf("sync failed: %w", err)
	}
	if err := tmpFile.Close(); err != nil {
		os.Remove(tmpPath)
		return fmt.Errorf("close failed: %w", err)
	}
	if err := os.Rename(tmpPath, targetPath); err != nil {
		os.Remove(tmpPath)
		return fmt.Errorf("rename failed: %w", err)
	}

	return nil
}

func replaceByLineRange(content string, startLine, endLine int, newContent string) (string, error) {
	lines := strings.Split(content, "\n")

	if startLine < 1 || startLine > len(lines) {
		return "", fmt.Errorf("start_line %d is out of range (file has %d lines)", startLine, len(lines))
	}
	if endLine < startLine || endLine > len(lines) {
		return "", fmt.Errorf("end_line %d is out of range (file has %d lines)", endLine, len(lines))
	}

	// Convert to 0-indexed
	startIdx := startLine - 1
	endIdx := endLine - 1

	// Replace lines[startIdx..endIdx] inclusive
	newLines := append(lines[:startIdx], append(strings.Split(newContent, "\n"), lines[endIdx+1:]...)...)

	return strings.Join(newLines, "\n"), nil
}

func cascadingReplace(content, oldString, newString string) (string, string, error) {
	// Strategy 2: Exact match
	if idx := strings.Index(content, oldString); idx >= 0 {
		return strings.Replace(content, oldString, newString, 1), "exact-match", nil
	}

	// Strategy 3: Line-trimmed match
	result, err := lineTrimmedReplace(content, oldString, newString)
	if err == nil {
		return result, "line-trimmed", nil
	}

	// Strategy 4: Whitespace-normalized match
	result, err = whitespaceNormalizedReplace(content, oldString, newString)
	if err == nil {
		return result, "whitespace-normalized", nil
	}

	// Strategy 5: Fuzzy anchor match
	result, err = fuzzyAnchorReplace(content, oldString, newString)
	if err == nil {
		return result, "fuzzy-anchor", nil
	}

	// All strategies failed
	return "", "", fmt.Errorf(
		"Could not find old_string in file.\n\nFile has %d lines, %d characters.\n\n"+
			"Tips:\n"+
			"- Use start_line and end_line for precise line-range edits\n"+
			"- Ensure old_string matches exactly (check indentation and whitespace)\n"+
			"- Read the file first to get the current content",
		strings.Count(content, "\n")+1, len(content),
	)
}

func lineTrimmedReplace(content, oldString, newString string) (string, error) {
	oldLines := strings.Split(oldString, "\n")
	contentLines := strings.Split(content, "\n")

	oldTrimmed := make([]string, len(oldLines))
	for i, line := range oldLines {
		oldTrimmed[i] = strings.TrimSpace(line)
	}

	for i := 0; i <= len(contentLines)-len(oldTrimmed); i++ {
		match := true
		for j := range oldTrimmed {
			if strings.TrimSpace(contentLines[i+j]) != oldTrimmed[j] {
				match = false
				break
			}
		}
		if match {
			// Found match at line i, replace preserving original indentation
			newLines := make([]string, 0, len(contentLines))
			newLines = append(newLines, contentLines[:i]...)

			newContentLines := strings.Split(newString, "\n")
			// Try to preserve indentation from original lines
			for k, ncLine := range newContentLines {
				if k < len(oldLines) && i+k < len(contentLines) {
					indent := leadingWhitespace(contentLines[i+k])
					ncLine = indent + strings.TrimSpace(ncLine)
				}
				newLines = append(newLines, ncLine)
			}
			// Bounds check: only append remaining lines if match is not at end of file
			if i+len(oldLines) < len(contentLines) {
				newLines = append(newLines, contentLines[i+len(oldLines):]...)
			}

			return strings.Join(newLines, "\n"), nil
		}
	}

	return "", fmt.Errorf("no line-trimmed match found")
}

func whitespaceNormalizedReplace(content, oldString, newString string) (string, error) {
	normalize := func(s string) string {
		fields := strings.Fields(s)
		return strings.Join(fields, " ")
	}

	normalizedContent := normalize(content)
	normalizedOld := normalize(oldString)

	idx := strings.Index(normalizedContent, normalizedOld)
	if idx < 0 {
		return "", fmt.Errorf("no whitespace-normalized match found")
	}

	// Map normalized index back to original content
	// Find the original substring that corresponds to the match
	return strings.Replace(content, oldString, newString, 1), nil
}

func fuzzyAnchorReplace(content, oldString, newString string) (string, error) {
	oldLines := strings.Split(oldString, "\n")
	if len(oldLines) < MinLinesForFuzzy {
		return "", fmt.Errorf("fuzzy anchor requires at least %d lines", MinLinesForFuzzy)
	}

	contentLines := strings.Split(content, "\n")
	firstLine := strings.TrimSpace(oldLines[0])
	lastLine := strings.TrimSpace(oldLines[len(oldLines)-1])

	for i := 0; i <= len(contentLines)-len(oldLines); i++ {
		if strings.TrimSpace(contentLines[i]) != firstLine {
			continue
		}
		endIdx := i + len(oldLines) - 1
		if endIdx >= len(contentLines) {
			continue
		}
		if strings.TrimSpace(contentLines[endIdx]) != lastLine {
			continue
		}

		// First and last line match — check middle lines with Levenshtein
		middleOld := oldLines[1 : len(oldLines)-1]
		middleContent := contentLines[i+1 : endIdx]

		if len(middleOld) != len(middleContent) {
			continue
		}

		totalSimilarity := 0.0
		for j := range middleOld {
			sim := levenshteinSimilarity(strings.TrimSpace(middleOld[j]), strings.TrimSpace(middleContent[j]))
			totalSimilarity += sim
		}
		avgSimilarity := totalSimilarity / float64(len(middleOld))

		if avgSimilarity >= LevenshteinThreshold {
			// Good enough match
			newLines := make([]string, len(contentLines))
			copy(newLines, contentLines[:i])
			newLines = append(newLines, strings.Split(newString, "\n")...)
			newLines = append(newLines, contentLines[endIdx+1:]...)
			return strings.Join(newLines, "\n"), nil
		}
	}

	return "", fmt.Errorf("no fuzzy anchor match found")
}

func levenshteinSimilarity(a, b string) float64 {
	if a == b {
		return 1.0
	}
	if len(a) == 0 || len(b) == 0 {
		return 0.0
	}

	dist := levenshteinDistance(a, b)
	maxLen := len(a)
	if len(b) > maxLen {
		maxLen = len(b)
	}
	return 1.0 - float64(dist)/float64(maxLen)
}

func levenshteinDistance(a, b string) int {
	if len(a) == 0 {
		return len(b)
	}
	if len(b) == 0 {
		return len(a)
	}

	// Use single row for space efficiency
	prev := make([]int, len(b)+1)
	curr := make([]int, len(b)+1)

	for j := 0; j <= len(b); j++ {
		prev[j] = j
	}

	for i := 1; i <= len(a); i++ {
		curr[0] = i
		for j := 1; j <= len(b); j++ {
			cost := 1
			if a[i-1] == b[j-1] {
				cost = 0
			}
			curr[j] = min(curr[j-1]+1, min(prev[j]+1, prev[j-1]+cost))
		}
		prev, curr = curr, prev
	}

	return prev[len(b)]
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func leadingWhitespace(s string) string {
	var indent strings.Builder
	for _, ch := range s {
		if ch == ' ' || ch == '\t' {
			indent.WriteRune(ch)
		} else {
			break
		}
	}
	return indent.String()
}

func detectLineEnding(content string) string {
	if strings.Contains(content, "\r\n") {
		return "\r\n"
	}
	return "\n"
}

func toInt(v interface{}) (int, bool) {
	switch n := v.(type) {
	case int:
		return n, true
	case float64:
		return int(n), true
	case int64:
		return int(n), true
	default:
		return 0, false
	}
}

func generateDiffSummary(path, oldContent, newContent string) string {
	oldLines := strings.Split(oldContent, "\n")
	newLines := strings.Split(newContent, "\n")

	added := 0
	removed := 0

	// Simple diff: count line differences
	minLen := len(oldLines)
	if len(newLines) < minLen {
		minLen = len(newLines)
	}

	for i := 0; i < minLen; i++ {
		if oldLines[i] != newLines[i] {
			removed++
			added++
		}
	}
	removed += len(oldLines) - minLen
	added += len(newLines) - minLen

	var lines []string
	lines = append(lines, fmt.Sprintf("--- %s", path))
	lines = append(lines, fmt.Sprintf("+++ %s", path))
	if removed > 0 {
		lines = append(lines, fmt.Sprintf("-%d lines", removed))
	}
	if added > 0 {
		lines = append(lines, fmt.Sprintf("+%d lines", added))
	}

	return strings.Join(lines, "\n")
}
