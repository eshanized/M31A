package commands

import (
	"fmt"
	"strings"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"
)

// handleNew starts a fresh workflow by resetting workflow state and opening the goal input.
func handleNew(_ []string, ctx CommandContext) CommandResult {
	if ctx.CancelAgent != nil {
		ctx.CancelAgent()
	}
	if ctx.ClearMessages != nil {
		ctx.ClearMessages()
	}
	screen := tuitypes.ScreenGoalInput
	return CommandResult{
		Success: true,
		Message: "Starting new workflow...",
		Screen:  &screen,
	}
}

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
	fmt.Fprintf(&sb, "  Session: **%s**\n", ctx.SessionID)
	if goal != "" {
		fmt.Fprintf(&sb, "  Goal:    %s\n", goal)
	} else {
		sb.WriteString("  Goal:    (none set)\n")
	}
	fmt.Fprintf(&sb, "  Phase:   **%s**\n", phase)
	if len(questions) > 0 {
		fmt.Fprintf(&sb, "  Pending questions: %d\n", len(questions))
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

	// Guard: don't navigate to phase screens if no workflow is active
	if currentPhase == types.PhaseIdle || currentPhase == "" {
		return CommandResult{
			Success: false,
			Message: "No active workflow. Use /new to start a workflow first.",
		}
	}

	// Cancel any running agent loop before transitioning phases
	if ctx.CancelAgent != nil {
		ctx.CancelAgent()
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
	case "runtime":
		targetPhase = types.PhaseRuntime
	case "ship":
		targetPhase = types.PhaseShip
	case "idle":
		targetPhase = types.PhaseIdle
	default:
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Unknown phase %q. Valid: discuss, plan, execute, verify, runtime, ship.", target),
		}
	}

	// Map phase to screen
	var screen tuitypes.Screen
	switch targetPhase {
	case types.PhaseDiscuss:
		screen = tuitypes.ScreenDiscuss
	case types.PhasePlan:
		screen = tuitypes.ScreenPlan
	case types.PhaseExecute:
		screen = tuitypes.ScreenExecute
	case types.PhaseVerify:
		screen = tuitypes.ScreenVerify
	case types.PhaseRuntime:
		screen = tuitypes.ScreenRuntimeCheck
	case types.PhaseShip:
		screen = tuitypes.ScreenShip
	default:
		screen = tuitypes.ScreenREPL
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

// handleRefine opens the plan refinement input when in the Plan phase.
func handleRefine(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session."}
	}

	_, phase, _, err := ctx.SessionManager.LoadWorkflowState(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to load workflow state: %v", err)}
	}

	if phase != types.PhasePlan {
		return CommandResult{Success: false, Message: "Refine is only available during the Plan phase. Current phase: " + string(phase)}
	}

	screen := tuitypes.ScreenPlan
	return CommandResult{
		Success: true,
		Message: "Opening plan refinement. Press `r` on the plan screen to enter refine mode.",
		Screen:  &screen,
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
		screen := tuitypes.ScreenResume
		return CommandResult{
			Success: true,
			Screen:  &screen,
			Message: "No active workflow to resume. Opening session browser.",
		}
	}

	// Map the persisted phase to a screen and resume
	var screen tuitypes.Screen
	switch phase {
	case types.PhaseDiscuss:
		screen = tuitypes.ScreenDiscuss
	case types.PhasePlan:
		screen = tuitypes.ScreenPlan
	case types.PhaseExecute:
		screen = tuitypes.ScreenExecute
	case types.PhaseVerify:
		screen = tuitypes.ScreenVerify
	case types.PhaseShip:
		screen = tuitypes.ScreenShip
	default:
		screen = tuitypes.ScreenREPL
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

// handleAgentMode toggles autonomous agent mode on or off.
func handleAgentMode(args []string, ctx CommandContext) CommandResult {
	if len(args) == 0 {
		status := "on"
		if ctx.AgentMode != nil && !*ctx.AgentMode {
			status = "off"
		}
		return CommandResult{
			Success: true,
			Message: fmt.Sprintf("**Autonomous agent mode:** %s\n\nWhen on, plain text input triggers the agent loop with tool use. When off, text goes to plain LLM chat.\n\nUsage: `/agent on` or `/agent off`", status),
		}
	}

	switch strings.ToLower(args[0]) {
	case "on":
		if ctx.SetAgentMode != nil {
			ctx.SetAgentMode(true)
		}
		return CommandResult{Success: true, Message: "Autonomous agent mode **enabled**. Plain text will now trigger the agent loop with tool use."}
	case "off":
		if ctx.SetAgentMode != nil {
			ctx.SetAgentMode(false)
		}
		return CommandResult{Success: true, Message: "Autonomous agent mode **disabled**. Plain text will go to plain LLM chat without tools."}
	default:
		return CommandResult{Success: false, Message: "Usage: `/agent [on|off]`"}
	}
}

// handlePending shows pending permission requests and batch approval status.
func handlePending(_ []string, ctx CommandContext) CommandResult {
	var sb strings.Builder
	sb.WriteString("**Pending approvals:**\n\n")

	if ctx.Dispatcher == nil {
		sb.WriteString("  No dispatcher available.\n")
		return CommandResult{Success: true, Message: sb.String()}
	}

	// Batch approval status
	batchCount := ctx.Dispatcher.BatchApprovalCount()
	batchTools := ctx.Dispatcher.ActiveBatchToolNames()
	if batchCount > 0 {
		fmt.Fprintf(&sb, "  Batch approvals active: **%s** (%d)\n", batchTools, batchCount)
	} else {
		sb.WriteString("  No active batch approvals.\n")
	}

	// Pending permission count
	pending := ctx.Dispatcher.PendingPermCount()
	if pending > 0 {
		fmt.Fprintf(&sb, "  %d permission request(s) queued.\n", pending)
	} else {
		sb.WriteString("  No pending permission requests.\n")
	}

	return CommandResult{Success: true, Message: sb.String()}
}
