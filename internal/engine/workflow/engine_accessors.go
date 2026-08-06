package workflow

// Engine accessor methods: getters, setters, decision logging, and context management.

import (
	"context"
	"fmt"
	"strings"

	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/decision"
	"github.com/eshanized/M31A/internal/integrations/git"
	"github.com/eshanized/M31A/internal/integrations/ledger"
	"github.com/eshanized/M31A/internal/integrations/metrics"
)

// SetIntentResult stores the LLM-classified intent result for downstream enrichment.
func (e *Engine) SetIntentResult(ir *m31types.IntentResult) {
	e.state.SetIntentResult(ir)
	// Log intent classification decision
	if ir != nil {
		e.LogDecision(decision.DecisionReceipt{
			Decision:  fmt.Sprintf("intent:%s (complexity:%s, confidence:%.0f%%)", ir.Intent, ir.Complexity, ir.Confidence*100),
			Rationale: ir.Summary,
			Category:  decision.CategoryIntent,
		})
	}
}

// IntentResult returns the stored intent classification result, or nil if unset.
func (e *Engine) IntentResult() *m31types.IntentResult {
	return e.state.IntentResult()
}

// ScopeIncludes returns true if the intent result's scope contains the given term.
func (e *Engine) ScopeIncludes(term string) bool {
	ir := e.state.IntentResult()
	if ir == nil {
		return false
	}
	for _, s := range ir.Scope {
		if strings.EqualFold(s, term) {
			return true
		}
	}
	return false
}

// LogDecision records a decision in the session log.
func (e *Engine) LogDecision(r decision.DecisionReceipt) {
	dl := e.state.DecisionLog()
	if dl != nil {
		dl.Log(r)
		e.emitDecisionsSnapshot()
	}
}

// emitDecisionsSnapshot emits a snapshot of current decisions to the TUI.
func (e *Engine) emitDecisionsSnapshot() {
	if e.msgEmitter == nil {
		return
	}
	decisions := e.SnapshotDecisions()
	if len(decisions) > 0 {
		e.msgEmitter.Emit(DecisionsSnapshotMsg{Decisions: decisions})
	}
}

// FlushDecisions synchronously returns all logged decisions and resets the buffer.
func (e *Engine) FlushDecisions() []decision.DecisionReceipt {
	dl := e.state.DecisionLog()
	if dl == nil {
		return nil
	}
	return dl.Flush()
}

// SnapshotDecisions returns a copy of buffered decisions without flushing.
func (e *Engine) SnapshotDecisions() []decision.DecisionReceipt {
	dl := e.state.DecisionLog()
	if dl == nil {
		return nil
	}
	return dl.Snapshot()
}

// LastHealReport returns the most recent self-heal report, or nil if none.
func (e *Engine) LastHealReport() *m31types.HealReport {
	return e.state.LastHealReport()
}

// WithContext returns a derived context that is cancelled when the engine
// shuts down. All long-running operations should use this context so
// cancellation propagates through the engine's lifecycle.
func (e *Engine) WithContext(ctx context.Context) context.Context {
	if e.ctx != nil {
		return e.ctx
	}
	return ctx
}

// Context returns the engine's root context. It is cancelled when Shutdown
// is called. Returns context.Background() if the engine has no root context
// (e.g., in tests).
func (e *Engine) Context() context.Context {
	if e.ctx != nil {
		return e.ctx
	}
	return context.Background()
}

// SetGit sets the git instance on the engine.
// Required before running any phase that uses git operations.
func (e *Engine) SetGit(g *git.Git) {
	e.git = g
	if h, err := g.HeadHash(); err == nil {
		e.sessionStartHash = h
	}
}

// SetLedger sets the shared ledger instance on the engine so the ship phase
// writes session records to the application-configured path instead of
// creating a new ledger at a hardcoded location.
func (e *Engine) SetLedger(l *ledger.Ledger) {
	e.ledger = l
}

// SessionID returns the current session ID.
func (e *Engine) SessionID() string {
	return e.sessionID
}

// SetSessionID updates the engine's session ID.
func (e *Engine) SetSessionID(id string) {
	e.sessionID = id
}

// SetMsgEmitter sets the callback for emitting messages back to the TUI.
func (e *Engine) SetMsgEmitter(em MsgEmitter) {
	e.msgEmitter = em
}

// SetCollector attaches a metrics collector to the engine for recording
// tool calls, LLM interactions, phase durations, and heal/bisect events.
func (e *Engine) SetCollector(c *metrics.Collector) {
	e.collector = c
}

// GetCostInfo returns the current cost and budget information.
func (e *Engine) GetCostInfo() (totalCost float64, budgetLimit float64, budgetRemaining float64) {
	totalCost = e.costTracker.TotalCost()
	if e.cfg != nil && e.cfg.Features.BudgetLimitUSD > 0 {
		budgetLimit = e.cfg.Features.BudgetLimitUSD
		budgetRemaining = budgetLimit - totalCost
		if budgetRemaining < 0 {
			budgetRemaining = 0
		}
	}
	return
}

// emit sends a message to the TUI if an emitter is configured.
func (e *Engine) emit(msg any) {
	if e.msgEmitter != nil {
		e.msgEmitter.Emit(msg)
	}
}
