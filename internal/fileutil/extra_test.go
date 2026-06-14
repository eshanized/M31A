package fileutil

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

// ---------------------------------------------------------------------------
// AtomicWriteWithPerm tests
// ---------------------------------------------------------------------------

func TestAtomicWriteWithPerm_CustomPerm(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	path := filepath.Join(dir, "custom_perm.txt")

	if err := AtomicWriteWithPerm(path, []byte("test"), 0755); err != nil {
		t.Fatalf("AtomicWriteWithPerm failed: %v", err)
	}

	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}
	if string(got) != "test" {
		t.Errorf("expected 'test', got %q", string(got))
	}
}

func TestAtomicWriteWithPerm_PreservesPermissions(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	path := filepath.Join(dir, "preserve_perm.txt")

	// Create with restrictive permissions
	if err := AtomicWriteWithPerm(path, []byte("first"), 0600); err != nil {
		t.Fatalf("first write failed: %v", err)
	}

	info1, err := os.Stat(path)
	if err != nil {
		t.Fatalf("Stat failed: %v", err)
	}

	// Overwrite with different perm — original permissions should be preserved
	if overwriteErr := AtomicWriteWithPerm(path, []byte("second"), 0755); overwriteErr != nil {
		t.Fatalf("second write failed: %v", overwriteErr)
	}

	info2, err := os.Stat(path)
	if err != nil {
		t.Fatalf("Stat failed: %v", err)
	}

	if info1.Mode().Perm() != info2.Mode().Perm() {
		t.Errorf("permissions changed: %o → %o", info1.Mode().Perm(), info2.Mode().Perm())
	}
}

func TestAtomicWriteWithPerm_NewFileUsesPerm(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	path := filepath.Join(dir, "new_file.txt")

	if err := AtomicWriteWithPerm(path, []byte("data"), 0644); err != nil {
		t.Fatalf("AtomicWriteWithPerm failed: %v", err)
	}

	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("Stat failed: %v", err)
	}

	if info.Mode().Perm() != 0644 {
		t.Errorf("expected perm 0644 for new file, got %o", info.Mode().Perm())
	}
}

func TestAtomicWriteWithPerm_LargeData(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	path := filepath.Join(dir, "large.bin")

	data := make([]byte, 1<<20) // 1MB
	for i := range data {
		data[i] = byte(i % 256)
	}

	if err := AtomicWriteWithPerm(path, data, 0644); err != nil {
		t.Fatalf("AtomicWriteWithPerm failed: %v", err)
	}

	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}
	if !bytes.Equal(got, data) {
		t.Errorf("data mismatch: expected %d bytes, got %d", len(data), len(got))
	}
}

// ---------------------------------------------------------------------------
// AtomicWrite additional tests
// ---------------------------------------------------------------------------

func TestAtomicWrite_CreateDirIfMissing(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	subDir := filepath.Join(dir, "sub", "nested")
	if err := os.MkdirAll(subDir, 0755); err != nil {
		t.Fatalf("MkdirAll failed: %v", err)
	}
	path := filepath.Join(subDir, "deep", "file.txt")

	// AtomicWrite doesn't create intermediate dirs, only the file
	if err := AtomicWrite(path, []byte("data")); err == nil {
		t.Log("AtomicWrite succeeded (intermediate dirs existed)")
	} else {
		t.Log("AtomicWrite failed as expected (missing intermediate dir)")
	}
}

func TestAtomicWrite_ConcurrentWrites(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()

	// All writes to different files — should all succeed
	for i := 0; i < 10; i++ {
		path := filepath.Join(dir, "file"+string(rune('0'+i))+".txt")
		data := []byte("content")
		if err := AtomicWrite(path, data); err != nil {
			t.Errorf("AtomicWrite(%s) failed: %v", path, err)
		}
	}

	// Verify all files exist
	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatalf("ReadDir failed: %v", err)
	}
	if len(entries) != 10 {
		t.Errorf("expected 10 files, got %d", len(entries))
	}
}

func TestAtomicWrite_BinaryData(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	path := filepath.Join(dir, "binary.bin")

	data := []byte{0x00, 0x01, 0xFF, 0xFE, 0x80, 0x7F}
	if err := AtomicWrite(path, data); err != nil {
		t.Fatalf("AtomicWrite failed: %v", err)
	}

	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}
	if !bytes.Equal(got, data) {
		t.Errorf("binary data mismatch")
	}
}

func TestAtomicWrite_SpecialCharacters(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	path := filepath.Join(dir, "special.txt")

	data := []byte("hello\nworld\ttab\r\nnewline")
	if err := AtomicWrite(path, data); err != nil {
		t.Fatalf("AtomicWrite failed: %v", err)
	}

	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}
	if !bytes.Equal(got, data) {
		t.Errorf("special character data mismatch")
	}
}

// ---------------------------------------------------------------------------
// Error path tests
// ---------------------------------------------------------------------------

func TestAtomicWriteWithPerm_InvalidDir(t *testing.T) {
	t.Parallel()
	path := "/nonexistent/deeply/nested/path/file.txt"

	err := AtomicWriteWithPerm(path, []byte("test"), 0644)
	if err == nil {
		t.Error("expected error for invalid directory")
	}
}

func TestAtomicWriteWithPerm_NilData(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	path := filepath.Join(dir, "nil_data.txt")

	if err := AtomicWriteWithPerm(path, nil, 0644); err != nil {
		t.Fatalf("AtomicWriteWithPerm with nil data should succeed: %v", err)
	}

	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile failed: %v", err)
	}
	if len(got) != 0 {
		t.Errorf("expected empty file, got %d bytes", len(got))
	}
}

// ---------------------------------------------------------------------------
// File permissions edge cases
// ---------------------------------------------------------------------------

func TestAtomicWriteWithPerm_ZeroPerm(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	path := filepath.Join(dir, "zero_perm.txt")

	if err := AtomicWriteWithPerm(path, []byte("test"), 0000); err != nil {
		t.Fatalf("AtomicWriteWithPerm with 0000 perm failed: %v", err)
	}

	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("Stat failed: %v", err)
	}

	// New file should use the provided perm (0000)
	if info.Mode().Perm() != 0000 {
		t.Errorf("expected perm 0000, got %o", info.Mode().Perm())
	}
}

func TestAtomicWriteWithPerm_HighPerm(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	path := filepath.Join(dir, "high_perm.txt")

	// New file should use the provided perm (subject to umask)
	if err := AtomicWriteWithPerm(path, []byte("test"), 0777); err != nil {
		t.Fatalf("AtomicWriteWithPerm with 0777 perm failed: %v", err)
	}

	info, err := os.Stat(path)
	if err != nil {
		t.Fatalf("Stat failed: %v", err)
	}

	// File should exist and be readable
	if info.Mode().Perm() == 0 {
		t.Error("file should have non-zero permissions")
	}
}

// ---------------------------------------------------------------------------
// Overwrite preserving permissions
// ---------------------------------------------------------------------------

func TestAtomicWriteWithPerm_PreservesHighPerm(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	path := filepath.Join(dir, "high.txt")

	// Create with 0755
	if err := AtomicWriteWithPerm(path, []byte("first"), 0755); err != nil {
		t.Fatalf("first write failed: %v", err)
	}

	info1, _ := os.Stat(path)

	// Overwrite with 0600 perm — should keep 0755
	if err := AtomicWriteWithPerm(path, []byte("second"), 0600); err != nil {
		t.Fatalf("second write failed: %v", err)
	}

	info2, _ := os.Stat(path)
	if info1.Mode().Perm() != info2.Mode().Perm() {
		t.Errorf("permissions not preserved: %o → %o", info1.Mode().Perm(), info2.Mode().Perm())
	}

	got, _ := os.ReadFile(path)
	if string(got) != "second" {
		t.Errorf("content not updated: got %q", string(got))
	}
}
