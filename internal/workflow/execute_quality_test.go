package workflow

import (
	"os"
	"path/filepath"
	"testing"
)

func TestExtractKeyPhrase(t *testing.T) {
	tests := []struct {
		name      string
		criterion string
		wantMax   int
	}{
		{
			name:      "simple criterion",
			criterion: "file contains hello world",
			wantMax:   50,
		},
		{
			name:      "with verify prefix",
			criterion: "Verify that the API returns 200",
			wantMax:   50,
		},
		{
			name:      "with check prefix",
			criterion: "Check that authentication works",
			wantMax:   50,
		},
		{
			name:      "with ensure prefix",
			criterion: "Ensure that database connection is stable",
			wantMax:   50,
		},
		{
			name:      "with confirm prefix",
			criterion: "Confirm that tests pass",
			wantMax:   50,
		},
		{
			name:      "with assert prefix",
			criterion: "Assert that response is valid",
			wantMax:   50,
		},
		{
			name:      "with validate prefix",
			criterion: "Validate that input is sanitized",
			wantMax:   50,
		},
		{
			name:      "long criterion truncated",
			criterion: "This is a very long criterion that should definitely be truncated because it exceeds the maximum length limit of fifty characters",
			wantMax:   50,
		},
		{
			name:      "empty criterion",
			criterion: "",
			wantMax:   50,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := extractKeyPhrase(tt.criterion)
			if len(got) > tt.wantMax {
				t.Errorf("extractKeyPhrase() returned %d chars, max %d: %q", len(got), tt.wantMax, got)
			}
		})
	}
}

func TestFileContains(t *testing.T) {
	// Create a temporary file for testing
	tmpDir := t.TempDir()
	content := "Hello, World! This is a test file with some content."
	if err := os.WriteFile(filepath.Join(tmpDir, "test.txt"), []byte(content), 0644); err != nil {
		t.Fatalf("failed to write test file: %v", err)
	}

	tests := []struct {
		name     string
		workDir  string
		filename string
		substr   string
		want     bool
	}{
		{
			name:     "substring exists",
			workDir:  tmpDir,
			filename: "test.txt",
			substr:   "Hello",
			want:     true,
		},
		{
			name:     "substring exists case insensitive",
			workDir:  tmpDir,
			filename: "test.txt",
			substr:   "hello",
			want:     true,
		},
		{
			name:     "substring not exists",
			workDir:  tmpDir,
			filename: "test.txt",
			substr:   "Goodbye",
			want:     false,
		},
		{
			name:     "file not exists",
			workDir:  tmpDir,
			filename: "nonexistent.txt",
			substr:   "anything",
			want:     false,
		},
		{
			name:     "partial match",
			workDir:  tmpDir,
			filename: "test.txt",
			substr:   "test file",
			want:     true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := fileContains(tt.workDir, tt.filename, tt.substr)
			if got != tt.want {
				t.Errorf("fileContains(%q, %q, %q) = %v, want %v",
					tt.workDir, tt.filename, tt.substr, got, tt.want)
			}
		})
	}
}
