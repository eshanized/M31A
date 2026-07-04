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
	"github.com/eshanized/M31A/pkg/metrics"
)

// Compile-time interface check
var _ types.Tool = (*Edit)(nil)

// DefaultConfidenceThreshold is the minimum confidence score required for
// a non-exact match to be accepted. Matches below this threshold return an
// error instead of silently performing a potentially incorrect replacement.
const DefaultConfidenceThreshold = 0.8

// EditStrategy defines a matching strategy with its confidence score.
type EditStrategy struct {
	Name       string
	Confidence float64
}

// Known edit strategies with their confidence scores.
var (
	StrategyExact      = EditStrategy{Name: "exact-match", Confidence: 1.0}
	StrategyTrimmed    = EditStrategy{Name: "trimmed", Confidence: 0.95}
	StrategyNormalized = EditStrategy{Name: "normalized", Confidence: 0.9}
	StrategyAnchor     = EditStrategy{Name: "anchor", Confidence: 0.85}
	StrategyFuzzy      = EditStrategy{Name: "fuzzy", Confidence: 0.7}
)

type Edit struct {
	workDir             string
	backupDir           string
	collector           *metrics.Collector
	ConfidenceThreshold float64
}

func NewEdit(workDir, backupDir string) *Edit {
	return &Edit{
		workDir:             workDir,
		backupDir:           backupDir,
		ConfidenceThreshold: DefaultConfidenceThreshold,
	}
}

// SetCollector attaches a metrics collector for recording edit strategy usage.
func (t *Edit) SetCollector(c *metrics.Collector) {
	t.collector = c
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
			"end_line": {"type": "integer", "description": "Last line to replace, 1-indexed inclusive (use with start_line)"},
			"replace_all": {"type": "boolean", "description": "Replace all occurrences, not just the first (default false)"}
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
		return types.ToolResult{}, fmt.Errorf("%w: missing parameter: path", m31errors.ErrToolExecution)
	}
	path, ok := pathRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: parameter path must be a string", m31errors.ErrToolExecution)
	}

	newStringRaw, ok := input.Params["new_string"]
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: missing parameter: new_string", m31errors.ErrToolExecution)
	}
	newString, ok := newStringRaw.(string)
	if !ok {
		return types.ToolResult{}, fmt.Errorf("%w: parameter new_string must be a string", m31errors.ErrToolExecution)
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
	var confidence float64
	var matchErr error

	// Parse replace_all flag
	replaceAll := false
	if raRaw, ok := input.Params["replace_all"]; ok {
		if raBool, ok := raRaw.(bool); ok {
			replaceAll = raBool
		}
	}

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
		confidence = 1.0
	} else {
		// Strategy 2-5: Cascading string matching with confidence scoring
		oldStringRaw, ok := input.Params["old_string"]
		if !ok {
			return types.ToolResult{}, fmt.Errorf("must provide either start_line+end_line or old_string")
		}
		oldString, ok := oldStringRaw.(string)
		if !ok {
			return types.ToolResult{}, fmt.Errorf("parameter old_string must be a string")
		}
		normalizedOldString := strings.ReplaceAll(oldString, "\r\n", "\n")

		newContent, strategy, confidence, matchErr = cascadingReplace(normalizedContent, normalizedOldString, normalizedNewString, replaceAll, t.ConfidenceThreshold)
	}

	// Record edit strategy usage
	if t.collector != nil && strategy != "" {
		t.collector.RecordEditStrategy(strategy)
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
		Output:     fmt.Sprintf("Edited %s using %s strategy (confidence: %.2f)\n\n%s", path, strategy, confidence, diffSummary),
		DurationMs: elapsed,
	}, nil
}

func (t *Edit) resolvePath(path string) (string, error) {
	return ResolveAndContainPath(path, t.workDir)
}

func (t *Edit) atomicWrite(targetPath, newContent string, oldContent []byte) error {
	// Backup existing file
	if _, err := os.Stat(targetPath); err == nil {
		relPath, _ := filepath.Rel(t.workDir, targetPath)
		sanitized := strings.ReplaceAll(relPath, string(filepath.Separator), "_")
		// Use timestamp + random hex to prevent collision on rapid edits
		randBytes := make([]byte, 4)
		if _, err := rand.Read(randBytes); err != nil {
			return fmt.Errorf("cannot generate backup name: %w", err)
		}
		backupPath := filepath.Join(t.backupDir, fmt.Sprintf("%s.%d.%s.bak", sanitized, time.Now().UnixNano(), hex.EncodeToString(randBytes)))

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
	pruneBackupsByPrefix(t.backupDir, sanitizedPrefix, MaxBackupsPerFile)
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

func cascadingReplace(content, oldString, newString string, replaceAll bool, threshold float64) (string, string, float64, error) {
	// Split content once for all strategies
	contentLines := strings.Split(content, "\n")

	// Strategy 1: Exact match (confidence: 1.0)
	if idx := strings.Index(content, oldString); idx >= 0 {
		if replaceAll {
			return strings.ReplaceAll(content, oldString, newString), StrategyExact.Name, StrategyExact.Confidence, nil
		}
		return strings.Replace(content, oldString, newString, 1), StrategyExact.Name, StrategyExact.Confidence, nil
	}

	// Strategy 2: Trimmed match — trim each line before comparing (confidence: 0.95)
	if result, err := trimmedReplace(content, contentLines, oldString, newString); err == nil {
		return result, StrategyTrimmed.Name, StrategyTrimmed.Confidence, nil
	}

	// Strategy 3: Normalized match — full whitespace + indent normalization (confidence: 0.9)
	if result, err := normalizedReplace(content, contentLines, oldString, newString); err == nil {
		return result, StrategyNormalized.Name, StrategyNormalized.Confidence, nil
	}

	// Strategy 4: Anchor match — first+last line anchoring with Levenshtein (confidence: 0.85)
	if result, err := anchorReplace(content, contentLines, oldString, newString); err == nil {
		return result, StrategyAnchor.Name, StrategyAnchor.Confidence, nil
	}

	// Strategy 5: Fuzzy match — Levenshtein-based approximate matching (confidence: 0.5-0.8)
	if result, conf, err := fuzzyReplace(content, contentLines, oldString, newString); err == nil {
		if conf >= threshold {
			return result, StrategyFuzzy.Name, conf, nil
		}
		// Fuzzy match found but below threshold — return error with details
		return "", "", conf, types.NewToolError(
			fmt.Errorf("fuzzy match found but confidence %.2f is below threshold %.2f", conf, threshold),
			"The file contains a similar string but the match is not confident enough. "+
				"Use start_line and end_line for precise line-range edits, or read the file first to get the current content.",
		)
	}

	// All strategies failed
	return "", "", 0, types.NewToolError(
		fmt.Errorf("could not find old_string in file (file has %d lines, %d characters)", len(contentLines), len(content)),
		"Use start_line and end_line for precise line-range edits, or read the file first to get the current content. "+
			"Ensure old_string matches exactly (check indentation and whitespace).",
	)
}

// trimmedReplace matches by trimming whitespace from each line.
// Merges the old line-trimmed and whitespace-normalized strategies.
func trimmedReplace(content string, contentLines []string, oldString, newString string) (string, error) {
	oldLines := strings.Split(oldString, "\n")

	oldTrimmed := make([]string, len(oldLines))
	for i, line := range oldLines {
		oldTrimmed[i] = strings.TrimSpace(line)
	}

	// Pre-trim content lines to avoid repeated TrimSpace in inner loop
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
			newLines := make([]string, 0, len(contentLines))
			newLines = append(newLines, contentLines[:i]...)

			newContentLines := strings.Split(newString, "\n")
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
					relativeIndent := ""
					if len(ncIndent) >= len(newBaseIndent) {
						relativeIndent = ncIndent[len(newBaseIndent):]
					}
					ncLine = baseIndent + relativeIndent + trimmed
				}
				newLines = append(newLines, ncLine)
			}
			if i+len(oldLines) < len(contentLines) {
				newLines = append(newLines, contentLines[i+len(oldLines):]...)
			}

			return strings.Join(newLines, "\n"), nil
		}
	}

	return "", fmt.Errorf("no trimmed match found")
}

// normalizedReplace matches after normalizing whitespace and indentation.
// Merges the old whitespace-normalized and indent-normalized strategies.
func normalizedReplace(content string, contentLines []string, oldString, newString string) (string, error) {
	// Full normalization: collapse whitespace, normalize tabs to spaces
	normalize := func(s string) string {
		s = strings.ReplaceAll(s, "\t", "    ")
		fields := strings.Fields(s)
		return strings.Join(fields, " ")
	}

	oldLines := strings.Split(oldString, "\n")

	normalizedOldLines := make([]string, len(oldLines))
	for i, line := range oldLines {
		normalizedOldLines[i] = normalize(line)
	}

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

	return "", fmt.Errorf("no normalized match found")
}

// anchorReplace matches using first+last line anchoring with Levenshtein
// similarity for middle lines. Requires at least MinLinesForFuzzy lines.
func anchorReplace(content string, contentLines []string, oldString, newString string) (string, error) {
	oldLines := strings.Split(oldString, "\n")
	if len(oldLines) < MinLinesForFuzzy {
		return "", fmt.Errorf("anchor replace requires at least %d lines", MinLinesForFuzzy)
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

		middleOld := oldLines[1 : len(oldLines)-1]
		middleContent := contentLines[i+1 : endIdx]

		if len(middleOld) != len(middleContent) {
			continue
		}

		if len(middleOld) == 0 {
			newLines := make([]string, 0, len(contentLines))
			newLines = append(newLines, contentLines[:i]...)
			newLines = append(newLines, strings.Split(newString, "\n")...)
			newLines = append(newLines, contentLines[endIdx+1:]...)
			return strings.Join(newLines, "\n"), nil
		}

		maxLineLen := 0
		for _, l := range middleOld {
			if n := len(strings.TrimSpace(l)); n > maxLineLen {
				maxLineLen = n
			}
		}
		for _, l := range middleContent {
			if n := len(strings.TrimSpace(l)); n > maxLineLen {
				maxLineLen = n
			}
		}
		prev := make([]int, maxLineLen+1)
		curr := make([]int, maxLineLen+1)

		totalSimilarity := 0.0
		for j := range middleOld {
			a := strings.TrimSpace(middleOld[j])
			b := strings.TrimSpace(middleContent[j])
			var sim float64
			if a == b {
				sim = 1.0
			} else if len(a) == 0 || len(b) == 0 {
				sim = 0.0
			} else {
				dist := levenshteinBuf(a, b, prev, curr)
				ml := len(a)
				if len(b) > ml {
					ml = len(b)
				}
				sim = 1.0 - float64(dist)/float64(ml)
			}
			totalSimilarity += sim
		}
		avgSimilarity := totalSimilarity / float64(len(middleOld))

		if avgSimilarity >= LevenshteinThreshold {
			newLines := make([]string, 0, len(contentLines))
			newLines = append(newLines, contentLines[:i]...)
			newLines = append(newLines, strings.Split(newString, "\n")...)
			newLines = append(newLines, contentLines[endIdx+1:]...)
			return strings.Join(newLines, "\n"), nil
		}
	}

	return "", fmt.Errorf("no anchor match found")
}

// fuzzyReplace performs approximate matching using Levenshtein distance.
// Returns the result, confidence score (0.5-0.8 based on similarity), and error.
func fuzzyReplace(content string, contentLines []string, oldString, newString string) (string, float64, error) {
	oldLines := strings.Split(oldString, "\n")
	if len(oldLines) < 2 {
		return "", 0, fmt.Errorf("fuzzy replace requires at least 2 lines")
	}

	bestConfidence := 0.0
	bestIdx := -1

	// Sliding window: try to find a contiguous block of lines that
	// is similar to oldString using average line similarity
	for i := 0; i <= len(contentLines)-len(oldLines); i++ {
		totalSim := 0.0
		for j, oldLine := range oldLines {
			a := strings.TrimSpace(oldLine)
			b := strings.TrimSpace(contentLines[i+j])
			if a == b {
				totalSim += 1.0
			} else if len(a) == 0 || len(b) == 0 {
				totalSim += 0.0
			} else {
				totalSim += levenshteinSimilarity(a, b)
			}
		}
		avgSim := totalSim / float64(len(oldLines))

		// Only consider matches that are reasonably similar
		if avgSim >= LevenshteinThreshold && avgSim > bestConfidence {
			bestConfidence = avgSim
			bestIdx = i
		}
	}

	if bestIdx < 0 {
		return "", 0, fmt.Errorf("no fuzzy match found")
	}

	// Map similarity score to confidence range [0.5, 0.8]
	confidence := 0.5 + 0.3*(bestConfidence-LevenshteinThreshold)/(1.0-LevenshteinThreshold)

	newLines := make([]string, 0, len(contentLines))
	newLines = append(newLines, contentLines[:bestIdx]...)
	newLines = append(newLines, strings.Split(newString, "\n")...)
	if bestIdx+len(oldLines) < len(contentLines) {
		newLines = append(newLines, contentLines[bestIdx+len(oldLines):]...)
	}
	return strings.Join(newLines, "\n"), confidence, nil
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
	return LevenshteinDistance(a, b)
}

// levenshteinBuf is like levenshteinDistance but reuses pre-allocated buffers.
func levenshteinBuf(a, b string, prev, curr []int) int {
	return LevenshteinBuf(a, b, prev, curr)
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

// maxLCSMatrixSize caps the LCS matrix at ~16MB (4M cells × 4 bytes/int).
const maxLCSMatrixSize = 2000 * 2000

func generateDiffSummary(path, oldContent, newContent string) string {
	oldLines := strings.Split(oldContent, "\n")
	newLines := strings.Split(newContent, "\n")

	if len(oldLines)*len(newLines) > maxLCSMatrixSize {
		return fmt.Sprintf("--- %s\n+++ %s\n(%d lines removed, %d lines added)", path, path, len(oldLines), len(newLines))
	}

	lcs := buildLCS(oldLines, newLines)
	hunks := diffHunks(oldLines, newLines, lcs, 3)

	if len(hunks) == 0 {
		return fmt.Sprintf("--- %s\n+++ %s\n(no changes)", path, path)
	}

	var out strings.Builder
	fmt.Fprintf(&out, "--- %s\n+++ %s\n", path, path)
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

type diffEntry struct {
	op   diffOp
	line string
}

func diffLines(a, b []string, lcs [][]int) []diffEntry {
	var ops []diffEntry
	i, j := len(a), len(b)
	for i > 0 || j > 0 {
		if i > 0 && j > 0 && a[i-1] == b[j-1] {
			ops = append(ops, diffEntry{diffEqual, a[i-1]})
			i--
			j--
		} else if j > 0 && (i == 0 || lcs[i][j-1] >= lcs[i-1][j]) {
			ops = append(ops, diffEntry{diffInsert, b[j-1]})
			j--
		} else if i > 0 {
			ops = append(ops, diffEntry{diffDelete, a[i-1]})
			i--
		}
	}
	for l, r := 0, len(ops)-1; l < r; l, r = l+1, r-1 {
		ops[l], ops[r] = ops[r], ops[l]
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
		fmt.Fprintf(&hunk, "@@ -%d,%d +%d,%d @@\n", oldStart, oldCount, newStart, newCount)
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
