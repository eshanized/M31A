package workflow

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"strings"
	"time"

	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/provider"
)

const (
	defaultIntentClassifyTimeout = 25 * time.Second
)

// ClassifyIntent uses an LLM to classify the user's input intent.
// Falls back to the keyword-based ClassifyPrompt on failure or timeout.
// timeoutSecs controls the per-attempt deadline; 0 uses the default (25s).
func ClassifyIntent(ctx context.Context, p provider.LLMProvider, modelID string, input string, prompts *PromptRegistry, timeoutSecs int) (*m31types.IntentResult, error) {
	if p == nil || modelID == "" {
		return nil, fmt.Errorf("provider or model not available")
	}

	timeout := defaultIntentClassifyTimeout
	if timeoutSecs > 0 {
		timeout = time.Duration(timeoutSecs) * time.Second
	}

	systemPrompt := prompts.IntentClassify
	if systemPrompt == "" {
		systemPrompt = defaultIntentClassifyPrompt()
	}

	messages := []m31types.Message{
		{Role: "system", Content: systemPrompt},
		{Role: "user", Content: input},
	}

	req := provider.ChatRequest{
		Model:            modelID,
		Messages:         messages,
		MaxTokens:        200,
		ReasoningEnabled: false,
	}

	retryCfg := RetryConfig{
		MaxAttempts:       3,
		BaseDelay:         2 * time.Second,
		MaxDelay:          10 * time.Second,
		BackoffMultiplier: 2.0,
	}

	return RetryWithResult(ctx, retryCfg, func() (*m31types.IntentResult, error) {
		classifyCtx, cancel := context.WithTimeout(ctx, timeout)
		defer cancel()

		iterator, err := p.ChatCompletionStream(classifyCtx, req)
		if err != nil {
			return nil, fmt.Errorf("classify LLM call failed: %w", err)
		}

		content, streamErr := consumeClassifyStream(iterator)
		if streamErr != nil {
			return nil, fmt.Errorf("classify stream read failed: %w", streamErr)
		}

		result, parseErr := parseIntentJSON(content)
		if parseErr != nil {
			return nil, fmt.Errorf("classify JSON parse failed: %w", parseErr)
		}

		return result, nil
	})
}

// consumeClassifyStream reads all chunks from the classification iterator.
func consumeClassifyStream(iterator *m31types.StreamIterator) (string, error) {
	var sb strings.Builder
	defer func() {
		if err := iterator.Close(); err != nil {
			slog.Debug("close stream iterator", "error", err, "resource", "consumeClassifyStream")
		}
	}()

	for {
		chunk, err := iterator.Next()
		if err != nil {
			if chunk != nil && chunk.Delta != "" {
				sb.WriteString(chunk.Delta)
			}
			if err.Error() == "EOF" || strings.Contains(err.Error(), "EOF") {
				return sb.String(), nil
			}
			return sb.String(), fmt.Errorf("classify stream error: %w", err)
		}
		if chunk != nil && chunk.Delta != "" {
			sb.WriteString(chunk.Delta)
			if sb.Len() > 2000 {
				return sb.String(), fmt.Errorf("classify response too large")
			}
		}
	}
}

// parseIntentJSON extracts an IntentResult from raw LLM JSON output.
func parseIntentJSON(raw string) (*m31types.IntentResult, error) {
	jsonStr := extractJSONFromResponse(raw)
	if jsonStr == "" {
		return nil, fmt.Errorf("no JSON found in response")
	}

	var result m31types.IntentResult
	if err := json.Unmarshal([]byte(jsonStr), &result); err != nil {
		return nil, fmt.Errorf("JSON unmarshal: %w", err)
	}

	result.Intent = normalizeIntent(result.Intent)
	result.Complexity = normalizeComplexity(result.Complexity)

	if result.Confidence < 0 || result.Confidence > 1 {
		result.Confidence = 0.5
	}

	if result.Summary == "" {
		result.Summary = "unclassified prompt"
	}

	return &result, nil
}

// extractJSONFromResponse finds the JSON object in the LLM response,
// handling markdown code fences and surrounding text.
func extractJSONFromResponse(raw string) string {
	raw = strings.TrimSpace(raw)

	if idx := strings.Index(raw, "```"); idx >= 0 {
		end := strings.Index(raw[idx+3:], "```")
		if end >= 0 {
			inner := raw[idx+3 : idx+3+end]
			if nlIdx := strings.Index(inner, "\n"); nlIdx >= 0 {
				inner = inner[nlIdx+1:]
			}
			return strings.TrimSpace(inner)
		}
	}

	if idx := strings.Index(raw, "{"); idx >= 0 {
		end := strings.LastIndex(raw, "}")
		if end > idx {
			return raw[idx : end+1]
		}
	}

	return ""
}

// normalizeIntent maps various LLM outputs to known intent types.
func normalizeIntent(raw m31types.IntentType) m31types.IntentType {
	switch strings.ToLower(string(raw)) {
	case "feature", "new_feature", "new feature", "add_feature":
		return m31types.IntentFeature
	case "bugfix", "bug_fix", "bug fix", "fix", "bug":
		return m31types.IntentBugfix
	case "refactor", "refactoring", "restructure", "cleanup":
		return m31types.IntentRefactor
	case "question", "ask", "query":
		return m31types.IntentQuestion
	case "explanation", "explain", "describe":
		return m31types.IntentExplanation
	case "exploration", "explore", "debug", "investigate":
		return m31types.IntentExploration
	case "chore", "maintenance", "config", "dependency":
		return m31types.IntentChore
	default:
		return m31types.IntentQuestion
	}
}

// normalizeComplexity maps various LLM outputs to known complexity levels.
func normalizeComplexity(raw m31types.ComplexityLevel) m31types.ComplexityLevel {
	switch strings.ToLower(string(raw)) {
	case "trivial", "very_simple", "very simple":
		return m31types.ComplexityTrivial
	case "simple", "easy", "basic":
		return m31types.ComplexitySimple
	case "moderate", "medium", "normal":
		return m31types.ComplexityModerate
	case "complex", "hard", "difficult", "advanced":
		return m31types.ComplexityComplex
	default:
		return m31types.ComplexityModerate
	}
}

// ShouldStartWorkflow returns true when the classified intent warrants
// a structured workflow (feature, bugfix, refactor, chore) with sufficient
// confidence (≥ 0.7).
func ShouldStartWorkflow(result *m31types.IntentResult) bool {
	if result == nil {
		return false
	}
	return result.IsWorkflowWorthy() && result.Confidence >= 0.7
}

// FallbackClassifyIntent converts the keyword-based ClassifyPrompt result
// into an IntentResult for consistent downstream handling.
func FallbackClassifyIntent(goal string, workDir string) *m31types.IntentResult {
	complexity := ClassifyPrompt(goal, workDir)
	intent := guessIntentFromKeywords(goal)

	return &m31types.IntentResult{
		Intent:     intent,
		Complexity: complexity,
		Confidence: 0.5,
		Summary:    goal,
	}
}

// guessIntentFromKeywords uses simple keyword matching to guess intent
// when the LLM classifier is unavailable.
func guessIntentFromKeywords(input string) m31types.IntentType {
	lower := strings.ToLower(strings.TrimSpace(input))

	questionPrefixes := []string{"how ", "what ", "why ", "when ", "where ", "who ", "can you explain", "describe "}
	for _, prefix := range questionPrefixes {
		if strings.HasPrefix(lower, prefix) {
			return m31types.IntentQuestion
		}
	}

	if strings.HasSuffix(lower, "?") {
		return m31types.IntentQuestion
	}

	bugfixSignals := []string{"fix ", "bug", "crash", "error", "broken", "not working", "fails"}
	for _, sig := range bugfixSignals {
		if strings.Contains(lower, sig) {
			return m31types.IntentBugfix
		}
	}

	choreSignals := []string{"bump ", "update version", "upgrade ", "dependency", "config"}
	for _, sig := range choreSignals {
		if strings.Contains(lower, sig) {
			return m31types.IntentChore
		}
	}

	refactorSignals := []string{"refactor", "clean up", "restructure", "reorganize", "simplify"}
	for _, sig := range refactorSignals {
		if strings.Contains(lower, sig) {
			return m31types.IntentRefactor
		}
	}

	return m31types.IntentFeature
}

// defaultIntentClassifyPrompt returns a fallback prompt when the embedded
// template is not available.
func defaultIntentClassifyPrompt() string {
	return `You are a JSON-only classifier. You MUST respond with exactly one JSON object and nothing else.
No explanation, no markdown, no code fences — just raw JSON.
Categories: "feature", "bugfix", "refactor", "question", "explanation", "exploration", "chore".
Complexity: "trivial", "simple", "moderate", "complex".
Format: {"intent":"...","complexity":"...","confidence":0.0-1.0,"scope":[],"summary":"..."}
Example input: "add a dark mode toggle"
Example output: {"intent":"feature","complexity":"simple","confidence":0.9,"scope":["ui"],"summary":"Add dark mode toggle"}`
}
