package types

import (
	"errors"
	"testing"
)

// ---------------------------------------------------------------------------
// WorkflowModeForIntent tests
// ---------------------------------------------------------------------------

func TestWorkflowModeForIntent(t *testing.T) {
	tests := []struct {
		name string
		ir   IntentResult
		want WorkflowMode
	}{
		{
			"chore returns direct",
			IntentResult{Intent: IntentChore},
			ModeDirect,
		},
		{
			"feature with trivial complexity returns direct",
			IntentResult{Intent: IntentFeature, Complexity: ComplexityTrivial},
			ModeDirect,
		},
		{
			"feature with simple complexity returns fast",
			IntentResult{Intent: IntentFeature, Complexity: ComplexitySimple},
			ModeFast,
		},
		{
			"feature with moderate complexity returns full",
			IntentResult{Intent: IntentFeature, Complexity: ComplexityModerate},
			ModeFull,
		},
		{
			"feature with complex complexity returns full",
			IntentResult{Intent: IntentFeature, Complexity: ComplexityComplex},
			ModeFull,
		},
		{
			"bugfix delegates to complexity",
			IntentResult{Intent: IntentBugfix, Complexity: ComplexitySimple},
			ModeFast,
		},
		{
			"refactor delegates to complexity",
			IntentResult{Intent: IntentRefactor, Complexity: ComplexityTrivial},
			ModeDirect,
		},
		{
			"question returns full",
			IntentResult{Intent: IntentQuestion},
			ModeFull,
		},
		{
			"explanation returns full",
			IntentResult{Intent: IntentExplanation},
			ModeFull,
		},
		{
			"exploration returns full",
			IntentResult{Intent: IntentExploration},
			ModeFull,
		},
		{
			"unknown intent returns full",
			IntentResult{Intent: "unknown"},
			ModeFull,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := WorkflowModeForIntent(tt.ir)
			if got != tt.want {
				t.Errorf("WorkflowModeForIntent() = %q, want %q", got, tt.want)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// WorkflowModeForIntentComplexity tests
// ---------------------------------------------------------------------------

func TestWorkflowModeForIntentComplexity(t *testing.T) {
	tests := []struct {
		name       string
		complexity ComplexityLevel
		want       WorkflowMode
	}{
		{"trivial", ComplexityTrivial, ModeDirect},
		{"simple", ComplexitySimple, ModeFast},
		{"moderate", ComplexityModerate, ModeFull},
		{"complex", ComplexityComplex, ModeFull},
		{"empty", "", ModeFull},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := WorkflowModeForIntentComplexity(tt.complexity)
			if got != tt.want {
				t.Errorf("WorkflowModeForIntentComplexity() = %q, want %q", got, tt.want)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// IsWorkflowWorthy tests
// ---------------------------------------------------------------------------

func TestIsWorkflowWorthy(t *testing.T) {
	tests := []struct {
		name string
		ir   IntentResult
		want bool
	}{
		{"feature", IntentResult{Intent: IntentFeature}, true},
		{"bugfix", IntentResult{Intent: IntentBugfix}, true},
		{"refactor", IntentResult{Intent: IntentRefactor}, true},
		{"chore", IntentResult{Intent: IntentChore}, true},
		{"question", IntentResult{Intent: IntentQuestion}, false},
		{"explanation", IntentResult{Intent: IntentExplanation}, false},
		{"exploration", IntentResult{Intent: IntentExploration}, false},
		{"empty", IntentResult{}, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.ir.IsWorkflowWorthy(); got != tt.want {
				t.Errorf("IsWorkflowWorthy() = %v, want %v", got, tt.want)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// NewToolError tests
// ---------------------------------------------------------------------------

func TestNewToolError(t *testing.T) {
	err := errors.New("something went wrong")
	te := NewToolError(err, "try doing X")

	if te.Err != err {
		t.Errorf("Err = %v, want %v", te.Err, err)
	}
	if te.Hint != "try doing X" {
		t.Errorf("Hint = %q, want %q", te.Hint, "try doing X")
	}
}

func TestToolError_Error_WithHint(t *testing.T) {
	err := errors.New("something went wrong")
	te := NewToolError(err, "try doing X")

	got := te.Error()
	want := "something went wrong\nHint: try doing X"
	if got != want {
		t.Errorf("Error() = %q, want %q", got, want)
	}
}

func TestToolError_Error_WithoutHint(t *testing.T) {
	err := errors.New("something went wrong")
	te := NewToolError(err, "")

	got := te.Error()
	want := "something went wrong"
	if got != want {
		t.Errorf("Error() = %q, want %q", got, want)
	}
}

func TestToolError_Unwrap(t *testing.T) {
	err := errors.New("inner error")
	te := NewToolError(err, "hint")

	if !errors.Is(te, err) {
		t.Error("errors.Is should find the inner error")
	}

	var unwrapped *ToolError
	if !errors.As(te, &unwrapped) {
		t.Error("errors.As should find *ToolError")
	}
}

// ---------------------------------------------------------------------------
// IntentType constants
// ---------------------------------------------------------------------------

func TestIntentType_Constants(t *testing.T) {
	if IntentFeature != "feature" {
		t.Errorf("IntentFeature = %q", IntentFeature)
	}
	if IntentBugfix != "bugfix" {
		t.Errorf("IntentBugfix = %q", IntentBugfix)
	}
	if IntentRefactor != "refactor" {
		t.Errorf("IntentRefactor = %q", IntentRefactor)
	}
	if IntentQuestion != "question" {
		t.Errorf("IntentQuestion = %q", IntentQuestion)
	}
	if IntentExplanation != "explanation" {
		t.Errorf("IntentExplanation = %q", IntentExplanation)
	}
	if IntentExploration != "exploration" {
		t.Errorf("IntentExploration = %q", IntentExploration)
	}
	if IntentChore != "chore" {
		t.Errorf("IntentChore = %q", IntentChore)
	}
}

// ---------------------------------------------------------------------------
// WorkflowPhase constants
// ---------------------------------------------------------------------------

func TestWorkflowPhase_Constants(t *testing.T) {
	phases := map[WorkflowPhase]string{
		PhaseIdle:       "idle",
		PhaseInitialize: "initialize",
		PhaseDiscuss:    "discuss",
		PhasePlan:       "plan",
		PhaseExecute:    "execute",
		PhaseVerify:     "verify",
		PhaseRuntime:    "runtime",
		PhaseShip:       "ship",
	}
	for phase, want := range phases {
		if string(phase) != want {
			t.Errorf("Phase = %q, want %q", phase, want)
		}
	}
}

// ---------------------------------------------------------------------------
// WorkflowMode constants
// ---------------------------------------------------------------------------

func TestWorkflowMode_Constants(t *testing.T) {
	modes := map[WorkflowMode]string{
		ModeAuto:   "auto",
		ModeFull:   "full",
		ModeFast:   "fast",
		ModeDirect: "direct",
	}
	for mode, want := range modes {
		if string(mode) != want {
			t.Errorf("Mode = %q, want %q", mode, want)
		}
	}
}

// ---------------------------------------------------------------------------
// ComplexityLevel constants
// ---------------------------------------------------------------------------

func TestComplexityLevel_Constants(t *testing.T) {
	levels := map[ComplexityLevel]string{
		ComplexityTrivial:  "trivial",
		ComplexitySimple:   "simple",
		ComplexityModerate: "moderate",
		ComplexityComplex:  "complex",
	}
	for level, want := range levels {
		if string(level) != want {
			t.Errorf("Level = %q, want %q", level, want)
		}
	}
}

// ---------------------------------------------------------------------------
// RiskLevel constants
// ---------------------------------------------------------------------------

func TestRiskLevel_Constants(t *testing.T) {
	levels := map[RiskLevel]string{
		RiskSafe:        "safe",
		RiskMedium:      "medium",
		RiskDangerous:   "dangerous",
		RiskDestructive: "destructive",
	}
	for level, want := range levels {
		if string(level) != want {
			t.Errorf("Level = %q, want %q", level, want)
		}
	}
}

// ---------------------------------------------------------------------------
// TaskStatus constants
// ---------------------------------------------------------------------------

func TestTaskStatus_Constants(t *testing.T) {
	statuses := map[TaskStatus]string{
		StatusPending:       "pending",
		StatusRunning:       "running",
		StatusDone:          "done",
		StatusFailed:        "failed",
		StatusSkipped:       "skipped",
		StatusUnrecoverable: "unrecoverable",
	}
	for status, want := range statuses {
		if string(status) != want {
			t.Errorf("Status = %q, want %q", status, want)
		}
	}
}
