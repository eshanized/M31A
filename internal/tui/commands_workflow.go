package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
)

// handleGoal shows or sets the session goal.
func handleGoal(args []string, ctx CommandContext) CommandResult {
	if len(args) == 0 {
		if ctx.SessionManager != nil && ctx.SessionID != "" {
			s, err := ctx.SessionManager.LoadSession(ctx.SessionID)
			if err == nil && s.Project != nil && s.Project.Goal != "" {
				return CommandResult{Success: true, Message: fmt.Sprintf("Session goal: %s", s.Project.Goal)}
			}
		}
		return CommandResult{Success: true, Message: "No goal set. Use /goal <your goal> to set one."}
	}

	goal := strings.Join(args, " ")

	if ctx.SessionManager != nil && ctx.SessionID != "" {
		s, err := ctx.SessionManager.LoadSession(ctx.SessionID)
		if err == nil {
			if s.Project == nil {
				s.Project = &types.ProjectState{Goal: goal}
			} else {
				s.Project.Goal = goal
			}
			ctx.SessionManager.SaveProject(ctx.SessionID, s.Project)
		}
	}

	return CommandResult{Success: true, Message: fmt.Sprintf("Goal set: %s", goal)}
}

// handlePhase shows the current workflow phase or transitions to a new one.
func handlePhase(args []string, ctx CommandContext) CommandResult {
	// Phase aliases (/plan, /execute, /verify, /ship) pass the phase name as the
	// first arg. Detect bare alias usage (single arg matching a valid phase) and
	// route to transition instead of showing status.
	validPhases := map[string]bool{
		"idle": true, "initialize": true, "discuss": true,
		"plan": true, "execute": true, "verify": true, "ship": true,
	}

	if len(args) == 1 && validPhases[args[0]] {
		// Bare phase alias — attempt transition
		phase := args[0]
		if ctx.WorkflowEngine != nil {
			return CommandResult{
				Success: true,
				Message: fmt.Sprintf("Starting %s phase...", phase),
				Cmd: func() tea.Msg {
					return SlashCommandMsg{Command: fmt.Sprintf("/phase %s", phase)}
				},
			}
		}
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Cannot start %s phase: no workflow engine available. Use /workflow <goal> to begin.", phase),
		}
	}

	if len(args) > 0 {
		return CommandResult{
			Success: false,
			Message: "Use /phase <name> to start a workflow phase. Valid phases: idle, initialize, discuss, plan, execute, verify, ship",
		}
	}

	if ctx.SessionManager != nil && ctx.SessionID != "" {
		s, err := ctx.SessionManager.LoadSession(ctx.SessionID)
		if err == nil {
			return CommandResult{Success: true, Message: fmt.Sprintf("Current phase: %s", s.WorkflowPhase)}
		}
	}
	return CommandResult{Success: true, Message: fmt.Sprintf("Current phase: %s", types.PhaseIdle)}
}

// handleTools lists all registered tools with their descriptions and risk levels.
func handleTools(args []string, ctx CommandContext) CommandResult {
	if ctx.Dispatcher == nil {
		return CommandResult{Success: false, Message: "Dispatcher not available."}
	}

	names := ctx.Dispatcher.List()
	if len(names) == 0 {
		return CommandResult{Success: true, Message: "No tools registered."}
	}

	var b strings.Builder
	b.WriteString("Available tools:\n")
	for _, name := range names {
		tool, ok := ctx.Dispatcher.GetTool(name)
		if !ok {
			continue
		}
		risk := "safe"
		switch tool.RiskLevel() {
		case types.RiskDangerous:
			risk = "dangerous"
		case types.RiskDestructive:
			risk = "destructive"
		}
		b.WriteString(fmt.Sprintf("  %-15s [%-12s] %s\n", name, risk, tool.Description()))
	}
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleWorkflow shows the current workflow phase, tasks, and progress.
func handleWorkflow(args []string, ctx CommandContext) CommandResult {
	if len(args) > 0 && args[0] == "resume" {
		if ctx.SessionManager == nil {
			return CommandResult{
				Success: false,
				Message: "No session manager available — cannot resume workflow.",
			}
		}
		if ctx.SessionID == "" {
			return CommandResult{
				Success: false,
				Message: "No active session. Start a workflow first.",
			}
		}
		goal, phase, questions, err := ctx.SessionManager.LoadWorkflowState(ctx.SessionID)
		if err != nil {
			return CommandResult{
				Success: false,
				Message: fmt.Sprintf("Cannot load workflow state: %v", err),
			}
		}
		if phase == types.PhaseIdle || phase == types.PhaseShip {
			return CommandResult{
				Success: false,
				Message: "No workflow in progress. Use /workflow to start one.",
			}
		}
		return CommandResult{
			Success:         true,
			Message:         fmt.Sprintf("Resuming workflow at phase: %s", phase),
			WorkflowResume:  true,
			ResumePhase:     phase,
			ResumeGoal:      goal,
			ResumeQuestions: questions,
		}
	}

	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session. Start a workflow first."}
	}

	s, err := ctx.SessionManager.LoadSession(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to load session: %v", err)}
	}

	var b strings.Builder
	b.WriteString("Workflow STATUS:\n")
	b.WriteString(fmt.Sprintf("  Session:  %s\n", ctx.SessionID))
	b.WriteString(fmt.Sprintf("  Phase:    %s\n", s.WorkflowPhase))

	if ctx.SessionManager != nil {
		tasks, err := ctx.SessionManager.LoadTasks(ctx.SessionID)
		if err == nil && len(tasks) > 0 {
			total, done, failed, skipped := 0, 0, 0, 0
			for _, t := range tasks {
				total++
				switch t.Status {
				case types.StatusDone:
					done++
				case types.StatusFailed:
					failed++
				case types.StatusSkipped:
					skipped++
				}
			}
			b.WriteString(fmt.Sprintf("  Tasks:    %d total, %d done, %d failed, %d skipped\n", total, done, failed, skipped))
		}
	}

	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handlePause informs the user that pause is available from the Execute screen.
func handlePause(args []string, ctx CommandContext) CommandResult {
	return CommandResult{
		Success: true,
		Message: "Pause is available from the Execute screen (press P during task execution).",
	}
}

// handleResumeTask provides workflow restart information.
func handleResumeTask(args []string, ctx CommandContext) CommandResult {
	return CommandResult{
		Success: true,
		Message: "Workflow restart is not yet implemented. Use /workflow <goal> to start a new workflow, or /workflow resume to restart from a persisted phase.",
	}
}
