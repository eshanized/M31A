package workflow

import (
	"sync"
	"testing"
)

func TestNewCostTracker(t *testing.T) {
	ct := NewCostTracker(10.0)
	if ct == nil {
		t.Fatal("NewCostTracker returned nil")
	}
	if ct.budgetUSD != 10.0 {
		t.Errorf("budgetUSD = %v, want 10.0", ct.budgetUSD)
	}
}

func TestNewCostTracker_Unlimited(t *testing.T) {
	ct := NewCostTracker(0)
	if ct.budgetUSD != 0 {
		t.Errorf("budgetUSD = %v, want 0", ct.budgetUSD)
	}
}

func TestRecordCost(t *testing.T) {
	ct := NewCostTracker(10.0)
	ct.RecordCost(1.5)
	if ct.TotalCost() != 1.5 {
		t.Errorf("TotalCost = %v, want 1.5", ct.TotalCost())
	}
}

func TestRecordCost_Accumulates(t *testing.T) {
	ct := NewCostTracker(10.0)
	ct.RecordCost(1.0)
	ct.RecordCost(2.0)
	ct.RecordCost(0.5)
	if ct.TotalCost() != 3.5 {
		t.Errorf("TotalCost = %v, want 3.5", ct.TotalCost())
	}
}

func TestTotalCost_Zero(t *testing.T) {
	ct := NewCostTracker(10.0)
	if ct.TotalCost() != 0 {
		t.Errorf("TotalCost = %v, want 0", ct.TotalCost())
	}
}

func TestBudgetRemaining(t *testing.T) {
	ct := NewCostTracker(10.0)
	ct.RecordCost(3.0)
	remaining := ct.BudgetRemaining()
	if remaining != 7.0 {
		t.Errorf("BudgetRemaining = %v, want 7.0", remaining)
	}
}

func TestBudgetRemaining_Unlimited(t *testing.T) {
	ct := NewCostTracker(0)
	ct.RecordCost(100.0)
	remaining := ct.BudgetRemaining()
	if remaining != 0 {
		t.Errorf("BudgetRemaining = %v, want 0 (unlimited)", remaining)
	}
}

func TestBudgetRemaining_Exceeded(t *testing.T) {
	ct := NewCostTracker(5.0)
	ct.RecordCost(10.0)
	remaining := ct.BudgetRemaining()
	if remaining != 0 {
		t.Errorf("BudgetRemaining = %v, want 0 (exceeded)", remaining)
	}
}

func TestBudgetExceeded(t *testing.T) {
	ct := NewCostTracker(5.0)
	ct.RecordCost(5.0)
	if !ct.BudgetExceeded() {
		t.Error("BudgetExceeded should be true when cost == budget")
	}
}

func TestBudgetExceeded_NotExceeded(t *testing.T) {
	ct := NewCostTracker(10.0)
	ct.RecordCost(5.0)
	if ct.BudgetExceeded() {
		t.Error("BudgetExceeded should be false when cost < budget")
	}
}

func TestBudgetExceeded_Unlimited(t *testing.T) {
	ct := NewCostTracker(0)
	ct.RecordCost(1000.0)
	if ct.BudgetExceeded() {
		t.Error("BudgetExceeded should be false for unlimited budget")
	}
}

func TestRecordCost_Concurrent(t *testing.T) {
	ct := NewCostTracker(1000.0)
	var wg sync.WaitGroup
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			ct.RecordCost(0.1)
		}()
	}
	wg.Wait()
	// Due to floating point, exact equality is not guaranteed,
	// but total should be approximately 10.0
	total := ct.TotalCost()
	if total < 9.9 || total > 10.1 {
		t.Errorf("TotalCost = %v, want approximately 10.0", total)
	}
}
