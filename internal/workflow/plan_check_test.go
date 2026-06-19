package workflow

import (
	"testing"
)

func TestParseCheckResult(t *testing.T) {
	tests := []struct {
		name       string
		content    string
		wantPassed bool
		wantIssues int
	}{
		{
			name:       "passed",
			content:    "## PLAN CHECK PASSED\nNo issues found.",
			wantPassed: true,
			wantIssues: 0,
		},
		{
			name: "blockers and warnings",
			content: `## ISSUES FOUND

### Blockers
- [B1] Task 3: granularity — Task has 5 files (>3)
- [B2] Task 7: acceptance — Missing acceptance criteria

### Warnings
- [W1] Task 1: coverage — File foo.js has no task

Summary: 2 blockers, 1 warnings`,
			wantPassed: false,
			wantIssues: 3,
		},
		{
			name: "warnings only",
			content: `## ISSUES FOUND

### Warnings
- [W1] Task 2: granularity — Description too long

Summary: 0 blockers, 1 warnings`,
			wantPassed: true,
			wantIssues: 1,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := parseCheckResult(tt.content)
			if result.Passed != tt.wantPassed {
				t.Errorf("Passed = %v, want %v", result.Passed, tt.wantPassed)
			}
			if len(result.Issues) != tt.wantIssues {
				t.Errorf("Issues count = %d, want %d", len(result.Issues), tt.wantIssues)
			}
		})
	}
}

func TestIsPlanCheckStalled(t *testing.T) {
	tests := []struct {
		current  int
		previous int
		want     bool
	}{
		{5, 5, true},
		{6, 5, true},
		{3, 5, false},
		{0, 5, false},
		{5, 0, false},
		{0, 0, false},
	}

	for _, tt := range tests {
		got := isPlanCheckStalled(tt.current, tt.previous)
		if got != tt.want {
			t.Errorf("isPlanCheckStalled(%d, %d) = %v, want %v",
				tt.current, tt.previous, got, tt.want)
		}
	}
}

func TestCountIssuesByType(t *testing.T) {
	issues := []PlanIssue{
		{Severity: "blocker", Category: "granularity"},
		{Severity: "warning", Category: "coverage"},
		{Severity: "blocker", Category: "acceptance"},
		{Severity: "warning", Category: "alignment"},
		{Severity: "warning", Category: "dependency"},
	}

	blockers, warnings := countIssuesByType(issues)
	if blockers != 2 {
		t.Errorf("blockers = %d, want 2", blockers)
	}
	if warnings != 3 {
		t.Errorf("warnings = %d, want 3", warnings)
	}
}
