package workflow

import (
	"os"
	"path/filepath"
	"testing"
)

func TestFindFreePort(t *testing.T) {
	port := findFreePort()
	if port == 0 {
		t.Error("findFreePort() returned 0, expected a valid port")
	}
	if port < 1024 || port > 65535 {
		t.Errorf("findFreePort() returned port %d, expected 1024-65535", port)
	}

	// Verify that two consecutive calls return different ports
	port2 := findFreePort()
	if port == port2 {
		t.Errorf("findFreePort() returned same port %d twice", port)
	}
}

func TestHasIndexHTML(t *testing.T) {
	// Directory with index.html
	dirWithIndex := t.TempDir()
	if err := os.WriteFile(filepath.Join(dirWithIndex, "index.html"), []byte("<html></html>"), 0644); err != nil {
		t.Fatalf("failed to write index.html: %v", err)
	}
	if !hasIndexHTML(dirWithIndex) {
		t.Error("hasIndexHTML() = false, want true for dir with index.html")
	}

	// Directory without index.html
	dirWithoutIndex := t.TempDir()
	if hasIndexHTML(dirWithoutIndex) {
		t.Error("hasIndexHTML() = true, want false for empty dir")
	}

	// Non-existent directory
	if hasIndexHTML("/nonexistent/path") {
		t.Error("hasIndexHTML() = true, want false for non-existent dir")
	}
}
