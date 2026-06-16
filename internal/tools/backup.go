package tools

import (
	"log/slog"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// pruneBackupsByPrefix removes the oldest backups matching the given prefix
// when the count reaches maxBackups. Backups are sorted lexicographically
// (timestamp in the name ensures chronological order).
func pruneBackupsByPrefix(backupDir, prefix string, maxBackups int) {
	entries, err := os.ReadDir(backupDir)
	if err != nil {
		return
	}

	var matches []string
	for _, e := range entries {
		if !e.IsDir() && strings.HasPrefix(e.Name(), prefix+".") {
			matches = append(matches, e.Name())
		}
	}

	if len(matches) < maxBackups {
		return
	}

	sort.Strings(matches)

	toDelete := matches[:len(matches)-maxBackups+1]
	for _, name := range toDelete {
		path := filepath.Join(backupDir, name)
		if err := os.Remove(path); err != nil {
			slog.Warn("failed to prune old backup", "path", path, "error", err)
		}
	}
}
