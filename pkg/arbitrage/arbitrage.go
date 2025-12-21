package arbitrage

import (
	"fmt"
	"sort"
	"strings"

	"github.com/eshanized/M31A/internal/types"
)

// ComplexityLevel represents the complexity classification of a task.
type ComplexityLevel string

const (
	ComplexitySimple   ComplexityLevel = "simple"
	ComplexityModerate ComplexityLevel = "moderate"
	ComplexityComplex  ComplexityLevel = "complex"
)

// CostEstimate holds the cost estimate for a model given estimated token counts.
type CostEstimate struct {
	ModelID      string  `json:"model_id"`
	Provider     string  `json:"provider"`
	InputCost    float64 `json:"input_cost"`
	OutputCost   float64 `json:"output_cost"`
	TotalCost    float64 `json:"total_cost"`
	Currency     string  `json:"currency"`
	InputTokens  int     `json:"input_tokens"`
	OutputTokens int     `json:"output_tokens"`
}

// ArbitrageRecommendation holds the recommended model and alternatives.
type ArbitrageRecommendation struct {
	RecommendedModel CostEstimate    `json:"recommended_model"`
	Alternatives     []CostEstimate  `json:"alternatives"`
	Complexity       ComplexityLevel `json:"complexity"`
	Savings          float64         `json:"savings"`
	Reason           string          `json:"reason"`
}

// Scorer evaluates task complexity and estimates token usage.
type Scorer struct {
	SimpleThreshold  int
	ComplexThreshold int
}

// NewScorer creates a Scorer with the given thresholds.
// Values <= 0 are replaced with defaults (SimpleThreshold=2000, ComplexThreshold=8000).
func NewScorer(simpleThreshold, complexThreshold int) *Scorer {
	if simpleThreshold <= 0 {
		simpleThreshold = 2000
	}
	if complexThreshold <= 0 {
		complexThreshold = 8000
	}
	return &Scorer{
		SimpleThreshold:  simpleThreshold,
		ComplexThreshold: complexThreshold,
	}
}

// Score classifies task complexity based on keyword analysis and boost rules.
// Returns the complexity level and the estimated total token count.
func (s *Scorer) Score(task types.Task) (ComplexityLevel, int) {
	level := classifyText(task.Action, task.Description)

	// Boost one level when the task touches many files (>3).
	if len(task.Files) > 3 {
		level = boostLevel(level, 1)
	}

	// Boost one level when the task has many dependencies (>3).
	if len(task.Dependencies) > 3 {
		level = boostLevel(level, 1)
	}

	input, output := s.EstimateTokens(level, task)
	return level, input + output
}

// EstimateTokens returns estimated input and output tokens for a given
// complexity level and task. Per-file adjustments add 500 tokens each.
func (s *Scorer) EstimateTokens(complexity ComplexityLevel, task types.Task) (int, int) {
	var baseInput, baseOutput int

	switch complexity {
	case ComplexitySimple:
		baseInput, baseOutput = 2000, 1000
	case ComplexityModerate:
		baseInput, baseOutput = 5500, 2750
	case ComplexityComplex:
		baseInput, baseOutput = 14000, 7000
	default:
		baseInput, baseOutput = 2000, 1000
	}

	adj := len(task.Files) * 500
	return baseInput + adj, baseOutput + adj
}

// CompareModels computes cost estimates for each model and returns them sorted
// ascending by TotalCost. Models where both Pricing fields are zero are skipped.
func CompareModels(models []types.ModelInfo, inputTokens, outputTokens int) []CostEstimate {
	estimates := make([]CostEstimate, 0, len(models))

	for _, m := range models {
		if m.Pricing.InputPerMToken == 0 && m.Pricing.OutputPerMToken == 0 {
			continue
		}

		inputCost := float64(inputTokens) * (m.Pricing.InputPerMToken / 1_000_000.0)
		outputCost := float64(outputTokens) * (m.Pricing.OutputPerMToken / 1_000_000.0)

		estimates = append(estimates, CostEstimate{
			ModelID:      m.ID,
			Provider:     m.Provider,
			InputCost:    inputCost,
			OutputCost:   outputCost,
			TotalCost:    inputCost + outputCost,
			Currency:     "USD",
			InputTokens:  inputTokens,
			OutputTokens: outputTokens,
		})
	}

	sort.Slice(estimates, func(i, j int) bool {
		return estimates[i].TotalCost < estimates[j].TotalCost
	})

	return estimates
}

// Recommend scores a task, estimates tokens, compares models, and returns the
// best model recommendation. For complex tasks the cheapest model may be
// rejected if its context window is insufficient (<=64K). When the cheapest is
// rejected, the next model whose cost is within the threshold factor is selected.
func Recommend(models []types.ModelInfo, task types.Task, threshold float64) (*ArbitrageRecommendation, error) {
	if len(models) == 0 {
		return nil, fmt.Errorf("no models available")
	}

	scorer := NewScorer(0, 0)
	complexity, _ := scorer.Score(task)
	inputTokens, outputTokens := scorer.EstimateTokens(complexity, task)
	estimates := CompareModels(models, inputTokens, outputTokens)

	if len(estimates) == 0 {
		return nil, fmt.Errorf("no models with pricing data available")
	}

	var recommended CostEstimate
	alternatives := make([]CostEstimate, 0, 3)
	var reason string

	cheapest := estimates[0]

	if complexity == ComplexityComplex {
		// Complex tasks require a model with a context window >64K.
		var found bool
		for _, est := range estimates {
			hasLargeContext := false
			for _, m := range models {
				if m.ID == est.ModelID && m.ContextLength > 64000 {
					hasLargeContext = true
					break
				}
			}
			if !hasLargeContext {
				continue
			}
			// Check that the capable model is within the price threshold of the cheapest.
			if est.TotalCost <= cheapest.TotalCost*(1+threshold) {
				recommended = est
				reason = fmt.Sprintf(
					"Model with sufficient context window for %s task, within %.0f%% of cheapest price.",
					complexity, threshold*100,
				)
				found = true
				break
			}
		}
		if !found {
			recommended = cheapest
			reason = fmt.Sprintf(
				"Cheapest model for %s task (no model with 64K+ context within %.0f%% threshold).",
				complexity, threshold*100,
			)
		}
	} else {
		// Simple and moderate tasks use the cheapest model.
		recommended = cheapest
		reason = fmt.Sprintf("Cheapest model suitable for %s task.", complexity)
	}

	// Collect up to 3 alternatives cheaper than the recommended model.
	for _, est := range estimates {
		if est.ModelID == recommended.ModelID {
			break
		}
		if len(alternatives) >= 3 {
			break
		}
		alternatives = append(alternatives, est)
	}

	// Savings vs the most expensive model.
	mostExpensive := estimates[len(estimates)-1].TotalCost
	savings := mostExpensive - recommended.TotalCost
	if savings < 0 {
		savings = 0
	}

	return &ArbitrageRecommendation{
		RecommendedModel: recommended,
		Alternatives:     alternatives,
		Complexity:       complexity,
		Savings:          savings,
		Reason:           reason,
	}, nil
}

// ShouldArbitrage returns true when switching from the current model to the
// alternative would save more than the threshold proportion of the current cost.
func ShouldArbitrage(currentCost, alternativeCost, threshold float64) bool {
	if currentCost <= 0 {
		return false
	}
	if alternativeCost >= currentCost {
		return false
	}
	return (currentCost-alternativeCost)/currentCost > threshold
}

// boostLevel increases a complexity level by the given number of steps,
// never exceeding ComplexityComplex.
func boostLevel(level ComplexityLevel, steps int) ComplexityLevel {
	if steps <= 0 {
		return level
	}
	switch level {
	case ComplexitySimple:
		if steps >= 2 {
			return ComplexityComplex
		}
		return ComplexityModerate
	case ComplexityModerate:
		return ComplexityComplex
	default:
		return ComplexityComplex
	}
}

// classifyText examines the action and description text for complexity
// keywords. Checks complex keywords first, then moderate, then simple.
// Returns ComplexitySimple as the default when no keywords match.
func classifyText(action, description string) ComplexityLevel {
	text := strings.ToLower(action + " " + description)

	complexKeywords := []string{"design", "architect", "migrate", "rewrite", "system",
		"containerize", "deploy", "infrastructure", "microservice", "distributed",
		"benchmark", "security audit"}
	for _, kw := range complexKeywords {
		if strings.Contains(text, kw) {
			return ComplexityComplex
		}
	}

	moderateKeywords := []string{"implement", "create", "refactor", "restructure",
		"database", "api", "frontend", "backend", "authentication", "integration",
		"test suite", "monitor", "pipeline"}
	for _, kw := range moderateKeywords {
		if strings.Contains(text, kw) {
			return ComplexityModerate
		}
	}

	simpleKeywords := []string{"fix", "add", "update", "change", "rename",
		"typo", "format", "lint", "comment", "readme"}
	for _, kw := range simpleKeywords {
		if strings.Contains(text, kw) {
			return ComplexitySimple
		}
	}

	return ComplexitySimple
}
