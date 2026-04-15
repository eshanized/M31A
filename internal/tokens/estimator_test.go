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
		t.Error("expected non-nil tokenizer for gpt-4o")
	}
	if e.ModelID() != "gpt-4o" {
		t.Errorf("expected modelID 'gpt-4o', got %q", e.ModelID())
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
}

func TestEstimator_EstimateWithTokenizer(t *testing.T) {
	e := NewEstimator("gpt-4o")
	if e.tokenizer == nil {
		t.Skip("tiktoken-go not available for gpt-4o")
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
	e := NewEstimator("gpt-4o")
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
	e := NewEstimator("gpt-4o")
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
	e := NewEstimator("gpt-4o")
	initialFactor := e.emaFactor()

	// Call with estimated=0 should not cause division by zero
	e.Calibrate(0, 100)

	if e.emaFactor() != initialFactor {
		t.Errorf("expected emaFactor unchanged after zero estimated, got %f (was %f)", e.emaFactor(), initialFactor)
	}
}

func TestEstimator_CalibrateConvergence(t *testing.T) {
	e := NewEstimator("gpt-4o")

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
	e := NewEstimator("gpt-4o")

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
	e := NewEstimator("gpt-4o")

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
	e := NewEstimator("gpt-4o")
	result := e.FormatUsage(500, 1000)

	if result != "500 / 1000 (50%)" {
		t.Errorf("expected '500 / 1000 (50%%)', got %q", result)
	}
}

func TestEstimator_FormatUsageZeroTotal(t *testing.T) {
	e := NewEstimator("gpt-4o")
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
	e := NewEstimator("gpt-4o")
	result := e.FormatUsage(100, 200)

	if result != "100 / 200 (50%)" {
		t.Errorf("expected '100 / 200 (50%%)', got %q", result)
	}
}

func TestEstimator_FormatUsageFullContext(t *testing.T) {
	e := NewEstimator("gpt-4o")
	result := e.FormatUsage(128000, 128000)

	if result != "128000 / 128000 (100%)" {
		t.Errorf("expected '128000 / 128000 (100%%)', got %q", result)
	}
}

func TestEstimator_ContextWarningBannerBelowThreshold(t *testing.T) {
	e := NewEstimator("gpt-4o")

	// 50% usage, threshold 80% — no banner
	result := e.ContextWarningBanner(50000, 100000, 0.80)

	if result != "" {
		t.Errorf("expected empty string for below-threshold usage, got %q", result)
	}
}

func TestEstimator_ContextWarningBannerAboveThreshold(t *testing.T) {
	e := NewEstimator("gpt-4o")

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
	e := NewEstimator("gpt-4o")

	result := e.ContextWarningBanner(100, 0, 0.80)

	if result != "" {
		t.Errorf("expected empty string for zero total, got %q", result)
	}
}

func TestEstimator_ContextWarningBannerAtThreshold(t *testing.T) {
	e := NewEstimator("gpt-4o")

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
	e := NewEstimator("gpt-4o")
	if e.ModelID() != "gpt-4o" {
		t.Errorf("expected 'gpt-4o', got %q", e.ModelID())
	}

	e2 := NewEstimator("custom-model-v3")
	if e2.ModelID() != "custom-model-v3" {
		t.Errorf("expected 'custom-model-v3', got %q", e2.ModelID())
	}
}
