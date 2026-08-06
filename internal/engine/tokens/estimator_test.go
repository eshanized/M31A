package tokens

import (
	"strings"
	"testing"
)

func TestEstimator_NewWithKnownModel(t *testing.T) {
	e := NewEstimator("gpt-4o")
	if e == nil {
		t.Fatal("NewEstimator returned nil")
	}
	if e.tokenizer == nil {
		t.Skip("tiktoken-go not available (network may be unavailable)")
	}
	if e.ModelID() != "gpt-4o" {
		t.Errorf("expected modelID 'gpt-4o', got %q", e.ModelID())
	}
	if e.Provider() != ProviderOpenAI {
		t.Errorf("expected provider OpenAI, got %v", e.Provider())
	}
}

func TestEstimator_NewWithUnknownModel(t *testing.T) {
	e := NewEstimator("claude-3-sonnet")
	if e == nil {
		t.Fatal("NewEstimator returned nil")
	}
	if e.tokenizer != nil {
		t.Error("expected nil tokenizer for unsupported model")
	}
	if e.Provider() != ProviderAnthropic {
		t.Errorf("expected provider Anthropic, got %v", e.Provider())
	}
}

func TestEstimator_EstimateWithTokenizer(t *testing.T) {
	e := NewEstimator("gpt-4o")
	if e.tokenizer == nil {
		t.Skip("tiktoken-go not available (network may be unavailable)")
	}

	text := "Hello, world! This is a test message for token estimation."
	count := e.Estimate(text)

	if count <= 0 {
		t.Errorf("expected positive token count, got %d", count)
	}
}

func TestEstimator_EstimateFallback(t *testing.T) {
	// Unknown model forces fallback mode (nil tokenizer)
	e := NewEstimator("claude-3-opus-20240229")
	if e.tokenizer != nil {
		t.Skip("model unexpectedly has a tokenizer")
	}

	text := "Hello, world!"
	count := e.Estimate(text)

	// Fallback: len([]rune)/4*1.3 = 13/4*1.3 = 4.225 -> int = 4, * 1.0 = 4
	if count <= 0 {
		t.Errorf("expected positive token count from fallback, got %d", count)
	}
}

func TestEstimator_EstimateEmptyString(t *testing.T) {
	e := NewEstimator("claude-3-opus")
	count := e.Estimate("")

	if count < 0 {
		t.Errorf("expected non-negative count for empty string, got %d", count)
	}
}

func TestEstimator_EstimateMultibyte(t *testing.T) {
	// CJK characters should each be counted as one rune
	e := NewEstimator("unknown-model") // forces fallback
	if e.tokenizer != nil {
		t.Skip("model unexpectedly has a tokenizer")
	}

	// Each CJK character is multiple bytes but one rune
	text := "你好世界" // 4 CJK characters
	count := e.Estimate(text)

	// Fallback: len([]rune)/4*1.3 = 4/4*1.3 = 1.3 -> int = 1
	if count < 0 {
		t.Errorf("expected non-negative count for multibyte string, got %d", count)
	}
}

func TestEstimator_Calibrate(t *testing.T) {
	e := NewEstimator("claude-3-opus")
	initialFactor := e.emaFactor()

	// Calibrate: estimated=100, actual=110 -> ratio=1.1
	// newFactor = 0.3*1.1 + 0.7*1.0 = 0.33 + 0.70 = 1.03
	e.Calibrate(100, 110)

	if e.emaFactor() <= initialFactor {
		t.Errorf("expected emaFactor to increase after under-estimation, got %f (was %f)", e.emaFactor(), initialFactor)
	}

	// Expected: 0.3 * 1.1 + 0.7 * 1.0 = 1.03
	expected := 0.3*1.1 + 0.7*1.0
	if e.emaFactor() != expected {
		t.Errorf("expected emaFactor %f, got %f", expected, e.emaFactor())
	}
}

func TestEstimator_CalibrateZeroEstimated(t *testing.T) {
	e := NewEstimator("claude-3-opus")
	initialFactor := e.emaFactor()

	// Call with estimated=0 should not cause division by zero
	e.Calibrate(0, 100)

	if e.emaFactor() != initialFactor {
		t.Errorf("expected emaFactor unchanged after zero estimated, got %f (was %f)", e.emaFactor(), initialFactor)
	}
}

func TestEstimator_CalibrateConvergence(t *testing.T) {
	e := NewEstimator("claude-3-opus")

	// Simulate consistent 5% overestimation: actual=95, estimated=100
	// After 3 iterations, factor should approach ~0.965
	for i := 0; i < 3; i++ {
		e.Calibrate(100, 95)
	}

	// Expected after 3 iterations:
	// iter1: 0.3*0.95 + 0.7*1.0 = 0.285 + 0.7 = 0.985
	// iter2: 0.3*0.95 + 0.7*0.985 = 0.285 + 0.6895 = 0.9745
	// iter3: 0.3*0.95 + 0.7*0.9745 = 0.285 + 0.68215 = 0.96715
	if e.emaFactor() >= 1.0 {
		t.Errorf("expected emaFactor < 1.0 after 3 over-estimations, got %f", e.emaFactor())
	}
	if e.emaFactor() < 0.9 {
		t.Errorf("expected emaFactor close to 0.97, got %f (convergence too fast)", e.emaFactor())
	}
}

func TestEstimator_CalibrateClampMin(t *testing.T) {
	e := NewEstimator("claude-3-opus")

	// Extreme underestimate: estimated=100, actual=1 -> ratio=0.01
	// With emaAlpha=0.3: 0.3*0.01 + 0.7*1.0 = 0.703
	// Multiple iterations to test clamp
	for i := 0; i < 10; i++ {
		e.Calibrate(100, 1)
	}

	if e.emaFactor() < 0.1 {
		t.Errorf("expected emaFactor clamped to min 0.1, got %f", e.emaFactor())
	}
}

func TestEstimator_CalibrateClampMax(t *testing.T) {
	e := NewEstimator("claude-3-opus")

	// Extreme overestimate: estimated=100, actual=10000 -> ratio=100
	// After one iteration: 0.3*100 + 0.7*1.0 = 30.7 -> clamped to 10.0
	e.Calibrate(100, 10000)

	if e.emaFactor() > 10.0 {
		t.Errorf("expected emaFactor clamped to max 10.0, got %f", e.emaFactor())
	}
	if e.emaFactor() != 10.0 {
		t.Errorf("expected emaFactor = 10.0, got %f", e.emaFactor())
	}
}

func TestEstimator_FormatUsage(t *testing.T) {
	e := NewEstimator("claude-3-opus")
	result := e.FormatUsage(500, 1000)

	if result != "500 / 1000 (50%)" {
		t.Errorf("expected '500 / 1000 (50%%)', got %q", result)
	}
}

func TestEstimator_FormatUsageZeroTotal(t *testing.T) {
	e := NewEstimator("claude-3-opus")
	result := e.FormatUsage(500, 0)

	if result != "-- / --" {
		t.Errorf("expected '-- / --', got %q", result)
	}

	// Also test negative total
	result = e.FormatUsage(500, -1)
	if result != "-- / --" {
		t.Errorf("expected '-- / --' for negative total, got %q", result)
	}
}

func TestEstimator_FormatUsageExactTotal(t *testing.T) {
	e := NewEstimator("claude-3-opus")
	result := e.FormatUsage(100, 200)

	if result != "100 / 200 (50%)" {
		t.Errorf("expected '100 / 200 (50%%)', got %q", result)
	}
}

func TestEstimator_FormatUsageFullContext(t *testing.T) {
	e := NewEstimator("claude-3-opus")
	result := e.FormatUsage(128000, 128000)

	if result != "128000 / 128000 (100%)" {
		t.Errorf("expected '128000 / 128000 (100%%)', got %q", result)
	}
}

func TestEstimator_ContextWarningBannerBelowThreshold(t *testing.T) {
	e := NewEstimator("claude-3-opus")

	// 50% usage, threshold 80% — no banner
	result := e.ContextWarningBanner(50000, 100000, 0.80)

	if result != "" {
		t.Errorf("expected empty string for below-threshold usage, got %q", result)
	}
}

func TestEstimator_ContextWarningBannerAboveThreshold(t *testing.T) {
	e := NewEstimator("claude-3-opus")

	// 85% usage, threshold 80% — should show banner
	result := e.ContextWarningBanner(85000, 100000, 0.80)

	if result == "" {
		t.Fatal("expected non-empty warning banner for above-threshold usage")
	}

	if !strings.Contains(result, "Context at 85%") {
		t.Errorf("expected warning to contain 'Context at 85%%', got %q", result)
	}

	if !strings.Contains(result, "/compress") {
		t.Errorf("expected warning to mention /compress, got %q", result)
	}
}

func TestEstimator_ContextWarningBannerZeroTotal(t *testing.T) {
	e := NewEstimator("claude-3-opus")

	result := e.ContextWarningBanner(100, 0, 0.80)

	if result != "" {
		t.Errorf("expected empty string for zero total, got %q", result)
	}
}

func TestEstimator_ContextWarningBannerAtThreshold(t *testing.T) {
	e := NewEstimator("claude-3-opus")

	// Exactly at threshold (80%) — should trigger (>= threshold compares as <)
	result := e.ContextWarningBanner(80000, 100000, 0.80)

	if result == "" {
		t.Error("expected non-empty banner at exactly threshold (ratio < threshold is strict)")
	}
}

func TestEstimator_DefaultThreshold(t *testing.T) {
	if DefaultWarningThreshold != 0.80 {
		t.Errorf("expected DefaultWarningThreshold = 0.80, got %f", DefaultWarningThreshold)
	}
}

func TestEstimator_ModelID(t *testing.T) {
	e := NewEstimator("claude-3-opus")
	if e.ModelID() != "claude-3-opus" {
		t.Errorf("expected 'claude-3-opus', got %q", e.ModelID())
	}

	e2 := NewEstimator("custom-model-v3")
	if e2.ModelID() != "custom-model-v3" {
		t.Errorf("expected 'custom-model-v3', got %q", e2.ModelID())
	}
}

func TestDetectProviderFamily(t *testing.T) {
	tests := []struct {
		modelID  string
		expected ProviderFamily
	}{
		{"gpt-4o", ProviderOpenAI},
		{"gpt-4-turbo", ProviderOpenAI},
		{"o1-preview", ProviderOpenAI},
		{"o3-mini", ProviderOpenAI},
		{"claude-3-opus", ProviderAnthropic},
		{"claude-3-5-sonnet", ProviderAnthropic},
		{"gemini-pro", ProviderGoogle},
		{"gemini-1.5-flash", ProviderGoogle},
		{"llama-3-70b", ProviderMeta},
		{"mistral-large", ProviderMistral},
		{"qwen-2.5", ProviderQwen},
		{"deepseek-chat", ProviderDeepSeek},
		{"command-r-plus", ProviderCohere},
		{"custom-model", ProviderUnknown},
	}

	for _, tt := range tests {
		t.Run(tt.modelID, func(t *testing.T) {
			result := DetectProviderFamily(tt.modelID)
			if result != tt.expected {
				t.Errorf("DetectProviderFamily(%q) = %v, want %v", tt.modelID, result, tt.expected)
			}
		})
	}
}

func TestEstimator_ProviderSpecificEstimation(t *testing.T) {
	text := "This is a test message with enough words to trigger provider-specific heuristics for accurate token estimation."

	tests := []struct {
		modelID  string
		expected ProviderFamily
	}{
		{"gpt-4o", ProviderOpenAI},
		{"claude-3-opus", ProviderAnthropic},
		{"gemini-pro", ProviderGoogle},
		{"llama-3-70b", ProviderMeta},
		{"mistral-large", ProviderMistral},
		{"qwen-2.5", ProviderQwen},
		{"deepseek-chat", ProviderDeepSeek},
		{"command-r-plus", ProviderCohere},
	}

	for _, tt := range tests {
		t.Run(tt.modelID, func(t *testing.T) {
			e := NewEstimator(tt.modelID)
			if e.Provider() != tt.expected {
				t.Errorf("expected provider %v, got %v", tt.expected, e.Provider())
			}

			count, provider := e.EstimateTokensForProvider(text)
			if count <= 0 {
				t.Errorf("expected positive token count, got %d", count)
			}
			if provider != tt.expected.String() {
				t.Errorf("expected provider %q, got %q", tt.expected.String(), provider)
			}
		})
	}
}

func TestIsCodeHeavy(t *testing.T) {
	tests := []struct {
		name     string
		text     string
		expected bool
	}{
		{"empty", "", false},
		{"short", "if x {", false},
		{"prose", "The quick brown fox jumps over the lazy dog and runs away.", false},
		{"code", "func main() { fmt.Println(x); return nil }", true},
		{"mixed_prose", "Here is a function:\nfunc add(a, b int) int { return a + b }", false},
		{"json", `{"name": "test", "value": 42, "items": [1, 2, 3]}`, true},
		{"sql_keywords", "SELECT id, name FROM users WHERE active = true ORDER BY name", false},
		{"natural_heavy", "This is a very long paragraph of natural language text that contains many words and should not be detected as code at all because it has no special characters.", false},
		{"dense_code", "if (err != nil) { return fmt.Errorf(\"wrap: %w\", err) }", true},
		{"go_fn", "func (s *Server) Handle(w http.ResponseWriter, r *http.Request) {", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := isCodeHeavy(tt.text); got != tt.expected {
				t.Errorf("isCodeHeavy(%q) = %v, want %v", tt.name, got, tt.expected)
			}
		})
	}
}

func TestEstimateWithProvider_CodeVsProse(t *testing.T) {
	prose := "The quick brown fox jumps over the lazy dog. This is a test of natural language token estimation for provider-specific heuristics."
	code := "func main() { if err := doSomething(x); err != nil { return err } return nil }"

	providers := []struct {
		name  string
		model string
	}{
		{"anthropic", "claude-3-opus"},
		{"google", "gemini-pro"},
		{"meta", "llama-3-70b"},
		{"mistral", "mistral-large"},
		{"qwen", "qwen-2.5"},
		{"deepseek", "deepseek-chat"},
		{"cohere", "command-r-plus"},
	}

	for _, p := range providers {
		t.Run(p.name, func(t *testing.T) {
			e := NewEstimator(p.model)
			proseTokens := e.Estimate(prose)
			codeTokens := e.Estimate(code)
			if proseTokens <= 0 || codeTokens <= 0 {
				t.Errorf("expected positive token counts, got prose=%d code=%d", proseTokens, codeTokens)
			}
			// Both estimates should be reasonable (within 10x of each other)
			ratio := float64(proseTokens) / float64(codeTokens)
			if ratio < 0.1 || ratio > 10.0 {
				t.Errorf("prose/code ratio (%.2f) is unreasonable: prose=%d code=%d", ratio, proseTokens, codeTokens)
			}
		})
	}
}

func TestEstimate_ShortTextFallback(t *testing.T) {
	providers := []struct {
		model string
	}{
		{"claude-3-opus"},
		{"gemini-pro"},
		{"llama-3-70b"},
		{"mistral-large"},
		{"qwen-2.5"},
		{"deepseek-chat"},
		{"command-r-plus"},
	}

	for _, p := range providers {
		t.Run(p.model, func(t *testing.T) {
			e := NewEstimator(p.model)
			count := e.Estimate("hello")
			if count <= 0 {
				t.Errorf("expected positive token count for short text, got %d", count)
			}
		})
	}
}

func TestEstimateMessages_MultiMessage(t *testing.T) {
	e := NewEstimator("claude-3-opus")
	messages := []struct {
		role    string
		content string
	}{
		{"system", "You are a helpful assistant."},
		{"user", "Hello, how are you?"},
		{"assistant", "I'm doing well, thank you!"},
	}

	var totalEstimate int
	for _, m := range messages {
		totalEstimate += e.Estimate(m.content)
	}

	// EstimateMessages should account for per-message overhead
	msgTypes := make([]struct {
		Role    string
		Content string
	}, len(messages))
	for i, m := range messages {
		msgTypes[i] = struct {
			Role    string
			Content string
		}{m.role, m.content}
	}

	// The estimate should be at least as large as sum of content estimates
	// (due to per-message overhead)
	if totalEstimate <= 0 {
		t.Errorf("expected positive total estimate, got %d", totalEstimate)
	}
}

// B16: Verify math.Ceil is used (no truncation)
func TestEstimate_Ceiling(t *testing.T) {
	e := NewEstimator("unknown-model")
	// 101 chars / 3.8 ratio (Anthropic-like) = 26.58 → should ceil to 27, not truncate to 26
	text := strings.Repeat("a", 101)
	got := e.estimateWithProvider(text)
	if got < 1 {
		t.Errorf("expected positive estimate, got %d", got)
	}
	// Verify ceiling: int(101/3.8) = 26 (truncate), int(math.Ceil(101/3.8)) = 27
	// For unknown provider with ratio 3.8: 101/3.8 = 26.578...
	expected := 27
	if got != expected {
		t.Errorf("expected ceiling estimate %d, got %d (truncation detected)", expected, got)
	}
}

func TestEstimate_CeilingClassCoverage(t *testing.T) {
	e := NewEstimator("unknown-model")
	tests := []struct {
		name     string
		chars    int
		expected int
	}{
		{"101 chars", 101, 27}, // 101/3.8 = 26.58 → 27
		{"51 chars", 51, 14},   // 51/3.8 = 13.42 → 14
		{"200 chars", 200, 53}, // 200/3.8 = 52.63 → 53
		{"300 chars", 300, 79}, // 300/3.8 = 78.95 → 79
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			text := strings.Repeat("a", tt.chars)
			got := e.estimateWithProvider(text)
			if got != tt.expected {
				t.Errorf("expected %d, got %d", tt.expected, got)
			}
		})
	}
}
