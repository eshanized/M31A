package arbitrage

import (
	"testing"

	"github.com/eshanized/M31A/pkg/types"
)

// Helpers

func newTestTask(desc string, files, deps int) types.Task {
	fileList := make([]string, files)
	for i := 0; i < files; i++ {
		fileList[i] = "f" + string(rune('0'+i+1)) + ".go"
	}

	depList := make([]int, deps)
	for i := 0; i < deps; i++ {
		depList[i] = i + 1
	}

	return types.Task{
		Description:  desc,
		Action:       desc,
		Files:        fileList,
		Dependencies: depList,
	}
}

func newTestModel(id, provider string, inputPrice, outputPrice float64, contextLen int64) types.ModelInfo {
	return types.ModelInfo{
		ID:            id,
		Provider:      provider,
		Pricing:       types.Pricing{InputPerMToken: inputPrice, OutputPerMToken: outputPrice},
		ContextLength: contextLen,
	}
}

// Scoring Tests

func TestScore_Simple(t *testing.T) {
	scorer := NewScorer(0, 0)
	task := newTestTask("fix typo in README", 1, 0)
	level, _ := scorer.Score(task)
	if level != ComplexitySimple {
		t.Errorf("expected Simple, got %s", level)
	}
}

func TestScore_Moderate(t *testing.T) {
	scorer := NewScorer(0, 0)
	task := newTestTask("implement user authentication", 1, 0)
	level, _ := scorer.Score(task)
	if level != ComplexityModerate {
		t.Errorf("expected Moderate, got %s", level)
	}
}

func TestScore_Complex(t *testing.T) {
	scorer := NewScorer(0, 0)
	task := newTestTask("design and implement microservice architecture", 1, 0)
	level, _ := scorer.Score(task)
	if level != ComplexityComplex {
		t.Errorf("expected Complex, got %s", level)
	}
}

func TestScore_Files(t *testing.T) {
	scorer := NewScorer(0, 0)
	// Moderate task with 5 files — >3 files triggers a one-level boost to Complex.
	task := newTestTask("implement new feature", 5, 0)
	level, _ := scorer.Score(task)
	if level != ComplexityComplex {
		t.Errorf("expected Complex (boosted from Moderate by 5 files), got %s", level)
	}
}

func TestScore_Dependencies(t *testing.T) {
	scorer := NewScorer(0, 0)
	// Simple task with 4 dependencies — >3 deps triggers a one-level boost to Moderate.
	task := newTestTask("fix bug", 1, 4)
	level, _ := scorer.Score(task)
	if level != ComplexityModerate {
		t.Errorf("expected Moderate (boosted from Simple by 4 deps), got %s", level)
	}
}

// Token Estimation Tests

func TestEstimateTokens(t *testing.T) {
	scorer := NewScorer(0, 0)

	// Use 0 files so estimates are the raw midpoint values.
	task := newTestTask("test", 0, 0)

	// Simple: midpoint (2000, 1000)
	in, out := scorer.EstimateTokens(ComplexitySimple, task)
	if in < 1500 || in > 2500 {
		t.Errorf("Simple input out of range: got %d, want [1500, 2500]", in)
	}
	if out < 750 || out > 1250 {
		t.Errorf("Simple output out of range: got %d, want [750, 1250]", out)
	}

	// Moderate: midpoint (5500, 2750)
	in, out = scorer.EstimateTokens(ComplexityModerate, task)
	if in < 4500 || in > 6500 {
		t.Errorf("Moderate input out of range: got %d, want [4500, 6500]", in)
	}
	if out < 2000 || out > 3500 {
		t.Errorf("Moderate output out of range: got %d, want [2000, 3500]", out)
	}

	// Complex: midpoint (14000, 7000)
	in, out = scorer.EstimateTokens(ComplexityComplex, task)
	if in < 12000 || in > 16000 {
		t.Errorf("Complex input out of range: got %d, want [12000, 16000]", in)
	}
	if out < 5500 || out > 8500 {
		t.Errorf("Complex output out of range: got %d, want [5500, 8500]", out)
	}
}

func TestEstimateTokens_FileAdjustment(t *testing.T) {
	scorer := NewScorer(0, 0)
	// 2 files should add 1000 to both input and output.
	task := newTestTask("test", 2, 0)

	in, out := scorer.EstimateTokens(ComplexitySimple, task)
	expectedIn, expectedOut := 3000, 2000 // 2000+500*2, 1000+500*2
	if in != expectedIn {
		t.Errorf("Simple input with 2 files: got %d, want %d", in, expectedIn)
	}
	if out != expectedOut {
		t.Errorf("Simple output with 2 files: got %d, want %d", out, expectedOut)
	}
}

// Model Comparison Tests

func TestCompareModels(t *testing.T) {
	models := []types.ModelInfo{
		newTestModel("model-a", "provider-a", 5, 15, 128000), // expensive
		newTestModel("model-b", "provider-b", 1, 3, 128000),  // cheapest
		newTestModel("model-c", "provider-c", 2, 6, 128000),  // mid
	}

	estimates := CompareModels(models, 1000, 500)

	if len(estimates) != 3 {
		t.Fatalf("expected 3 estimates, got %d", len(estimates))
	}

	// Expected order: model-b (cheapest), model-c (mid), model-a (expensive)
	if estimates[0].ModelID != "model-b" {
		t.Errorf("expected cheapest model first (model-b), got %s", estimates[0].ModelID)
	}
	if estimates[1].ModelID != "model-c" {
		t.Errorf("expected mid model second (model-c), got %s", estimates[1].ModelID)
	}
	if estimates[2].ModelID != "model-a" {
		t.Errorf("expected most expensive last (model-a), got %s", estimates[2].ModelID)
	}

	// Verify cost ordering
	if estimates[0].TotalCost > estimates[1].TotalCost {
		t.Error("estimates not sorted ascending: estimates[0] > estimates[1]")
	}
	if estimates[1].TotalCost > estimates[2].TotalCost {
		t.Error("estimates not sorted ascending: estimates[1] > estimates[2]")
	}
}

func TestCompareModels_MissingPricing(t *testing.T) {
	models := []types.ModelInfo{
		newTestModel("has-pricing", "provider-a", 1, 3, 128000),
		newTestModel("no-pricing", "provider-b", 0, 0, 128000),
	}

	estimates := CompareModels(models, 1000, 500)

	if len(estimates) != 1 {
		t.Fatalf("expected 1 estimate (skipping zero-pricing model), got %d", len(estimates))
	}
	if estimates[0].ModelID != "has-pricing" {
		t.Errorf("expected has-pricing, got %s", estimates[0].ModelID)
	}
}

// Recommendation Tests

func TestRecommend_SimpleTask(t *testing.T) {
	models := []types.ModelInfo{
		newTestModel("fast", "provider-a", 5, 15, 128000),
		newTestModel("cheap", "provider-b", 0.15, 0.45, 128000),
	}

	task := newTestTask("fix bug", 1, 0)
	rec, err := Recommend(models, task, 0.1)
	if err != nil {
		t.Fatalf("Recommend returned error: %v", err)
	}

	if rec.RecommendedModel.ModelID != "cheap" {
		t.Errorf("expected cheapest model (cheap), got %s", rec.RecommendedModel.ModelID)
	}
	if rec.Complexity != ComplexitySimple {
		t.Errorf("expected Simple complexity, got %s", rec.Complexity)
	}
}

func TestRecommend_ComplexTask(t *testing.T) {
	models := []types.ModelInfo{
		newTestModel("cheap", "provider-a", 0.15, 0.45, 32000),    // cheapest but small context
		newTestModel("capable", "provider-b", 0.16, 0.48, 128000), // slightly more, large context
	}

	task := newTestTask("design system architecture", 1, 0)
	rec, err := Recommend(models, task, 0.1)
	if err != nil {
		t.Fatalf("Recommend returned error: %v", err)
	}

	// capable is within 10% of cheap's cost and has >64K context
	if rec.RecommendedModel.ModelID != "capable" {
		t.Errorf("expected capable model (with 128K context), got %s", rec.RecommendedModel.ModelID)
	}
	if rec.Complexity != ComplexityComplex {
		t.Errorf("expected Complex complexity, got %s", rec.Complexity)
	}
}

func TestRecommend_WithThreshold(t *testing.T) {
	models := []types.ModelInfo{
		newTestModel("cheap", "provider-a", 0.15, 0.45, 32000),    // cheapest, small context
		newTestModel("capable", "provider-b", 0.16, 0.48, 128000), // slightly more, large context
	}

	task := newTestTask("design system architecture", 1, 0)

	// With a very tight threshold (1%), the capable model is too expensive
	// relative to the cheapest, so we fall back to the cheapest.
	rec, err := Recommend(models, task, 0.01)
	if err != nil {
		t.Fatalf("Recommend returned error: %v", err)
	}

	if rec.RecommendedModel.ModelID != "cheap" {
		t.Errorf("expected fallback to cheapest with tight threshold, got %s", rec.RecommendedModel.ModelID)
	}
}

// ShouldArbitrage Tests

func TestShouldArbitrage(t *testing.T) {
	tests := []struct {
		name        string
		current     float64
		alternative float64
		threshold   float64
		expected    bool
	}{
		{
			name:        "significant savings",
			current:     100,
			alternative: 80,
			threshold:   0.1,
			expected:    true,
		},
		{
			name:        "savings below threshold",
			current:     100,
			alternative: 95,
			threshold:   0.1,
			expected:    false,
		},
		{
			name:        "equal costs",
			current:     100,
			alternative: 100,
			threshold:   0.1,
			expected:    false,
		},
		{
			name:        "zero current cost",
			current:     0,
			alternative: 50,
			threshold:   0.1,
			expected:    false,
		},
		{
			name:        "alternative more expensive",
			current:     50,
			alternative: 100,
			threshold:   0.1,
			expected:    false,
		},
		{
			name:        "negative threshold always false",
			current:     100,
			alternative: 50,
			threshold:   0.1,
			expected:    true, // (100-50)/100 = 0.5 > 0.1
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := ShouldArbitrage(tt.current, tt.alternative, tt.threshold)
			if got != tt.expected {
				t.Errorf("ShouldArbitrage(%v, %v, %v) = %v, want %v",
					tt.current, tt.alternative, tt.threshold, got, tt.expected)
			}
		})
	}
}

// Edge Cases

func TestArbitrage_NoModels(t *testing.T) {
	_, err := Recommend(nil, newTestTask("test", 0, 0), 0.1)
	if err == nil {
		t.Error("expected error for empty models list, got nil")
	}
}

func TestArbitrage_NoModelsWithPricing(t *testing.T) {
	models := []types.ModelInfo{
		newTestModel("no-price", "provider-a", 0, 0, 128000),
	}
	_, err := Recommend(models, newTestTask("test", 0, 0), 0.1)
	if err == nil {
		t.Error("expected error when all models have zero pricing, got nil")
	}
}

func TestArbitrage_SingleModel(t *testing.T) {
	models := []types.ModelInfo{
		newTestModel("only-model", "provider-a", 1, 3, 128000),
	}

	task := newTestTask("fix bug", 1, 0)
	rec, err := Recommend(models, task, 0.1)
	if err != nil {
		t.Fatalf("Recommend returned error: %v", err)
	}

	if rec.RecommendedModel.ModelID != "only-model" {
		t.Errorf("expected only-model, got %s", rec.RecommendedModel.ModelID)
	}
	if len(rec.Alternatives) != 0 {
		t.Errorf("expected 0 alternatives for single model, got %d", len(rec.Alternatives))
	}
}
