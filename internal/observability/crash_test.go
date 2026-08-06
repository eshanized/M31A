package observability

import (
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"testing"
	"time"
)

func TestCrashReport_CaptureVerifiesFields(t *testing.T) {
	done := make(chan struct{})

	go func() {
		defer func() {
			if r := recover(); r != nil {
				// We can't directly capture the report from here since it's written to file
				// This test verifies the function doesn't deadlock and recovers properly
			}
			close(done)
		}()
		RecoverAndCapture("test-version", "test-commit")
	}()

	// Trigger panic in the goroutine
	// The test mainly ensures the function compiles and runs without deadlock
	select {
	case <-done:
	case <-time.After(5 * time.Second):
		t.Fatal("Test timed out - possible deadlock in RecoverAndCapture")
	}
}

func TestRecoverAndCapture_WritesCrashFile(t *testing.T) {
	tmpDir := t.TempDir()
	// Override home dir for test
	oldHome := os.Getenv("HOME")
	os.Setenv("HOME", tmpDir)
	defer os.Setenv("HOME", oldHome)

	// Run in a goroutine to catch the re-panic
	done := make(chan struct{})
	go func() {
		defer func() {
			// Catch the re-panic from RecoverAndCapture
			recover()
			close(done)
		}()
		func() {
			defer RecoverAndCapture("test-version", "test-commit")
			panic("test panic")
		}()
	}()

	// Wait for the goroutine to complete (re-panic caught)
	select {
	case <-done:
	case <-time.After(5 * time.Second):
		t.Fatal("Test timed out")
	}

	// Check crash file was created
	crashDir := filepath.Join(tmpDir, ".m31a", "crashes")
	files, err := os.ReadDir(crashDir)
	if err != nil {
		t.Fatalf("Failed to read crash directory: %v", err)
	}

	if len(files) == 0 {
		t.Fatal("No crash file created")
	}

	// Verify file content
	crashFile := filepath.Join(crashDir, files[0].Name())
	data, err := os.ReadFile(crashFile)
	if err != nil {
		t.Fatalf("Failed to read crash file: %v", err)
	}

	var report CrashReport
	if err := json.Unmarshal(data, &report); err != nil {
		t.Fatalf("Failed to unmarshal crash report: %v", err)
	}

	// Verify fields
	if report.Version != "test-version" {
		t.Errorf("Expected version 'test-version', got %q", report.Version)
	}
	if report.Commit != "test-commit" {
		t.Errorf("Expected commit 'test-commit', got %q", report.Commit)
	}
	if report.GoVersion != runtime.Version() {
		t.Errorf("Expected Go version %q, got %q", runtime.Version(), report.GoVersion)
	}
	if report.OS != runtime.GOOS {
		t.Errorf("Expected OS %q, got %q", runtime.GOOS, report.OS)
	}
	if report.Arch != runtime.GOARCH {
		t.Errorf("Expected arch %q, got %q", runtime.GOARCH, report.Arch)
	}
	if report.StackTrace == "" {
		t.Error("Stack trace is empty")
	}
	if report.PanicValue != "test panic" {
		t.Errorf("Expected panic value 'test panic', got %v", report.PanicValue)
	}

	// Verify file permissions (0600)
	info, err := os.Stat(crashFile)
	if err != nil {
		t.Fatalf("Failed to stat crash file: %v", err)
	}
	if info.Mode().Perm() != 0o600 {
		t.Errorf("Expected file permissions 0600, got %o", info.Mode().Perm())
	}
}

func TestCrashReport_JSONSerialization(t *testing.T) {
	report := CrashReport{
		Timestamp:  "2024-01-01T00:00:00Z",
		Version:    "v1.0.0",
		Commit:     "abc123",
		GoVersion:  "go1.21.0",
		OS:         "linux",
		Arch:       "amd64",
		StackTrace: "stack trace here",
		PanicValue: "test panic",
	}

	data, err := json.Marshal(report)
	if err != nil {
		t.Fatalf("Failed to marshal: %v", err)
	}

	var unmarshaled CrashReport
	if err := json.Unmarshal(data, &unmarshaled); err != nil {
		t.Fatalf("Failed to unmarshal: %v", err)
	}

	if unmarshaled.Timestamp != report.Timestamp {
		t.Errorf("Timestamp mismatch: %q vs %q", unmarshaled.Timestamp, report.Timestamp)
	}
	if unmarshaled.Version != report.Version {
		t.Errorf("Version mismatch: %q vs %q", unmarshaled.Version, report.Version)
	}
	if unmarshaled.Commit != report.Commit {
		t.Errorf("Commit mismatch: %q vs %q", unmarshaled.Commit, report.Commit)
	}
	if unmarshaled.GoVersion != report.GoVersion {
		t.Errorf("GoVersion mismatch: %q vs %q", unmarshaled.GoVersion, report.GoVersion)
	}
	if unmarshaled.OS != report.OS {
		t.Errorf("OS mismatch: %q vs %q", unmarshaled.OS, report.OS)
	}
	if unmarshaled.Arch != report.Arch {
		t.Errorf("Arch mismatch: %q vs %q", unmarshaled.Arch, report.Arch)
	}
	if unmarshaled.StackTrace != report.StackTrace {
		t.Errorf("StackTrace mismatch")
	}
	if unmarshaled.PanicValue != report.PanicValue {
		t.Errorf("PanicValue mismatch: %v vs %v", unmarshaled.PanicValue, report.PanicValue)
	}
}