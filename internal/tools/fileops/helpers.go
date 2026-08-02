package fileops

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// pruneBackupsByPrefix removes the oldest backups matching the given prefix
// when the count reaches maxBackups, to leave room for the new backup.
// This ensures after adding the new backup, we have at most maxBackups total.
func PruneBackupsByPrefix(backupDir, prefix string, maxBackups int) {
	entries, err := os.ReadDir(backupDir)
	if err != nil {
		return
	}

	var backups []string
	for _, entry := range entries {
		if strings.HasPrefix(entry.Name(), prefix) {
			backups = append(backups, entry.Name())
		}
	}

	// Prune when we have maxBackups or more, to leave room for the new backup
	if len(backups) < maxBackups {
		return
	}

	// Sort by modification time (oldest first)
	sort.Slice(backups, func(i, j int) bool {
		iInfo, _ := os.Stat(filepath.Join(backupDir, backups[i]))
		jInfo, _ := os.Stat(filepath.Join(backupDir, backups[j]))
		if iInfo == nil || jInfo == nil {
			return false
		}
		return iInfo.ModTime().Before(jInfo.ModTime())
	})

	// Remove oldest backups to leave at most (maxBackups - 1) entries
	// so that after adding the new backup we have at most maxBackups
	keepCount := maxBackups - 1
	for i := 0; i < len(backups)-keepCount; i++ {
		_ = os.Remove(filepath.Join(backupDir, backups[i]))
	}
}

// LevenshteinDistance computes the Levenshtein distance between two strings.
func LevenshteinDistance(a, b string) int {
	if len(a) == 0 {
		return len(b)
	}
	if len(b) == 0 {
		return len(a)
	}

	// Use a 2D matrix for simplicity
	dp := make([][]int, len(a)+1)
	for i := range dp {
		dp[i] = make([]int, len(b)+1)
		dp[i][0] = i
	}
	for j := range dp[0] {
		dp[0][j] = j
	}

	for i := 1; i <= len(a); i++ {
		for j := 1; j <= len(b); j++ {
			if a[i-1] == b[j-1] {
				dp[i][j] = dp[i-1][j-1]
			} else {
				dp[i][j] = 1 + min(dp[i-1][j], dp[i][j-1], dp[i-1][j-1])
			}
		}
	}

	return dp[len(a)][len(b)]
}

// LevenshteinBuf computes the Levenshtein distance using pre-allocated buffers.
func LevenshteinBuf(a, b string, prev, curr []int) int {
	if len(a) == 0 {
		return len(b)
	}
	if len(b) == 0 {
		return len(a)
	}

	if len(prev) < len(b)+1 {
		prev = make([]int, len(b)+1)
	}
	if len(curr) < len(b)+1 {
		curr = make([]int, len(b)+1)
	}

	for j := 0; j <= len(b); j++ {
		prev[j] = j
	}

	for i := 1; i <= len(a); i++ {
		curr[0] = i
		for j := 1; j <= len(b); j++ {
			if a[i-1] == b[j-1] {
				curr[j] = prev[j-1]
			} else {
				curr[j] = 1 + min(prev[j], curr[j-1], prev[j-1])
			}
		}
		prev, curr = curr, prev
	}

	return prev[len(b)]
}

func min(a, b, c int) int {
	if a < b && a < c {
		return a
	}
	if b < c {
		return b
	}
	return c
}

// min2 returns the minimum of two integers.
func min2(a, b int) int {
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
