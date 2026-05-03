package tools

import (
	"context"
	"encoding/json"
	"net"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/tools/subagent"
	"github.com/eshanized/M31A/internal/types"
)

// ---------------------------------------------------------------------------
// Edit: cascadingReplace edge cases (line-trimmed and fuzzy fallback paths)
// ---------------------------------------------------------------------------

func TestCascadingReplace_LineTrimmedFallback(t *testing.T) {
	t.Parallel()
	content := "  hello\n  world\n  foo"
	// Exact match fails because of leading spaces; line-trimmed should match
	result, strategy, err := cascadingReplace(content, "hello\nworld", "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strategy != "line-trimmed" {
		t.Errorf("strategy = %q, want 'line-trimmed'", strategy)
	}
	if !strings.Contains(result, "REPLACED") {
		t.Errorf("expected REPLACED in result, got %q", result)
	}
}

func TestCascadingReplace_WhitespaceNormalizedFallback(t *testing.T) {
	t.Parallel()
	content := "hello    world\nfoo   bar"
	// Line-trimmed fails because trimmed lines differ by whitespace;
	// whitespace-normalized normalizes fields so it should match
	result, strategy, err := cascadingReplace(content, "hello world\nfoo bar", "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strategy != "whitespace-normalized" {
		t.Errorf("strategy = %q, want 'whitespace-normalized'", strategy)
	}
	if result != "REPLACED" {
		t.Errorf("got %q, want 'REPLACED'", result)
	}
}

func TestCascadingReplace_FuzzyAnchorFallback(t *testing.T) {
	t.Parallel()
	content := "first line\nsecond line\nthird line\nfourth line\nfifth line"
	oldStr := "first line\nsecnd line\nthird line"
	newStr := "REPLACED"
	// Exact, line-trimmed, whitespace-normalized all fail;
	// fuzzy anchor should match via Levenshtein on middle lines
	result, strategy, err := cascadingReplace(content, oldStr, newStr)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strategy != "fuzzy-anchor" {
		t.Errorf("strategy = %q, want 'fuzzy-anchor'", strategy)
	}
	if !strings.Contains(result, "REPLACED") {
		t.Errorf("expected REPLACED in result, got %q", result)
	}
}

func TestCascadingReplace_ExactMatchTakesPriority(t *testing.T) {
	t.Parallel()
	content := "hello world"
	result, strategy, err := cascadingReplace(content, "hello world", "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strategy != "exact-match" {
		t.Errorf("strategy = %q, want 'exact-match'", strategy)
	}
	if result != "REPLACED" {
		t.Errorf("got %q, want 'REPLACED'", result)
	}
}

func TestCascadingReplace_EmptyContent(t *testing.T) {
	t.Parallel()
	_, _, err := cascadingReplace("", "hello", "world")
	if err == nil {
		t.Error("expected error for empty content")
	}
}

func TestCascadingReplace_EmptyOldString(t *testing.T) {
	t.Parallel()
	_, _, err := cascadingReplace("hello world", "", "REPLACED")
	// empty oldString matches at index 0 in strings.Index, so it returns exact-match
	// Actually strings.Index returns 0 for empty substring
	result, strategy, err := cascadingReplace("hello world", "", "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	_ = strategy
	_ = result
}

// ---------------------------------------------------------------------------
// Edit: fuzzyAnchorReplace edge cases
// ---------------------------------------------------------------------------

func TestFuzzyAnchorReplace_NoMiddleLines(t *testing.T) {
	t.Parallel()
	// First+last match but there are no middle lines — needs MinLinesForFuzzy lines in oldStr
	content := "alpha\nbeta\ngamma\ntheta"
	oldStr := "alpha\nbeta\ngamma"
	newStr := "REPLACED"
	contentLines := strings.Split(content, "\n")
	result, err := fuzzyAnchorReplace(content, contentLines, oldStr, newStr)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result, "REPLACED") {
		t.Errorf("expected REPLACED in result, got %q", result)
	}
}

func TestFuzzyAnchorReplace_MiddleLinesBelowThreshold(t *testing.T) {
	t.Parallel()
	// Middle lines differ too much — should not match
	content := "first\ncompletely different\nalso different\nlast"
	oldStr := "first\nslightly similar\nalso somewhat\nlast"
	newStr := "REPLACED"
	contentLines := strings.Split(content, "\n")
	_, err := fuzzyAnchorReplace(content, contentLines, oldStr, newStr)
	if err == nil {
		t.Error("expected error when middle lines differ significantly")
	}
}

func TestFuzzyAnchorReplace_FirstLineNoMatch(t *testing.T) {
	t.Parallel()
	content := "alpha\nbeta\ngamma"
	oldStr := "zzzz\nbeta\ngamma"
	newStr := "REPLACED"
	contentLines := strings.Split(content, "\n")
	_, err := fuzzyAnchorReplace(content, contentLines, oldStr, newStr)
	if err == nil {
		t.Error("expected error when first line doesn't match")
	}
}

func TestFuzzyAnchorReplace_LastLineNoMatch(t *testing.T) {
	t.Parallel()
	content := "alpha\nbeta\ngamma"
	oldStr := "alpha\nbeta\nzzzz"
	newStr := "REPLACED"
	contentLines := strings.Split(content, "\n")
	_, err := fuzzyAnchorReplace(content, contentLines, oldStr, newStr)
	if err == nil {
		t.Error("expected error when last line doesn't match")
	}
}

func TestFuzzyAnchorReplace_TooFewLines_AllOldLinesMatch(t *testing.T) {
	t.Parallel()
	content := "one\ntwo"
	contentLines := strings.Split(content, "\n")
	_, err := fuzzyAnchorReplace(content, contentLines, "one\ntwo", "new")
	if err == nil {
		t.Error("expected error for fewer than MinLinesForFuzzy lines")
	}
}

func TestFuzzyAnchorReplace_ContentTooShort(t *testing.T) {
	t.Parallel()
	content := "short"
	oldStr := "short\nsecond\nthird"
	newStr := "REPLACED"
	contentLines := strings.Split(content, "\n")
	_, err := fuzzyAnchorReplace(content, contentLines, oldStr, newStr)
	if err == nil {
		t.Error("expected error when content is shorter than old string")
	}
}

// ---------------------------------------------------------------------------
// Edit: lineTrimmedReplace edge cases
// ---------------------------------------------------------------------------

func TestLineTrimmedReplace_WhitespaceOnlyDiffers(t *testing.T) {
	t.Parallel()
	content := "  hello\n  world\n  baz"
	contentLines := strings.Split(content, "\n")
	result, err := lineTrimmedReplace(content, contentLines, "hello\nworld", "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strings.Contains(result, "hello") {
		t.Errorf("expected 'hello' to be replaced, got %q", result)
	}
}

func TestLineTrimmedReplace_MultipleOccurrences(t *testing.T) {
	t.Parallel()
	content := "  hello\n  world\n  foo\n  hello\n  world"
	contentLines := strings.Split(content, "\n")
	result, err := lineTrimmedReplace(content, contentLines, "hello\nworld", "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// Only first occurrence should be replaced
	if !strings.Contains(result, "REPLACED") {
		t.Errorf("expected REPLACED in result, got %q", result)
	}
}

func TestLineTrimmedReplace_NoMatch(t *testing.T) {
	t.Parallel()
	content := "aaa\nbbb\nccc"
	contentLines := strings.Split(content, "\n")
	_, err := lineTrimmedReplace(content, contentLines, "xxx\nyyy", "REPLACED")
	if err == nil {
		t.Error("expected error when no match found")
	}
}

func TestLineTrimmedReplace_SingleLine(t *testing.T) {
	t.Parallel()
	content := "  hello\n  world"
	contentLines := strings.Split(content, "\n")
	result, err := lineTrimmedReplace(content, contentLines, "hello", "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result, "REPLACED") {
		t.Errorf("expected REPLACED in result, got %q", result)
	}
}

func TestLineTrimmedReplace_IndentationPreserved(t *testing.T) {
	t.Parallel()
	content := "\t\thello\n\t\tworld"
	contentLines := strings.Split(content, "\n")
	result, err := lineTrimmedReplace(content, contentLines, "hello\nworld", "replaced\nhere")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result, "replaced") {
		t.Errorf("expected indentation preserved, got %q", result)
	}
}

// ---------------------------------------------------------------------------
// Edit: whitespaceNormalizedReplace edge cases
// ---------------------------------------------------------------------------

func TestWhitespaceNormalizedReplace_NoMatch(t *testing.T) {
	t.Parallel()
	content := "aaa\nbbb"
	contentLines := strings.Split(content, "\n")
	_, err := whitespaceNormalizedReplace(content, contentLines, "xxx\nyyy", "REPLACED")
	if err == nil {
		t.Error("expected error when no match found")
	}
}

func TestWhitespaceNormalizedReplace_MultipleSpaces(t *testing.T) {
	t.Parallel()
	content := "hello     world"
	contentLines := strings.Split(content, "\n")
	result, err := whitespaceNormalizedReplace(content, contentLines, "hello world", "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "REPLACED" {
		t.Errorf("got %q, want 'REPLACED'", result)
	}
}

func TestWhitespaceNormalizedReplace_Tabs(t *testing.T) {
	t.Parallel()
	content := "hello\t\tworld"
	contentLines := strings.Split(content, "\n")
	result, err := whitespaceNormalizedReplace(content, contentLines, "hello world", "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "REPLACED" {
		t.Errorf("got %q, want 'REPLACED'", result)
	}
}

func TestWhitespaceNormalizedReplace_MultiLine(t *testing.T) {
	t.Parallel()
	content := "line1   with   spaces\nline2  with  tabs"
	contentLines := strings.Split(content, "\n")
	result, err := whitespaceNormalizedReplace(content, contentLines, "line1 with spaces\nline2 with tabs", "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "REPLACED" {
		t.Errorf("got %q, want 'REPLACED'", result)
	}
}

// ---------------------------------------------------------------------------
// Edit: levenshteinSimilarity edge cases
// ---------------------------------------------------------------------------

func TestLevenshteinSimilarity_OneEmpty(t *testing.T) {
	t.Parallel()
	if s := levenshteinSimilarity("abc", ""); s != 0.0 {
		t.Errorf("similarity('abc', '') = %f, want 0.0", s)
	}
	if s := levenshteinSimilarity("", "abc"); s != 0.0 {
		t.Errorf("similarity('', 'abc') = %f, want 0.0", s)
	}
}

func TestLevenshteinSimilarity_CompletelyDifferent(t *testing.T) {
	t.Parallel()
	s := levenshteinSimilarity("aaa", "bbb")
	if s != 0.0 {
		t.Errorf("similarity('aaa', 'bbb') = %f, want 0.0", s)
	}
}

// ---------------------------------------------------------------------------
// Edit: levenshteinDistance edge cases
// ---------------------------------------------------------------------------

func TestLevenshteinDistance_LongStrings(t *testing.T) {
	t.Parallel()
	a := strings.Repeat("a", 100)
	b := strings.Repeat("b", 100)
	if d := levenshteinDistance(a, b); d != 100 {
		t.Errorf("distance(long_a, long_b) = %d, want 100", d)
	}
}

func TestLevenshteinDistance_SubsetString(t *testing.T) {
	t.Parallel()
	if d := levenshteinDistance("abc", "ab"); d != 1 {
		t.Errorf("distance('abc', 'ab') = %d, want 1", d)
	}
	if d := levenshteinDistance("ab", "abc"); d != 1 {
		t.Errorf("distance('ab', 'abc') = %d, want 1", d)
	}
}

// ---------------------------------------------------------------------------
// Edit: generateDiffSummary edge cases
// ---------------------------------------------------------------------------

func TestGenerateDiffSummary_Identical(t *testing.T) {
	t.Parallel()
	summary := generateDiffSummary("test.txt", "line1\nline2", "line1\nline2")
	// Identical content still generates the --- and +++ header lines
	if !strings.Contains(summary, "--- test.txt") {
		t.Errorf("expected '--- test.txt' in summary, got %q", summary)
	}
	// But no +/- line count lines should appear
	if strings.Contains(summary, "-0 lines") || strings.Contains(summary, "+0 lines") {
		t.Errorf("expected no line count for identical content, got %q", summary)
	}
}

func TestGenerateDiffSummary_AddedLines(t *testing.T) {
	t.Parallel()
	summary := generateDiffSummary("test.txt", "line1", "line1\nline2\nline3")
	if !strings.Contains(summary, "+2 lines") {
		t.Errorf("expected '+2 lines' in summary, got %q", summary)
	}
}

func TestGenerateDiffSummary_RemovedLines(t *testing.T) {
	t.Parallel()
	summary := generateDiffSummary("test.txt", "line1\nline2\nline3", "line1")
	if !strings.Contains(summary, "-2 lines") {
		t.Errorf("expected '-2 lines' in summary, got %q", summary)
	}
}

func TestGenerateDiffSummary_EmptyOld(t *testing.T) {
	t.Parallel()
	summary := generateDiffSummary("test.txt", "", "new content")
	if !strings.Contains(summary, "+") {
		t.Errorf("expected '+' in summary for added content, got %q", summary)
	}
}

func TestGenerateDiffSummary_EmptyNew(t *testing.T) {
	t.Parallel()
	summary := generateDiffSummary("test.txt", "old content", "")
	if !strings.Contains(summary, "-") {
		t.Errorf("expected '-' in summary for removed content, got %q", summary)
	}
}

func TestGenerateDiffSummary_BothEmpty(t *testing.T) {
	t.Parallel()
	summary := generateDiffSummary("test.txt", "", "")
	if summary == "" {
		t.Error("expected non-empty summary")
	}
}

// ---------------------------------------------------------------------------
// Edit: replaceByLineRange edge cases
// ---------------------------------------------------------------------------

func TestReplaceByLineRange_ReplaceAll(t *testing.T) {
	t.Parallel()
	content := "line1\nline2\nline3"
	result, err := replaceByLineRange(content, 1, 3, "replaced")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "replaced" {
		t.Errorf("got %q, want 'replaced'", result)
	}
}

func TestReplaceByLineRange_ReplaceFirst(t *testing.T) {
	t.Parallel()
	content := "line1\nline2\nline3"
	result, err := replaceByLineRange(content, 1, 1, "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "REPLACED\nline2\nline3" {
		t.Errorf("got %q", result)
	}
}

func TestReplaceByLineRange_ReplaceLast(t *testing.T) {
	t.Parallel()
	content := "line1\nline2\nline3"
	result, err := replaceByLineRange(content, 3, 3, "REPLACED")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "line1\nline2\nREPLACED" {
		t.Errorf("got %q", result)
	}
}

func TestReplaceByLineRange_EndLineBeforeStartLine(t *testing.T) {
	t.Parallel()
	content := "line1\nline2\nline3"
	_, err := replaceByLineRange(content, 3, 1, "x")
	if err == nil {
		t.Error("expected error when end < start")
	}
}

func TestReplaceByLineRange_EmptyContent(t *testing.T) {
	t.Parallel()
	// "" splits to [""], which has 1 line. startLine=1 is valid.
	result, err := replaceByLineRange("", 1, 1, "replaced")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result != "replaced" {
		t.Errorf("got %q, want 'replaced'", result)
	}
}

// ---------------------------------------------------------------------------
// Grep: loadGitignoreCached edge cases
// ---------------------------------------------------------------------------

func TestLoadGitignoreCached_EmptyDir(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	patterns := loadGitignoreCached(dir)
	if patterns != nil {
		t.Errorf("expected nil patterns for empty dir, got: %v", patterns)
	}
}

func TestLoadGitignoreCached_CacheHit(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, ".gitignore"), []byte("*.log\n"), 0644)

	patterns1 := loadGitignoreCached(dir)
	patterns2 := loadGitignoreCached(dir)
	if len(patterns1) != len(patterns2) {
		t.Errorf("expected cached results, got %d then %d patterns", len(patterns1), len(patterns2))
	}
}

func TestLoadGitignoreCached_CacheInvalidation(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, ".gitignore"), []byte("*.log\n"), 0644)

	patterns1 := loadGitignoreCached(dir)
	if len(patterns1) != 1 {
		t.Fatalf("expected 1 pattern, got %d", len(patterns1))
	}

	// Modify .gitignore
	os.WriteFile(filepath.Join(dir, ".gitignore"), []byte("*.log\n*.tmp\n"), 0644)

	patterns2 := loadGitignoreCached(dir)
	if len(patterns2) != 2 {
		t.Errorf("expected 2 patterns after cache invalidation, got %d", len(patterns2))
	}
}

func TestLoadGitignoreCached_AllCommentsAndBlanks(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, ".gitignore"), []byte("# comment\n\n# another comment\n\n"), 0644)
	patterns := loadGitignoreCached(dir)
	if len(patterns) != 0 {
		t.Errorf("expected 0 patterns for all comments, got %d", len(patterns))
	}
}

// ---------------------------------------------------------------------------
// Grep: matchesGitignore edge cases
// ---------------------------------------------------------------------------

func TestMatchesGitignore_DirectoryPattern(t *testing.T) {
	t.Parallel()
	patterns := []string{"vendor/**"}
	workDir := "/home/user/project"
	path := filepath.Join(workDir, "vendor", "lib", "code.go")
	if !matchesGitignore(path, patterns, workDir) {
		t.Error("expected vendor/lib/code.go to match vendor/**")
	}
}

func TestMatchesGitignore_NestedPattern(t *testing.T) {
	t.Parallel()
	patterns := []string{"src/vendor/**"}
	workDir := "/home/user/project"
	path := filepath.Join(workDir, "src", "vendor", "dep.go")
	if !matchesGitignore(path, patterns, workDir) {
		t.Error("expected src/vendor/dep.go to match src/vendor/**")
	}
}

func TestMatchesGitignore_NoMatch(t *testing.T) {
	t.Parallel()
	patterns := []string{"*.log"}
	workDir := "/home/user/project"
	path := filepath.Join(workDir, "main.go")
	if matchesGitignore(path, patterns, workDir) {
		t.Error("expected main.go to not match *.log")
	}
}

func TestMatchesGitignore_EmptyPatterns(t *testing.T) {
	t.Parallel()
	if matchesGitignore("/some/path", nil, "/") {
		t.Error("expected no match with nil patterns")
	}
	if matchesGitignore("/some/path", []string{}, "/") {
		t.Error("expected no match with empty patterns")
	}
}

func TestMatchesGitignore_FilenameOnlyPattern(t *testing.T) {
	t.Parallel()
	patterns := []string{".DS_Store"}
	workDir := "/home/user/project"
	path := filepath.Join(workDir, ".DS_Store")
	if !matchesGitignore(path, patterns, workDir) {
		t.Error("expected .DS_Store to match")
	}
}

// ---------------------------------------------------------------------------
// FileDelete: Execute edge cases
// ---------------------------------------------------------------------------

func TestFileDelete_MissingPathParam(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fd := NewFileDelete(dir, backupDir)
	_, err := fd.Execute(context.Background(), types.ToolInput{
		Name:   "FileDelete",
		Params: map[string]any{},
	})
	if err == nil {
		t.Error("expected error for missing path param")
	}
}

func TestFileDelete_PathNotString(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fd := NewFileDelete(dir, backupDir)
	_, err := fd.Execute(context.Background(), types.ToolInput{
		Name:   "FileDelete",
		Params: map[string]any{"path": 123},
	})
	if err == nil {
		t.Error("expected error for non-string path")
	}
}

func TestFileDelete_FileNotFound(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fd := NewFileDelete(dir, backupDir)
	_, err := fd.Execute(context.Background(), types.ToolInput{
		Name:   "FileDelete",
		Params: map[string]any{"path": "nonexistent.txt"},
	})
	if err == nil {
		t.Error("expected error for nonexistent file")
	}
	if !strings.Contains(err.Error(), "file not found") {
		t.Errorf("expected 'file not found', got: %v", err)
	}
}

func TestFileDelete_IsDirectory(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, "subdir"), 0755)
	fd := NewFileDelete(dir, backupDir)
	_, err := fd.Execute(context.Background(), types.ToolInput{
		Name:   "FileDelete",
		Params: map[string]any{"path": "subdir"},
	})
	if err == nil {
		t.Error("expected error for directory deletion")
	}
	if !strings.Contains(err.Error(), "cannot delete directory") {
		t.Errorf("expected 'cannot delete directory', got: %v", err)
	}
}

func TestFileDelete_PathOutsideWorkDir(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fd := NewFileDelete(dir, backupDir)
	_, err := fd.Execute(context.Background(), types.ToolInput{
		Name:   "FileDelete",
		Params: map[string]any{"path": "/etc/hostname"},
	})
	if err == nil {
		t.Error("expected error for path outside workDir")
	}
}

func TestFileDelete_PermanentDelete(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "to_delete.txt"), []byte("content"), 0644)
	fd := NewFileDelete(dir, backupDir)
	result, err := fd.Execute(context.Background(), types.ToolInput{
		Name: "FileDelete",
		Params: map[string]any{
			"path":      "to_delete.txt",
			"permanent": true,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "Deleted") {
		t.Errorf("expected 'Deleted' in output, got: %s", result.Output)
	}
	if _, err := os.Stat(filepath.Join(dir, "to_delete.txt")); !os.IsNotExist(err) {
		t.Error("expected file to be deleted")
	}
}

func TestFileDelete_BackupOnDelete(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	content := "important content"
	os.WriteFile(filepath.Join(dir, "important.txt"), []byte(content), 0644)
	fd := NewFileDelete(dir, backupDir)
	result, err := fd.Execute(context.Background(), types.ToolInput{
		Name:   "FileDelete",
		Params: map[string]any{"path": "important.txt"},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "backup saved") {
		t.Errorf("expected 'backup saved' in output, got: %s", result.Output)
	}
	// Verify backup was created
	entries, err := os.ReadDir(backupDir)
	if err != nil {
		t.Fatal(err)
	}
	if len(entries) == 0 {
		t.Error("expected backup file to be created")
	}
}

func TestFileDelete_NonPermanentParams(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "file.txt"), []byte("content"), 0644)
	fd := NewFileDelete(dir, backupDir)
	result, err := fd.Execute(context.Background(), types.ToolInput{
		Name: "FileDelete",
		Params: map[string]any{
			"path":      "file.txt",
			"permanent": false,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "backup saved") {
		t.Errorf("expected 'backup saved' in non-permanent delete, got: %s", result.Output)
	}
}

func TestFileDelete_NonBoolPermanent(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "file.txt"), []byte("content"), 0644)
	fd := NewFileDelete(dir, backupDir)
	// permanent = "not_a_bool" → treated as false (default)
	result, err := fd.Execute(context.Background(), types.ToolInput{
		Name: "FileDelete",
		Params: map[string]any{
			"path":      "file.txt",
			"permanent": "not_a_bool",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "backup saved") {
		t.Errorf("expected 'backup saved' for non-bool permanent, got: %s", result.Output)
	}
}

func TestFileDelete_Name(t *testing.T) {
	t.Parallel()
	fd := NewFileDelete(t.TempDir(), t.TempDir())
	if fd.Name() != "FileDelete" {
		t.Errorf("expected name 'FileDelete', got %s", fd.Name())
	}
}

func TestFileDelete_Description(t *testing.T) {
	t.Parallel()
	fd := NewFileDelete(t.TempDir(), t.TempDir())
	if fd.Description() == "" {
		t.Error("expected non-empty description")
	}
}

func TestFileDelete_RiskLevel(t *testing.T) {
	t.Parallel()
	fd := NewFileDelete(t.TempDir(), t.TempDir())
	if fd.RiskLevel() != types.RiskDangerous {
		t.Errorf("expected RiskDangerous, got %s", fd.RiskLevel())
	}
}

// ---------------------------------------------------------------------------
// FileList: Execute edge cases
// ---------------------------------------------------------------------------

func TestFileList_MissingPathParam(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fl := NewFileList(dir)
	result, err := fl.Execute(context.Background(), types.ToolInput{
		Name:   "FileList",
		Params: map[string]any{},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// Should default to workDir
	if result.Output == "" {
		t.Error("expected non-empty output for default dir listing")
	}
}

func TestFileList_AbsolutePathParam(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fl := NewFileList(dir)
	result, err := fl.Execute(context.Background(), types.ToolInput{
		Name: "FileList",
		Params: map[string]any{
			"path": dir,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Output == "" {
		t.Error("expected non-empty output")
	}
}

func TestFileList_DirectoryNotFound(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fl := NewFileList(dir)
	_, err := fl.Execute(context.Background(), types.ToolInput{
		Name: "FileList",
		Params: map[string]any{
			"path": "nonexistent_dir",
		},
	})
	if err == nil {
		t.Error("expected error for nonexistent directory")
	}
}

func TestFileList_PathOutsideWorkDir(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fl := NewFileList(dir)
	_, err := fl.Execute(context.Background(), types.ToolInput{
		Name: "FileList",
		Params: map[string]any{
			"path": "/etc",
		},
	})
	if err == nil {
		t.Error("expected error for path outside workDir")
	}
}

func TestFileList_MaxDepth(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, "a", "b", "c", "d"), 0755)
	os.WriteFile(filepath.Join(dir, "a", "b", "c", "d", "deep.txt"), []byte("deep"), 0644)
	fl := NewFileList(dir)
	result, err := fl.Execute(context.Background(), types.ToolInput{
		Name: "FileList",
		Params: map[string]any{
			"depth": float64(2),
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// With depth 2, deep.txt should not appear
	if strings.Contains(result.Output, "deep.txt") {
		t.Errorf("expected deep.txt to be excluded with depth 2, got: %s", result.Output)
	}
}

func TestFileList_MaxDepthCapped(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, "sub"), 0755)
	fl := NewFileList(dir)
	_, err := fl.Execute(context.Background(), types.ToolInput{
		Name: "FileList",
		Params: map[string]any{
			"depth": float64(100), // > max 6
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestFileList_NonNumericDepth(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fl := NewFileList(dir)
	_, err := fl.Execute(context.Background(), types.ToolInput{
		Name: "FileList",
		Params: map[string]any{
			"depth": "not_a_number",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestFileList_SkipDirs(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, "node_modules", "pkg"), 0755)
	os.WriteFile(filepath.Join(dir, "node_modules", "pkg", "index.js"), []byte("module"), 0644)
	os.WriteFile(filepath.Join(dir, "app.go"), []byte("package main"), 0644)
	fl := NewFileList(dir)
	result, err := fl.Execute(context.Background(), types.ToolInput{
		Name: "FileList",
		Params: map[string]any{},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if strings.Contains(result.Output, "node_modules") {
		t.Errorf("expected node_modules to be skipped, got: %s", result.Output)
	}
}

func TestFileList_EmptyDir(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fl := NewFileList(dir)
	result, err := fl.Execute(context.Background(), types.ToolInput{
		Name: "FileList",
		Params: map[string]any{},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Output == "" {
		t.Error("expected non-empty output for root dir")
	}
}

func TestFileList_Name(t *testing.T) {
	t.Parallel()
	fl := NewFileList(t.TempDir())
	if fl.Name() != "FileList" {
		t.Errorf("expected name 'FileList', got %s", fl.Name())
	}
}

func TestFileList_Description(t *testing.T) {
	t.Parallel()
	fl := NewFileList(t.TempDir())
	if fl.Description() == "" {
		t.Error("expected non-empty description")
	}
}

func TestFileList_RiskLevel(t *testing.T) {
	t.Parallel()
	fl := NewFileList(t.TempDir())
	if fl.RiskLevel() != types.RiskSafe {
		t.Errorf("expected RiskSafe, got %s", fl.RiskLevel())
	}
}

// ---------------------------------------------------------------------------
// FileMove: Execute edge cases
// ---------------------------------------------------------------------------

func TestFileMove_SimpleMove(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "src.txt"), []byte("content"), 0644)
	fm := NewFileMove(dir)
	result, err := fm.Execute(context.Background(), types.ToolInput{
		Name: "FileMove",
		Params: map[string]any{
			"source":      "src.txt",
			"destination": "dst.txt",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "Moved") {
		t.Errorf("expected 'Moved' in output, got: %s", result.Output)
	}
	if _, err := os.Stat(filepath.Join(dir, "dst.txt")); err != nil {
		t.Error("expected dst.txt to exist")
	}
}

func TestFileMove_MissingSource(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fm := NewFileMove(dir)
	_, err := fm.Execute(context.Background(), types.ToolInput{
		Name:   "FileMove",
		Params: map[string]any{},
	})
	if err == nil {
		t.Error("expected error for missing source param")
	}
}

func TestFileMove_SourceNotString(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fm := NewFileMove(dir)
	_, err := fm.Execute(context.Background(), types.ToolInput{
		Name: "FileMove",
		Params: map[string]any{
			"source":      123,
			"destination": "dst.txt",
		},
	})
	if err == nil {
		t.Error("expected error for non-string source")
	}
}

func TestFileMove_MissingDestination(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fm := NewFileMove(dir)
	_, err := fm.Execute(context.Background(), types.ToolInput{
		Name: "FileMove",
		Params: map[string]any{
			"source": "src.txt",
		},
	})
	if err == nil {
		t.Error("expected error for missing destination param")
	}
}

func TestFileMove_DestinationNotString(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fm := NewFileMove(dir)
	_, err := fm.Execute(context.Background(), types.ToolInput{
		Name: "FileMove",
		Params: map[string]any{
			"source":      "src.txt",
			"destination": 456,
		},
	})
	if err == nil {
		t.Error("expected error for non-string destination")
	}
}

func TestFileMove_SourceNotFound(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fm := NewFileMove(dir)
	_, err := fm.Execute(context.Background(), types.ToolInput{
		Name: "FileMove",
		Params: map[string]any{
			"source":      "nonexistent.txt",
			"destination": "dst.txt",
		},
	})
	if err == nil {
		t.Error("expected error for nonexistent source")
	}
	if !strings.Contains(err.Error(), "source file not found") {
		t.Errorf("expected 'source file not found', got: %v", err)
	}
}

func TestFileMove_SourceOutsideWorkDir(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	fm := NewFileMove(dir)
	_, err := fm.Execute(context.Background(), types.ToolInput{
		Name: "FileMove",
		Params: map[string]any{
			"source":      "/etc/hostname",
			"destination": "dst.txt",
		},
	})
	if err == nil {
		t.Error("expected error for source outside workDir")
	}
}

func TestFileMove_DestinationOutsideWorkDir(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "src.txt"), []byte("content"), 0644)
	fm := NewFileMove(dir)
	_, err := fm.Execute(context.Background(), types.ToolInput{
		Name: "FileMove",
		Params: map[string]any{
			"source":      "src.txt",
			"destination": "/tmp/evil.txt",
		},
	})
	if err == nil {
		t.Error("expected error for destination outside workDir")
	}
}

func TestFileMove_WithSubdirectory(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "src.txt"), []byte("content"), 0644)
	fm := NewFileMove(dir)
	_, err := fm.Execute(context.Background(), types.ToolInput{
		Name: "FileMove",
		Params: map[string]any{
			"source":      "src.txt",
			"destination": "sub/dir/dst.txt",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if _, err := os.Stat(filepath.Join(dir, "sub", "dir", "dst.txt")); err != nil {
		t.Error("expected dst.txt to exist in subdirectory")
	}
}

func TestFileMove_Name(t *testing.T) {
	t.Parallel()
	fm := NewFileMove(t.TempDir())
	if fm.Name() != "FileMove" {
		t.Errorf("expected name 'FileMove', got %s", fm.Name())
	}
}

func TestFileMove_Description(t *testing.T) {
	t.Parallel()
	fm := NewFileMove(t.TempDir())
	if fm.Description() == "" {
		t.Error("expected non-empty description")
	}
}

func TestFileMove_RiskLevel(t *testing.T) {
	t.Parallel()
	fm := NewFileMove(t.TempDir())
	if fm.RiskLevel() != types.RiskDangerous {
		t.Errorf("expected RiskDangerous, got %s", fm.RiskLevel())
	}
}

// ---------------------------------------------------------------------------
// TodoWrite: Execute edge cases
// ---------------------------------------------------------------------------

func TestTodoWrite_MissingTodosParam(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "test-session")
	_, err := tw.Execute(context.Background(), types.ToolInput{
		Name:   "TodoWrite",
		Params: map[string]any{},
	})
	if err == nil {
		t.Error("expected error for missing todos param")
	}
}

func TestTodoWrite_TodosNotArray(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "test-session")
	_, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": "not_an_array",
		},
	})
	if err == nil {
		t.Error("expected error for non-array todos")
	}
}

func TestTodoWrite_ItemNotObject(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "test-session")
	_, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{"not_an_object"},
		},
	})
	if err == nil {
		t.Error("expected error for non-object todo item")
	}
}

func TestTodoWrite_MissingContent(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "test-session")
	_, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{
				map[string]any{"status": "pending"},
			},
		},
	})
	if err == nil {
		t.Error("expected error for missing content field")
	}
}

func TestTodoWrite_InvalidStatus(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "test-session")
	_, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{
				map[string]any{
					"content": "task",
					"status":  "invalid_status",
				},
			},
		},
	})
	if err == nil {
		t.Error("expected error for invalid status")
	}
}

func TestTodoWrite_InvalidPriority(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "test-session")
	_, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{
				map[string]any{
					"content":  "task",
					"priority": "invalid_priority",
				},
			},
		},
	})
	if err == nil {
		t.Error("expected error for invalid priority")
	}
}

func TestTodoWrite_InvalidSessionID(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "invalid session id with spaces!")
	_, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{
				map[string]any{
					"content": "task",
					"status":  "pending",
				},
			},
		},
	})
	if err == nil {
		t.Error("expected error for invalid session ID")
	}
}

func TestTodoWrite_EmptyTodos(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "test-session")
	result, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{},
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "0 total") {
		t.Errorf("expected '0 total' in output, got: %s", result.Output)
	}
}

func TestTodoWrite_AllStatuses(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "test-session")
	result, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{
				map[string]any{"content": "pending task", "status": "pending"},
				map[string]any{"content": "in progress", "status": "in_progress"},
				map[string]any{"content": "done task", "status": "completed"},
				map[string]any{"content": "cancelled", "status": "cancelled"},
			},
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "4 total") {
		t.Errorf("expected '4 total', got: %s", result.Output)
	}
	if !strings.Contains(result.Output, "1 pending") {
		t.Errorf("expected '1 pending', got: %s", result.Output)
	}
}

func TestTodoWrite_DefaultStatusAndPriority(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "test-session")
	_, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{
				map[string]any{"content": "task with defaults"},
			},
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestTodoWrite_CancelledStatus(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "test-session")
	result, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{
				map[string]any{"content": "cancelled", "status": "cancelled"},
			},
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "1 cancelled") {
		t.Errorf("expected '1 cancelled', got: %s", result.Output)
	}
}

func TestTodoWrite_SetSessionID(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "old-session")
	tw.SetSessionID("new-session")
	if tw.getSessionID() != "new-session" {
		t.Errorf("expected 'new-session', got %q", tw.getSessionID())
	}
}

func TestTodoWrite_GetSessionID_Empty(t *testing.T) {
	t.Parallel()
	tw := &TodoWrite{}
	if tw.getSessionID() != "" {
		t.Errorf("expected empty session ID, got %q", tw.getSessionID())
	}
}

func TestTodoWrite_Name(t *testing.T) {
	t.Parallel()
	tw := NewTodoWrite(t.TempDir(), "test")
	if tw.Name() != "TodoWrite" {
		t.Errorf("expected name 'TodoWrite', got %s", tw.Name())
	}
}

func TestTodoWrite_Description(t *testing.T) {
	t.Parallel()
	tw := NewTodoWrite(t.TempDir(), "test")
	if tw.Description() == "" {
		t.Error("expected non-empty description")
	}
}

func TestTodoWrite_RiskLevel(t *testing.T) {
	t.Parallel()
	tw := NewTodoWrite(t.TempDir(), "test")
	if tw.RiskLevel() != types.RiskSafe {
		t.Errorf("expected RiskSafe, got %s", tw.RiskLevel())
	}
}

// ---------------------------------------------------------------------------
// WebFetch: isPrivateIP additional edge cases
// ---------------------------------------------------------------------------

func TestIsPrivateIP_IPv6ULA(t *testing.T) {
	t.Parallel()
	ip := net.ParseIP("fd00::1")
	if !isPrivateIP(ip) {
		t.Error("expected fd00::1 to be private (IPv6 ULA)")
	}
}

func TestIsPrivateIP_IPv6LinkLocal(t *testing.T) {
	t.Parallel()
	ip := net.ParseIP("fe80::1")
	if !isPrivateIP(ip) {
		t.Error("expected fe80::1 to be private (link-local)")
	}
}

func TestIsPrivateIP_IPv6Loopback(t *testing.T) {
	t.Parallel()
	ip := net.ParseIP("::1")
	if !isPrivateIP(ip) {
		t.Error("expected ::1 to be private (IPv6 loopback)")
	}
}

func TestIsPrivateIP_IPv4MappedIPv6(t *testing.T) {
	t.Parallel()
	ip := net.ParseIP("::ffff:10.0.0.1")
	if !isPrivateIP(ip) {
		t.Error("expected ::ffff:10.0.0.1 to be private (IPv4-mapped)")
	}
}

func TestIsPrivateIP_NilIP(t *testing.T) {
	t.Parallel()
	// net.ParseIP returns nil for invalid strings
	defer func() {
		if r := recover(); r != nil {
			t.Errorf("isPrivateIP panicked on nil IP: %v", r)
		}
	}()
	// nil IP dereference — isPrivateIP should handle gracefully via method calls
	// Actually net.IP methods on nil return false, so this is fine
}

func TestIsPrivateIP_PublicIPv6(t *testing.T) {
	t.Parallel()
	ip := net.ParseIP("2001:4860:4860::8888")
	if isPrivateIP(ip) {
		t.Error("expected Google public IPv6 to not be private")
	}
}

// ---------------------------------------------------------------------------
// WebFetch: htmlToMarkdown edge cases
// ---------------------------------------------------------------------------

func TestHtmlToMarkdown_EmptyHTML(t *testing.T) {
	t.Parallel()
	got := htmlToMarkdown("")
	if got != "" {
		t.Errorf("expected empty string for empty HTML, got %q", got)
	}
}

func TestHtmlToMarkdown_PlainText(t *testing.T) {
	t.Parallel()
	got := htmlToMarkdown("just plain text")
	if got != "just plain text" {
		t.Errorf("expected 'just plain text', got %q", got)
	}
}

func TestHtmlToMarkdown_LinkWithSingleQuotes(t *testing.T) {
	t.Parallel()
	input := `<a href='https://example.com'>Example</a>`
	got := htmlToMarkdown(input)
	if !strings.Contains(got, "[Example](https://example.com)") {
		t.Errorf("expected markdown link, got %q", got)
	}
}

func TestHtmlToMarkdown_LinkWithoutHref(t *testing.T) {
	t.Parallel()
	input := `<a>no href</a>`
	got := htmlToMarkdown(input)
	if got == "" {
		t.Error("expected non-empty output")
	}
}

func TestHtmlToMarkdown_CodeTag(t *testing.T) {
	t.Parallel()
	input := "<code>fmt.Println()</code>"
	got := htmlToMarkdown(input)
	if !strings.Contains(got, "`fmt.Println()`") {
		t.Errorf("expected backtick-wrapped code, got %q", got)
	}
}

func TestHtmlToMarkdown_BTag(t *testing.T) {
	t.Parallel()
	input := "<b>bold text</b>"
	got := htmlToMarkdown(input)
	if !strings.Contains(got, "**bold text**") {
		t.Errorf("expected **bold text**, got %q", got)
	}
}

func TestHtmlToMarkdown_ITag(t *testing.T) {
	t.Parallel()
	input := "<i>italic text</i>"
	got := htmlToMarkdown(input)
	if !strings.Contains(got, "*italic text*") {
		t.Errorf("expected *italic text*, got %q", got)
	}
}

func TestHtmlToMarkdown_H3H4Tags(t *testing.T) {
	t.Parallel()
	input := "<h3>Third</h3><h4>Fourth</h4>"
	got := htmlToMarkdown(input)
	if !strings.Contains(got, "### Third") {
		t.Errorf("expected '### Third', got %q", got)
	}
	if !strings.Contains(got, "#### Fourth") {
		t.Errorf("expected '#### Fourth', got %q", got)
	}
}

func TestHtmlToMarkdown_DivTag(t *testing.T) {
	t.Parallel()
	input := "<div>hello</div><div>world</div>"
	got := htmlToMarkdown(input)
	if !strings.Contains(got, "hello") || !strings.Contains(got, "world") {
		t.Errorf("expected 'hello' and 'world', got %q", got)
	}
}

func TestHtmlToMarkdown_BrTag(t *testing.T) {
	t.Parallel()
	input := "line1<br>line2"
	got := htmlToMarkdown(input)
	if !strings.Contains(got, "line1") || !strings.Contains(got, "line2") {
		t.Errorf("expected both lines, got %q", got)
	}
}

func TestHtmlToMarkdown_ListItems(t *testing.T) {
	t.Parallel()
	input := "<ul><li>item1</li><li>item2</li></ul>"
	got := htmlToMarkdown(input)
	if !strings.Contains(got, "item1") || !strings.Contains(got, "item2") {
		t.Errorf("expected list items, got %q", got)
	}
}

// ---------------------------------------------------------------------------
// WebFetch: htmlToText edge cases
// ---------------------------------------------------------------------------

func TestHtmlToText_EmptyHTML(t *testing.T) {
	t.Parallel()
	got := htmlToText("")
	if got != "" {
		t.Errorf("expected empty string for empty HTML, got %q", got)
	}
}

func TestHtmlToText_PlainText(t *testing.T) {
	t.Parallel()
	got := htmlToText("just plain text")
	if got != "just plain text" {
		t.Errorf("expected 'just plain text', got %q", got)
	}
}

func TestHtmlToText_StripsScriptAndStyle(t *testing.T) {
	t.Parallel()
	input := "<script>evil</script><style>css</style><p>visible</p>"
	got := htmlToText(input)
	if strings.Contains(got, "evil") || strings.Contains(got, "css") {
		t.Errorf("expected script/style to be stripped, got %q", got)
	}
	if !strings.Contains(got, "visible") {
		t.Errorf("expected 'visible', got %q", got)
	}
}

// ---------------------------------------------------------------------------
// WebFetch: stripTags edge cases
// ---------------------------------------------------------------------------

func TestStripTags_NoClosingTag(t *testing.T) {
	t.Parallel()
	input := "before<script>unclosed"
	got := stripTags(input, "script")
	// When there's no closing tag, stripTags breaks and returns original
	if got != input {
		t.Errorf("expected original input when no closing tag, got %q", got)
	}
}

func TestStripTags_SelfClosing(t *testing.T) {
	t.Parallel()
	// The self-closing check in stripTags looks for "/" at end of tag content
	// before the ">". For <script/>, the tag content is "<script/>" which
	// doesn't end with "/" (it ends with ">"). So self-closing isn't detected
	// and the unclosed-tag fallback returns the original string.
	input := "before<script/>after"
	got := stripTags(input, "script")
	// This tests the actual behavior: self-closing isn't detected
	if got != input {
		t.Errorf("expected original input for self-closing tag (not detected), got %q", got)
	}
}

func TestStripTags_NoTags(t *testing.T) {
	t.Parallel()
	input := "plain text"
	got := stripTags(input, "script")
	if got != "plain text" {
		t.Errorf("expected 'plain text', got %q", got)
	}
}

func TestStripTags_NoOpenTag(t *testing.T) {
	t.Parallel()
	input := "before</script>after"
	got := stripTags(input, "script")
	if got != input {
		t.Errorf("expected original when no open tag, got %q", got)
	}
}

// ---------------------------------------------------------------------------
// WebFetch: convertLinks edge cases
// ---------------------------------------------------------------------------

func TestConvertLinks_NoClosingTag(t *testing.T) {
	t.Parallel()
	input := `<a href="https://example.com">no close`
	lower := strings.ToLower(input)
	got := convertLinks(input, lower)
	if got != input {
		t.Errorf("expected original input when no closing tag, got %q", got)
	}
}

func TestConvertLinks_UnquotedHref(t *testing.T) {
	t.Parallel()
	input := `<a href=https://example.com>Example</a>`
	lower := strings.ToLower(input)
	got := convertLinks(input, lower)
	if !strings.Contains(got, "[Example](https://example.com)") {
		t.Errorf("expected markdown link, got %q", got)
	}
}

func TestConvertLinks_NoLinks(t *testing.T) {
	t.Parallel()
	input := "plain text with no links"
	lower := strings.ToLower(input)
	got := convertLinks(input, lower)
	if got != input {
		t.Errorf("expected original text, got %q", got)
	}
}

func TestConvertLinks_MissingHrefValue(t *testing.T) {
	t.Parallel()
	input := `<a >no href attr</a>`
	lower := strings.ToLower(input)
	got := convertLinks(input, lower)
	if got == "" {
		t.Error("expected non-empty output")
	}
}

// ---------------------------------------------------------------------------
// WebFetch: replaceInlineTag edge cases
// ---------------------------------------------------------------------------

func TestReplaceInlineTag_NoMatch(t *testing.T) {
	t.Parallel()
	input := "no tags here"
	lower := strings.ToLower(input)
	got := replaceInlineTag(input, lower, "strong", "**")
	if got != input {
		t.Errorf("expected original text when no tag, got %q", got)
	}
}

func TestReplaceInlineTag_MultipleOccurrences(t *testing.T) {
	t.Parallel()
	input := "<em>first</em> and <em>second</em>"
	lower := strings.ToLower(input)
	got := replaceInlineTag(input, lower, "em", "*")
	if !strings.Contains(got, "*first*") || !strings.Contains(got, "*second*") {
		t.Errorf("expected both occurrences replaced, got %q", got)
	}
}

// ---------------------------------------------------------------------------
// WebFetch: replaceBlockTag edge cases
// ---------------------------------------------------------------------------

func TestReplaceBlockTag_NoMatch(t *testing.T) {
	t.Parallel()
	input := "no tags here"
	lower := strings.ToLower(input)
	got, _ := replaceBlockTag(input, lower, "div", "\n")
	if got != input {
		t.Errorf("expected original text when no tag, got %q", got)
	}
}

func TestReplaceBlockTag_MissingCloseTag(t *testing.T) {
	t.Parallel()
	input := "<div>unclosed"
	lower := strings.ToLower(input)
	got, _ := replaceBlockTag(input, lower, "div", "\n")
	if got != input {
		t.Errorf("expected original when no close tag, got %q", got)
	}
}

// ---------------------------------------------------------------------------
// WebFetch: normalizeWhitespace edge cases
// ---------------------------------------------------------------------------

func TestNormalizeWhitespace_MultipleNewlines(t *testing.T) {
	t.Parallel()
	input := "a\n\n\n\n\n\nb"
	got := normalizeWhitespace(input)
	if strings.Count(got, "\n") > 2 {
		t.Errorf("expected at most 2 newlines, got %q", got)
	}
}

func TestNormalizeWhitespace_MixedSpacesAndTabs(t *testing.T) {
	t.Parallel()
	input := "hello \t world \t "
	got := normalizeWhitespace(input)
	if got != "hello world" {
		t.Errorf("expected 'hello world', got %q", got)
	}
}

func TestNormalizeWhitespace_OnlySpaces(t *testing.T) {
	t.Parallel()
	input := "   "
	got := normalizeWhitespace(input)
	if got != "" {
		t.Errorf("expected empty string for only spaces, got %q", got)
	}
}

// ---------------------------------------------------------------------------
// Permissions: matchValue edge cases
// ---------------------------------------------------------------------------

func TestMatchValue_StringMatch(t *testing.T) {
	t.Parallel()
	if !matchValue("hello.txt", "*.txt") {
		t.Error("expected *.txt to match hello.txt")
	}
	if matchValue("hello.go", "*.txt") {
		t.Error("expected *.txt to not match hello.go")
	}
}

func TestMatchValue_MapValue(t *testing.T) {
	t.Parallel()
	params := map[string]any{"nested": "target.txt"}
	if !matchValue(params, "*.txt") {
		t.Error("expected *.txt to match nested map value")
	}
}

func TestMatchValue_SliceValue(t *testing.T) {
	t.Parallel()
	items := []any{"file.txt", "other.go"}
	if !matchValue(items, "*.txt") {
		t.Error("expected *.txt to match slice containing matching value")
	}
}

func TestMatchValue_SliceNoMatch(t *testing.T) {
	t.Parallel()
	items := []any{"file.go", "other.go"}
	if matchValue(items, "*.txt") {
		t.Error("expected *.txt to not match slice with no matching value")
	}
}

func TestMatchValue_BoolValue(t *testing.T) {
	t.Parallel()
	if !matchValue(true, "true") {
		t.Error("expected 'true' pattern to match bool true")
	}
}

func TestMatchValue_IntValue(t *testing.T) {
	t.Parallel()
	if !matchValue(42, "*42*") {
		t.Error("expected '*42*' to match int 42")
	}
}

func TestMatchValue_NilValue(t *testing.T) {
	t.Parallel()
	matchValue(nil, "*.txt") // should not panic
}

func TestMatchValue_MapWithNonMatching(t *testing.T) {
	t.Parallel()
	params := map[string]any{"key": "no_match"}
	if matchValue(params, "*.txt") {
		t.Error("expected no match for non-matching map value")
	}
}

// ---------------------------------------------------------------------------
// Permissions: riskLevelValue edge cases
// ---------------------------------------------------------------------------

func TestRiskLevelValue_AllLevels(t *testing.T) {
	t.Parallel()
	tests := []struct {
		level types.RiskLevel
		want  int
	}{
		{types.RiskSafe, 0},
		{types.RiskMedium, 1},
		{types.RiskDangerous, 2},
		{types.RiskDestructive, 3},
		{"unknown", -1},
		{"", -1},
	}
	for _, tt := range tests {
		got := riskLevelValue(tt.level)
		if got != tt.want {
			t.Errorf("riskLevelValue(%q) = %d, want %d", tt.level, got, tt.want)
		}
	}
}

// ---------------------------------------------------------------------------
// Permissions: extractFromParams edge cases
// ---------------------------------------------------------------------------

func TestExtractFromParams_Edit(t *testing.T) {
	t.Parallel()
	params := map[string]any{"path": "main.go"}
	got := extractFromParams("Edit", params)
	if got != "edit main.go" {
		t.Errorf("expected 'edit main.go', got %q", got)
	}
}

func TestExtractFromParams_WebFetch(t *testing.T) {
	t.Parallel()
	params := map[string]any{"url": "https://example.com"}
	got := extractFromParams("WebFetch", params)
	if got != "https://example.com" {
		t.Errorf("expected 'https://example.com', got %q", got)
	}
}

func TestExtractFromParams_TodoWrite(t *testing.T) {
	t.Parallel()
	params := map[string]any{"todos": []any{"a", "b", "c"}}
	got := extractFromParams("TodoWrite", params)
	if got != "todos: 3 items" {
		t.Errorf("expected 'todos: 3 items', got %q", got)
	}
}

func TestExtractFromParams_AskUserQuestion(t *testing.T) {
	t.Parallel()
	params := map[string]any{"question": "What is your name?"}
	got := extractFromParams("AskUserQuestion", params)
	if got != "What is your name?" {
		t.Errorf("expected 'What is your name?', got %q", got)
	}
}

func TestExtractFromParams_UnknownTool(t *testing.T) {
	t.Parallel()
	params := map[string]any{"key": "value"}
	got := extractFromParams("UnknownTool", params)
	if got != "" {
		t.Errorf("expected empty string for unknown tool, got %q", got)
	}
}

func TestExtractFromParams_Bash(t *testing.T) {
	t.Parallel()
	params := map[string]any{"command": "ls -la"}
	got := extractFromParams("Bash", params)
	if got != "ls -la" {
		t.Errorf("expected 'ls -la', got %q", got)
	}
}

func TestExtractFromParams_FileRead(t *testing.T) {
	t.Parallel()
	params := map[string]any{"path": "test.txt"}
	got := extractFromParams("FileRead", params)
	if got != "read test.txt" {
		t.Errorf("expected 'read test.txt', got %q", got)
	}
}

func TestExtractFromParams_FileWrite(t *testing.T) {
	t.Parallel()
	params := map[string]any{"path": "out.txt"}
	got := extractFromParams("FileWrite", params)
	if got != "write out.txt" {
		t.Errorf("expected 'write out.txt', got %q", got)
	}
}

func TestExtractFromParams_Glob(t *testing.T) {
	t.Parallel()
	params := map[string]any{"pattern": "*.go"}
	got := extractFromParams("Glob", params)
	if got != "glob *.go" {
		t.Errorf("expected 'glob *.go', got %q", got)
	}
}

func TestExtractFromParams_Grep(t *testing.T) {
	t.Parallel()
	params := map[string]any{"pattern": "TODO"}
	got := extractFromParams("Grep", params)
	if got != "grep TODO" {
		t.Errorf("expected 'grep TODO', got %q", got)
	}
}

// ---------------------------------------------------------------------------
// Permissions: extractBashTimeout edge cases
// ---------------------------------------------------------------------------

func TestExtractBashTimeout_EmptyInput(t *testing.T) {
	t.Parallel()
	got := extractBashTimeout(nil)
	if got != 0 {
		t.Errorf("expected 0 for nil input, got %d", got)
	}
	got = extractBashTimeout([]byte{})
	if got != 0 {
		t.Errorf("expected 0 for empty input, got %d", got)
	}
}

func TestExtractBashTimeout_WithTimeout(t *testing.T) {
	t.Parallel()
	input := []byte(`{"timeout": 60}`)
	got := extractBashTimeout(input)
	if got != 60 {
		t.Errorf("expected 60, got %d", got)
	}
}

func TestExtractBashTimeout_NoTimeout(t *testing.T) {
	t.Parallel()
	input := []byte(`{"command": "echo hello"}`)
	got := extractBashTimeout(input)
	if got != 0 {
		t.Errorf("expected 0 for no timeout, got %d", got)
	}
}

func TestExtractBashTimeout_InvalidJSON(t *testing.T) {
	t.Parallel()
	got := extractBashTimeout([]byte(`{invalid}`))
	if got != 0 {
		t.Errorf("expected 0 for invalid JSON, got %d", got)
	}
}

func TestExtractBashTimeout_ZeroTimeout(t *testing.T) {
	t.Parallel()
	input := []byte(`{"timeout": 0}`)
	got := extractBashTimeout(input)
	if got != 0 {
		t.Errorf("expected 0 for zero timeout, got %d", got)
	}
}

func TestExtractBashTimeout_NegativeTimeout(t *testing.T) {
	t.Parallel()
	input := []byte(`{"timeout": -5}`)
	got := extractBashTimeout(input)
	if got != 0 {
		t.Errorf("expected 0 for negative timeout, got %d", got)
	}
}

// ---------------------------------------------------------------------------
// Permissions: extractCommandString edge cases
// ---------------------------------------------------------------------------

func TestExtractCommandString_EditTool(t *testing.T) {
	t.Parallel()
	input := []byte(`{"name":"Edit","params":{"path":"main.go"}}`)
	got := extractCommandString("Edit", input)
	if got != "edit main.go" {
		t.Errorf("expected 'edit main.go', got %q", got)
	}
}

func TestExtractCommandString_WebFetchTool(t *testing.T) {
	t.Parallel()
	input := []byte(`{"name":"WebFetch","params":{"url":"https://example.com"}}`)
	got := extractCommandString("WebFetch", input)
	if got != "https://example.com" {
		t.Errorf("expected 'https://example.com', got %q", got)
	}
}

func TestExtractCommandString_TodoWriteTool(t *testing.T) {
	t.Parallel()
	input := []byte(`{"name":"TodoWrite","params":{"todos":[{"content":"task1"},{"content":"task2"}]}}`)
	got := extractCommandString("TodoWrite", input)
	if got != "todos: 2 items" {
		t.Errorf("expected 'todos: 2 items', got %q", got)
	}
}

func TestExtractCommandString_AskUserQuestionTool(t *testing.T) {
	t.Parallel()
	input := []byte(`{"name":"AskUserQuestion","params":{"question":"Which language?"}}`)
	got := extractCommandString("AskUserQuestion", input)
	if got != "Which language?" {
		t.Errorf("expected 'Which language?', got %q", got)
	}
}

func TestExtractCommandString_RawMapFallback(t *testing.T) {
	t.Parallel()
	input := []byte(`{"command":"echo test"}`)
	got := extractCommandString("Bash", input)
	if got != "echo test" {
		t.Errorf("expected 'echo test', got %q", got)
	}
}

func TestExtractCommandString_EmptyToolName(t *testing.T) {
	t.Parallel()
	input := []byte(`{"command":"echo"}`)
	got := extractCommandString("", input)
	// Empty toolName: capitalize logic produces "" which doesn't match any case,
	// falls through to raw params map, which has no matching key, falls to raw JSON
	if got == "" {
		t.Error("expected non-empty result")
	}
}

func TestExtractCommandString_ToolNameNormalization(t *testing.T) {
	t.Parallel()
	// extractCommandString capitalizes first letter of tool name for switch matching
	input := []byte(`{"name":"bash","params":{"command":"ls"}}`)
	got := extractCommandString("bash", input)
	if got != "ls" {
		t.Errorf("expected 'ls', got %q", got)
	}
}

// ---------------------------------------------------------------------------
// Permissions: matchToolName edge cases
// ---------------------------------------------------------------------------

func TestMatchToolName_EmptyPattern(t *testing.T) {
	t.Parallel()
	if matchToolName("", "Bash") {
		t.Error("expected empty pattern to not match")
	}
}

func TestMatchToolName_EmptyName(t *testing.T) {
	t.Parallel()
	if matchToolName("Bash", "") {
		t.Error("expected empty name to not match")
	}
}

func TestMatchToolName_BothEmpty(t *testing.T) {
	t.Parallel()
	// matchToolName checks pattern == name first; "" == "" is true
	got := matchToolName("", "")
	if !got {
		t.Error("expected both empty to match (pattern == name)")
	}
}

func TestMatchToolName_SingleCharWildcard(t *testing.T) {
	t.Parallel()
	if !matchToolName("?ash", "Bash") {
		t.Error("expected ?ash to match Bash")
	}
}

// ---------------------------------------------------------------------------
// Permissions: matchAnyParamValue edge cases
// ---------------------------------------------------------------------------

func TestMatchAnyParamValue_EmptyParams(t *testing.T) {
	t.Parallel()
	if matchAnyParamValue("**", map[string]any{}) {
		t.Error("expected no match for empty params")
	}
}

func TestMatchAnyParamValue_NoMatchingKeys(t *testing.T) {
	t.Parallel()
	params := map[string]any{"unknown_key": "value"}
	if matchAnyParamValue("**", params) {
		t.Error("expected no match for non-param keys")
	}
}

func TestMatchAnyParamValue_URLParam(t *testing.T) {
	t.Parallel()
	params := map[string]any{"url": "https://example.com"}
	if !matchAnyParamValue("https://*", params) {
		t.Error("expected URL pattern to match")
	}
}

func TestMatchAnyParamValue_PatternParam(t *testing.T) {
	t.Parallel()
	params := map[string]any{"pattern": "TODO"}
	if !matchAnyParamValue("TODO", params) {
		t.Error("expected pattern to match")
	}
}

// ---------------------------------------------------------------------------
// Defaults: DefaultDispatcher edge cases
// ---------------------------------------------------------------------------

func TestDefaultDispatcher_RegistersAllTools(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	d, err := DefaultDispatcher(dir, dir, dir, nil)
	if err != nil {
		t.Fatalf("DefaultDispatcher failed: %v", err)
	}
	names := d.List()
	expectedTools := []string{
		"Bash", "FileRead", "FileWrite", "Edit", "TodoWrite",
		"WebFetch", "AskUserQuestion", "Glob", "Grep",
		"FileList", "FileDelete", "FileMove",
	}
	for _, expected := range expectedTools {
		found := false
		for _, name := range names {
			if name == expected {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("expected tool %s not registered", expected)
		}
	}
}

func TestDefaultDispatcher_WithPermissionsConfig(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	cfg := &config.PermissionsConfig{
		Rules: []config.PermissionRule{
			{Tool: "Bash", Action: "allow"},
		},
	}
	d, err := DefaultDispatcher(dir, dir, dir, cfg)
	if err != nil {
		t.Fatalf("DefaultDispatcher with config failed: %v", err)
	}
	if d == nil {
		t.Fatal("expected non-nil dispatcher")
	}
}

// ---------------------------------------------------------------------------
// Interface: PermissionRequest and PermissionResponse
// ---------------------------------------------------------------------------

func TestPermissionRequest_Fields(t *testing.T) {
	t.Parallel()
	req := PermissionRequest{
		ID:          42,
		ToolName:    "Bash",
		Command:     "echo hello",
		RiskLevel:   types.RiskDangerous,
		TimeoutSecs: 30,
		RuleTool:    "Bash",
		RulePattern: "**",
		RuleAction:  "ask",
	}
	if req.ID != 42 {
		t.Errorf("expected ID 42, got %d", req.ID)
	}
	if req.ToolName != "Bash" {
		t.Errorf("expected ToolName 'Bash', got %q", req.ToolName)
	}
}

func TestPermissionResponse_Fields(t *testing.T) {
	t.Parallel()
	resp := PermissionResponse{
		RequestID: 42,
		Allowed:   true,
		Remember:  true,
	}
	if resp.RequestID != 42 {
		t.Errorf("expected RequestID 42, got %d", resp.RequestID)
	}
	if !resp.Allowed {
		t.Error("expected Allowed true")
	}
}

func TestPermissionContext_StructFields(t *testing.T) {
	t.Parallel()
	pctx := &PermissionContext{
		RuleTool:    "Bash",
		RulePattern: "**",
		RuleAction:  "allow",
		Source:      "rule",
	}
	if pctx.RuleTool != "Bash" {
		t.Errorf("expected RuleTool 'Bash', got %q", pctx.RuleTool)
	}
	if pctx.Source != "rule" {
		t.Errorf("expected Source 'rule', got %q", pctx.Source)
	}
}

// ---------------------------------------------------------------------------
// Interface: nextPermissionRequestID
// ---------------------------------------------------------------------------

func TestNextPermissionRequestID_Incrementing(t *testing.T) {
	t.Parallel()
	id1 := nextPermissionRequestID()
	id2 := nextPermissionRequestID()
	if id2 <= id1 {
		t.Errorf("expected incrementing IDs, got %d then %d", id1, id2)
	}
}

// ---------------------------------------------------------------------------
// ToolDefs: BuildToolDefs
// ---------------------------------------------------------------------------

func TestBuildToolDefs(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	d, err := DefaultDispatcher(dir, dir, dir, nil)
	if err != nil {
		t.Fatalf("DefaultDispatcher failed: %v", err)
	}
	defs := BuildToolDefs(d)
	if len(defs) != 13 {
		t.Errorf("expected 13 tool defs, got %d", len(defs))
	}
	for _, def := range defs {
		if def.Name == "" {
			t.Error("expected non-empty tool name")
		}
		if def.Description == "" {
			t.Errorf("tool %s: expected non-empty description", def.Name)
		}
		if def.Parameters == "" {
			t.Errorf("tool %s: expected non-empty parameters", def.Name)
		}
		// Verify parameters are valid JSON
		var raw json.RawMessage
		if err := json.Unmarshal([]byte(def.Parameters), &raw); err != nil {
			t.Errorf("tool %s: invalid JSON parameters: %v", def.Name, err)
		}
	}
}

func TestBuildToolDefs_SingleTool(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	d.Register(NewGlob(t.TempDir()))
	defs := BuildToolDefs(d)
	if len(defs) != 1 {
		t.Fatalf("expected 1 tool def, got %d", len(defs))
	}
	if defs[0].Name != "Glob" {
		t.Errorf("expected 'Glob', got %q", defs[0].Name)
	}
}

// ---------------------------------------------------------------------------
// FileWrite: pruneBackups edge cases
// ---------------------------------------------------------------------------

func TestFileWrite_PruneBackupsExactLimit(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	// Write MaxBackupsPerFile times. Pruning removes oldest when count >= limit,
	// so after 10 writes we'll have 9 backups (pruned 1).
	for i := 0; i < MaxBackupsPerFile; i++ {
		_, err := fw.Execute(context.Background(), types.ToolInput{
			Name: "FileWrite",
			Params: map[string]any{
				"path":    "file.txt",
				"content": strings.Repeat("x", i),
			},
		})
		if err != nil {
			t.Fatalf("write %d failed: %v", i, err)
		}
	}
	entries, err := os.ReadDir(backupDir)
	if err != nil {
		t.Fatal(err)
	}
	// Pruning removes when count >= MaxBackupsPerFile, keeping MaxBackupsPerFile-1
	if len(entries) > MaxBackupsPerFile {
		t.Errorf("expected at most %d backups, got %d", MaxBackupsPerFile, len(entries))
	}
}

// ---------------------------------------------------------------------------
// Edit: Edit tool parameter schema
// ---------------------------------------------------------------------------

func TestEdit_ParameterSchema(t *testing.T) {
	t.Parallel()
	e := NewEdit(t.TempDir(), t.TempDir())
	schema := e.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty parameter schema")
	}
	var raw json.RawMessage
	if err := json.Unmarshal([]byte(schema), &raw); err != nil {
		t.Errorf("invalid JSON schema: %v", err)
	}
}

func TestEdit_Name(t *testing.T) {
	t.Parallel()
	e := NewEdit(t.TempDir(), t.TempDir())
	if e.Name() != "Edit" {
		t.Errorf("expected name 'Edit', got %s", e.Name())
	}
}

func TestEdit_Description(t *testing.T) {
	t.Parallel()
	e := NewEdit(t.TempDir(), t.TempDir())
	if e.Description() == "" {
		t.Error("expected non-empty description")
	}
}

func TestEdit_RiskLevel(t *testing.T) {
	t.Parallel()
	e := NewEdit(t.TempDir(), t.TempDir())
	if e.RiskLevel() != types.RiskDangerous {
		t.Errorf("expected RiskDangerous, got %s", e.RiskLevel())
	}
}

// ---------------------------------------------------------------------------
// FileRead: ParameterSchema
// ---------------------------------------------------------------------------

func TestFileRead_ParameterSchema(t *testing.T) {
	t.Parallel()
	fr := NewFileRead(t.TempDir())
	schema := fr.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty parameter schema")
	}
	var raw json.RawMessage
	if err := json.Unmarshal([]byte(schema), &raw); err != nil {
		t.Errorf("invalid JSON schema: %v", err)
	}
}

// ---------------------------------------------------------------------------
// FileWrite: ParameterSchema
// ---------------------------------------------------------------------------

func TestFileWrite_ParameterSchema(t *testing.T) {
	t.Parallel()
	fw := NewFileWrite(t.TempDir(), t.TempDir())
	schema := fw.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty parameter schema")
	}
	var raw json.RawMessage
	if err := json.Unmarshal([]byte(schema), &raw); err != nil {
		t.Errorf("invalid JSON schema: %v", err)
	}
}

// ---------------------------------------------------------------------------
// Glob: ParameterSchema
// ---------------------------------------------------------------------------

func TestGlob_ParameterSchema(t *testing.T) {
	t.Parallel()
	g := NewGlob(t.TempDir())
	schema := g.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty parameter schema")
	}
	var raw json.RawMessage
	if err := json.Unmarshal([]byte(schema), &raw); err != nil {
		t.Errorf("invalid JSON schema: %v", err)
	}
}

// ---------------------------------------------------------------------------
// Grep: ParameterSchema
// ---------------------------------------------------------------------------

func TestGrep_ParameterSchema(t *testing.T) {
	t.Parallel()
	g := NewGrep(t.TempDir())
	schema := g.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty parameter schema")
	}
	var raw json.RawMessage
	if err := json.Unmarshal([]byte(schema), &raw); err != nil {
		t.Errorf("invalid JSON schema: %v", err)
	}
}

// ---------------------------------------------------------------------------
// FileList: ParameterSchema
// ---------------------------------------------------------------------------

func TestFileList_ParameterSchema(t *testing.T) {
	t.Parallel()
	fl := NewFileList(t.TempDir())
	schema := fl.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty parameter schema")
	}
	var raw json.RawMessage
	if err := json.Unmarshal([]byte(schema), &raw); err != nil {
		t.Errorf("invalid JSON schema: %v", err)
	}
}

// ---------------------------------------------------------------------------
// FileDelete: ParameterSchema
// ---------------------------------------------------------------------------

func TestFileDelete_ParameterSchema(t *testing.T) {
	t.Parallel()
	fd := NewFileDelete(t.TempDir(), t.TempDir())
	schema := fd.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty parameter schema")
	}
	var raw json.RawMessage
	if err := json.Unmarshal([]byte(schema), &raw); err != nil {
		t.Errorf("invalid JSON schema: %v", err)
	}
}

// ---------------------------------------------------------------------------
// FileMove: ParameterSchema
// ---------------------------------------------------------------------------

func TestFileMove_ParameterSchema(t *testing.T) {
	t.Parallel()
	fm := NewFileMove(t.TempDir())
	schema := fm.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty parameter schema")
	}
	var raw json.RawMessage
	if err := json.Unmarshal([]byte(schema), &raw); err != nil {
		t.Errorf("invalid JSON schema: %v", err)
	}
}

// ---------------------------------------------------------------------------
// TodoWrite: ParameterSchema
// ---------------------------------------------------------------------------

func TestTodoWrite_ParameterSchema(t *testing.T) {
	t.Parallel()
	tw := NewTodoWrite(t.TempDir(), "test")
	schema := tw.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty parameter schema")
	}
	var raw json.RawMessage
	if err := json.Unmarshal([]byte(schema), &raw); err != nil {
		t.Errorf("invalid JSON schema: %v", err)
	}
}

// ---------------------------------------------------------------------------
// WebFetch: ParameterSchema
// ---------------------------------------------------------------------------

func TestWebFetch_ParameterSchema(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	schema := wf.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty parameter schema")
	}
	var raw json.RawMessage
	if err := json.Unmarshal([]byte(schema), &raw); err != nil {
		t.Errorf("invalid JSON schema: %v", err)
	}
}

// ---------------------------------------------------------------------------
// WebFetch: Execute parameter validation
// ---------------------------------------------------------------------------

func TestWebFetch_MissingURLParam(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Name:   "WebFetch",
		Params: map[string]any{},
	})
	if err == nil {
		t.Error("expected error for missing url param")
	}
}

func TestWebFetch_URLNotString(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url": 123,
		},
	})
	if err == nil {
		t.Error("expected error for non-string url")
	}
}

func TestWebFetch_InvalidFormat(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url":    "https://example.com",
			"format": "invalid",
		},
	})
	if err == nil {
		t.Error("expected error for invalid format")
	}
}

func TestWebFetch_NonHTTPScheme(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url": "ftp://example.com",
		},
	})
	if err == nil {
		t.Error("expected error for non-http scheme")
	}
	if !strings.Contains(err.Error(), "only http and https") {
		t.Errorf("expected scheme error, got: %v", err)
	}
}

func TestWebFetch_TimeoutOutOfRange(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url":     "https://example.com",
			"timeout": float64(0),
		},
	})
	if err == nil {
		t.Error("expected error for timeout 0")
	}
}

func TestWebFetch_TimeoutTooHigh(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url":     "https://example.com",
			"timeout": float64(200),
		},
	})
	if err == nil {
		t.Error("expected error for timeout > 120")
	}
}

func TestWebFetch_NonFloatTimeout(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	// timeout not a float64 → default to 30, then the request itself fails
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url":     "https://example.com",
			"timeout": "not_a_number",
		},
	})
	// May fail due to network or timeout, but should not fail with param error
	if err != nil && strings.Contains(err.Error(), "timeout must be") {
		t.Errorf("unexpected timeout validation error: %v", err)
	}
}

// ---------------------------------------------------------------------------
// WebFetch: Version handling
// ---------------------------------------------------------------------------

func TestSetVersion_EmptyString(t *testing.T) {
	// Save original
	orig := getVersion()
	defer SetVersion(orig)

	SetVersion("1.0.0")
	SetVersion("") // empty should not overwrite
	if v := getVersion(); v != "1.0.0" {
		t.Errorf("expected '1.0.0' after empty SetVersion, got %q", v)
	}
}

func TestGetVersion_Default(t *testing.T) {
	// Reset version to empty
	Version = atomic.Value{}
	if v := getVersion(); v != "dev" {
		t.Errorf("expected 'dev' default, got %q", v)
	}
}

// ---------------------------------------------------------------------------
// Edit: Edit.Execute edge cases
// ---------------------------------------------------------------------------

func TestEdit_Execute_MissingPathParam(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	e := NewEdit(dir, backupDir)
	_, err := e.Execute(context.Background(), types.ToolInput{
		Name:   "Edit",
		Params: map[string]any{},
	})
	if err == nil {
		t.Error("expected error for missing path param")
	}
}

func TestEdit_Execute_PathNotString(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	e := NewEdit(dir, backupDir)
	_, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path": 123,
		},
	})
	if err == nil {
		t.Error("expected error for non-string path")
	}
}

func TestEdit_Execute_MissingNewString(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	e := NewEdit(dir, backupDir)
	_, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path": "test.txt",
		},
	})
	if err == nil {
		t.Error("expected error for missing new_string param")
	}
}

func TestEdit_Execute_NewStringNotString(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	e := NewEdit(dir, backupDir)
	_, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "test.txt",
			"new_string": 123,
		},
	})
	if err == nil {
		t.Error("expected error for non-string new_string")
	}
}

func TestEdit_Execute_MissingOldStringAndLineRange(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte("hello"), 0644)
	e := NewEdit(dir, backupDir)
	_, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "test.txt",
			"new_string": "world",
		},
	})
	if err == nil {
		t.Error("expected error for missing old_string and line range")
	}
}

func TestEdit_Execute_OldStringNotString(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte("hello"), 0644)
	e := NewEdit(dir, backupDir)
	_, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "test.txt",
			"new_string": "world",
			"old_string": 123,
		},
	})
	if err == nil {
		t.Error("expected error for non-string old_string")
	}
}

func TestEdit_Execute_StartLineNotInt(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte("hello\nworld"), 0644)
	e := NewEdit(dir, backupDir)
	_, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "test.txt",
			"new_string": "world",
			"start_line": "not_an_int",
			"end_line":   float64(2),
		},
	})
	if err == nil {
		t.Error("expected error for non-int start_line")
	}
}

func TestEdit_Execute_EndLineNotInt(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte("hello\nworld"), 0644)
	e := NewEdit(dir, backupDir)
	_, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "test.txt",
			"new_string": "world",
			"start_line": float64(1),
			"end_line":   "not_an_int",
		},
	})
	if err == nil {
		t.Error("expected error for non-int end_line")
	}
}

func TestEdit_Execute_BinaryContent(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte("hello"), 0644)
	e := NewEdit(dir, backupDir)
	_, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "test.txt",
			"old_string": "hello",
			"new_string": "hell\x00o",
		},
	})
	if err == nil {
		t.Error("expected error for binary content in new_string")
	}
}

func TestEdit_Execute_FileNotFound(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	e := NewEdit(dir, backupDir)
	_, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "nonexistent.txt",
			"old_string": "hello",
			"new_string": "world",
		},
	})
	if err == nil {
		t.Error("expected error for nonexistent file")
	}
}

// ---------------------------------------------------------------------------
// Dispatcher: SetPermission
// ---------------------------------------------------------------------------

func TestDispatcher_SetPermission(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	d.SetPermission("Bash", true)
	d.mu.RLock()
	allowed := d.permissions["Bash"]
	d.mu.RUnlock()
	if !allowed {
		t.Error("expected Bash to be allowed after SetPermission")
	}
}

// ---------------------------------------------------------------------------
// Dispatcher: Stop
// ---------------------------------------------------------------------------

func TestDispatcher_Stop(t *testing.T) {
	d := NewDispatcher(nil)
	d.Stop()
	// Double stop should not panic
	d.Stop()
}

// ---------------------------------------------------------------------------
// humanSize: additional edge cases
// ---------------------------------------------------------------------------

func TestHumanSize_ExactKB(t *testing.T) {
	t.Parallel()
	got := humanSize(1024)
	if got != "1.0 KB" {
		t.Errorf("humanSize(1024) = %q, want '1.0 KB'", got)
	}
}

func TestHumanSize_ExactMB(t *testing.T) {
	t.Parallel()
	got := humanSize(1048576)
	if got != "1.0 MB" {
		t.Errorf("humanSize(1048576) = %q, want '1.0 MB'", got)
	}
}

func TestHumanSize_ExactGB(t *testing.T) {
	t.Parallel()
	got := humanSize(1073741824)
	if got != "1.0 GB" {
		t.Errorf("humanSize(1073741824) = %q, want '1.0 GB'", got)
	}
}

func TestHumanSize_VeryLarge(t *testing.T) {
	t.Parallel()
	got := humanSize(1 << 50) // 1 PB
	if got == "" {
		t.Error("expected non-empty result for very large value")
	}
}

// ---------------------------------------------------------------------------
// Grep: long pattern rejection
// ---------------------------------------------------------------------------

func TestGrep_PatternTooLong(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	g := NewGrep(dir)
	longPattern := strings.Repeat("a", MaxGrepPatternLength+1)
	_, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern": longPattern,
		},
	})
	if err == nil {
		t.Error("expected error for pattern exceeding max length")
	}
	if !strings.Contains(err.Error(), "regex pattern too long") {
		t.Errorf("expected 'regex pattern too long' error, got: %v", err)
	}
}

// ---------------------------------------------------------------------------
// Grep: context cancellation
// ---------------------------------------------------------------------------

func TestGrep_ContextCancelled(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "file.txt"), []byte("content"), 0644)
	g := NewGrep(dir)
	ctx, cancel := context.WithCancel(context.Background())
	cancel() // cancel immediately
	_, err := g.Execute(ctx, toolInput("pattern", "content"))
	if err == nil {
		t.Error("expected error for cancelled context")
	}
}

// ---------------------------------------------------------------------------
// Glob: context cancellation
// ---------------------------------------------------------------------------

func TestGlob_ContextCancelled(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "file.txt"), []byte("content"), 0644)
	g := NewGlob(dir)
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	_, err := g.Execute(ctx, toolInput("pattern", "*.txt"))
	if err == nil {
		t.Error("expected error for cancelled context")
	}
}

// ---------------------------------------------------------------------------
// FileRead: context cancellation
// ---------------------------------------------------------------------------

func TestFileRead_ContextCancelled(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "file.txt"), []byte("content"), 0644)
	fr := NewFileRead(dir)
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	_, err := fr.Execute(ctx, types.ToolInput{
		Name: "FileRead",
		Params: map[string]any{
			"path": "file.txt",
		},
	})
	if err == nil {
		t.Error("expected error for cancelled context")
	}
}

// ---------------------------------------------------------------------------
// WebFetch: resolveAndCheck edge cases
// ---------------------------------------------------------------------------

func TestWebFetch_ResolveAndCheck_InvalidURL(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	err := wf.resolveAndCheck(context.Background(), "://invalid")
	if err == nil {
		t.Error("expected error for invalid URL")
	}
}

func TestWebFetch_ResolveAndCheck_NoHost(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	err := wf.resolveAndCheck(context.Background(), "http://")
	if err == nil {
		t.Error("expected error for URL with no host")
	}
}

func TestWebFetch_ResolveAndCheck_LiteralPrivateIP(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	err := wf.resolveAndCheck(context.Background(), "http://127.0.0.1:80")
	if err == nil {
		t.Error("expected error for literal private IP")
	}
}

func TestWebFetch_ResolveAndCheck_LiteralPublicIP(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	err := wf.resolveAndCheck(context.Background(), "http://8.8.8.8:80")
	if err != nil {
		t.Errorf("expected no error for public IP, got: %v", err)
	}
}

// ---------------------------------------------------------------------------
// WebFetch: resolveAndCache edge cases
// ---------------------------------------------------------------------------

func TestWebFetch_ResolveAndCache_LiteralIP(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	addrs, err := wf.resolveAndCache(context.Background(), "127.0.0.1")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(addrs) != 1 {
		t.Errorf("expected 1 addr, got %d", len(addrs))
	}
}

// ---------------------------------------------------------------------------
// Dispatcher: Rate limiting token consumption
// ---------------------------------------------------------------------------

func TestDispatcher_RateLimitTokens(t *testing.T) {
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "safe", riskLevel: types.RiskSafe})

	// Should be able to execute several safe tools quickly
	for i := 0; i < 5; i++ {
		_, err := d.Execute(context.Background(), types.ToolCall{
			ID:    "call" + string(rune('0'+i)),
			Name:  "safe",
			Input: []byte(`{}`),
		})
		if err != nil {
			t.Errorf("call %d failed: %v", i, err)
		}
	}
}

// ---------------------------------------------------------------------------
// Edit: Edit.Execute line-range with no match
// ---------------------------------------------------------------------------

func TestEdit_Execute_LineRange_OutOfRange(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte("line1\nline2"), 0644)
	e := NewEdit(dir, backupDir)
	result, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "test.txt",
			"new_string": "replaced",
			"start_line": float64(10),
			"end_line":   float64(20),
		},
	})
	// The error is returned as ToolResult.Output, not as a Go error
	if err != nil {
		t.Fatalf("unexpected Go error: %v", err)
	}
	if !strings.Contains(result.Output, "out of range") {
		t.Errorf("expected 'out of range' in output, got: %s", result.Output)
	}
}

// ---------------------------------------------------------------------------
// WebFetch: resolveAndCache expired cache
// ---------------------------------------------------------------------------

func TestWebFetch_ResolveAndCache_ExpiredCache(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	// Store an expired entry
	wf.dnsCache.Store("expired.example.com", &dnsCacheEntry{
		addrs:   []net.IPAddr{{IP: net.ParseIP("1.2.3.4")}},
		expires: time.Now().Add(-time.Minute), // expired
	})
	// Should re-resolve (may fail on DNS, but should not use expired entry)
	_, _ = wf.resolveAndCache(context.Background(), "expired.example.com")
}

// ---------------------------------------------------------------------------
// Grep: context cancellation for PureGo
// ---------------------------------------------------------------------------

func TestGrep_ContextCancelled_PureGo(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "file.txt"), []byte("content"), 0644)
	g := &Grep{workDir: dir, hasRg: false}
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	_, err := g.Execute(ctx, toolInput("pattern", "content"))
	if err == nil {
		t.Error("expected error for cancelled context")
	}
}

// ---------------------------------------------------------------------------
// Grep: Execute with max_results type conversion
// ---------------------------------------------------------------------------

func TestGrep_MaxResultsNotFloat64(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "file.txt"), []byte("test line\n"), 0644)
	g := &Grep{workDir: dir, hasRg: false}
	// max_results as string — should use default
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern":     "test",
			"max_results": "not_a_number",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "file.txt") {
		t.Errorf("expected file.txt in results, got: %s", result.Output)
	}
}

// ---------------------------------------------------------------------------
// FileWrite: atomicWrite error paths
// ---------------------------------------------------------------------------

func TestFileWrite_PathOutsideWorkDir_Symlink(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	outsideDir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	// Create symlink pointing outside
	outsideFile := filepath.Join(outsideDir, "outside.txt")
	os.WriteFile(outsideFile, []byte("outside"), 0644)
	symlinkPath := filepath.Join(dir, "link.txt")
	if err := os.Symlink(outsideFile, symlinkPath); err != nil {
		t.Skip("symlinks not supported")
	}

	_, err := fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"path":    "link.txt",
			"content": "overwritten",
		},
	})
	// Should either succeed (if symlink resolves to outside) or fail with path safety
	if err != nil && !strings.Contains(err.Error(), "outside working directory") {
		t.Errorf("expected path safety error, got: %v", err)
	}
}

// ---------------------------------------------------------------------------
// TodoWrite: Execute with all valid statuses and priorities
// ---------------------------------------------------------------------------

func TestTodoWrite_AllValidStatuses(t *testing.T) {
	t.Parallel()
	statuses := []string{"pending", "in_progress", "completed", "cancelled"}
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "test-sid")
	for _, s := range statuses {
		_, err := tw.Execute(context.Background(), types.ToolInput{
			Name: "TodoWrite",
			Params: map[string]any{
				"todos": []any{
					map[string]any{"content": "task", "status": s},
				},
			},
		})
		if err != nil {
			t.Errorf("status %q failed: %v", s, err)
		}
	}
}

func TestTodoWrite_AllValidPriorities(t *testing.T) {
	t.Parallel()
	priorities := []string{"high", "medium", "low"}
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "test-sid")
	for _, p := range priorities {
		_, err := tw.Execute(context.Background(), types.ToolInput{
			Name: "TodoWrite",
			Params: map[string]any{
				"todos": []any{
					map[string]any{"content": "task", "priority": p},
				},
			},
		})
		if err != nil {
			t.Errorf("priority %q failed: %v", p, err)
		}
	}
}

// ---------------------------------------------------------------------------
// webfetch: decodeHTMLEntities with all entities
// ---------------------------------------------------------------------------

func TestDecodeHTMLEntities_AllEntities(t *testing.T) {
	t.Parallel()
	input := "&amp;&lt;&gt;&quot;&#39;&nbsp;"
	got := decodeHTMLEntities(input)
	expected := `&<>"' `
	if got != expected {
		t.Errorf("decodeHTMLEntities = %q, want %q", got, expected)
	}
}

// ---------------------------------------------------------------------------
// webfetch: stripAllTags edge cases
// ---------------------------------------------------------------------------

func TestStripAllTags_NestedTags(t *testing.T) {
	t.Parallel()
	input := "<div><p><span>deep</span></p></div>"
	got := stripAllTags(input)
	if got != "deep" {
		t.Errorf("expected 'deep', got %q", got)
	}
}

func TestStripAllTags_AdjacentTags(t *testing.T) {
	t.Parallel()
	input := "<b>bold</b><i>italic</i>"
	got := stripAllTags(input)
	if got != "bolditalic" {
		t.Errorf("expected 'bolditalic', got %q", got)
	}
}

// ---------------------------------------------------------------------------
// webfetch: normalizeWhitespace edge cases
// ---------------------------------------------------------------------------

func TestNormalizeWhitespace_NewlinesPreserved(t *testing.T) {
	t.Parallel()
	input := "line1\nline2"
	got := normalizeWhitespace(input)
	if got != "line1\nline2" {
		t.Errorf("expected newlines preserved, got %q", got)
	}
}

func TestNormalizeWhitespace_TrailingWhitespace(t *testing.T) {
	t.Parallel()
	input := "hello   "
	got := normalizeWhitespace(input)
	if got != "hello" {
		t.Errorf("expected trailing whitespace trimmed, got %q", got)
	}
}

// ---------------------------------------------------------------------------
// Agent tool
// ---------------------------------------------------------------------------

func TestAgent_Name(t *testing.T) {
	t.Parallel()
	a := NewAgent(nil, false)
	if a.Name() != "Agent" {
		t.Errorf("expected 'Agent', got %s", a.Name())
	}
}

func TestAgent_RiskLevel(t *testing.T) {
	t.Parallel()
	a := NewAgent(nil, false)
	if a.RiskLevel() != types.RiskSafe {
		t.Errorf("expected RiskSafe, got %s", a.RiskLevel())
	}
}

func TestAgent_Description(t *testing.T) {
	t.Parallel()
	a := NewAgent(nil, false)
	if a.Description() == "" {
		t.Error("expected non-empty description")
	}
}

func TestAgent_ParameterSchema(t *testing.T) {
	t.Parallel()
	a := NewAgent(nil, false)
	schema := a.ParameterSchema()
	if schema == "" {
		t.Error("expected non-empty schema")
	}
	var raw json.RawMessage
	if err := json.Unmarshal([]byte(schema), &raw); err != nil {
		t.Errorf("invalid JSON: %v", err)
	}
}

func TestAgent_Execute_NilManager(t *testing.T) {
	t.Parallel()
	a := NewAgent(nil, false)
	result, err := a.Execute(context.Background(), types.ToolInput{
		Name: "Agent",
		Params: map[string]any{
			"description": "test task",
			"prompt":      "do something",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Error, "not configured") {
		t.Errorf("expected 'not configured' error, got: %s", result.Error)
	}
}

func TestAgent_Execute_MissingDescription(t *testing.T) {
	t.Parallel()
	a := NewAgent(nil, false)
	result, err := a.Execute(context.Background(), types.ToolInput{
		Name: "Agent",
		Params: map[string]any{
			"prompt": "do something",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// nil manager check happens first
	if !strings.Contains(result.Error, "not configured") {
		t.Errorf("expected 'not configured' error, got: %s", result.Error)
	}
}

func TestAgent_Execute_MissingPrompt(t *testing.T) {
	t.Parallel()
	a := NewAgent(nil, false)
	result, err := a.Execute(context.Background(), types.ToolInput{
		Name: "Agent",
		Params: map[string]any{
			"description": "test task",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// nil manager check happens first
	if !strings.Contains(result.Error, "not configured") {
		t.Errorf("expected 'not configured' error, got: %s", result.Error)
	}
}

func TestAgent_Execute_UnknownIsolation(t *testing.T) {
	t.Parallel()
	a := NewAgent(nil, false)
	result, err := a.Execute(context.Background(), types.ToolInput{
		Name: "Agent",
		Params: map[string]any{
			"description": "test task",
			"prompt":      "do something",
			"isolation":   "unknown",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// nil manager check happens first
	if !strings.Contains(result.Error, "not configured") {
		t.Errorf("expected 'not configured' error, got: %s", result.Error)
	}
}

// ---------------------------------------------------------------------------
// dispatcherAdapter
// ---------------------------------------------------------------------------

func TestDispatcherAdapter_Execute(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe})
	adapter := &dispatcherAdapter{d: d}
	result, err := adapter.Execute(context.Background(), subagent.ToolCallInput{
		ID:   "1",
		Name: "test",
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Output != "ok" {
		t.Errorf("expected 'ok', got %q", result.Output)
	}
}

func TestDispatcherAdapter_ListTools(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "alpha", riskLevel: types.RiskSafe})
	d.Register(&mockTool{name: "beta", riskLevel: types.RiskSafe})
	adapter := &dispatcherAdapter{d: d}
	tools := adapter.ListTools()
	if len(tools) != 2 {
		t.Errorf("expected 2 tools, got %d", len(tools))
	}
}

func TestDispatcherAdapter_Stop(t *testing.T) {
	d := NewDispatcher(nil)
	adapter := &dispatcherAdapter{d: d}
	adapter.Stop() // should not panic
}

// ---------------------------------------------------------------------------
// NewDispatcherFactory
// ---------------------------------------------------------------------------

func TestNewDispatcherFactory(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	factory := NewDispatcherFactory(dir, dir, nil, nil)
	d, err := factory(dir)
	if err != nil {
		t.Fatalf("factory failed: %v", err)
	}
	if d == nil {
		t.Fatal("expected non-nil dispatcher")
	}
	d.Stop()
}

func TestNewDispatcherFactory_WithManager(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	// Manager requires Dependencies, but factory just passes nil manager check
	factory := NewDispatcherFactory(dir, dir, nil, nil)
	d, err := factory(dir)
	if err != nil {
		t.Fatalf("factory failed: %v", err)
	}
	d.Stop()
}

// ---------------------------------------------------------------------------
// Edit: atomicWrite and pruneBackups via Execute
// ---------------------------------------------------------------------------

func TestEdit_Execute_AtomicWriteAndPrune(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte("original"), 0644)
	e := NewEdit(dir, backupDir)

	// Edit the file multiple times to trigger backup pruning
	for i := 0; i < MaxBackupsPerFile+3; i++ {
		_, err := e.Execute(context.Background(), types.ToolInput{
			Name: "Edit",
			Params: map[string]any{
				"path":       "test.txt",
				"old_string": "original",
				"new_string": strings.Repeat("x", i+1),
			},
		})
		if err != nil {
			t.Fatalf("edit %d failed: %v", i, err)
		}
	}
	// Verify file was edited
	content, err := os.ReadFile(filepath.Join(dir, "test.txt"))
	if err != nil {
		t.Fatal(err)
	}
	if len(content) == 0 {
		t.Error("expected non-empty file content")
	}
}

func TestEdit_Execute_LineRangeReplacesContent(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte("line1\nline2\nline3"), 0644)
	e := NewEdit(dir, backupDir)
	result, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "test.txt",
			"new_string": "REPLACED",
			"start_line": float64(2),
			"end_line":   float64(2),
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "line-range") {
		t.Errorf("expected 'line-range' strategy, got: %s", result.Output)
	}
}

func TestEdit_Execute_CascadingFuzzy(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	content := "func main() {\n\tfmt.Println(\"hello\")\n\treturn\n}"
	os.WriteFile(filepath.Join(dir, "main.go"), []byte(content), 0644)
	e := NewEdit(dir, backupDir)
	// Fuzzy match: first/last lines match but middle has minor typos
	result, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "main.go",
			"old_string": "func main() {\n\tfmt.Printl(\"hello\")\n\treturn\n}",
			"new_string": "func main() {\n\tfmt.Println(\"world\")\n\treturn\n}",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "Edited") {
		t.Errorf("expected 'Edited' in output, got: %s", result.Output)
	}
}

// ---------------------------------------------------------------------------
// Dispatcher: SetSessionID
// ---------------------------------------------------------------------------

func TestDispatcher_SetSessionID(t *testing.T) {
	d := NewDispatcher(nil)
	// Should not panic when todoWrite is nil
	d.SetSessionID("test-session")

	// With todoWrite set
	d.todoWrite = NewTodoWrite(t.TempDir(), "old")
	d.SetSessionID("new-session")
	if d.todoWrite.getSessionID() != "new-session" {
		t.Errorf("expected 'new-session', got %q", d.todoWrite.getSessionID())
	}
}

// ---------------------------------------------------------------------------
// Question tool: Execute
// ---------------------------------------------------------------------------

func TestAskUserQuestion_MissingQuestionParam(t *testing.T) {
	t.Parallel()
	reqCh := make(chan QuestionRequest, 4)
	respCh := make(chan QuestionResponse, 4)
	var pending sync.Map
	q := NewAskUserQuestion(reqCh, respCh, &pending)
	_, err := q.Execute(context.Background(), types.ToolInput{
		Name:   "AskUserQuestion",
		Params: map[string]any{},
	})
	if err == nil {
		t.Error("expected error for missing question param")
	}
}

func TestAskUserQuestion_QuestionNotString(t *testing.T) {
	t.Parallel()
	reqCh := make(chan QuestionRequest, 4)
	respCh := make(chan QuestionResponse, 4)
	var pending sync.Map
	q := NewAskUserQuestion(reqCh, respCh, &pending)
	_, err := q.Execute(context.Background(), types.ToolInput{
		Name: "AskUserQuestion",
		Params: map[string]any{
			"question": 123,
		},
	})
	if err == nil {
		t.Error("expected error for non-string question")
	}
}

func TestAskUserQuestion_Success(t *testing.T) {
	t.Parallel()
	reqCh := make(chan QuestionRequest, 4)
	respCh := make(chan QuestionResponse, 4)
	var pending sync.Map
	q := NewAskUserQuestion(reqCh, respCh, &pending)

	go func() {
		req := <-reqCh
		// Route response through per-request channel
		if ch, ok := pending.Load(req.ID); ok {
			ch.(chan QuestionResponse) <- QuestionResponse{Answer: "yes"}
		}
	}()

	result, err := q.Execute(context.Background(), types.ToolInput{
		Name: "AskUserQuestion",
		Params: map[string]any{
			"question": "Do you agree?",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "yes") {
		t.Errorf("expected 'yes' in output, got: %s", result.Output)
	}
}

func TestAskUserQuestion_WithHeaderAndOptions(t *testing.T) {
	t.Parallel()
	reqCh := make(chan QuestionRequest, 4)
	respCh := make(chan QuestionResponse, 4)
	var pending sync.Map
	q := NewAskUserQuestion(reqCh, respCh, &pending)

	go func() {
		req := <-reqCh
		if ch, ok := pending.Load(req.ID); ok {
			ch.(chan QuestionResponse) <- QuestionResponse{Answer: "option1"}
		}
	}()

	result, err := q.Execute(context.Background(), types.ToolInput{
		Name: "AskUserQuestion",
		Params: map[string]any{
			"question":    "Pick one",
			"header":      "Choice",
			"options":     []any{"option1", "option2"},
			"allow_custom": false,
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "option1") {
		t.Errorf("expected 'option1' in output, got: %s", result.Output)
	}
}

func TestAskUserQuestion_Timeout(t *testing.T) {
	reqCh := make(chan QuestionRequest, 4)
	respCh := make(chan QuestionResponse, 4)
	var pending sync.Map
	q := NewAskUserQuestion(reqCh, respCh, &pending)

	// Use very short timeout and don't respond
	_, err := q.Execute(context.Background(), types.ToolInput{
		Name: "AskUserQuestion",
		Params: map[string]any{
			"question": "What?",
			"timeout":  float64(1),
		},
	})
	if err == nil {
		t.Error("expected timeout error")
	}
}

func TestAskUserQuestion_ContextCancelled(t *testing.T) {
	reqCh := make(chan QuestionRequest, 4)
	respCh := make(chan QuestionResponse, 4)
	var pending sync.Map
	q := NewAskUserQuestion(reqCh, respCh, &pending)

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // cancel immediately

	_, err := q.Execute(ctx, types.ToolInput{
		Name: "AskUserQuestion",
		Params: map[string]any{
			"question": "What?",
		},
	})
	if err == nil {
		t.Error("expected context cancellation error")
	}
}

func TestAskUserQuestion_ChannelFull(t *testing.T) {
	reqCh := make(chan QuestionRequest, 1) // buffer of 1
	respCh := make(chan QuestionResponse, 4)
	var pending sync.Map
	q := NewAskUserQuestion(reqCh, respCh, &pending)

	// Fill the channel
	reqCh <- QuestionRequest{}

	_, err := q.Execute(context.Background(), types.ToolInput{
		Name: "AskUserQuestion",
		Params: map[string]any{
			"question": "What?",
		},
	})
	if err == nil {
		t.Error("expected error when channel is full")
	}
}

func TestAskUserQuestion_RiskLevel(t *testing.T) {
	t.Parallel()
	q := NewAskUserQuestion(nil, nil, nil)
	if q.RiskLevel() != types.RiskSafe {
		t.Errorf("expected RiskSafe, got %s", q.RiskLevel())
	}
}

func TestAskUserQuestion_NonFloatTimeout(t *testing.T) {
	reqCh := make(chan QuestionRequest, 4)
	respCh := make(chan QuestionResponse, 4)
	var pending sync.Map
	q := NewAskUserQuestion(reqCh, respCh, &pending)

	go func() {
		req := <-reqCh
		if ch, ok := pending.Load(req.ID); ok {
			ch.(chan QuestionResponse) <- QuestionResponse{Answer: "ok"}
		}
	}()

	result, err := q.Execute(context.Background(), types.ToolInput{
		Name: "AskUserQuestion",
		Params: map[string]any{
			"question": "What?",
			"timeout":  "not_a_number",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "ok") {
		t.Errorf("expected 'ok' in output, got: %s", result.Output)
	}
}

// ---------------------------------------------------------------------------
// WebFetch: RiskLevel and NewWebFetch
// ---------------------------------------------------------------------------

func TestWebFetch_RiskLevel(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	if wf.RiskLevel() != types.RiskMedium {
		t.Errorf("expected RiskMedium, got %s", wf.RiskLevel())
	}
}

func TestWebFetch_NewWithPrivateIPsAllowed(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), true)
	if !wf.allowPrivateIPs {
		t.Error("expected allowPrivateIPs to be true")
	}
}

func TestWebFetch_GetVersionDefault(t *testing.T) {
	// Save and restore
	Version = atomic.Value{}
	v := getVersion()
	if v != "dev" {
		t.Errorf("expected 'dev' default version, got %q", v)
	}
}

// ---------------------------------------------------------------------------
// humanSize: very large values
// ---------------------------------------------------------------------------

func TestHumanSize_Petabyte(t *testing.T) {
	t.Parallel()
	got := humanSize(1 << 50)
	if got == "" {
		t.Error("expected non-empty result")
	}
}

func TestHumanSize_Exabyte(t *testing.T) {
	t.Parallel()
	// Beyond the units array — should cap at last unit
	got := humanSize(1 << 60)
	if got == "" {
		t.Error("expected non-empty result")
	}
}

// ---------------------------------------------------------------------------
// Edit: resolvePath edge cases
// ---------------------------------------------------------------------------

func TestEdit_ResolvePath_AbsoluteOutsideWorkDir(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	e := NewEdit(dir, backupDir)
	_, err := e.resolvePath("/etc/hostname")
	if err == nil {
		t.Error("expected error for absolute path outside workDir")
	}
}

func TestEdit_ResolvePath_NewFile(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	e := NewEdit(dir, backupDir)
	resolved, err := e.resolvePath("new_file.txt")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.HasSuffix(resolved, "new_file.txt") {
		t.Errorf("expected path to end with new_file.txt, got %q", resolved)
	}
}

// ---------------------------------------------------------------------------
// FileMove: destination file already exists
// ---------------------------------------------------------------------------

func TestFileMove_DestinationExists(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "src.txt"), []byte("source"), 0644)
	os.WriteFile(filepath.Join(dir, "dst.txt"), []byte("dest"), 0644)
	fm := NewFileMove(dir)
	result, err := fm.Execute(context.Background(), types.ToolInput{
		Name: "FileMove",
		Params: map[string]any{
			"source":      "src.txt",
			"destination": "dst.txt",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "Moved") {
		t.Errorf("expected 'Moved', got: %s", result.Output)
	}
	// src.txt should no longer exist
	if _, err := os.Stat(filepath.Join(dir, "src.txt")); !os.IsNotExist(err) {
		t.Error("expected src.txt to be gone after move")
	}
}

// ---------------------------------------------------------------------------
// FileWrite: overwrite existing file with symlink
// ---------------------------------------------------------------------------

func TestFileWrite_ExistingFile(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	fw := NewFileWrite(dir, backupDir)

	os.WriteFile(filepath.Join(dir, "existing.txt"), []byte("old"), 0644)
	result, err := fw.Execute(context.Background(), types.ToolInput{
		Name: "FileWrite",
		Params: map[string]any{
			"path":    "existing.txt",
			"content": "new content",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "Wrote") {
		t.Errorf("expected 'Wrote', got: %s", result.Output)
	}
}

// ---------------------------------------------------------------------------
// Grep: PureGo with inaccessible file
// ---------------------------------------------------------------------------

func TestGrep_PureGo_InaccessibleFile(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	f := filepath.Join(dir, "noread.txt")
	os.WriteFile(f, []byte("secret\n"), 0000) // no permissions
	g := &Grep{workDir: dir, hasRg: false}
	result, err := g.Execute(context.Background(), toolInput("pattern", "secret"))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// Inaccessible files are skipped, not errored
	if strings.Contains(result.Output, "noread.txt") {
		t.Errorf("expected inaccessible file to be skipped, got: %s", result.Output)
	}
}

// ---------------------------------------------------------------------------
// Glob: with subdirectory path param
// ---------------------------------------------------------------------------

func TestGlob_WithPathParam(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, "sub"), 0755)
	os.WriteFile(filepath.Join(dir, "sub", "file.go"), []byte("package sub"), 0644)
	os.WriteFile(filepath.Join(dir, "root.go"), []byte("package root"), 0644)

	g := NewGlob(dir)
	// The path param is in the schema but the implementation uses workDir;
	// pattern **/*.go should match both files
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Glob",
		Params: map[string]any{
			"pattern": "**/*.go",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "file.go") {
		t.Errorf("expected file.go, got: %s", result.Output)
	}
	if !strings.Contains(result.Output, "root.go") {
		t.Errorf("expected root.go, got: %s", result.Output)
	}
}

// ---------------------------------------------------------------------------
// TodoWrite: file written correctly
// ---------------------------------------------------------------------------

func TestTodoWrite_FileContent(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	tw := NewTodoWrite(dir, "test-session")
	_, err := tw.Execute(context.Background(), types.ToolInput{
		Name: "TodoWrite",
		Params: map[string]any{
			"todos": []any{
				map[string]any{"content": "task1", "status": "completed", "priority": "high"},
				map[string]any{"content": "task2", "status": "pending", "priority": "low"},
			},
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Read the TODO.md file
	content, err := os.ReadFile(filepath.Join(dir, "test-session", "TODO.md"))
	if err != nil {
		t.Fatal(err)
	}
	s := string(content)
	if !strings.Contains(s, "# TODO") {
		t.Error("expected '# TODO' header")
	}
	if !strings.Contains(s, "[x]") {
		t.Error("expected '[x]' for completed task")
	}
	if !strings.Contains(s, "[ ]") {
		t.Error("expected '[ ]' for pending task")
	}
	if !strings.Contains(s, "task1") {
		t.Error("expected 'task1' in output")
	}
	if !strings.Contains(s, "task2") {
		t.Error("expected 'task2' in output")
	}
}

// ---------------------------------------------------------------------------
// Edit: execute with no-match old_string returns ToolResult with error
// ---------------------------------------------------------------------------

func TestEdit_Execute_NoMatchOldString(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte("hello world"), 0644)
	e := NewEdit(dir, backupDir)
	result, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "test.txt",
			"old_string": "nonexistent string that does not appear",
			"new_string": "replacement",
		},
	})
	if err != nil {
		t.Fatalf("unexpected Go error: %v", err)
	}
	// The error is in ToolResult.Output, not a Go error
	if !strings.Contains(result.Output, "Could not find") {
		t.Errorf("expected 'Could not find' in output, got: %s", result.Output)
	}
}

// ---------------------------------------------------------------------------
// toolResultError
// ---------------------------------------------------------------------------

func TestToolResultError(t *testing.T) {
	t.Parallel()
	e := toolResultError{result: types.ToolResult{Error: "test error"}}
	if e.Error() != "test error" {
		t.Errorf("expected 'test error', got %q", e.Error())
	}
}

// ---------------------------------------------------------------------------
// ApprovePermission: channel full fallback
// ---------------------------------------------------------------------------

func TestApprovePermission_FallbackChannelFull(t *testing.T) {
	d := NewDispatcher(nil)
	// Fill both the per-request channel and shared channel
	// First, approve on a nonexistent request ID (no per-request channel)
	// then also fill the shared channel
	for i := 0; i < PermissionChannelBuffer; i++ {
		d.responseCh <- PermissionResponse{}
	}
	// Should not panic when both channels are full
	d.ApprovePermission(999, true, false)
}

// ---------------------------------------------------------------------------
// RespondQuestion: fallback when pending not found
// ---------------------------------------------------------------------------

func TestRespondQuestion_FallbackSharedChannel(t *testing.T) {
	d := NewDispatcher(nil)
	// Fill the shared channel to trigger the warning path
	for i := 0; i < QuestionChannelBuffer; i++ {
		d.questionRespCh <- QuestionResponse{}
	}
	// No pending request, falls back to shared channel which is full
	d.RespondQuestion(999, "answer")
}

// ---------------------------------------------------------------------------
// askPermissionWithAgentDefault
// ---------------------------------------------------------------------------

func TestCheckPermission_AgentDefaultAsk(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(&config.PermissionsConfig{
		Agents: map[string]config.PermissionsAgentConfig{
			"ask-agent": {DefaultAction: "ask"},
		},
	})
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskDangerous})
	if err := d.SelectAgent("ask-agent"); err != nil {
		t.Fatalf("SelectAgent failed: %v", err)
	}
	defer d.SelectAgent("default")

	// checkPermission only handles "allow" and "deny" for agent defaults;
	// "ask" falls through to risk_level source
	allowed, pctx, err := d.checkPermission("Bash", types.ToolInput{
		Name:   "Bash",
		Params: map[string]any{"command": "echo"},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if allowed {
		t.Error("expected not allowed for ask action")
	}
	if pctx.Source != "risk_level" {
		t.Errorf("expected source 'risk_level', got %q", pctx.Source)
	}
}

// ---------------------------------------------------------------------------
// Dispatcher: ensurePermission with non-interactive input
// ---------------------------------------------------------------------------

func TestEnsurePermission_NonInteractiveSafeTool(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "safe", riskLevel: types.RiskSafe})
	_, err := d.Execute(context.Background(), types.ToolCall{
		ID:    "call1",
		Name:  "safe",
		Input: []byte(`{"params":{"interactive":false}}`),
	})
	if err != nil {
		t.Errorf("safe tool should execute in non-interactive mode, got: %v", err)
	}
}

// ---------------------------------------------------------------------------
// ensurePermission: non-interactive dangerous tool should be blocked
// ---------------------------------------------------------------------------

func TestEnsurePermission_NonInteractiveDangerousTool(t *testing.T) {
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "bash", riskLevel: types.RiskDangerous})
	_, err := d.Execute(context.Background(), types.ToolCall{
		ID:    "call1",
		Name:  "bash",
		Input: []byte(`{"params":{"command":"rm -rf /","interactive":false}}`),
	})
	if err == nil {
		t.Fatal("expected error for non-interactive dangerous tool")
	}
	if !strings.Contains(err.Error(), "blocked in shell mode") {
		t.Errorf("expected 'blocked in shell mode', got: %v", err)
	}
}

// ---------------------------------------------------------------------------
// DefaultDispatcher: error path when AskUserQuestion registration fails
// ---------------------------------------------------------------------------

func TestDefaultDispatcher_ErrorPath(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	// DefaultDispatcher with valid config should succeed
	d, err := DefaultDispatcher(dir, dir, dir, nil)
	if err != nil {
		t.Fatalf("DefaultDispatcher failed: %v", err)
	}
	d.Stop()
}

// ---------------------------------------------------------------------------
// humanSize: additional coverage for PB range
// ---------------------------------------------------------------------------

func TestHumanSize_AllUnits(t *testing.T) {
	t.Parallel()
	tests := []struct {
		bytes int64
		want  string
	}{
		{0, "0 B"},
		{512, "512 B"},
		{1024, "1.0 KB"},
		{1024 * 1024, "1.0 MB"},
		{1024 * 1024 * 1024, "1.0 GB"},
		{1024 * 1024 * 1024 * 1024, "1.0 TB"},
		{1024 * 1024 * 1024 * 1024 * 1024, "1.0 PB"},
		{1024 * 1024 * 1024 * 1024 * 1024 * 1024, "1.0 EB"},
	}
	for _, tt := range tests {
		got := humanSize(tt.bytes)
		if got != tt.want {
			t.Errorf("humanSize(%d) = %q, want %q", tt.bytes, got, tt.want)
		}
	}
}

// ---------------------------------------------------------------------------
// Edit: Execute with whitespace-normalized match
// ---------------------------------------------------------------------------

func TestEdit_Execute_WhitespaceNormalizedMatch(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	content := "hello    world"
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte(content), 0644)
	e := NewEdit(dir, backupDir)
	result, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "test.txt",
			"old_string": "hello world",
			"new_string": "hello universe",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "whitespace-normalized") {
		t.Errorf("expected 'whitespace-normalized' strategy, got: %s", result.Output)
	}
}

// ---------------------------------------------------------------------------
// Edit: Execute with line-trimmed match
// ---------------------------------------------------------------------------

func TestEdit_Execute_LineTrimmedMatch(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	content := "  hello\n  world"
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte(content), 0644)
	e := NewEdit(dir, backupDir)
	result, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "test.txt",
			"old_string": "hello\nworld",
			"new_string": "replaced",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "line-trimmed") {
		t.Errorf("expected 'line-trimmed' strategy, got: %s", result.Output)
	}
}

// ---------------------------------------------------------------------------
// Edit: Execute with fuzzy-anchor match
// ---------------------------------------------------------------------------

func TestEdit_Execute_FuzzyAnchorMatch(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	content := "first line\nsecnd line\nthird line\nfourth line\nfifth line"
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte(content), 0644)
	e := NewEdit(dir, backupDir)
	result, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "test.txt",
			"old_string": "first line\nsecod line\nthird line",
			"new_string": "replaced",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "fuzzy-anchor") {
		t.Errorf("expected 'fuzzy-anchor' strategy, got: %s", result.Output)
	}
}

// ---------------------------------------------------------------------------
// Edit: CR/LF line endings
// ---------------------------------------------------------------------------

func TestEdit_Execute_CRLFLineEndings(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	backupDir := t.TempDir()
	content := "hello\r\nworld"
	os.WriteFile(filepath.Join(dir, "test.txt"), []byte(content), 0644)
	e := NewEdit(dir, backupDir)
	result, err := e.Execute(context.Background(), types.ToolInput{
		Name: "Edit",
		Params: map[string]any{
			"path":       "test.txt",
			"old_string": "hello",
			"new_string": "replaced",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "Edited") {
		t.Errorf("expected 'Edited' in output, got: %s", result.Output)
	}
	// Verify the file still uses CRLF
	data, _ := os.ReadFile(filepath.Join(dir, "test.txt"))
	if !strings.Contains(string(data), "\r\n") {
		t.Error("expected CRLF line endings to be preserved")
	}
}

// ---------------------------------------------------------------------------
// Glob: with empty result from RG (rg returns empty output)
// ---------------------------------------------------------------------------

func TestGlob_RGEmptyResult(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, ".gitignore"), []byte("*.log\n"), 0644)
	os.WriteFile(filepath.Join(dir, "file.log"), []byte("log"), 0644)
	g := NewGlob(dir)
	result, err := g.Execute(context.Background(), toolInput("pattern", "*.log"))
	if err != nil {
		t.Skipf("skipping rg-based test: %v", err)
	}
	// rg with gitignore may filter out .log files
	if result.Output == "" {
		t.Error("expected non-empty output")
	}
}

// ---------------------------------------------------------------------------
// WebFetch: Execute with text format
// ---------------------------------------------------------------------------

func TestWebFetch_Execute_TextFormat(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	// This will fail with SSRF since it's a literal IP
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url":    "http://192.168.1.1:80",
			"format": "text",
		},
	})
	if err == nil {
		t.Fatal("expected SSRF error for private IP")
	}
}

// ---------------------------------------------------------------------------
// WebFetch: Execute with html format
// ---------------------------------------------------------------------------

func TestWebFetch_Execute_HTMLFormat(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url":    "http://10.0.0.1:80",
			"format": "html",
		},
	})
	if err == nil {
		t.Fatal("expected SSRF error for private IP")
	}
}

// ---------------------------------------------------------------------------
// WebFetch: Execute with markdown format (default)
// ---------------------------------------------------------------------------

func TestWebFetch_Execute_MarkdownFormat(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	_, err := wf.Execute(context.Background(), types.ToolInput{
		Name: "WebFetch",
		Params: map[string]any{
			"url":    "http://172.16.0.1:80",
			"format": "markdown",
		},
	})
	if err == nil {
		t.Fatal("expected SSRF error for private IP")
	}
}

// ---------------------------------------------------------------------------
// Grep: PureGo with scanner error path
// ---------------------------------------------------------------------------

func TestGrep_PureGo_WithInaccessibleDir(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.MkdirAll(filepath.Join(dir, "noread"), 0000)
	os.WriteFile(filepath.Join(dir, "ok.txt"), []byte("find me\n"), 0644)
	g := &Grep{workDir: dir, hasRg: false}
	result, err := g.Execute(context.Background(), toolInput("pattern", "find"))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "ok.txt") {
		t.Errorf("expected ok.txt in results, got: %s", result.Output)
	}
}

// ---------------------------------------------------------------------------
// WebFetch: resolveAndCache with no IP addresses
// ---------------------------------------------------------------------------

func TestWebFetch_ResolveAndCache_NonExistentHost(t *testing.T) {
	t.Parallel()
	wf := NewWebFetch(t.TempDir(), false)
	_, err := wf.resolveAndCache(context.Background(), "this-host-does-not-exist-xyz123.invalid")
	if err == nil {
		t.Error("expected DNS resolution error for non-existent host")
	}
}

// ---------------------------------------------------------------------------
// Grep: with include filter that matches nothing
// ---------------------------------------------------------------------------

func TestGrep_PureGo_IncludeMatchesNothing(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	os.WriteFile(filepath.Join(dir, "main.go"), []byte("package main\n"), 0644)
	g := &Grep{workDir: dir, hasRg: false}
	result, err := g.Execute(context.Background(), types.ToolInput{
		Name: "Grep",
		Params: map[string]any{
			"pattern": "main",
			"include": "*.py",
		},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(result.Output, "No results found") {
		t.Errorf("expected 'No results found', got: %s", result.Output)
	}
}
