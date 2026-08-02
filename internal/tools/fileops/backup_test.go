package fileops

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestPruneBackupsByPrefix_NoBackups(t *testing.T) {
	t.Parallel()
	backupDir := t.TempDir()
	PruneBackupsByPrefix(backupDir, "prefix", 2)
	// Should not panic
}

func TestPruneBackupsByPrefix_UnderLimit(t *testing.T) {
	t.Parallel()
	backupDir := t.TempDir()
	prefix := "test"

	for i := 0; i < 2; i++ {
		path := filepath.Join(backupDir, fmt.Sprintf("%s.%d.%s", prefix, i, time.Now().Format("20060102150405")))
		_ = os.WriteFile(path, []byte("content"), 0600)
		time.Sleep(10 * time.Millisecond)
	}

	PruneBackupsByPrefix(backupDir, prefix, 5)

	entries, _ := os.ReadDir(backupDir)
	// Function should not prune when under limit
	if len(entries) != 2 {
		t.Errorf("expected 2 files (no pruning when under limit), got %d: %v", len(entries), entries)
	}
}

func TestPruneBackupsByPrefix_OverLimit(t *testing.T) {
	t.Parallel()
	backupDir := t.TempDir()
	prefix := "test"

	for i := 0; i < 5; i++ {
		path := filepath.Join(backupDir, fmt.Sprintf("%s.%d.%s", prefix, i, time.Now().Format("20060102150405")))
		_ = os.WriteFile(path, []byte("content"), 0600)
		time.Sleep(10 * time.Millisecond)
	}

	PruneBackupsByPrefix(backupDir, prefix, 2)

	entries, _ := os.ReadDir(backupDir)
	// Function keeps maxBackups-1 items (to leave room for new backup)
	if len(entries) != 1 {
		t.Errorf("expected 1 file after pruning (maxBackups-1), got %d: %v", len(entries), entries)
	}
}

func TestPruneBackupsByPrefix_NonexistentDir(t *testing.T) {
	t.Parallel()
	PruneBackupsByPrefix("/nonexistent/path", "prefix", 2)
	// Should not panic
}

func TestPruneBackupsByPrefix_WrongPrefix(t *testing.T) {
	t.Parallel()
	backupDir := t.TempDir()
	prefix := "test"

	for i := 0; i < 5; i++ {
		path := filepath.Join(backupDir, fmt.Sprintf("other.%d.%s", i, time.Now().Format("20060102150405")))
		_ = os.WriteFile(path, []byte("content"), 0600)
		time.Sleep(10 * time.Millisecond)
	}

	PruneBackupsByPrefix(backupDir, prefix, 2)

entries, _ := os.ReadDir(backupDir)
	// Wrong prefix should not match, so no pruning
	if len(entries) != 5 {
		t.Errorf("expected 5 files (wrong prefix ignored), got %d: %v", len(entries), entries)
	}
}

func TestLevenshteinDistance(t *testing.T) {
	tests := []struct {
		a, b     string
		expected int
	}{
		{"", "", 0},
		{"a", "", 1},
		{"", "a", 1},
		{"a", "a", 0},
		{"kitten", "sitting", 3},
		{"saturday", "sunday", 3},
		{"hello", "world", 4},
		{"fileops", "fileops", 0},
		{"fileops", "fileop", 1},
	}

	for _, tc := range tests {
		t.Run(tc.a+"->"+tc.b, func(t *testing.T) {
			result := LevenshteinDistance(tc.a, tc.b)
			if result != tc.expected {
				t.Errorf("LevenshteinDistance(%q, %q) = %d, want %d", tc.a, tc.b, result, tc.expected)
			}
		})
	}
}

func TestLevenshteinBuf(t *testing.T) {
	prev := make([]int, 10)
	curr := make([]int, 10)

	tests := []struct {
		a, b     string
		expected int
	}{
		{"kitten", "sitting", 3},
		{"saturday", "sunday", 3},
	}

	for _, tc := range tests {
		t.Run(tc.a+"->"+tc.b, func(t *testing.T) {
			result := LevenshteinBuf(tc.a, tc.b, prev, curr)
			if result != tc.expected {
				t.Errorf("LevenshteinBuf(%q, %q) = %d, want %d", tc.a, tc.b, result, tc.expected)
			}
		})
	}
}

func TestDetectLineEnding(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"line1\nline2\n", "\n"},
		{"line1\r\nline2\r\n", "\r\n"},
		{"mixed\nline2\r\nline3", "\r\n"}, // Contains \r\n
		{"no newlines", "\n"},
		{"", "\n"},
	}

	for _, tc := range tests {
		t.Run(tc.expected, func(t *testing.T) {
			result := detectLineEnding(tc.input)
			if result != tc.expected {
				t.Errorf("detectLineEnding(%q) = %q, want %q", tc.input, result, tc.expected)
			}
		})
	}
}

func TestLeadingWhitespace(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"  hello", "  "},
		{"\tworld", "\t"},
		{"  \t mixed", "  \t "},
		{"no indent", ""},
		{"", ""},
	}

	for _, tc := range tests {
		t.Run(tc.expected, func(t *testing.T) {
			result := leadingWhitespace(tc.input)
			if result != tc.expected {
				t.Errorf("leadingWhitespace(%q) = %q, want %q", tc.input, result, tc.expected)
			}
		})
	}
}

func TestToInt(t *testing.T) {
	tests := []struct {
		input    any
		expected int
		ok       bool
	}{
		{int(42), 42, true},
		{float64(42.0), 42, true},
		{int64(42), 42, true},
		{"string", 0, false},
		{nil, 0, false},
		{float64(3.14), 3, true},
	}

	for _, tc := range tests {
		t.Run("", func(t *testing.T) {
			result, ok := toInt(tc.input)
			if ok != tc.ok || result != tc.expected {
				t.Errorf("toInt(%v) = (%d, %v), want (%d, %v)", tc.input, result, ok, tc.expected, tc.ok)
			}
		})
	}
}

func TestReplaceByLineRange(t *testing.T) {
	tests := []struct {
		name         string
		content      string
		startLine    int
		endLine      int
		newString    string
		expected     string
		expectError  bool
	}{
		{
			name:      "replace middle lines",
			content:   "line 1\nline 2\nline 3\nline 4\n",
			startLine: 2,
			endLine:   3,
			newString: "new 2\nnew 3",
			expected:  "line 1\nnew 2\nnew 3\nline 4\n",
		},
		{
			name:      "replace first line",
			content:   "line 1\nline 2\n",
			startLine: 1,
			endLine:   1,
			newString: "new first",
			expected:  "new first\nline 2\n",
		},
		{
			name:      "replace last line",
			content:   "line 1\nline 2\n",
			startLine: 2,
			endLine:   2,
			newString: "new last",
			expected:  "line 1\nnew last\n",
		},
		{
			name:      "replace all lines",
			content:   "line 1\nline 2\n",
			startLine: 1,
			endLine:   2,
			newString: "all new",
			expected:  "all new\n",
		},
		{
			name:        "start line out of bounds",
			content:     "line 1\n",
			startLine:   5,
			endLine:     5,
			newString:   "new",
			expectError: true,
		},
		{
			name:        "end line out of bounds",
			content:     "line 1\n",
			startLine:   1,
			endLine:     5,
			newString:   "new",
			expectError: true,
		},
		{
			name:        "start > end",
			content:     "line 1\nline 2\n",
			startLine:   2,
			endLine:     1,
			newString:   "new",
			expectError: true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			result, err := ReplaceByLineRange(tc.content, tc.startLine, tc.endLine, tc.newString)
			if tc.expectError {
				if err == nil {
					t.Error("expected error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if result != tc.expected {
				t.Errorf("expected %q, got %q", tc.expected, result)
			}
		})
	}
}