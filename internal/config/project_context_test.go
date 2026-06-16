package config

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestLoadProjectContext_NoContextFile(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	got, path := LoadProjectContext(dir)
	if got != "" {
		t.Errorf("expected empty content, got %q", got)
	}
	if path != "" {
		t.Errorf("expected empty path, got %q", path)
	}
}

func TestLoadProjectContext_PriorityOverM31A(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	// Create both AGENTS.md and .m31a/agents.md
	os.WriteFile(filepath.Join(dir, "AGENTS.md"), []byte("agents-content"), 0o644)
	os.MkdirAll(filepath.Join(dir, ".m31a"), 0o755)
	os.WriteFile(filepath.Join(dir, ".m31a", "agents.md"), []byte("m31a-content"), 0o644)

	got, _ := LoadProjectContext(dir)
	if got != "agents-content" {
		t.Errorf("expected AGENTS.md priority, got %q", got)
	}
}

func TestLoadProjectContext_M31AAgentsFile(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	content := "# M31A Context"
	os.MkdirAll(filepath.Join(dir, ".m31a"), 0o755)
	os.WriteFile(filepath.Join(dir, ".m31a", "agents.md"), []byte(content), 0o644)

	got, path := LoadProjectContext(dir)
	if got != content {
		t.Errorf("expected content %q, got %q", content, got)
	}
	if !strings.Contains(path, ".m31a/agents.md") {
		t.Errorf("expected path with .m31a/agents.md, got %q", path)
	}
}

func TestLoadProjectContext_TruncatesLargeFile(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	// Create a file larger than maxProjectContextBytes (8KB)
	largeContent := strings.Repeat("x", 10000)
	os.WriteFile(filepath.Join(dir, "AGENTS.md"), []byte(largeContent), 0o644)

	got, _ := LoadProjectContext(dir)
	if len(got) > maxProjectContextBytes {
		t.Errorf("expected truncation to %d bytes, got %d", maxProjectContextBytes, len(got))
	}
}

func TestLoadProjectContext_MemoryMDOnly(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	content := "# Memory File"
	os.WriteFile(filepath.Join(dir, "MEMORY.md"), []byte(content), 0o644)

	got, path := LoadProjectContext(dir)
	if got != content {
		t.Errorf("expected content %q, got %q", content, got)
	}
	if !strings.HasSuffix(path, "MEMORY.md") {
		t.Errorf("expected MEMORY.md path, got %q", path)
	}
}
