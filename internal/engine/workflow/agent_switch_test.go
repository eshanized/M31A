package workflow

import (
	"strings"
	"testing"
)

func TestBuildAgentSwitchMessage(t *testing.T) {
	tests := []struct {
		name         string
		planPath     string
		planContent  string
		wantRole     string
		wantContains []string
	}{
		{
			name:         "basic plan",
			planPath:     "/plans/session123.md",
			planContent:  "## Step 1: Create API\n\nBuild the REST API endpoint.",
			wantRole:     "user",
			wantContains: []string{"Switch to build mode", "/plans/session123.md", "Step 1: Create API"},
		},
		{
			name:         "empty plan content",
			planPath:     "/plans/empty.md",
			planContent:  "",
			wantRole:     "user",
			wantContains: []string{"Switch to build mode", "/plans/empty.md"},
		},
		{
			name:         "plan with long content gets truncated",
			planPath:     "/plans/long.md",
			planContent:  strings.Repeat("A", 5000),
			wantRole:     "user",
			wantContains: []string{"Switch to build mode", "/plans/long.md", "plan truncated"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			msg := BuildAgentSwitchMessage(tt.planPath, tt.planContent)

			if msg.Role != tt.wantRole {
				t.Errorf("Role = %q, want %q", msg.Role, tt.wantRole)
			}

			for _, s := range tt.wantContains {
				if !strings.Contains(msg.Content, s) {
					t.Errorf("Content missing %q\nContent: %s", s, msg.Content)
				}
			}
		})
	}
}

func TestTruncatePlan(t *testing.T) {
	tests := []struct {
		name    string
		content string
		max     int
		want    string
	}{
		{
			name:    "short content not truncated",
			content: "hello world",
			max:     20,
			want:    "hello world",
		},
		{
			name:    "exact length not truncated",
			content: "hello",
			max:     5,
			want:    "hello",
		},
		{
			name:    "long content truncated",
			content: "hello world this is a long plan",
			max:     5,
			want:    "hello\n\n...[plan truncated; read the plan file for full content]",
		},
		{
			name:    "empty content",
			content: "",
			max:     10,
			want:    "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := truncatePlan(tt.content, tt.max)
			if got != tt.want {
				t.Errorf("truncatePlan(%q, %d) =\n  %q\nwant:\n  %q", tt.content, tt.max, got, tt.want)
			}
		})
	}
}
