package tui

import (
	"fmt"
	"strings"

	"github.com/eshanized/M31A/internal/types"
)


// handleWorkflow shows the current workflow phase and status.
func handleWorkflow(_ []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session."}
	}

	goal, phase, questions, err := ctx.SessionManager.LoadWorkflowState(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to load workflow state: %v", err)}
	}

	var sb strings.Builder
	sb.WriteString("**Workflow status:**\n\n")
	sb.WriteString(fmt.Sprintf("  Session: **%s**\n", ctx.SessionID))
	if goal != "" {
		sb.WriteString(fmt.Sprintf("  Goal:    %s\n", goal))
	} else {
		sb.WriteString("  Goal:    (none set)\n")
	}
	sb.WriteString(fmt.Sprintf("  Phase:   **%s**\n", phase))
	if len(questions) > 0 {
		sb.WriteString(fmt.Sprintf("  Pending questions: %d\n", len(questions)))
	}

	return CommandResult{Success: true, Message: sb.String()}
}

// handlePhase handles phase navigation commands: /plan, /execute, /verify, /ship, /phase.
// It shows current phase info or transitions to the requested phase.
func handlePhase(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session."}
	}

	goal, currentPhase, _, err := ctx.SessionManager.LoadWorkflowState(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to load workflow state: %v", err)}
	}

	if len(args) == 0 {
		// Show current phase
		return CommandResult{
			Success: true,
			Message: fmt.Sprintf("**Current phase:** %s\n**Goal:** %s", currentPhase, goal),
		}
	}

	// Determine target phase from the first argument (or command name).
	target := strings.ToLower(args[0])
	var targetPhase types.WorkflowPhase
	switch target {
	case "discuss":
		targetPhase = types.PhaseDiscuss
	case "plan":
		targetPhase = types.PhasePlan
	case "execute":
		targetPhase = types.PhaseExecute
	case "verify":
		targetPhase = types.PhaseVerify
	case "ship":
		targetPhase = types.PhaseShip
	case "idle":
		targetPhase = types.PhaseIdle
	default:
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Unknown phase %q. Valid: discuss, plan, execute, verify, ship.", target),
		}
	}

	// Map phase to screen
	var screen Screen
	switch targetPhase {
	case types.PhaseDiscuss:
		screen = ScreenDiscuss
	case types.PhasePlan:
		screen = ScreenPlan
	case types.PhaseExecute:
		screen = ScreenExecute
	case types.PhaseVerify:
		screen = ScreenVerify
	case types.PhaseShip:
		screen = ScreenShip
	default:
		screen = ScreenREPL
	}

	return CommandResult{
		Success:        true,
		Message:        fmt.Sprintf("Transitioning to **%s** phase.", targetPhase),
		Screen:         &screen,
		WorkflowResume: true,
		ResumePhase:    targetPhase,
		ResumeGoal:     goal,
	}
}

// handlePause pauses the current workflow / AutoDream consolidation.
func handlePause(_ []string, ctx CommandContext) CommandResult {
	if ctx.AutoDream != nil {
		ctx.AutoDream.Pause()
	}

	return CommandResult{
		Success: true,
		Message: "Workflow paused. Use **/resume-task** to continue.",
	}
}

// handleResumeTask resumes a paused workflow or opens the session browser.
func handleResumeTask(_ []string, ctx CommandContext) CommandResult {
	if ctx.AutoDream != nil && ctx.AutoDream.IsPaused() {
		ctx.AutoDream.Resume()
	}

	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session."}
	}

	goal, phase, questions, err := ctx.SessionManager.LoadWorkflowState(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to load workflow state: %v", err)}
	}

	if phase == types.PhaseIdle || phase == "" {
		// Nothing in progress, open session browser
		screen := ScreenResume
		return CommandResult{
			Success: true,
			Screen:  &screen,
			Message: "No active workflow to resume. Opening session browser.",
		}
	}

	// Map the persisted phase to a screen and resume
	var screen Screen
	switch phase {
	case types.PhaseDiscuss:
		screen = ScreenDiscuss
	case types.PhasePlan:
		screen = ScreenPlan
	case types.PhaseExecute:
		screen = ScreenExecute
	case types.PhaseVerify:
		screen = ScreenVerify
	case types.PhaseShip:
		screen = ScreenShip
	default:
		screen = ScreenREPL
	}

	return CommandResult{
		Success:         true,
		Message:         fmt.Sprintf("Resuming **%s** phase for goal: %s", phase, goal),
		Screen:          &screen,
		WorkflowResume:  true,
		ResumePhase:     phase,
		ResumeGoal:      goal,
		ResumeQuestions: questions,
	}
}
