package workflow

import (
	"testing"

	m31types "github.com/eshanized/M31A/internal/types"
)

func TestParseNameStatus(t *testing.T) {
	tests := []struct {
		name string
		out  string
		want map[string]string
	}{
		{
			name: "added and modified",
			out:  "A\tfile1.go\nM\tfile2.go",
			want: map[string]string{"file1.go": "added", "file2.go": "modified"},
		},
		{
			name: "deleted",
			out:  "D\told_file.go",
			want: map[string]string{"old_file.go": "deleted"},
		},
		{
			name: "renamed uses old name as key",
			out:  "R100\told_name.go\tnew_name.go",
			want: map[string]string{"old_name.go": "renamed"},
		},
		{
			name: "empty input",
			out:  "",
			want: map[string]string{},
		},
		{
			name: "unknown status defaults to modified",
			out:  "X\tunknown.go",
			want: map[string]string{"unknown.go": "modified"},
		},
		{
			name: "malformed line skipped",
			out:  "A\nM\tfile.go",
			want: map[string]string{"file.go": "modified"},
		},
		{
			name: "multiple added files",
			out:  "A\tfile1.go\nA\tfile2.go",
			want: map[string]string{"file1.go": "added", "file2.go": "added"},
		},
		{
			name: "copied file",
			out:  "C100\tsource.go\tdest.go",
			want: map[string]string{"source.go": "modified"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := parseNameStatus(tt.out)
			if len(got) != len(tt.want) {
				t.Errorf("parseNameStatus() returned %d entries, want %d: got=%v", len(got), len(tt.want), got)
				return
			}
			for k, v := range tt.want {
				if got[k] != v {
					t.Errorf("parseNameStatus()[%q] = %q, want %q", k, got[k], v)
				}
			}
		})
	}
}

func TestFormatDiffSummary(t *testing.T) {
	tests := []struct {
		name    string
		summary *m31types.DiffSummary
		want    string
	}{
		{
			name:    "nil summary",
			summary: nil,
			want:    "No file changes.",
		},
		{
			name:    "empty files",
			summary: &m31types.DiffSummary{},
			want:    "No file changes.",
		},
		{
			name: "single file modified",
			summary: &m31types.DiffSummary{
				Files:     []m31types.FileDiff{{File: "main.go", Status: "modified", Additions: 10, Deletions: 5}},
				Additions: 10,
				Deletions: 5,
			},
			want: "Files changed: 1 (+10/-5)\n  [M] main.go (+10/-5)\n",
		},
		{
			name: "multiple files with different statuses",
			summary: &m31types.DiffSummary{
				Files: []m31types.FileDiff{
					{File: "new.go", Status: "added", Additions: 50, Deletions: 0},
					{File: "old.go", Status: "deleted", Additions: 0, Deletions: 30},
					{File: "mod.go", Status: "modified", Additions: 5, Deletions: 3},
				},
				Additions: 55,
				Deletions: 33,
			},
			want: "Files changed: 3 (+55/-33)\n  [+] new.go (+50/-0)\n  [-] old.go (+0/-30)\n  [M] mod.go (+5/-3)\n",
		},
		{
			name: "renamed file",
			summary: &m31types.DiffSummary{
				Files:     []m31types.FileDiff{{File: "new_name.go", Status: "renamed", Additions: 0, Deletions: 0}},
				Additions: 0,
				Deletions: 0,
			},
			want: "Files changed: 1 (+0/-0)\n  [R] new_name.go (+0/-0)\n",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := FormatDiffSummary(tt.summary)
			if got != tt.want {
				t.Errorf("FormatDiffSummary() =\n%q\nwant:\n%q", got, tt.want)
			}
		})
	}
}

func TestStatusIcon(t *testing.T) {
	tests := []struct {
		status string
		want   string
	}{
		{"added", "[+]"},
		{"deleted", "[-]"},
		{"renamed", "[R]"},
		{"modified", "[M]"},
		{"unknown", "[M]"},
		{"", "[M]"},
	}

	for _, tt := range tests {
		t.Run(tt.status, func(t *testing.T) {
			got := statusIcon(tt.status)
			if got != tt.want {
				t.Errorf("statusIcon(%q) = %q, want %q", tt.status, got, tt.want)
			}
		})
	}
}
