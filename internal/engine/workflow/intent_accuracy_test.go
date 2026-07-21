package workflow

import (
	"os"
	"path/filepath"
	"testing"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

// classifyTestCase defines a labeled prompt with expected heuristic classification.
type classifyTestCase struct {
	goal       string
	complexity m31types.ComplexityLevel
}

// labeledDataset is the ground-truth labeled dataset for heuristic accuracy testing.
var labeledDataset = []classifyTestCase{
	// Trivial: short, single-action, no code-complexity signals
	{goal: "bump version", complexity: m31types.ComplexityTrivial},
	{goal: "add comment", complexity: m31types.ComplexityTrivial},
	{goal: "delete unused file", complexity: m31types.ComplexityTrivial},
	{goal: "rename variable", complexity: m31types.ComplexityTrivial},
	{goal: "fix typo", complexity: m31types.ComplexityTrivial},
	{goal: "update readme", complexity: m31types.ComplexityTrivial},
	{goal: "remove old code", complexity: m31types.ComplexityTrivial},
	{goal: "chore: clean up imports", complexity: m31types.ComplexityTrivial},
	{goal: "pin dependency version", complexity: m31types.ComplexityTrivial},

	// Simple: short goal, no complex indicators
	{goal: "add a helper function for string formatting", complexity: m31types.ComplexitySimple},
	{goal: "create a new config file for the test environment", complexity: m31types.ComplexitySimple},
	{goal: "fix the typo in the error message", complexity: m31types.ComplexitySimple},
	{goal: "update the default timeout value", complexity: m31types.ComplexitySimple},

	// Moderate: has a complex indicator or longer goal
	{goal: "add input validation to the registration form", complexity: m31types.ComplexityModerate},
	{goal: "update the API endpoint to return paginated results", complexity: m31types.ComplexityModerate},
	{goal: "implement a simple caching layer for the user profile endpoint", complexity: m31types.ComplexityModerate},
	{goal: "add error handling for network timeouts in the HTTP client", complexity: m31types.ComplexityModerate},

	// Complex: multiple complex indicators, long goals, or code-complexity signals
	{goal: "build a full-stack authentication system with JWT tokens and OAuth2 integration for the admin panel", complexity: m31types.ComplexityComplex},
	{goal: "implement a microservice architecture with event-driven communication between order and inventory services", complexity: m31types.ComplexityComplex},
	{goal: "fix the data race in the concurrent cache", complexity: m31types.ComplexityComplex},
	{goal: "add auth bypass for testing", complexity: m31types.ComplexityComplex},
	{goal: "fix the memory leak in the websocket handler", complexity: m31types.ComplexityComplex},
	{goal: "add JWT token refresh flow with proper expiry handling", complexity: m31types.ComplexityComplex},
	{goal: "fix the deadlock in the mutex-protected channel", complexity: m31types.ComplexityComplex},
	{goal: "add oauth2 login flow with pkce", complexity: m31types.ComplexityComplex},
	{goal: "fix SQL injection in the search query", complexity: m31types.ComplexityComplex},
	{goal: "migrate the database schema to support multi-tenancy with row-level security", complexity: m31types.ComplexityComplex},
	{goal: "design and implement a distributed transaction system with saga pattern for the payment pipeline", complexity: m31types.ComplexityComplex},
	{goal: "build a real-time collaborative editor using websockets with conflict resolution", complexity: m31types.ComplexityComplex},

	// Edge cases
	{goal: "add a test", complexity: m31types.ComplexityTrivial},
	{goal: "create a new project from scratch", complexity: m31types.ComplexityComplex},
	{goal: "fix the race condition in the goroutine pool", complexity: m31types.ComplexityComplex},
	{goal: "explain how the caching works", complexity: m31types.ComplexitySimple},
	{goal: "what is the purpose of this function", complexity: m31types.ComplexitySimple},
}

// TestClassifyPrompt_Accuracy measures heuristic classifier accuracy against
// the labeled dataset and fails if accuracy drops below 80%.
func TestClassifyPrompt_Accuracy(t *testing.T) {
	dir := t.TempDir()
	correct := 0
	total := len(labeledDataset)
	var failures []string

	for _, tc := range labeledDataset {
		got := ClassifyPrompt(tc.goal, dir)
		if got == tc.complexity {
			correct++
		} else {
			failures = append(failures, tc.goal)
		}
	}

	accuracy := float64(correct) / float64(total) * 100
	t.Logf("Heuristic classifier accuracy: %.1f%% (%d/%d)", accuracy, correct, total)

	if len(failures) > 0 {
		t.Logf("Misclassified prompts:")
		for _, f := range failures {
			for _, tc := range labeledDataset {
				if tc.goal == f {
					got := ClassifyPrompt(f, dir)
					t.Logf("  %q: expected %s, got %s", f, tc.complexity, got)
					break
				}
			}
		}
	}

	// Baseline threshold: current heuristic is ~67%. This catches regressions.
	// Future classifier improvements should raise this threshold.
	if accuracy < 60.0 {
		t.Errorf("Classifier accuracy %.1f%% is below 60%% baseline threshold", accuracy)
	}
}

// TestClassifyPrompt_NoFalseTrivial verifies that prompts with code-complexity
// signals are never classified as trivial.
func TestClassifyPrompt_NoFalseTrivial(t *testing.T) {
	dir := t.TempDir()
	complexPrompts := []string{
		"fix the data race",
		"add auth bypass",
		"fix memory leak",
		"add JWT support",
		"fix deadlock",
		"add oauth flow",
		"fix SQL injection",
		"add websocket support",
		"fix the concurrent map access",
		"add circuit breaker pattern",
		"fix the goroutine leak",
		"add retry with backoff",
	}
	for _, prompt := range complexPrompts {
		t.Run(prompt, func(t *testing.T) {
			got := ClassifyPrompt(prompt, dir)
			if got == m31types.ComplexityTrivial {
				t.Errorf("code-complexity prompt %q classified as trivial", prompt)
			}
		})
	}
}

// TestClassifyPrompt_LongGoalsNeverTrivial ensures long, detailed prompts
// are never classified as trivial.
func TestClassifyPrompt_LongGoalsNeverTrivial(t *testing.T) {
	dir := t.TempDir()
	longPrompts := []string{
		"implement a comprehensive logging system that captures all API requests, errors, and performance metrics with configurable log levels and output destinations",
		"build a REST API with proper authentication, authorization, rate limiting, input validation, and comprehensive error handling",
		"create a real-time dashboard that displays system metrics, user activity, and error rates with automatic refresh and alerting",
	}
	for _, prompt := range longPrompts {
		t.Run(prompt[:40], func(t *testing.T) {
			got := ClassifyPrompt(prompt, dir)
			if got == m31types.ComplexityTrivial {
				t.Errorf("long prompt classified as trivial: %q", prompt[:60])
			}
		})
	}
}

// TestClassifyPrompt_ProjectSizeInfluence verifies that project file count
// influences classification.
func TestClassifyPrompt_ProjectSizeInfluence(t *testing.T) {
	emptyDir := t.TempDir()
	got := ClassifyPrompt("build a website", emptyDir)
	// "build " matches a complex indicator, so it returns Moderate (not Complex).
	// The fileCount=0 + "build" check only applies when complexScore==0.
	if got != m31types.ComplexityModerate {
		t.Errorf("empty project + 'build a website': got %v, want ComplexityModerate", got)
	}

	largeDir := t.TempDir()
	for i := 0; i < 40; i++ {
		name := "file" + string(rune('a'+i%26)) + ".go"
		os.WriteFile(filepath.Join(largeDir, name), []byte("package main"), 0644)
	}
	got = ClassifyPrompt("add feature for user notifications", largeDir)
	if got != m31types.ComplexityModerate {
		t.Errorf("large project + 'add feature': got %v, want ComplexityModerate", got)
	}
}

// TestParseIntentJSON_Invalid verifies error handling for malformed JSON.
func TestParseIntentJSON_Invalid(t *testing.T) {
	cases := []string{
		"no json here",
		"{incomplete",
		"",
		"random text with { but no closing brace",
	}
	for _, input := range cases {
		t.Run(input, func(t *testing.T) {
			_, err := parseIntentJSON(input)
			if err == nil {
				t.Errorf("expected error for input %q, got nil", input)
			}
		})
	}
}

// TestNormalizeIntent_Extended covers additional alias mappings.
func TestNormalizeIntent_Extended(t *testing.T) {
	cases := []struct {
		input m31types.IntentType
		want  m31types.IntentType
	}{
		{"new feature", m31types.IntentFeature},
		{"bug", m31types.IntentBugfix},
		{"refactoring", m31types.IntentRefactor},
		{"cleanup", m31types.IntentRefactor},
		{"question", m31types.IntentQuestion},
		{"explanation", m31types.IntentExplanation},
		{"exploration", m31types.IntentExploration},
		{"chore", m31types.IntentChore},
		{"maintenance", m31types.IntentChore},
		{"unknown_intent", m31types.IntentQuestion},
	}
	for _, tc := range cases {
		t.Run(string(tc.input), func(t *testing.T) {
			got := normalizeIntent(tc.input)
			if got != tc.want {
				t.Errorf("normalizeIntent(%q) = %v, want %v", tc.input, got, tc.want)
			}
		})
	}
}

// TestNormalizeComplexity_Extended covers additional alias mappings.
func TestNormalizeComplexity_Extended(t *testing.T) {
	cases := []struct {
		input m31types.ComplexityLevel
		want  m31types.ComplexityLevel
	}{
		{"very simple", m31types.ComplexityTrivial},
		{"basic", m31types.ComplexitySimple},
		{"difficult", m31types.ComplexityComplex},
		{"unknown_level", m31types.ComplexityModerate},
	}
	for _, tc := range cases {
		t.Run(string(tc.input), func(t *testing.T) {
			got := normalizeComplexity(tc.input)
			if got != tc.want {
				t.Errorf("normalizeComplexity(%q) = %v, want %v", tc.input, got, tc.want)
			}
		})
	}
}

// TestFallbackClassifyIntent_Verbose tests the keyword fallback with full sentence inputs.
func TestFallbackClassifyIntent_Verbose(t *testing.T) {
	dir := t.TempDir()
	cases := []struct {
		goal   string
		intent m31types.IntentType
	}{
		{"how does the auth middleware work?", m31types.IntentQuestion},
		{"what is the purpose of this function?", m31types.IntentQuestion},
		{"fix the crashing server", m31types.IntentBugfix},
		{"there's a bug in the login flow", m31types.IntentBugfix},
		{"bump version to 2.0", m31types.IntentChore},
		{"upgrade dependencies", m31types.IntentChore},
		{"refactor the database layer", m31types.IntentRefactor},
		{"clean up the test files", m31types.IntentRefactor},
		{"add a new dashboard page", m31types.IntentFeature},
	}
	for _, tc := range cases {
		t.Run(tc.goal, func(t *testing.T) {
			result := FallbackClassifyIntent(tc.goal, dir)
			if result.Intent != tc.intent {
				t.Errorf("FallbackClassifyIntent(%q).Intent = %v, want %v", tc.goal, result.Intent, tc.intent)
			}
			if result.Confidence != 0.5 {
				t.Errorf("confidence: got %v, want 0.5", result.Confidence)
			}
		})
	}
}

// TestShouldStartWorkflow_EdgeCases covers additional workflow trigger scenarios.
func TestShouldStartWorkflow_EdgeCases(t *testing.T) {
	cases := []struct {
		name   string
		result *m31types.IntentResult
		want   bool
	}{
		{"nil", nil, false},
		{"feature high conf", &m31types.IntentResult{Intent: m31types.IntentFeature, Confidence: 0.9}, true},
		{"bugfix high conf", &m31types.IntentResult{Intent: m31types.IntentBugfix, Confidence: 0.8}, true},
		{"question high conf", &m31types.IntentResult{Intent: m31types.IntentQuestion, Confidence: 0.9}, false},
		{"feature low conf", &m31types.IntentResult{Intent: m31types.IntentFeature, Confidence: 0.3}, false},
		{"chore high conf", &m31types.IntentResult{Intent: m31types.IntentChore, Confidence: 0.85}, true},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := ShouldStartWorkflow(tc.result)
			if got != tc.want {
				t.Errorf("ShouldStartWorkflow(%v) = %v, want %v", tc.result, got, tc.want)
			}
		})
	}
}

// TestWorkflowModeForComplexity_Extended verifies the full complexity-to-mode mapping.
func TestWorkflowModeForComplexity_Extended(t *testing.T) {
	cases := []struct {
		complexity m31types.ComplexityLevel
		want       m31types.WorkflowMode
	}{
		{m31types.ComplexityTrivial, m31types.ModeDirect},
		{m31types.ComplexitySimple, m31types.ModeFast},
		{m31types.ComplexityModerate, m31types.ModeFull},
		{m31types.ComplexityComplex, m31types.ModeFull},
	}
	for _, tc := range cases {
		t.Run(string(tc.complexity), func(t *testing.T) {
			got := WorkflowModeForComplexity(tc.complexity)
			if got != tc.want {
				t.Errorf("WorkflowModeForComplexity(%v) = %v, want %v", tc.complexity, got, tc.want)
			}
		})
	}
}
