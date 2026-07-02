package workflow

import (
	"math"
	"sync/atomic"
)

// CostTracker provides lock-free atomic cost tracking with budget management.
// Costs are stored as raw uint64 bits (via math.Float64bits) for atomic access.
type CostTracker struct {
	totalCostBits atomic.Uint64 // cumulative cost stored as bits
	budgetUSD     float64       // budget limit in USD; 0 = unlimited
}

// NewCostTracker creates a CostTracker with the given budget limit.
// A budget of 0 means no limit.
func NewCostTracker(budgetUSD float64) *CostTracker {
	return &CostTracker{
		budgetUSD: budgetUSD,
	}
}

// RecordCost atomically adds a cost (in USD) to the running total.
func (ct *CostTracker) RecordCost(costUSD float64) {
	for {
		old := ct.totalCostBits.Load()
		new := math.Float64bits(math.Float64frombits(old) + costUSD)
		if ct.totalCostBits.CompareAndSwap(old, new) {
			return
		}
	}
}

// TotalCost returns the cumulative cost in USD.
func (ct *CostTracker) TotalCost() float64 {
	return math.Float64frombits(ct.totalCostBits.Load())
}

// BudgetRemaining returns the remaining budget in USD.
// Returns 0 if budget is exceeded or unlimited.
func (ct *CostTracker) BudgetRemaining() float64 {
	if ct.budgetUSD <= 0 {
		return 0 // unlimited
	}
	remaining := ct.budgetUSD - ct.TotalCost()
	if remaining < 0 {
		return 0
	}
	return remaining
}

// BudgetExceeded returns true if the total cost meets or exceeds the budget.
func (ct *CostTracker) BudgetExceeded() bool {
	if ct.budgetUSD <= 0 {
		return false // unlimited
	}
	return ct.TotalCost() >= ct.budgetUSD
}
