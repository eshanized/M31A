package workflow

import (
	"testing"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

func TestRequirementsCoverageGate(t *testing.T) {
	tests := []struct {
		name    string
		plan    *m31types.Plan
		goal    string
		wantMin int
		wantMax int
	}{
		{
			name: "goal keywords covered in task description",
			plan: &m31types.Plan{
				Tasks: []m31types.Task{
					{ID: 1, Description: "Build REST API endpoint", Files: []string{"api.go"}},
				},
			},
			goal:    "Build REST API endpoint",
			wantMin: 0,
			wantMax: 2,
		},
		{
			name: "goal keywords not covered",
			plan: &m31types.Plan{
				Tasks: []m31types.Task{
					{ID: 1, Description: "Create landing page", Files: []string{"index.html"}},
				},
			},
			goal:    "Build REST API with authentication",
			wantMin: 1,
			wantMax: 5,
		},
		{
			name: "empty plan",
			plan: &m31types.Plan{
				Tasks: []m31types.Task{},
			},
			goal:    "Build something",
			wantMin: 0,
			wantMax: 3,
		},
		{
			name: "single phrase goal",
			plan: &m31types.Plan{
				Tasks: []m31types.Task{
					{ID: 1, Description: "Build API", Files: []string{"api.go"}},
				},
			},
			goal:    "Build API",
			wantMin: 0,
			wantMax: 1,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			issues := requirementsCoverageGate(tt.plan, tt.goal)
			if len(issues) < tt.wantMin || len(issues) > tt.wantMax {
				t.Errorf("requirementsCoverageGate() returned %d issues, want [%d, %d]",
					len(issues), tt.wantMin, tt.wantMax)
			}
			for _, issue := range issues {
				if issue.Category != "coverage" {
					t.Errorf("issue.Category = %q, want %q", issue.Category, "coverage")
				}
			}
		})
	}
}

func TestExtractKeyPhrases(t *testing.T) {
	tests := []struct {
		name    string
		goal    string
		wantMin int
		wantMax int
	}{
		{
			name:    "short goal single word",
			goal:    "API",
			wantMin: 1,
			wantMax: 1,
		},
		{
			name:    "two words",
			goal:    "Build API",
			wantMin: 1,
			wantMax: 1,
		},
		{
			name:    "medium goal",
			goal:    "Build REST API with authentication",
			wantMin: 2,
			wantMax: 4,
		},
		{
			name:    "long goal",
			goal:    "Build a REST API with user authentication and database integration for the e-commerce platform",
			wantMin: 4,
			wantMax: 6,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			phrases := extractKeyPhrases(tt.goal)
			if len(phrases) < tt.wantMin || len(phrases) > tt.wantMax {
				t.Errorf("extractKeyPhrases(%q) returned %d phrases, want [%d, %d]: %v",
					tt.goal, len(phrases), tt.wantMin, tt.wantMax, phrases)
			}
		})
	}
}

func TestFindSecurityRelevantFiles(t *testing.T) {
	tests := []struct {
		name string
		plan *m31types.Plan
		want int
	}{
		{
			name: "security files in tasks",
			plan: &m31types.Plan{
				Tasks: []m31types.Task{
					{ID: 1, Files: []string{"auth.go", "main.go"}},
					{ID: 2, Files: []string{"handler.go", "middleware/auth.go"}},
				},
			},
			want: 2,
		},
		{
			name: "security files in proposed changes",
			plan: &m31types.Plan{
				Tasks: []m31types.Task{},
				ProposedChanges: []m31types.ProposedChangeGroup{
					{Changes: []m31types.ProposedChange{{File: "jwt.go"}}},
				},
			},
			want: 1,
		},
		{
			name: "no security files",
			plan: &m31types.Plan{
				Tasks: []m31types.Task{
					{ID: 1, Files: []string{"main.go", "handler.go"}},
				},
			},
			want: 0,
		},
		{
			name: "empty plan",
			plan: &m31types.Plan{},
			want: 0,
		},
		{
			name: "deduplication of same file across tasks",
			plan: &m31types.Plan{
				Tasks: []m31types.Task{
					{ID: 1, Files: []string{"auth.go"}},
					{ID: 2, Files: []string{"auth.go"}},
				},
			},
			want: 1,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := findSecurityRelevantFiles(tt.plan)
			if len(got) != tt.want {
				t.Errorf("findSecurityRelevantFiles() returned %d files, want %d: %v", len(got), tt.want, got)
			}
		})
	}
}
