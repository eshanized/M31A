package tui

import (
	"log/slog"
	"os"
	"path/filepath"
	"time"
)

// backupCurrentSessionAsync creates a backup snapshot of the current session
// directory and returns the backup path (empty string on failure or skip).
// This is a synchronous operation but designed to be called before state mutation.
func (m *AppState) backupCurrentSessionAsync() string {
	if m.sessionManager == nil || m.sessionID == "" {
		return ""
	}
	if m.configPath == "" {
		return ""
	}

	backupDir := filepath.Join(filepath.Dir(m.configPath), "backups")
	if err := os.MkdirAll(backupDir, 0o700); err != nil {
		slog.Warn("backup: failed to create backup dir", "err", err)
		return ""
	}

	// Only backup at most once per hour to avoid excessive disk usage
	stamp := time.Now().Format("20060102-150405")
	backupPath := filepath.Join(backupDir, m.sessionID+"-"+stamp+".bak")

	sessDir := filepath.Join(m.sessionManager.BaseDir(), m.sessionID)
	if sessDir == "" {
		return ""
	}

	if err := copyDir(sessDir, backupPath); err != nil {
		slog.Debug("backup: copyDir failed", "err", err)
		return ""
	}
	return backupPath
}

// copyDir recursively copies src to dst.
func copyDir(src, dst string) error {
	return filepath.Walk(src, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		rel, err := filepath.Rel(src, path)
		if err != nil {
			return err
		}
		target := filepath.Join(dst, rel)
		if info.IsDir() {
			return os.MkdirAll(target, info.Mode())
		}
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		return os.WriteFile(target, data, info.Mode())
	})
}
