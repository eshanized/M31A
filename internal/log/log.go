package log

import (
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"time"
	"unicode"
)

const (
	dirPermission  = 0755
	filePermission = 0644
	dateFormat     = "2006-01-02"
)

// SanitizeLogValue removes control characters and potential log injection
// patterns from user-provided values before logging.
func SanitizeLogValue(s string) string {
	var b strings.Builder
	b.Grow(len(s))
	for _, r := range s {
		// Allow printable characters except control characters
		if unicode.IsPrint(r) || r == '\n' || r == '\t' {
			b.WriteRune(r)
		} else if r == '\r' {
			// Replace carriage return with escaped representation
			b.WriteString("\\r")
		} else if r == '\x00' {
			// Replace null byte with escaped representation
			b.WriteString("\\0")
		}
		// Skip other control characters
	}
	result := b.String()

	// Remove potential log injection patterns
	suspiciousPatterns := []string{
		"\x1b[", // ANSI escape sequences
		"\r\n",  // CRLF injection
		"\n",    // Newline injection (replaced with space)
	}
	for _, pattern := range suspiciousPatterns {
		result = strings.ReplaceAll(result, pattern, " ")
	}

	return strings.TrimSpace(result)
}

// SanitizeLogKey removes control characters from log keys.
func SanitizeLogKey(key string) string {
	var b strings.Builder
	b.Grow(len(key))
	for _, r := range key {
		if unicode.IsLetter(r) || unicode.IsDigit(r) || r == '_' || r == '-' || r == '.' {
			b.WriteRune(r)
		}
	}
	return b.String()
}

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

	cleanup := func() {
		_ = f.Close()
	}

	return logger, cleanup, nil
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
