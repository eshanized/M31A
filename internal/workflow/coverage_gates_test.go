package workflow

import (
	"testing"

	m31types "github.com/eshanized/M31A/pkg/types"
)

func TestGranularityGate(t *testing.T) {
	plan := &m31types.Plan{
		Tasks: []m31types.Task{
			{ID: 1, Description: "Short task", Files: []string{"a.go", "b.go"}, AcceptanceCriteria: []string{"test passes"}},
			{ID: 2, Description: "Too many files", Files: []string{"a.go", "b.go", "c.go", "d.go"}, AcceptanceCriteria: []string{"build ok"}},
			{ID: 3, Description: "No files no criteria", Files: []string{}, AcceptanceCriteria: []string{}},
			{ID: 4, Description: "This is a very long task description that goes on and on and on with many many words that should definitely exceed the eighty word threshold set by the granularity gate check because we need to verify that extremely verbose task descriptions get flagged properly by the system so we keep adding more and more words to this description until it clearly surpasses the limit and triggers the expected warning from the gate which is the whole point of this particular test case so we just keep writing more words here", Files: []string{"e.go"}, AcceptanceCriteria: []string{"ok"}},
		},
	}

	issues := granularityGate(plan)

	// Should find: task 2 (too many files), task 3 (unbounded), task 4 (long description)
	hasTooManyFiles := false
	hasUnbounded := false
	hasLongDesc := false
	for _, issue := range issues {
		if issue.TaskID == 2 && issue.Category == "granularity" {
			hasTooManyFiles = true
		}
		if issue.TaskID == 3 && issue.Severity == "blocker" {
			hasUnbounded = true
		}
		if issue.TaskID == 4 && issue.Category == "granularity" {
			hasLongDesc = true
		}
	}

	if !hasTooManyFiles {
		t.Error("expected 'too many files' warning for task 2")
	}
	if !hasUnbounded {
		t.Error("expected 'unbounded task' blocker for task 3")
	}
	if !hasLongDesc {
		t.Error("expected 'long description' warning for task 4")
	}
}

func TestSecurityGate(t *testing.T) {
	plan := &m31types.Plan{
		Tasks: []m31types.Task{
			{ID: 1, Description: "Build auth middleware", Files: []string{"auth.go"}, AcceptanceCriteria: []string{"requests without token return 401"}},
		},
		ProposedChanges: []m31types.ProposedChangeGroup{
			{Changes: []m31types.ProposedChange{{File: "auth.go"}}},
		},
	}

	issues := securityGate(plan)
	// auth.go matches security keywords; task has security-related acceptance criteria
	// so "hasSecurityTests" should be true. But "hasInputValidation" is false.
	hasValidationWarning := false
	for _, issue := range issues {
		if issue.Category == "security" {
			hasValidationWarning = true
		}
	}
	if !hasValidationWarning {
		t.Error("expected security gate to flag missing input validation")
	}
}

func TestHasSecurityKeywords(t *testing.T) {
	tests := []struct {
		name string
		plan *m31types.Plan
		want bool
	}{
		{
			name: "no security",
			plan: &m31types.Plan{Tasks: []m31types.Task{{Description: "build a landing page", Files: []string{"page.html"}}}},
			want: false,
		},
		{
			name: "security in description",
			plan: &m31types.Plan{Tasks: []m31types.Task{{Description: "add JWT authentication"}}},
			want: true,
		},
		{
			name: "security in file path",
			plan: &m31types.Plan{Tasks: []m31types.Task{{Description: "update middleware", Files: []string{"middleware/auth.go"}}}},
			want: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := hasSecurityKeywords(tt.plan)
			if got != tt.want {
				t.Errorf("hasSecurityKeywords() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestGapAnalysisGate(t *testing.T) {
	plan := &m31types.Plan{
		Tasks: []m31types.Task{
			{ID: 1, Description: "Create the API endpoint", Files: []string{"api.go"}, AcceptanceCriteria: []string{"endpoint returns 200"}},
		},
		ProposedChanges: []m31types.ProposedChangeGroup{
			{Changes: []m31types.ProposedChange{
				{File: "api.go"},
				{File: "config.yaml"}, // no task covers this
			}},
		},
	}

	issues := gapAnalysisGate(plan, "build an API endpoint with configuration")

	// Should find config.yaml uncovered
	hasUncoveredFile := false
	for _, issue := range issues {
		if issue.Category == "gap" {
			hasUncoveredFile = true
		}
	}
	if !hasUncoveredFile {
		t.Error("expected gap analysis to flag uncovered config.yaml")
	}
}

func TestExtractGoalKeywords(t *testing.T) {
	keywords := extractGoalKeywords("Build a REST API with authentication and database integration")
	if len(keywords) == 0 {
		t.Error("expected non-empty keywords")
	}
	// Should contain meaningful words, not stop words
	for _, kw := range keywords {
		if len(kw) < 4 {
			t.Errorf("keyword %q is too short", kw)
		}
	}
}

func TestContainsAny(t *testing.T) {
	if !containsAny("hello world", []string{"world", "foo"}) {
		t.Error("expected true")
	}
	if containsAny("hello world", []string{"foo", "bar"}) {
		t.Error("expected false")
	}
}
