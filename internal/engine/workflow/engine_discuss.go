package workflow

// Discussion phase support: question management, answer submission, refinement feedback, and transition coordination.

import (
	"context"
	"fmt"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/decision"
)

// maxDiscussPlanCycles caps the number of Plan→Discuss→Plan round-trips to
// prevent infinite oscillation between the two phases (BUG-12). One cycle
// (e.g. Plan→Discuss→Plan once) is a normal refinement; beyond that suggests
// a TUI state bug or automated retry loop.
const maxDiscussPlanCycles = 3

// Transition saves a checkpoint and writes STATE.md for the new phase.
// Validates the transition is allowed by the phase ordering guard.
func (e *Engine) Transition(ctx context.Context, from, to m31types.WorkflowPhase) error {
	e.state.transitionMu.Lock()
	defer e.state.transitionMu.Unlock()

	// Delegate transition validation to StateMachine
	if err := e.stateMachine.Transition(from, to); err != nil {
		return fmt.Errorf("phase transition %s -> %s: %w", from, to, err)
	}

	// Delegate transition side effects to PhaseCoordinator
	goal := e.state.CurrentGoal()
	pv := e.state.PlanVersion()
	return e.phaseCoordinator.CoordinateTransition(ctx, from, to, goal, pv)
}

// SubmitDiscussAnswer records an answer for a discuss question.
func (e *Engine) SubmitDiscussAnswer(index int, answer string) error {
	if e.discussState.Questions == nil {
		return fmt.Errorf("%w: no discuss questions", m31errors.ErrPhaseTransition)
	}
	if index < 0 || index >= len(e.discussState.Questions) {
		return fmt.Errorf("%w: invalid question index %d", m31errors.ErrPhaseTransition, index)
	}
	if e.discussState.Answers == nil {
		e.discussState.Answers = make(map[int]string)
	}
	e.discussState.Answers[index] = answer
	return nil
}

// DiscussState returns a copy of the current discuss state. The TUI
// uses this to read the parsed questions after the discuss phase
// returns. The returned struct is a value copy so mutations by the
// TUI do not affect the engine's internal state.
func (e *Engine) DiscussState() DiscussState {
	return e.discussState
}

// SkipDiscuss fills default (empty) answers and saves.
func (e *Engine) SkipDiscuss() error {
	if e.discussState.Questions == nil {
		return nil
	}
	if e.discussState.Answers == nil {
		e.discussState.Answers = make(map[int]string)
	}
	for i := range e.discussState.Questions {
		if _, ok := e.discussState.Answers[i]; !ok {
			e.discussState.Answers[i] = ""
		}
	}
	return e.FinalizeDiscuss()
}

func (e *Engine) FinalizeDiscuss() error {
	project := e.loadProjectCached()
	var questions, answers []string
	for i, q := range e.discussState.Questions {
		questions = append(questions, q)
		a := ""
		if ans, ok := e.discussState.Answers[i]; ok {
			a = ans
		}
		answers = append(answers, a)
	}
	if err := e.saveDiscussAnswers(project, questions, answers); err != nil {
		return fmt.Errorf("save discuss answers: %w", err)
	}
	return nil
}

// SetRefinementFeedback stores user feedback for the next plan regeneration.
// The plan phase reads this field to inject feedback into the LLM context.
// Duplicate feedback is ignored — only new feedback bumps the plan version.
func (e *Engine) SetRefinementFeedback(feedback string) {
	if feedback != "" {
		currentFB := e.state.RefineFeedback()
		if feedback != currentFB {
			e.state.SetRefineFeedback(feedback)
			pv := e.state.IncrementPlanVersion()
			// Log plan revision decision
			e.LogDecision(decision.DecisionReceipt{
				Decision:  fmt.Sprintf("plan revision requested (v%d)", pv),
				Rationale: truncateForLog(feedback, 200),
				Category:  decision.CategoryPlan,
				Cost: decision.Cost{
					Attempts: pv,
				},
			})
		}
	} else {
		e.state.SetRefineFeedback(feedback)
	}
}

// PlanContent returns the current plan markdown content.
func (e *Engine) PlanContent() string {
	return e.state.PlanContent()
}

// PlanVersion returns the current plan version number.
// Version 1 is the initial plan; each refinement increments it.
func (e *Engine) PlanVersion() int {
	return e.state.PlanVersion()
}
