package workflow

import (
	"context"
	"fmt"
	"strings"

	m31types "github.com/eshanized/M31A/internal/core/types"
)

// replanFromFailure generates replacement tasks after a task failure.
// It builds a plan context that includes the original goal, the failed task's
// details and failure reason, the remaining tasks list, and a summary of what
// succeeded. It calls streamLLM to generate replacement tasks, parses them via
// ParsePlan, validates via validateTasks, and returns the new tasks.
func (e *Engine) replanFromFailure(ctx context.Context, failedTask m31types.Task, remainingTasks []m31types.Task, failureReason string, goal string) ([]m31types.Task, error) {
	// Build context for re-planning
	var contextBuilder strings.Builder

	contextBuilder.WriteString(fmt.Sprintf("Original Goal: %s\n\n", goal))

	contextBuilder.WriteString("## Failed Task\n")
	contextBuilder.WriteString(fmt.Sprintf("Task ID: %d\n", failedTask.ID))
	contextBuilder.WriteString(fmt.Sprintf("Action: %s\n", failedTask.Action))
	contextBuilder.WriteString(fmt.Sprintf("Description: %s\n", failedTask.Description))
	contextBuilder.WriteString(fmt.Sprintf("Files: %s\n", strings.Join(failedTask.Files, ", ")))
	contextBuilder.WriteString(fmt.Sprintf("Failure Reason: %s\n", failureReason))
	contextBuilder.WriteString(fmt.Sprintf("Heals Attempted: %d\n\n", failedTask.HealsAttempted))

	if len(remainingTasks) > 0 {
		contextBuilder.WriteString("## Remaining Tasks\n")
		for _, task := range remainingTasks {
			contextBuilder.WriteString(fmt.Sprintf("- Task %d: %s (%s)\n", task.ID, task.Description, task.Action))
		}
		contextBuilder.WriteString("\n")
	}

	contextBuilder.WriteString("## Instructions\n")
	contextBuilder.WriteString("Generate replacement tasks to accomplish the original goal, taking into account:\n")
	contextBuilder.WriteString("1. The failed task and its failure reason\n")
	contextBuilder.WriteString("2. What has already been accomplished (use file changes as context)\n")
	contextBuilder.WriteString("3. What remains to be done\n\n")
	contextBuilder.WriteString("Return a JSON array of tasks in the same format as the original plan.\n")
	contextBuilder.WriteString("Each task must have: id, description, action, files, dependencies, acceptance_criteria.\n")
	contextBuilder.WriteString("Keep task IDs sequential starting from 1.\n")

	// Build messages for LLM
	messages := []m31types.Message{
		{Role: "system", Content: e.buildSystemPrompt(e.promptOrGet("tool-use"), e.promptOrGet("plan-format"))},
		{Role: "user", Content: contextBuilder.String()},
	}

	// Call LLM to generate replacement tasks
	content, err := e.streamLLM(ctx, messages, false)
	if err != nil {
		return nil, fmt.Errorf("LLM call failed during re-planning: %w", err)
	}

	// Parse the response
	plan, parseErr := ParsePlan(content)
	if parseErr != nil {
		// Try JSON parsing as fallback
		tasks, jsonErr := parseTasksFromJSON(content)
		if jsonErr != nil {
			return nil, fmt.Errorf("failed to parse re-plan output: %w (JSON fallback: %v)", parseErr, jsonErr)
		}
		plan = &m31types.Plan{Tasks: tasks}
	}

	if len(plan.Tasks) == 0 {
		return nil, fmt.Errorf("re-plan produced no tasks")
	}

	// Validate tasks
	valErrs := validateTasks(plan.Tasks)
	if len(valErrs) > 0 {
		return nil, fmt.Errorf("re-plan validation errors: %s", strings.Join(valErrs, "; "))
	}

	return plan.Tasks, nil
}
