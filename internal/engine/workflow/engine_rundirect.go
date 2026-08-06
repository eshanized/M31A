package workflow

// RunPhaseDirect is a test-only variant that bypasses state machine transition validation.
// Production code MUST use RunPhase which validates transitions via stateMachine.Transition().

import (
	"context"
	"fmt"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	m31types "github.com/eshanized/M31A/internal/core/types"
)

// RunPhaseDirect executes a single phase WITHOUT state machine transition validation.
// INTENDED FOR TEST USE ONLY — does not enforce phase ordering.
// Production code MUST use RunPhase which validates transitions via stateMachine.Transition().
func (e *Engine) RunPhaseDirect(ctx context.Context, phase m31types.WorkflowPhase, goal string) (*PhaseResult, error) {
	e.state.SetCurrentGoal(goal)

	// Budget check (same as RunPhase)
	if e.cfg != nil && e.cfg.Features.BudgetLimitUSD > 0 {
		cost := e.costTracker.TotalCost()
		if cost >= e.cfg.Features.BudgetLimitUSD {
			return &PhaseResult{Phase: phase, Success: false, Error: "budget limit exceeded"}, fmt.Errorf("budget limit exceeded")
		}
	}

	start := time.Now()

	// PrePhaseSetup (same as RunPhase)
	currentMessages := e.state.MessagesSnapshot()
	var err error
	newMessages, err := e.phaseCoordinator.PrePhaseSetup(ctx, phase, &budgetConfigAdapter{cfg: e.cfg}, currentMessages, e.proactiveCompactCheck)
	e.state.SetMessages(newMessages)
	if err != nil {
		return &PhaseResult{Phase: phase, Success: false, Error: err.Error()}, err
	}

	e.toolCallsSinceLastCompact = 0

	// KEY DIFFERENCE: Skip e.stateMachine.Transition(from, phase) — this is the bypass

	var result *PhaseResult
	switch phase {
	case m31types.PhaseInitialize:
		result, err = e.runInitialize(ctx, goal)
	case m31types.PhaseDiscuss:
		result, err = e.runDiscuss(ctx, goal)
	case m31types.PhasePlan:
		result, err = e.runPlan(ctx, goal)
	case m31types.PhaseExecute:
		result, err = e.runExecute(ctx, goal)
	case m31types.PhaseVerify:
		result, err = e.runVerify(ctx, goal)
	case m31types.PhaseRuntime:
		result, err = e.runRuntime(ctx, goal)
	case m31types.PhaseShip:
		result, err = e.runShip(ctx, goal)
	default:
		return nil, fmt.Errorf("%w: unknown phase %s", m31errors.ErrPhaseTransition, phase)
	}

	if result != nil {
		result.WorkflowMode = e.WorkflowMode()
	}

	e.phaseCoordinator.PostPhaseExecution(phase, result, start)
	return result, err
}
