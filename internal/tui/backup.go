package tui

import (
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"time"
)

// backupCurrentSession copies the current session's directory to the
// auto-backup location (~/.m31a/backups/<id>-<timestamp>/) when the
// Features.AutoBackup config flag is set. Called on workflow Ship so
// the user always has a restore point for completed work.
//
// Failures are logged but never surfaced — backup is best-effort and
// must not block the Ship transition.
func (m *AppState) backupCurrentSession() {
	if m.config == nil || !m.config.Features.AutoBackup {
		return
	}
	if m.sessionID == "" {
		return
	}
	if m.sessionManager == nil {
		return
	}

	sessionsDir := m.sessionManager.BaseDir()
	if sessionsDir == "" {
		return
	}
	src := filepath.Join(sessionsDir, m.sessionID)
	if _, err := os.Stat(src); err != nil {
		slog.Debug("auto-backup: source missing", "err", err)
		return
	}

	backupRoot := filepath.Join(filepath.Dir(sessionsDir), "backups")
	if err := os.MkdirAll(backupRoot, 0755); err != nil {
		slog.Warn("auto-backup: cannot create backup dir", "err", err)
		return
	}

	stamp := time.Now().Format("20060102-150405")
	dst := filepath.Join(backupRoot, fmt.Sprintf("%s-%s", m.sessionID, stamp))

	if err := copyDir(src, dst); err != nil {
		slog.Warn("auto-backup failed", "err", err)
		return
	}

	slog.Info("auto-backup complete", "session", m.sessionID, "path", dst)
	// BUG-02 fix: do not mutate AppState fields from this goroutine context.
	// The caller (handlePhaseShip) must emit a ToastMsg via tea.Cmd instead.
}

// copyDir recursively copies src into dst (dst is created). It is used
// by backupCurrentSession and intentionally minimal — symlinks are not
// followed, and errors on individual files abort the copy.
func copyDir(src, dst string) error {
	info, err := os.Stat(src)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(dst, info.Mode()); err != nil {
		return err
	}
	entries, err := os.ReadDir(src)
	if err != nil {
		return err
	}
	for _, e := range entries {
		s := filepath.Join(src, e.Name())
		d := filepath.Join(dst, e.Name())
		if e.IsDir() {
			if err := copyDir(s, d); err != nil {
				return err
			}
			continue
		}
		data, err := os.ReadFile(s)
		if err != nil {
			return err
		}
		if err := os.WriteFile(d, data, 0644); err != nil {
			return err
		}
	}
	return nil
}

// backupCurrentSessionAsync is like backupCurrentSession but returns the
// backup path on success (for the caller to emit a ToastMsg) instead of
// mutating AppState fields directly. BUG-02 fix.
func (m *AppState) backupCurrentSessionAsync() string {
	if m.config == nil || !m.config.Features.AutoBackup {
		return ""
	}
	if m.sessionID == "" || m.sessionManager == nil {
		return ""
	}

	sessionsDir := m.sessionManager.BaseDir()
	if sessionsDir == "" {
		return ""
	}
	src := filepath.Join(sessionsDir, m.sessionID)
	if _, err := os.Stat(src); err != nil {
		slog.Debug("auto-backup: source missing", "err", err)
		return ""
	}

	backupRoot := filepath.Join(filepath.Dir(sessionsDir), "backups")
	if err := os.MkdirAll(backupRoot, 0755); err != nil {
		slog.Warn("auto-backup: cannot create backup dir", "err", err)
		return ""
	}

	stamp := time.Now().Format("20060102-150405")
	dst := filepath.Join(backupRoot, fmt.Sprintf("%s-%s", m.sessionID, stamp))

	if err := copyDir(src, dst); err != nil {
		slog.Warn("auto-backup failed", "err", err)
		return ""
	}

	slog.Info("auto-backup complete", "session", m.sessionID, "path", dst)
	return filepath.Base(dst)
}

// sessionHealthSparkline builds a small sparkline from the message counts
// of the most recent sessions, suitable for rendering in the welcome
// screen or status area. Returns an empty string when fewer than 3
// sessions exist or the session manager is unavailable.
func (m *AppState) sessionHealthSparkline() string {
	if m.sessionManager == nil {
		return ""
	}
	sessions, err := m.sessionManager.ListSessions()
	if err != nil || len(sessions) < 3 {
		return ""
	}
	// Use up to 20 most recent sessions; ListSessions returns newest first.
	n := len(sessions)
	if n > 20 {
		n = 20
	}
	values := make([]int, n)
	for i := 0; i < n; i++ {
		values[i] = sessions[i].MessageCount
	}
	return formatSparkline(values, "recent sessions")
}

// formatSparkline renders a simple Unicode sparkline for the given
// values. Exported so other TUI screens (welcome, sidebar) can embed
// it in their render pipelines.
func formatSparkline(values []int, label string) string {
	if len(values) == 0 {
		return ""
	}
	blocks := []rune{'▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'}
	minV, maxV := values[0], values[0]
	for _, v := range values[1:] {
		if v < minV {
			minV = v
		}
		if v > maxV {
			maxV = v
		}
	}
	runes := make([]rune, len(values))
	for i, v := range values {
		if maxV == minV {
			runes[i] = blocks[0]
			continue
		}
		idx := (v - minV) * (len(blocks) - 1) / (maxV - minV)
		if idx < 0 {
			idx = 0
		}
		if idx >= len(blocks) {
			idx = len(blocks) - 1
		}
		runes[i] = blocks[idx]
	}
	spark := string(runes)
	if label == "" {
		return spark
	}
	return spark + " " + label
}
