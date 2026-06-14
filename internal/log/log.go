package log

import (
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

const (
	dirPermission  = 0755
	filePermission = 0644
	dateFormat     = "2006-01-02"
)

var (
	defaultLogger *slog.Logger
	loggerOnce    sync.Once
)

func NewLogger(version string) (*slog.Logger, func(), error) {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return nil, nil, fmt.Errorf("cannot determine home directory: %w", err)
	}

	logDir := filepath.Join(homeDir, ".m31a")
	if mkdirErr := os.MkdirAll(logDir, dirPermission); mkdirErr != nil {
		return nil, nil, fmt.Errorf("cannot create log directory %s: %w", logDir, mkdirErr)
	}

	logFile := filepath.Join(logDir, "m31a.log")
	if rotateErr := rotateLogFiles(logDir, logFile); rotateErr != nil {
		// log rotation failure is non-fatal — warn and continue append-only
		fmt.Fprintf(os.Stderr, "m31a: log rotation failed (%v); continuing with append-only log\n", rotateErr)
	}

	f, err := os.OpenFile(logFile, os.O_APPEND|os.O_CREATE|os.O_WRONLY, filePermission)
	if err != nil {
		return nil, nil, fmt.Errorf("cannot open log file %s: %w", logFile, err)
	}

	var handler slog.Handler
	opts := &slog.HandlerOptions{Level: resolveLogLevel()}

	format := os.Getenv("M31A_LOG_FORMAT")
	if format == "" {
		format = "json"
	}
	if format == "text" {
		handler = slog.NewTextHandler(f, opts)
	} else {
		handler = slog.NewJSONHandler(f, opts)
	}

	logger := slog.New(handler)
	loggerOnce.Do(func() {
		defaultLogger = logger
	})

	cleanup := func() {
		_ = f.Close()
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

	now := time.Now()
	today := time.Date(now.Year(), now.Month(), now.Day(), 0, 0, 0, 0, now.Location())
	modDate := time.Date(info.ModTime().Year(), info.ModTime().Month(), info.ModTime().Day(), 0, 0, 0, 0, now.Location())

	if modDate.Before(today) {
		rotatedName := fmt.Sprintf("%s.%s", logFile, modDate.Format(dateFormat))
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
		parsed, err := time.Parse(dateFormat, datePart)
		if err != nil {
			continue
		}
		if parsed.Before(cutoff) {
			if err := os.Remove(filepath.Join(logDir, name)); err != nil {
				slog.Warn("failed to remove old rotated log file", "path", filepath.Join(logDir, name), "error", err)
			}
		}
	}

	return nil
}

func resolveLogLevel() slog.Level {
	switch strings.ToLower(os.Getenv("M31A_LOG_LEVEL")) {
	case "debug":
		return slog.LevelDebug
	case "info":
		return slog.LevelInfo
	case "warn":
		return slog.LevelWarn
	case "error":
		return slog.LevelError
	default:
		return slog.LevelInfo
	}
}
