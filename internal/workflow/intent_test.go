package workflow

import (
	"testing"

	m31types "github.com/eshanized/M31A/internal/types"
)

func TestParseIntentJSON_ValidJSON(t *testing.T) {
	raw := `{"intent":"feature","complexity":"moderate","confidence":0.85,"scope":["auth","middleware"],"summary":"Add JWT authentication middleware"}`
	result, err := parseIntentJSON(raw)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Intent != m31types.IntentFeature {
		t.Errorf("intent: got %q, want %q", result.Intent, m31types.IntentFeature)
	}
	if result.Complexity != m31types.ComplexityModerate {
		t.Errorf("complexity: got %q, want %q", result.Complexity, m31types.ComplexityModerate)
	}
	if result.Confidence != 0.85 {
		t.Errorf("confidence: got %f, want 0.85", result.Confidence)
	}
	if len(result.Scope) != 2 {
		t.Errorf("scope: got %d items, want 2", len(result.Scope))
	}
}

func TestParseIntentJSON_MarkdownFenced(t *testing.T) {
	raw := "```json\n{\"intent\":\"bugfix\",\"complexity\":\"simple\",\"confidence\":0.9,\"scope\":[],\"summary\":\"Fix login crash\"}\n```"
	result, err := parseIntentJSON(raw)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Intent != m31types.IntentBugfix {
		t.Errorf("intent: got %q, want %q", result.Intent, m31types.IntentBugfix)
	}
}

func TestParseIntentJSON_WithSurroundingText(t *testing.T) {
	raw := "Here is the classification: {\"intent\":\"question\",\"complexity\":\"trivial\",\"confidence\":0.95,\"scope\":[],\"summary\":\"How does routing work?\"} Hope this helps!"
	result, err := parseIntentJSON(raw)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Intent != m31types.IntentQuestion {
		t.Errorf("intent: got %q, want %q", result.Intent, m31types.IntentQuestion)
	}
}

func TestParseIntentJSON_InvalidConfidence(t *testing.T) {
	raw := `{"intent":"chore","complexity":"trivial","confidence":5.0,"scope":[],"summary":"bump version"}`
	result, err := parseIntentJSON(raw)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Confidence != 0.5 {
		t.Errorf("confidence: got %f, want 0.5 (clamped)", result.Confidence)
	}
}

func TestParseIntentJSON_EmptySummary(t *testing.T) {
	raw := `{"intent":"refactor","complexity":"moderate","confidence":0.7,"scope":[],"summary":""}`
	result, err := parseIntentJSON(raw)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if result.Summary == "" {
		t.Error("summary should not be empty after normalization")
	}
}

func TestParseIntentJSON_NoJSON(t *testing.T) {
	_, err := parseIntentJSON("I think this is a feature request")
	if err == nil {
		t.Fatal("expected error for non-JSON input")
	}
}

func TestNormalizeIntent(t *testing.T) {
	cases := []struct {
		input string
		want  m31types.IntentType
	}{
		{"feature", m31types.IntentFeature},
		{"new_feature", m31types.IntentFeature},
		{"add_feature", m31types.IntentFeature},
		{"bugfix", m31types.IntentBugfix},
		{"bug_fix", m31types.IntentBugfix},
		{"fix", m31types.IntentBugfix},
		{"refactor", m31types.IntentRefactor},
		{"restructure", m31types.IntentRefactor},
		{"question", m31types.IntentQuestion},
		{"ask", m31types.IntentQuestion},
		{"explanation", m31types.IntentExplanation},
		{"explain", m31types.IntentExplanation},
		{"exploration", m31types.IntentExploration},
		{"debug", m31types.IntentExploration},
		{"chore", m31types.IntentChore},
		{"maintenance", m31types.IntentChore},
		{"unknown", m31types.IntentQuestion},
	}
	for _, tc := range cases {
		t.Run(tc.input, func(t *testing.T) {
			got := normalizeIntent(m31types.IntentType(tc.input))
			if got != tc.want {
				t.Errorf("normalizeIntent(%q): got %q, want %q", tc.input, got, tc.want)
			}
		})
	}
}

func TestNormalizeComplexity(t *testing.T) {
	cases := []struct {
		input string
		want  m31types.ComplexityLevel
	}{
		{"trivial", m31types.ComplexityTrivial},
		{"very_simple", m31types.ComplexityTrivial},
		{"simple", m31types.ComplexitySimple},
		{"easy", m31types.ComplexitySimple},
		{"moderate", m31types.ComplexityModerate},
		{"medium", m31types.ComplexityModerate},
		{"complex", m31types.ComplexityComplex},
		{"hard", m31types.ComplexityComplex},
		{"gibberish", m31types.ComplexityModerate},
	}
	for _, tc := range cases {
		t.Run(tc.input, func(t *testing.T) {
			got := normalizeComplexity(m31types.ComplexityLevel(tc.input))
			if got != tc.want {
				t.Errorf("normalizeComplexity(%q): got %q, want %q", tc.input, got, tc.want)
			}
		})
	}
}

func TestShouldStartWorkflow(t *testing.T) {
	cases := []struct {
		name   string
		result *m31types.IntentResult
		want   bool
	}{
		{"nil result", nil, false},
		{"feature high confidence", &m31types.IntentResult{Intent: m31types.IntentFeature, Confidence: 0.8}, true},
		{"feature low confidence", &m31types.IntentResult{Intent: m31types.IntentFeature, Confidence: 0.5}, false},
		{"question high confidence", &m31types.IntentResult{Intent: m31types.IntentQuestion, Confidence: 0.9}, false},
		{"bugfix high confidence", &m31types.IntentResult{Intent: m31types.IntentBugfix, Confidence: 0.7}, true},
		{"refactor high confidence", &m31types.IntentResult{Intent: m31types.IntentRefactor, Confidence: 0.85}, true},
		{"chore high confidence", &m31types.IntentResult{Intent: m31types.IntentChore, Confidence: 0.7}, true},
		{"exploration high confidence", &m31types.IntentResult{Intent: m31types.IntentExploration, Confidence: 0.9}, false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := ShouldStartWorkflow(tc.result)
			if got != tc.want {
				t.Errorf("ShouldStartWorkflow: got %v, want %v", got, tc.want)
			}
		})
	}
}

func TestGuessIntentFromKeywords(t *testing.T) {
	cases := []struct {
		input string
		want  m31types.IntentType
	}{
		{"how does authentication work?", m31types.IntentQuestion},
		{"what is the purpose of this function?", m31types.IntentQuestion},
		{"fix the login crash", m31types.IntentBugfix},
		{"the app crashes on startup", m31types.IntentBugfix},
		{"bump version to 2.0", m31types.IntentChore},
		{"update dependency versions", m31types.IntentChore},
		{"refactor the auth module", m31types.IntentRefactor},
		{"clean up the unused imports", m31types.IntentRefactor},
		{"add a search feature", m31types.IntentFeature},
		{"implement user profiles", m31types.IntentFeature},
	}
	for _, tc := range cases {
		t.Run(tc.input, func(t *testing.T) {
			got := guessIntentFromKeywords(tc.input)
			if got != tc.want {
				t.Errorf("guessIntentFromKeywords(%q): got %q, want %q", tc.input, got, tc.want)
			}
		})
	}
}

func TestFallbackClassifyIntent(t *testing.T) {
	dir := t.TempDir()
	result := FallbackClassifyIntent("bump version", dir)
	if result == nil {
		t.Fatal("FallbackClassifyIntent returned nil")
	}
	if result.Confidence != 0.5 {
		t.Errorf("confidence: got %f, want 0.5", result.Confidence)
	}
	if result.Intent != m31types.IntentChore {
		t.Errorf("intent: got %q, want %q", result.Intent, m31types.IntentChore)
	}
	if result.Complexity != m31types.ComplexityTrivial {
		t.Errorf("complexity: got %q, want %q", result.Complexity, m31types.ComplexityTrivial)
	}
}

func TestExtractJSONFromResponse(t *testing.T) {
	cases := []struct {
		name  string
		input string
		want  string
	}{
		{"plain JSON", `{"intent":"feature"}`, `{"intent":"feature"}`},
		{"fenced JSON", "```json\n{\"intent\":\"feature\"}\n```", `{"intent":"feature"}`},
		{"surrounding text", `Here: {"intent":"feature"} done`, `{"intent":"feature"}`},
		{"no JSON", "no json here", ""},
		{"empty", "", ""},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := extractJSONFromResponse(tc.input)
			if got != tc.want {
				t.Errorf("extractJSONFromResponse: got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestIsWorkflowWorthy(t *testing.T) {
	cases := []struct {
		intent m31types.IntentType
		want   bool
	}{
		{m31types.IntentFeature, true},
		{m31types.IntentBugfix, true},
		{m31types.IntentRefactor, true},
		{m31types.IntentChore, true},
		{m31types.IntentQuestion, false},
		{m31types.IntentExplanation, false},
		{m31types.IntentExploration, false},
	}
	for _, tc := range cases {
		t.Run(string(tc.intent), func(t *testing.T) {
			ir := m31types.IntentResult{Intent: tc.intent}
			if got := ir.IsWorkflowWorthy(); got != tc.want {
				t.Errorf("IsWorkflowWorthy(%q): got %v, want %v", tc.intent, got, tc.want)
			}
		})
	}
}
