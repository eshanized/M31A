package decision

import (
	"encoding/json"
	"fmt"
	"time"
)

// DecisionReceipt records a single decision made by the system.
// Immutable after creation — exported getters only, unexported fields.
type DecisionReceipt struct {
	Timestamp  time.Time `json:"timestamp"`
	Decision   string    `json:"decision"`
	Rationale  string    `json:"rationale"`
	Alternatives []string `json:"alternatives,omitempty"`
	Cost       Cost      `json:"cost"`
	Category   Category  `json:"category"`
}

// Category classifies the type of decision.
type Category string

const (
	CategoryTool      Category = "tool"
	CategoryModel     Category = "model"
	CategoryPlan      Category = "plan"
	CategoryRetry     Category = "retry"
	CategoryIntent    Category = "intent"
	CategoryStrategy  Category = "strategy"
	CategoryAmbiguous Category = "ambiguous"
)

// Cost tracks the resource cost of a decision.
type Cost struct {
	Tokens      int     `json:"tokens"`
	Duration    float64 `json:"duration_seconds"`
	TokensUSD   float64 `json:"tokens_usd,omitempty"`
	Attempts    int     `json:"attempts,omitempty"`
	RetryCount  int     `json:"retry_count,omitempty"`
	StrategyIdx int     `json:"strategy_idx,omitempty"`
}

// GetTimestamp returns when the decision was made.
func (r DecisionReceipt) GetTimestamp() time.Time { return r.Timestamp }

// GetDecision returns the decision string.
func (r DecisionReceipt) GetDecision() string { return r.Decision }

// GetRationale returns the reasoning behind the decision.
func (r DecisionReceipt) GetRationale() string { return r.Rationale }

// GetAlternatives returns alternative options that were considered.
func (r DecisionReceipt) GetAlternatives() []string { return r.Alternatives }

// GetCost returns the resource cost.
func (r DecisionReceipt) GetCost() Cost { return r.Cost }

// GetCategory returns the decision category.
func (r DecisionReceipt) GetCategory() Category { return r.Category }

// Summary returns a one-line human-readable summary.
func (r DecisionReceipt) Summary() string {
	return fmt.Sprintf("[%s] %s (rationale: %s)", r.Category, r.Decision, r.Rationale)
}

// MarshalJSON implements json.Marshaler for DecisionReceipt.
func (r DecisionReceipt) MarshalJSON() ([]byte, error) {
	type Alias DecisionReceipt
	return json.Marshal(struct {
		Alias
		Alternatives []string `json:"alternatives,omitempty"`
	}{
		Alias:        Alias(r),
		Alternatives: r.Alternatives,
	})
}
