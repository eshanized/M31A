package tui

import (
	"testing"
)

func TestDiskUsage(t *testing.T) {
	t.Parallel()
	// diskUsage should work on the temp directory (Unix) or return error (Windows)
	dir := t.TempDir()
	totalBytes, err := diskUsage(dir)
	if err != nil {
		// On Windows, diskUsage returns an error — that's expected
		t.Skipf("diskUsage not supported on this platform: %v", err)
	}
	if totalBytes == 0 {
		t.Error("diskUsage returned 0 bytes — expected non-zero for a valid path")
	}
}

func TestDiskUsage_InvalidPath(t *testing.T) {
	t.Parallel()
	// An invalid path should return an error on Unix, or "not supported" on Windows
	_, err := diskUsage("/nonexistent/path/that/does/not/exist")
	if err == nil {
		t.Error("expected error for invalid path, got nil")
	}
}
