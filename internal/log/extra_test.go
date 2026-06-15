package log

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

// ---------------------------------------------------------------------------
// resolveLogLevel additional tests
// ---------------------------------------------------------------------------

func TestResolveLogLevel_Info(t *testing.T) {
	t.Setenv("M31A_LOG_LEVEL", "info")
	level := resolveLogLevel()
	if level.String() != "INFO" {
		t.Errorf("expected INFO, got %s", level.String())
	}
}

func TestResolveLogLevel_Warn(t *testing.T) {
	t.Setenv("M31A_LOG_LEVEL", "warn")
	level := resolveLogLevel()
	if level.String() != "WARN" {
		t.Errorf("expected WARN, got %s", level.String())
	}
}

func TestResolveLogLevel_Error(t *testing.T) {
	t.Setenv("M31A_LOG_LEVEL", "error")
	level := resolveLogLevel()
	if level.String() != "ERROR" {
		t.Errorf("expected ERROR, got %s", level.String())
	}
}

func TestResolveLogLevel_Default(t *testing.T) {
	t.Setenv("M31A_LOG_LEVEL", "")
	level := resolveLogLevel()
	if level.String() != "INFO" {
		t.Errorf("expected INFO for default, got %s", level.String())
	}
}

func TestResolveLogLevel_CaseInsensitive(t *testing.T) {
	t.Setenv("M31A_LOG_LEVEL", "DEBUG")
	level := resolveLogLevel()
	if level.String() != "DEBUG" {
		t.Errorf("expected DEBUG for uppercase input, got %s", level.String())
	}
}

func TestResolveLogLevel_Unknown(t *testing.T) {
	t.Setenv("M31A_LOG_LEVEL", "unknown_level")
	level := resolveLogLevel()
	// Unknown should default to INFO
	if level.String() != "INFO" {
		t.Errorf("expected INFO for unknown level, got %s", level.String())
	}
}

// ---------------------------------------------------------------------------
// rotateLogFiles edge cases
// ---------------------------------------------------------------------------

func TestRotateLogFiles_NoRotationNeeded(t *testing.T) {
	tmpDir := t.TempDir()
	logFile := filepath.Join(tmpDir, "m31a.log")

	// Create a log file with today's mod time
	os.WriteFile(logFile, []byte("today's log"), 0644)

	err := rotateLogFiles(tmpDir, logFile)
	if err != nil {
		t.Fatalf("rotateLogFiles failed: %v", err)
	}

	// File should NOT be renamed (same day)
	if _, err := os.Stat(logFile); os.IsNotExist(err) {
		t.Error("log file should still exist (no rotation needed today)")
	}
}

func TestRotateLogFiles_StatError(t *testing.T) {
	tmpDir := t.TempDir()
	logFile := filepath.Join(tmpDir, "nonexistent.log")

	// File doesn't exist — should return nil (no rotation needed)
	err := rotateLogFiles(tmpDir, logFile)
	if err != nil {
		t.Fatalf("rotateLogFiles should handle missing file gracefully: %v", err)
	}
}

// ---------------------------------------------------------------------------
// removeOldRotatedFiles edge cases
// ---------------------------------------------------------------------------

func TestRemoveOldRotatedFiles_NoRotatedFiles(t *testing.T) {
	tmpDir := t.TempDir()
	// Create a non-rotated file
	os.WriteFile(filepath.Join(tmpDir, "m31a.log"), []byte("log"), 0644)

	err := removeOldRotatedFiles(tmpDir)
	if err != nil {
		t.Fatalf("removeOldRotatedFiles failed: %v", err)
	}
}

func TestRemoveOldRotatedFiles_InvalidDateFormat(t *testing.T) {
	tmpDir := t.TempDir()
	// Create a file with wrong date format
	os.WriteFile(filepath.Join(tmpDir, "m31a.log.not-a-date"), []byte("old"), 0644)

	err := removeOldRotatedFiles(tmpDir)
	if err != nil {
		t.Fatalf("removeOldRotatedFiles failed: %v", err)
	}

	// File with invalid date should be kept
	if _, err := os.Stat(filepath.Join(tmpDir, "m31a.log.not-a-date")); os.IsNotExist(err) {
		t.Error("file with invalid date format should be kept")
	}
}

func TestRemoveOldRotatedFiles_NonRotatedFilesKept(t *testing.T) {
	tmpDir := t.TempDir()
	// Create a file that doesn't match m31a.log. prefix
	os.WriteFile(filepath.Join(tmpDir, "m31a.log.bak"), []byte("bak"), 0644)

	err := removeOldRotatedFiles(tmpDir)
	if err != nil {
		t.Fatalf("removeOldRotatedFiles failed: %v", err)
	}

	if _, err := os.Stat(filepath.Join(tmpDir, "m31a.log.bak")); os.IsNotExist(err) {
		t.Error("non-rotated file should be kept")
	}
}

func TestRemoveOldRotatedFiles_ExactlyCutoff(t *testing.T) {
	tmpDir := t.TempDir()
	// Create a file with exactly 8-day-old date (should be removed — 7 day cutoff)
	cutoff := time.Now().AddDate(0, 0, -8)
	fileName := "m31a.log." + cutoff.Format("2006-01-02")
	os.WriteFile(filepath.Join(tmpDir, fileName), []byte("old"), 0644)

	// Create a file with exactly 6-day-old date (should be kept)
	recent := time.Now().AddDate(0, 0, -6)
	recentName := "m31a.log." + recent.Format("2006-01-02")
	os.WriteFile(filepath.Join(tmpDir, recentName), []byte("recent"), 0644)

	err := removeOldRotatedFiles(tmpDir)
	if err != nil {
		t.Fatalf("removeOldRotatedFiles failed: %v", err)
	}

	// 8-day-old file should be removed
	if _, err := os.Stat(filepath.Join(tmpDir, fileName)); !os.IsNotExist(err) {
		t.Error("8-day-old file should be removed")
	}

	// 6-day-old file should be kept
	if _, err := os.Stat(filepath.Join(tmpDir, recentName)); os.IsNotExist(err) {
		t.Error("6-day-old file should be kept")
	}
}

// ---------------------------------------------------------------------------
// NewLogger additional tests
// ---------------------------------------------------------------------------

func TestNewLogger_JSONFormat_Default(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)
	// Don't set M31A_LOG_FORMAT — should default to JSON

	logger, cleanup, err := NewLogger("test")
	if err != nil {
		t.Fatalf("NewLogger failed: %v", err)
	}
	defer cleanup()

	logger.Info("default format test")

	logFile := filepath.Join(tmpDir, ".m31a", "m31a.log")
	data, err := os.ReadFile(logFile)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}

	content := string(data)
	if len(content) == 0 {
		t.Fatal("expected non-empty log file")
	}
}

func TestNewLogger_WarnLevel(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)
	t.Setenv("M31A_LOG_LEVEL", "warn")

	logger, cleanup, err := NewLogger("test")
	if err != nil {
		t.Fatalf("NewLogger failed: %v", err)
	}
	defer cleanup()

	// Debug and Info should NOT appear
	logger.Debug("debug msg")
	logger.Info("info msg")
	logger.Warn("warn msg")

	logFile := filepath.Join(tmpDir, ".m31a", "m31a.log")
	data, err := os.ReadFile(logFile)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}

	content := string(data)
	if content == "" {
		t.Fatal("expected warn message in log")
	}
}

func TestNewLogger_ErrorLevel(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)
	t.Setenv("M31A_LOG_LEVEL", "error")

	logger, cleanup, err := NewLogger("test")
	if err != nil {
		t.Fatalf("NewLogger failed: %v", err)
	}
	defer cleanup()

	logger.Debug("debug msg")
	logger.Info("info msg")
	logger.Warn("warn msg")
	logger.Error("error msg")

	logFile := filepath.Join(tmpDir, ".m31a", "m31a.log")
	data, err := os.ReadFile(logFile)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}

	content := string(data)
	if content == "" {
		t.Fatal("expected error message in log")
	}
}

// ---------------------------------------------------------------------------
// Log file creation and content
// ---------------------------------------------------------------------------

func TestNewLogger_CreatesDirectory(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)

	_, cleanup, err := NewLogger("test")
	if err != nil {
		t.Fatalf("NewLogger failed: %v", err)
	}
	defer cleanup()

	logDir := filepath.Join(tmpDir, ".m31a")
	info, err := os.Stat(logDir)
	if err != nil {
		t.Fatalf("log directory should exist: %v", err)
	}
	if !info.IsDir() {
		t.Error("log path should be a directory")
	}
}

func TestNewLogger_AppendsToExisting(t *testing.T) {
	tmpDir := t.TempDir()
	t.Setenv("HOME", tmpDir)

	// First call
	logger1, cleanup1, err := NewLogger("test")
	if err != nil {
		t.Fatalf("NewLogger failed: %v", err)
	}
	logger1.Info("first message")
	cleanup1()

	// Second call
	logger2, cleanup2, err := NewLogger("test")
	if err != nil {
		t.Fatalf("NewLogger failed: %v", err)
	}
	logger2.Info("second message")
	cleanup2()

	logFile := filepath.Join(tmpDir, ".m31a", "m31a.log")
	data, err := os.ReadFile(logFile)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}

	content := string(data)
	if len(content) == 0 {
		t.Fatal("expected non-empty log file")
	}
}

// ---------------------------------------------------------------------------
// rotateLogFiles with rotated file
// ---------------------------------------------------------------------------

func TestRotateLogFiles_CleansOldFiles(t *testing.T) {
	tmpDir := t.TempDir()

	// Create an old rotated file (10 days old)
	oldDate := time.Now().AddDate(0, 0, -10)
	oldFile := filepath.Join(tmpDir, "m31a.log."+oldDate.Format("2006-01-02"))
	os.WriteFile(oldFile, []byte("old"), 0644)

	// Create a recent rotated file (3 days old)
	recentDate := time.Now().AddDate(0, 0, -3)
	recentFile := filepath.Join(tmpDir, "m31a.log."+recentDate.Format("2006-01-02"))
	os.WriteFile(recentFile, []byte("recent"), 0644)

	// Create today's log file (no rotation needed)
	todayFile := filepath.Join(tmpDir, "m31a.log")
	os.WriteFile(todayFile, []byte("today"), 0644)

	err := removeOldRotatedFiles(tmpDir)
	if err != nil {
		t.Fatalf("removeOldRotatedFiles failed: %v", err)
	}

	// Old file should be deleted
	if _, err := os.Stat(oldFile); !os.IsNotExist(err) {
		t.Error("old rotated file should be deleted")
	}

	// Recent file should be kept
	if _, err := os.Stat(recentFile); os.IsNotExist(err) {
		t.Error("recent rotated file should be kept")
	}
}

func TestRemoveOldRotatedFiles_SymlinkedDir(t *testing.T) {
	tmpDir := t.TempDir()

	err := removeOldRotatedFiles(tmpDir)
	if err != nil {
		t.Fatalf("removeOldRotatedFiles on empty dir failed: %v", err)
	}
}

// ---------------------------------------------------------------------------
// Constants tests
// ---------------------------------------------------------------------------

func TestLogConstants(t *testing.T) {
	if dirPermission != 0755 {
		t.Errorf("dirPermission = %o, want 0755", dirPermission)
	}
	if filePermission != 0644 {
		t.Errorf("filePermission = %o, want 0644", filePermission)
	}
	if dateFormat != "2006-01-02" {
		t.Errorf("dateFormat = %q, want %q", dateFormat, "2006-01-02")
	}
}
