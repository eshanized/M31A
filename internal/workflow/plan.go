package workflow

import (
	"context"
	"fmt"
	"strings"
	"time"

	"github.com/eshanized/M31A/pkg/session"
	m31types "github.com/eshanized/M31A/internal/types"
)

// runPlan generates a task list to accomplish the goal.
func (e *Engine) runPlan(ctx context.Context, goal string) (*PhaseResult, error) {
	e.logger.Info("plan phase starting")

	var tasks []m31types.Task
	var lastErr error

	// Retry loop for invalid task lists
	for attempt := 0; attempt < m31types.MaxPlanRetries; attempt++ {
		// 1. Build context
		messages := e.buildPlanContext(goal, tasks)

		// 2. Stream LLM
		content, err := e.streamLLM(ctx, messages, false)
		if err != nil {
			lastErr = err
			e.logger.Warn("LLM error in plan phase", "attempt", attempt, "error", err)
			continue
		}

		// 3. Parse tasks
		parsed, err := parseTasksFromJSON(content)
		if err != nil {
			lastErr = fmt.Errorf("parse error: %w", err)
			e.logger.Warn("task parse error", "attempt", attempt, "error", err)
			continue
		}

		// 4. Validate
		valErrs := validateTasks(parsed)
		if len(valErrs) > 0 {
			lastErr = fmt.Errorf("validation errors: %s", strings.Join(valErrs, "; "))
			e.logger.Warn("task validation errors", "attempt", attempt, "errors", valErrs)
			continue
		}

		tasks = parsed
		break
	}

	if len(tasks) == 0 {
		e.logger.Error("failed to generate valid task list after retries", "retries", m31types.MaxPlanRetries)
		return &PhaseResult{
			Phase:   m31types.PhasePlan,
			Success: false,
			Error:   lastErr.Error(),
		}, lastErr
	}

	// 5. Set default status
	for i := range tasks {
		tasks[i].Status = m31types.StatusPending
	}

	// 6. Save TASKS.md
	if err := e.sessionMgr.SaveTasks(e.sessionID, tasks); err != nil {
		return nil, fmt.Errorf("save tasks: %w", err)
	}

	// 7. Save checkpoint
	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
		Phase:     m31types.PhasePlan,
		Timestamp: time.Now(),
	}); err != nil {
		e.logger.Warn("checkpoint save failed", "error", err)
	}

	// 8. Write STATE.md
	if err := e.sessionMgr.SaveState(e.sessionID, m31types.PhasePlan, fmt.Sprintf("%d tasks generated", len(tasks)), "plan complete"); err != nil {
		return nil, fmt.Errorf("save state: %w", err)
	}

	e.logger.Info("plan phase complete", "task_count", len(tasks))

	return &PhaseResult{
		Phase:   m31types.PhasePlan,
		Success: true,
		Tasks:   tasks,
	}, nil
}

// buildPlanContext creates messages for the plan phase.
func (e *Engine) buildPlanContext(goal string, existingTasks []m31types.Task) []m31types.Message {
	var messages []m31types.Message
	messages = append(messages, m31types.Message{Role: "system", Content: e.buildSystemPrompt(e.prompts.ToolUse, e.prompts.PlanFormat)})

	// Load PROJECT.md
	project, _ := e.sessionMgr.LoadProject(e.sessionID)
	projectType := "unknown"
	framework := ""
	if project != nil {
		projectType = project.ProjectType
		framework = project.Framework
	}

	// Build context
	ctx := fmt.Sprintf("Goal: %s\nProject Type: %s\nFramework: %s\n\n", goal, projectType, framework)

	// Add file schema
	fileSchema := listCwdFiles(e.workDir)
	if fileSchema != "" {
		ctx += "Existing files:\n" + fileSchema + "\n\n"
	}

	// Add discuss Q&A if available
	if project != nil && len(project.Answers) > 0 {
		ctx += "User answers from Discuss phase:\n"
		for q, a := range project.Answers {
			ctx += fmt.Sprintf("- Q: %s → A: %s\n", q, a)
		}
		ctx += "\n"
	}

	// Add previous attempt errors if any
	if len(existingTasks) > 0 {
		ctx += "Previous task list had errors. Please fix and return corrected JSON.\n\n"
	}

	ctx += "Generate a task list to accomplish the goal. Return a JSON array of tasks. Each task must have: id (int), action (string: Add/Modify/Delete/Create), description (string), dependencies (array of int, empty if none), files (array of string), acceptance_criteria (array of string). Do not include any text outside the JSON array."

	messages = append(messages, m31types.Message{Role: "user", Content: ctx})

	return messages
}
