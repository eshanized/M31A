package fileops

import (
	"os"
	"path/filepath"
	"testing"
)

func TestResolveAndContainPath_Relative(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()

	resolved, err := ResolveAndContainPath("subdir/file.txt", workDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	expected := filepath.Join(workDir, "subdir", "file.txt")
	if resolved != expected {
		t.Errorf("expected %q, got %q", expected, resolved)
	}
}

func TestResolveAndContainPath_Absolute(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()

	resolved, err := ResolveAndContainPath(workDir+"/file.txt", workDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resolved != workDir+"/file.txt" {
		t.Errorf("expected %q, got %q", workDir+"/file.txt", resolved)
	}
}

func TestResolveAndContainPath_PathTraversal(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()

	_, err := ResolveAndContainPath("../etc/passwd", workDir)
	if err == nil {
		t.Error("expected error for path traversal")
	}
	if !Contains(err.Error(), "resolves outside working directory") {
		t.Errorf("expected path traversal error, got %q", err.Error())
	}
}

func TestResolveAndContainPath_SymlinkTraversal(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()

	// Create a symlink pointing outside workDir
	outsideDir := t.TempDir()
	targetFile := filepath.Join(outsideDir, "secret.txt")
	_ = os.WriteFile(targetFile, []byte("secret"), 0644)

	linkPath := filepath.Join(workDir, "link.txt")
	_ = os.Symlink(targetFile, linkPath)

	_, err := ResolveAndContainPath("link.txt", workDir)
	if err == nil {
		t.Error("expected error for symlink traversal")
	}
	if !Contains(err.Error(), "resolves outside working directory") {
		t.Errorf("expected symlink traversal error, got %q", err.Error())
	}
}

func TestResolveAndContainPathExists_ExistingFile(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	filePath := filepath.Join(workDir, "test.txt")
	_ = os.WriteFile(filePath, []byte("content"), 0644)

	resolved, err := ResolveAndContainPathExists("test.txt", workDir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if resolved != filePath {
		t.Errorf("expected %q, got %q", filePath, resolved)
	}
}

func TestResolveAndContainPathExists_Nonexistent(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()

	_, err := ResolveAndContainPathExists("nonexistent.txt", workDir)
	if err == nil {
		t.Error("expected error for nonexistent file")
	}
	if !Contains(err.Error(), "file not found") {
		t.Errorf("expected 'file not found' error, got %q", err.Error())
	}
}

func TestResolveAndContainPathExists_PathTraversal(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()

	// For non-existent files, ResolveAndContainPathExists returns "file not found"
	// because it requires the path to exist before checking containment
	_, err := ResolveAndContainPathExists("../etc/passwd", workDir)
	if err == nil {
		t.Error("expected error for path traversal")
	}
	if !Contains(err.Error(), "file not found") {
		t.Errorf("expected 'file not found' error for non-existent path, got %q", err.Error())
	}
}

func TestResolveAndContainPathExists_SymlinkTraversal(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()

	// Create a symlink pointing outside workDir
	outsideDir := t.TempDir()
	targetFile := filepath.Join(outsideDir, "secret.txt")
	_ = os.WriteFile(targetFile, []byte("secret"), 0644)

	linkPath := filepath.Join(workDir, "link.txt")
	_ = os.Symlink(targetFile, linkPath)

	_, err := ResolveAndContainPathExists("link.txt", workDir)
	if err == nil {
		t.Error("expected error for symlink traversal")
	}
	if !Contains(err.Error(), "resolves outside working directory") {
		t.Errorf("expected symlink traversal error, got %q", err.Error())
	}
}

func TestContainedInWorkDir_Valid(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()

	// Same directory
	err := ContainedInWorkDir(workDir, workDir)
	if err != nil {
		t.Errorf("workDir should be contained in workDir: %v", err)
	}

	// Subdirectory
	subDir := filepath.Join(workDir, "subdir")
	err = ContainedInWorkDir(subDir, workDir)
	if err != nil {
		t.Errorf("subdir should be contained in workDir: %v", err)
	}
}

func TestContainedInWorkDir_Invalid(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	outsideDir := t.TempDir()

	err := ContainedInWorkDir(outsideDir, workDir)
	if err == nil {
		t.Error("expected error for outside directory")
	}
	if !Contains(err.Error(), "resolves outside working directory") {
		t.Errorf("expected path traversal error, got %q", err.Error())
	}
}

func TestContainedInWorkDir_PrefixAttack(t *testing.T) {
	t.Parallel()
	workDir := t.TempDir()
	// Create a directory with workDir as prefix but outside
	workDirParent := filepath.Dir(workDir)
	attackDir := filepath.Join(workDirParent, filepath.Base(workDir)+"_attack")
	_ = os.Mkdir(attackDir, 0755)

	err := ContainedInWorkDir(attackDir, workDir)
	if err == nil {
		t.Error("expected error for prefix attack")
	}
	if !Contains(err.Error(), "resolves outside working directory") {
		t.Errorf("expected prefix attack error, got %q", err.Error())
	}
}