package workflow

import (
	"testing"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

func TestCaptureDiffSummary_NilGit(t *testing.T) {
	_, err := CaptureDiffSummary(nil, "abc", "def")
	if err == nil {
		t.Error("CaptureDiffSummary should return error with nil git")
	}
}

func TestParseNameStatus(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected map[string]string
	}{
		{
			"empty",
			"",
			map[string]string{},
		},
		{
			"added",
			"A\tfile.go",
			map[string]string{"file.go": "added"},
		},
		{
			"deleted",
			"D\told.go",
			map[string]string{"old.go": "deleted"},
		},
		{
			"modified",
			"M\tmain.go",
			map[string]string{"main.go": "modified"},
		},
		{
			"renamed",
			"R100\told.go\tnew.go",
			map[string]string{"old.go": "renamed"},
		},
		{
			"multiple",
			"A\tnew.go\nD\told.go\nM\tmain.go",
			map[string]string{"new.go": "added", "old.go": "deleted", "main.go": "modified"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := parseNameStatus(tt.input)
			if len(got) != len(tt.expected) {
				t.Errorf("parseNameStatus() returned %d entries, want %d", len(got), len(tt.expected))
				return
			}
			for k, v := range tt.expected {
				if got[k] != v {
					t.Errorf("parseNameStatus()[%q] = %q, want %q", k, got[k], v)
				}
			}
		})
	}
}

func TestFormatDiffSummary(t *testing.T) {
	// Nil summary
	got := FormatDiffSummary(nil)
	if got != "No file changes." {
		t.Errorf("FormatDiffSummary(nil) = %q, want %q", got, "No file changes.")
	}

	// Empty files
	empty := &m31types.DiffSummary{}
	got = FormatDiffSummary(empty)
	if got != "No file changes." {
		t.Errorf("FormatDiffSummary(empty) = %q, want %q", got, "No file changes.")
	}

	// With files
	ds := &m31types.DiffSummary{
		Additions: 10,
		Deletions: 5,
		Files: []m31types.FileDiff{
			{File: "new.go", Status: "added", Additions: 10, Deletions: 0},
			{File: "old.go", Status: "deleted", Additions: 0, Deletions: 5},
		},
	}
	got = FormatDiffSummary(ds)
	if !contains(got, "Files changed: 2") {
		t.Errorf("FormatDiffSummary should contain file count, got: %q", got)
	}
	if !contains(got, "new.go") {
		t.Errorf("FormatDiffSummary should contain file name, got: %q", got)
	}
}

func TestStatusIcon(t *testing.T) {
	tests := []struct {
		status   string
		expected string
	}{
		{"added", "[+]"},
		{"deleted", "[-]"},
		{"renamed", "[R]"},
		{"modified", "[M]"},
		{"unknown", "[M]"},
	}

	for _, tt := range tests {
		got := statusIcon(tt.status)
		if got != tt.expected {
			t.Errorf("statusIcon(%q) = %q, want %q", tt.status, got, tt.expected)
		}
	}
}

func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > 0 && containsHelper(s, substr))
}

func containsHelper(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}
