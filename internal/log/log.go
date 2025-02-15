package log

import (
	"fmt"
	"io"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"time"
)

var defaultLogger *slog.Logger

func NewLogger(version string) (*slog.Logger, func(), error) {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return nil, nil, fmt.Errorf("cannot determine home directory: %w", err)
	}

	logDir := filepath.Join(homeDir, ".m31a")
	if err := os.MkdirAll(logDir, 0755); err != nil {
		return nil, nil, fmt.Errorf("cannot create log directory %s: %w", logDir, err)
	}

	logFile := filepath.Join(logDir, "m31a.log")
	if err := rotateLogFiles(logDir, logFile); err != nil {
		return nil, nil, fmt.Errorf("log rotation failed: %w", err)
	}

	f, err := os.OpenFile(logFile, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644)
	if err != nil {
		return nil, nil, fmt.Errorf("cannot open log file %s: %w", logFile, err)
	}

	var handler slog.Handler
	opts := &slog.HandlerOptions{Level: resolveLogLevel()}

	if os.Getenv("M31A_LOG_FORMAT") == "text" {
		handler = slog.NewTextHandler(f, opts)
	} else {
		handler = slog.NewJSONHandler(f, opts)
	}

	logger := slog.New(handler)
	defaultLogger = logger

	cleanup := func() {
		f.Close()
	}

	return logger, cleanup, nil
}

func DefaultLogger() *slog.Logger {
	return defaultLogger
}

func rotateLogFiles(logDir, logFile string) error {
	info, err := os.Stat(logFile)
	if err != nil {
		if os.IsNotExist(err) {
			return nil
		}
		return err
	}

	today := time.Now().Truncate(24 * time.Hour)
	modTime := info.ModTime().Truncate(24 * time.Hour)

	if modTime.Before(today) {
		rotatedName := fmt.Sprintf("%s.%s", logFile, modTime.Format("2006-01-02"))
		if err := os.Rename(logFile, rotatedName); err != nil {
			return fmt.Errorf("cannot rotate log file: %w", err)
		}
	}

	if err := removeOldRotatedFiles(logDir); err != nil {
		return err
	}

	return nil
}

func removeOldRotatedFiles(logDir string) error {
	entries, err := os.ReadDir(logDir)
	if err != nil {
		return err
	}

	now := time.Now()
	cutoff := now.AddDate(0, 0, -7)

	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		if !strings.HasPrefix(entry.Name(), "m31a.log.") {
			continue
		}
		name := entry.Name()
		datePart := strings.TrimPrefix(name, "m31a.log.")
		parsed, err := time.Parse("2006-01-02", datePart)
		if err != nil {
			continue
		}
		if parsed.Before(cutoff) {
			os.Remove(filepath.Join(logDir, name))
		}
	}

	return nil
}

func resolveLogLevel() slog.Level {
	if os.Getenv("M31A_LOG_LEVEL") == "debug" {
		return slog.LevelDebug
	}
	return slog.LevelInfo
}

var _ io.Writer = (*noopWriter)(nil)

type noopWriter struct{}

func (n noopWriter) Write(p []byte) (int, error) {
	return len(p), nil
}
