package log

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestNewLogger_CreatesLogFile(t *testing.T) {
	// Use a temp home dir
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)

	logger, cleanup, err := NewLogger("test")
	if err != nil {
		t.Fatalf("NewLogger failed: %v", err)
	}
	defer cleanup()

	if logger == nil {
		t.Fatal("Expected non-nil logger")
	}

	logFile := filepath.Join(tmpDir, ".m31a", "m31a.log")
	if _, err := os.Stat(logFile); os.IsNotExist(err) {
		t.Fatal("Expected log file to exist")
	}
}

func TestNewLogger_JSONFormat(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)
	t.Setenv("M31A_LOG_FORMAT", "json")

	logger, cleanup, err := NewLogger("test")
	if err != nil {
		t.Fatalf("NewLogger failed: %v", err)
	}
	defer cleanup()

	logger.Info("test message", "key", "value")

	logFile := filepath.Join(tmpDir, ".m31a", "m31a.log")
	data, err := os.ReadFile(logFile)
	if err != nil {
		t.Fatalf("Failed to read log file: %v", err)
	}

	content := string(data)
	if content == "" {
		t.Fatal("Expected log file to have content")
	}
	// JSON format should contain quotes around keys
	if content[0] != '{' {
		t.Errorf("Expected JSON format, got: %s", content)
	}
}

func TestNewLogger_TextFormat(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)
	t.Setenv("M31A_LOG_FORMAT", "text")

	logger, cleanup, err := NewLogger("test")
	if err != nil {
		t.Fatalf("NewLogger failed: %v", err)
	}
	defer cleanup()

	logger.Info("test message", "key", "value")

	logFile := filepath.Join(tmpDir, ".m31a", "m31a.log")
	data, err := os.ReadFile(logFile)
	if err != nil {
		t.Fatalf("Failed to read log file: %v", err)
	}

	content := string(data)
	if content == "" {
		t.Fatal("Expected log file to have content")
	}
	// Text format should contain "INFO" level marker
	if content[0] == '{' {
		t.Errorf("Expected text format, got JSON: %s", content)
	}
}

func TestNewLogger_DebugLevel(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)
	t.Setenv("M31A_LOG_LEVEL", "debug")

	logger, cleanup, err := NewLogger("test")
	if err != nil {
		t.Fatalf("NewLogger failed: %v", err)
	}
	defer cleanup()

	// Debug messages should appear when level is debug
	logger.Debug("debug message")

	logFile := filepath.Join(tmpDir, ".m31a", "m31a.log")
	data, err := os.ReadFile(logFile)
	if err != nil {
		t.Fatalf("Failed to read log file: %v", err)
	}

	content := string(data)
	if content == "" {
		t.Fatal("Expected debug message in log file")
	}
}

func TestNewLogger_InfoLevel_Default(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)

	logger, cleanup, err := NewLogger("test")
	if err != nil {
		t.Fatalf("NewLogger failed: %v", err)
	}
	defer cleanup()

	// Debug messages should NOT appear at default (info) level
	logger.Debug("debug message")

	logFile := filepath.Join(tmpDir, ".m31a", "m31a.log")
	data, err := os.ReadFile(logFile)
	if err != nil {
		t.Fatalf("Failed to read log file: %v", err)
	}

	content := string(data)
	if content != "" {
		t.Errorf("Expected no debug message at info level, got: %s", content)
	}
}

func TestDefaultLogger_ReturnsNil_BeforeInit(t *testing.T) {
	// Reset the default logger to test nil case
	old := defaultLogger
	defaultLogger = nil
	defer func() { defaultLogger = old }()

	got := DefaultLogger()
	if got != nil {
		t.Errorf("Expected nil default logger before init, got: %v", got)
	}
}

func TestRotateLogFiles_RenamesOldFile(t *testing.T) {
	tmpDir := t.TempDir()
	logFile := filepath.Join(tmpDir, "m31a.log")

	// Create a log file with yesterday's mod time
	os.WriteFile(logFile, []byte("old log"), 0644)
	now := time.Now()
	yesterday := time.Date(now.Year(), now.Month(), now.Day()-1, 12, 0, 0, 0, now.Location())
	os.Chtimes(logFile, yesterday, yesterday)

	err := rotateLogFiles(tmpDir, logFile)
	if err != nil {
		t.Fatalf("rotateLogFiles failed: %v", err)
	}

	// Original file should be renamed
	yesterdayStr := yesterday.Truncate(24 * time.Hour).Format("2006-01-02")
	rotatedName := logFile + "." + yesterdayStr
	if _, err := os.Stat(rotatedName); os.IsNotExist(err) {
		t.Fatal("Expected rotated log file to exist")
	}
}

func TestRotateLogFiles_NoOp_WhenFileNotExist(t *testing.T) {
	tmpDir := t.TempDir()
	logFile := filepath.Join(tmpDir, "m31a.log")

	err := rotateLogFiles(tmpDir, logFile)
	if err != nil {
		t.Fatalf("Expected no error when file does not exist, got: %v", err)
	}
}

func TestRemoveOldRotatedFiles(t *testing.T) {
	tmpDir := t.TempDir()

	// Create a rotated file from 10 days ago (should be deleted — 7 day cutoff)
	oldDate := time.Now().AddDate(0, 0, -10)
	oldFileName := "m31a.log." + oldDate.Format("2006-01-02")
	oldFilePath := filepath.Join(tmpDir, oldFileName)
	os.WriteFile(oldFilePath, []byte("old"), 0644)

	// Create a rotated file from 3 days ago (should be kept)
	recentDate := time.Now().AddDate(0, 0, -3)
	recentFileName := "m31a.log." + recentDate.Format("2006-01-02")
	recentFilePath := filepath.Join(tmpDir, recentFileName)
	os.WriteFile(recentFilePath, []byte("recent"), 0644)

	err := removeOldRotatedFiles(tmpDir)
	if err != nil {
		t.Fatalf("removeOldRotatedFiles failed: %v", err)
	}

	if _, err := os.Stat(oldFilePath); !os.IsNotExist(err) {
		t.Error("Expected old rotated file to be deleted")
	}

	if _, err := os.Stat(recentFilePath); os.IsNotExist(err) {
		t.Error("Expected recent rotated file to still exist")
	}
}

func TestResolveLogLevel(t *testing.T) {
	t.Setenv("M31A_LOG_LEVEL", "debug")
	level := resolveLogLevel()
	if level.String() != "DEBUG" {
		t.Errorf("Expected DEBUG, got %s", level.String())
	}

	t.Setenv("M31A_LOG_LEVEL", "")
	level = resolveLogLevel()
	if level.String() != "INFO" {
		t.Errorf("Expected INFO, got %s", level.String())
	}
}
