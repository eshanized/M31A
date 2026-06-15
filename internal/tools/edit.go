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

// Compile-time interface check
var _ types.Tool = (*Edit)(nil)

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
	return "Make targeted edits to a file: replace an exact string (old_string→new_string) or replace a line range (start_line+end_line+new_string). Prefer this over FileWrite for partial modifications."
}

func (t *Edit) RiskLevel() types.RiskLevel {
	return types.RiskDangerous
}

// ParameterSchema returns the JSON Schema for Edit tool parameters.
func (t *Edit) ParameterSchema() string {
	return `{
		"type": "object",
		"properties": {
			"path": {"type": "string", "description": "File path to edit (relative to working directory)"},
			"old_string": {"type": "string", "description": "Exact string to find and replace (use with new_string; mutually exclusive with start_line/end_line)"},
			"new_string": {"type": "string", "description": "Replacement string (required for both modes)"},
			"start_line": {"type": "integer", "description": "First line to replace, 1-indexed inclusive (use with end_line; mutually exclusive with old_string)"},
			"end_line": {"type": "integer", "description": "Last line to replace, 1-indexed inclusive (use with start_line)"}
		},
		"required": ["path", "new_string"]
	}`
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

	// Check file size before reading to prevent OOM
	fi, err := os.Stat(targetPath)
	if err != nil {
		return types.ToolResult{}, fmt.Errorf("cannot stat file: %w", err)
	}
	if fi.Size() > int64(types.MaxFileSize) {
		return types.ToolResult{}, fmt.Errorf("file %s exceeds size limit (%d bytes)", path, fi.Size())
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
	} else if os.IsNotExist(err) {
		// File doesn't exist — resolve parent directory through symlinks
		parentDir := filepath.Dir(targetPath)
		if resolvedParent, parentErr := filepath.EvalSymlinks(parentDir); parentErr == nil {
			resolved = filepath.Join(resolvedParent, filepath.Base(targetPath))
		}
	} else {
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
		t.pruneBackups(sanitized)
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
	tmpFile, err := os.OpenFile(tmpPath, os.O_CREATE|os.O_WRONLY|os.O_TRUNC, FilePermission)
	if err != nil {
		return fmt.Errorf("create temp file failed: %w", err)
	}
	if _, err := tmpFile.WriteString(newContent); err != nil {
		_ = tmpFile.Close()
		_ = os.Remove(tmpPath)
		return fmt.Errorf("write failed: %w", err)
	}
	if err := tmpFile.Sync(); err != nil {
		_ = tmpFile.Close()
		_ = os.Remove(tmpPath)
		return fmt.Errorf("sync failed: %w", err)
	}
	if err := tmpFile.Close(); err != nil {
		_ = os.Remove(tmpPath)
		return fmt.Errorf("close failed: %w", err)
	}
	// Belt-and-braces: explicitly set final mode in case the create-mode was
	// masked by a restrictive umask. After rename the inode keeps this mode.
	_ = os.Chmod(tmpPath, FilePermission)
	if err := os.Rename(tmpPath, targetPath); err != nil {
		_ = os.Remove(tmpPath)
		return fmt.Errorf("rename failed: %w", err)
	}

	return nil
}

func (t *Edit) pruneBackups(sanitizedPrefix string) {
	entries, err := os.ReadDir(t.backupDir)
	if err != nil {
		slog.Warn("edit: cannot read backup directory for pruning", "dir", t.backupDir, "error", err)
		return
	}

	var matches []string
	for _, e := range entries {
		if !e.IsDir() && strings.HasPrefix(e.Name(), sanitizedPrefix+".") && strings.HasSuffix(e.Name(), ".bak") {
			matches = append(matches, e.Name())
		}
	}

	if len(matches) <= MaxBackupsPerFile {
		return
	}

	sort.Strings(matches)

	toDelete := matches[:len(matches)-MaxBackupsPerFile]
	for _, name := range toDelete {
		path := filepath.Join(t.backupDir, name)
		if err := os.Remove(path); err != nil {
			slog.Warn("edit: failed to prune old backup", "path", path, "error", err)
		}
	}
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
	// Use explicit copies to avoid Go slice-append aliasing bugs where
	// appending to a sub-slice mutates the underlying array.
	newContentLines := strings.Split(newContent, "\n")
	newLines := make([]string, 0, startIdx+len(newContentLines)+len(lines)-endIdx-1)
	newLines = append(newLines, lines[:startIdx]...)
	newLines = append(newLines, newContentLines...)
	newLines = append(newLines, lines[endIdx+1:]...)

	return strings.Join(newLines, "\n"), nil
}

func cascadingReplace(content, oldString, newString string) (string, string, error) {
	// Split content once for all strategies (H1 fix)
	contentLines := strings.Split(content, "\n")

	// Strategy 2: Exact match
	if idx := strings.Index(content, oldString); idx >= 0 {
		return strings.Replace(content, oldString, newString, 1), "exact-match", nil
	}

	// Strategy 3: Line-trimmed match
	result, err := lineTrimmedReplace(content, contentLines, oldString, newString)
	if err == nil {
		return result, "line-trimmed", nil
	}

	// Strategy 4: Whitespace-normalized match
	result, err = whitespaceNormalizedReplace(content, contentLines, oldString, newString)
	if err == nil {
		return result, "whitespace-normalized", nil
	}

	// Strategy 5: Fuzzy anchor match
	result, err = fuzzyAnchorReplace(content, contentLines, oldString, newString)
	if err == nil {
		return result, "fuzzy-anchor", nil
	}

	// All strategies failed
	return "", "", fmt.Errorf(
		"could not find old_string in file.\n\nFile has %d lines, %d characters.\n\n"+
			"Tips:\n"+
			"- Use start_line and end_line for precise line-range edits\n"+
			"- Ensure old_string matches exactly (check indentation and whitespace)\n"+
			"- Read the file first to get the current content",
		len(contentLines), len(content),
	)
}

func lineTrimmedReplace(content string, contentLines []string, oldString, newString string) (string, error) {
	oldLines := strings.Split(oldString, "\n")

	oldTrimmed := make([]string, len(oldLines))
	for i, line := range oldLines {
		oldTrimmed[i] = strings.TrimSpace(line)
	}

	// Pre-trim content lines to avoid repeated TrimSpace in inner loop (PERF-16)
	contentTrimmed := make([]string, len(contentLines))
	for i, line := range contentLines {
		contentTrimmed[i] = strings.TrimSpace(line)
	}

	for i := 0; i <= len(contentLines)-len(oldTrimmed); i++ {
		match := true
		for j := range oldTrimmed {
			if contentTrimmed[i+j] != oldTrimmed[j] {
				match = false
				break
			}
		}
		if match {
			// Found match at line i, replace preserving original indentation
			newLines := make([]string, 0, len(contentLines))
			newLines = append(newLines, contentLines[:i]...)

			newContentLines := strings.Split(newString, "\n")
			// Compute base indent from first matched line and first new line
			baseIndent := ""
			if i < len(contentLines) {
				baseIndent = leadingWhitespace(contentLines[i])
			}
			newBaseIndent := ""
			if len(newContentLines) > 0 {
				newBaseIndent = leadingWhitespace(newContentLines[0])
			}
			for _, ncLine := range newContentLines {
				trimmed := strings.TrimSpace(ncLine)
				if trimmed != "" {
					ncIndent := leadingWhitespace(ncLine)
					// Compute relative indent: remove new content's base indent, add matched line's indent
					relativeIndent := ""
					if len(ncIndent) >= len(newBaseIndent) {
						relativeIndent = ncIndent[len(newBaseIndent):]
					}
					ncLine = baseIndent + relativeIndent + trimmed
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

func whitespaceNormalizedReplace(content string, contentLines []string, oldString, newString string) (string, error) {
	normalize := func(s string) string {
		fields := strings.Fields(s)
		return strings.Join(fields, " ")
	}

	oldLines := strings.Split(oldString, "\n")

	normalizedOldLines := make([]string, len(oldLines))
	for i, line := range oldLines {
		normalizedOldLines[i] = normalize(line)
	}

	// Pre-compute normalized content lines to avoid repeated normalize() calls (PERF-17)
	normalizedContentLines := make([]string, len(contentLines))
	for i, line := range contentLines {
		normalizedContentLines[i] = normalize(line)
	}

	for i := 0; i <= len(contentLines)-len(oldLines); i++ {
		match := true
		for j := range normalizedOldLines {
			if normalizedContentLines[i+j] != normalizedOldLines[j] {
				match = false
				break
			}
		}
		if match {
			newLines := make([]string, 0, len(contentLines)-len(oldLines)+strings.Count(newString, "\n")+1)
			newLines = append(newLines, contentLines[:i]...)
			newLines = append(newLines, strings.Split(newString, "\n")...)
			if i+len(oldLines) < len(contentLines) {
				newLines = append(newLines, contentLines[i+len(oldLines):]...)
			}
			return strings.Join(newLines, "\n"), nil
		}
	}

	return "", fmt.Errorf("no whitespace-normalized match found")
}

func fuzzyAnchorReplace(content string, contentLines []string, oldString, newString string) (string, error) {
	oldLines := strings.Split(oldString, "\n")
	if len(oldLines) < MinLinesForFuzzy {
		return "", fmt.Errorf("fuzzy anchor requires at least %d lines", MinLinesForFuzzy)
	}

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

		// When there are no middle lines (first+last match is sufficient),
		// accept the match directly to avoid 0/0 NaN from the similarity check.
		if len(middleOld) == 0 {
			newLines := make([]string, 0, len(contentLines))
			newLines = append(newLines, contentLines[:i]...)
			newLines = append(newLines, strings.Split(newString, "\n")...)
			newLines = append(newLines, contentLines[endIdx+1:]...)
			return strings.Join(newLines, "\n"), nil
		}

		totalSimilarity := 0.0
		for j := range middleOld {
			sim := levenshteinSimilarity(strings.TrimSpace(middleOld[j]), strings.TrimSpace(middleContent[j]))
			totalSimilarity += sim
		}
		avgSimilarity := totalSimilarity / float64(len(middleOld))

		if avgSimilarity >= LevenshteinThreshold {
			// Good enough match
			newLines := make([]string, 0, len(contentLines))
			newLines = append(newLines, contentLines[:i]...)
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

func toInt(v any) (int, bool) {
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

	// Build LCS table for line-level diff
	lcs := buildLCS(oldLines, newLines)
	hunks := diffHunks(oldLines, newLines, lcs, 3)

	if len(hunks) == 0 {
		return fmt.Sprintf("--- %s\n+++ %s\n(no changes)", path, path)
	}

	var out strings.Builder
	out.WriteString(fmt.Sprintf("--- %s\n+++ %s\n", path, path))
	for _, h := range hunks {
		out.WriteString(h)
	}
	return out.String()
}

func buildLCS(a, b []string) [][]int {
	m, n := len(a), len(b)
	dp := make([][]int, m+1)
	for i := range dp {
		dp[i] = make([]int, n+1)
	}
	for i := 1; i <= m; i++ {
		for j := 1; j <= n; j++ {
			if a[i-1] == b[j-1] {
				dp[i][j] = dp[i-1][j-1] + 1
			} else if dp[i-1][j] > dp[i][j-1] {
				dp[i][j] = dp[i-1][j]
			} else {
				dp[i][j] = dp[i][j-1]
			}
		}
	}
	return dp
}

type diffOp int

const (
	diffEqual diffOp = iota
	diffDelete
	diffInsert
)

func diffLines(a, b []string, lcs [][]int) []struct {
	op   diffOp
	line string
} {
	var ops []struct {
		op   diffOp
		line string
	}
	i, j := len(a), len(b)
	for i > 0 || j > 0 {
		if i > 0 && j > 0 && a[i-1] == b[j-1] {
			ops = append([]struct {
				op   diffOp
				line string
			}{{diffEqual, a[i-1]}}, ops...)
			i--
			j--
		} else if j > 0 && (i == 0 || lcs[i][j-1] >= lcs[i-1][j]) {
			ops = append([]struct {
				op   diffOp
				line string
			}{{diffInsert, b[j-1]}}, ops...)
			j--
		} else if i > 0 {
			ops = append([]struct {
				op   diffOp
				line string
			}{{diffDelete, a[i-1]}}, ops...)
			i--
		}
	}
	return ops
}

func diffHunks(a, b []string, lcs [][]int, contextLines int) []string {
	ops := diffLines(a, b, lcs)

	type region struct {
		start, end int
	}
	var changeRegions []region
	for i, op := range ops {
		if op.op != diffEqual {
			changeRegions = append(changeRegions, region{i, i})
		}
	}
	if len(changeRegions) == 0 {
		return nil
	}

	// Merge nearby change regions into hunks with context
	type hunkRange struct {
		start, end int
	}
	var hunkRanges []hunkRange
	cur := hunkRange{
		start: max(0, changeRegions[0].start-contextLines),
		end:   min(len(ops)-1, changeRegions[0].end+contextLines),
	}
	for _, r := range changeRegions[1:] {
		newStart := max(0, r.start-contextLines)
		newEnd := min(len(ops)-1, r.end+contextLines)
		if newStart <= cur.end+1 {
			cur.end = newEnd
		} else {
			hunkRanges = append(hunkRanges, cur)
			cur = hunkRange{newStart, newEnd}
		}
	}
	hunkRanges = append(hunkRanges, cur)

	var hunks []string
	for _, hr := range hunkRanges {
		var hunk strings.Builder
		oldStart, newStart := 1, 1
		for i := 0; i < hr.start; i++ {
			if ops[i].op == diffEqual || ops[i].op == diffDelete {
				oldStart++
			}
			if ops[i].op == diffEqual || ops[i].op == diffInsert {
				newStart++
			}
		}
		oldCount, newCount := 0, 0
		for i := hr.start; i <= hr.end; i++ {
			switch ops[i].op {
			case diffEqual:
				oldCount++
				newCount++
			case diffDelete:
				oldCount++
			case diffInsert:
				newCount++
			}
		}
		hunk.WriteString(fmt.Sprintf("@@ -%d,%d +%d,%d @@\n", oldStart, oldCount, newStart, newCount))
		for i := hr.start; i <= hr.end; i++ {
			switch ops[i].op {
			case diffEqual:
				hunk.WriteString(" " + ops[i].line + "\n")
			case diffDelete:
				hunk.WriteString("-" + ops[i].line + "\n")
			case diffInsert:
				hunk.WriteString("+" + ops[i].line + "\n")
			}
		}
		hunks = append(hunks, hunk.String())
	}
	return hunks
}

