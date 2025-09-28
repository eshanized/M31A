package workflow

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
)

// runPlan generates a task list to accomplish the goal.
func (e *Engine) runPlan(ctx context.Context, goal string) (*PhaseResult, error) {
	e.logger.Info("plan phase starting")

	var tasks []m31types.Task
	var lastErr error
	var rawResponse string
	var valErrs []string
	var allValErrs []string // accumulate all validation errors across retries

	// Retry loop for invalid task lists
	for attempt := 0; attempt < m31types.MaxPlanRetries; attempt++ {
		// Emit intermediate progress
		e.emit(IntermediateProgressMsg{
			Phase:   "plan",
			Message: fmt.Sprintf("Creating task list... (attempt %d)", attempt+1),
		})

		// 1. Build context with error feedback on retries
		messages := e.buildPlanContext(goal, tasks, valErrs, rawResponse)

		// 2. Stream LLM
		content, err := e.streamLLM(ctx, messages, false)
		if err != nil {
			lastErr = err
			e.logger.Warn("LLM error in plan phase", "attempt", attempt, "error", err)
			valErrs = []string{err.Error()}
			allValErrs = append(allValErrs, fmt.Sprintf("attempt %d LLM error: %s", attempt+1, err.Error()))
			rawResponse = ""
			continue
		}

		// 3. Parse tasks
		parsed, err := parseTasksFromJSON(content)
		if err != nil {
			lastErr = fmt.Errorf("parse error: %w", err)
			e.logger.Warn("task parse error", "attempt", attempt, "error", err)
			valErrs = []string{lastErr.Error()}
			allValErrs = append(allValErrs, fmt.Sprintf("attempt %d parse error: %s", attempt+1, lastErr.Error()))
			rawResponse = content
			continue
		}

		// 4. Validate
		valErrs = validateTasks(parsed)
		if len(valErrs) > 0 {
			lastErr = fmt.Errorf("validation errors: %s", strings.Join(valErrs, "; "))
			e.logger.Warn("task validation errors", "attempt", attempt, "errors", valErrs)
			allValErrs = append(allValErrs, fmt.Sprintf("attempt %d validation: %s", attempt+1, strings.Join(valErrs, "; ")))
			rawResponse = content
			continue
		}

		tasks = parsed
		break
	}

	if len(tasks) == 0 {
		e.logger.Error("failed to generate valid task list after retries", "retries", m31types.MaxPlanRetries)
		combinedErrs := strings.Join(allValErrs, "\n")
		return &PhaseResult{
			Phase:               m31types.PhasePlan,
			Success:             false,
			Error:               fmt.Sprintf("Plan generation failed after %d attempts:\n%s", m31types.MaxPlanRetries, combinedErrs),
			RequiresManualInput: true,
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
func (e *Engine) buildPlanContext(goal string, existingTasks []m31types.Task, validationErrors []string, rawResponse string) []m31types.Message {
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

	// Load MEMORY.md if exists
	sessionDir := filepath.Dir(e.planningDir)
	memPath := filepath.Join(sessionDir, "MEMORY.md")
	if mem, err := os.ReadFile(memPath); err == nil {
		ctx += "## Cross-Session Memory\n" + string(mem) + "\n\n"
	}

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

	// Add previous attempt errors and raw response if any
	if len(existingTasks) > 0 {
		ctx += "## Previous Attempt Failed\n"
		if len(validationErrors) > 0 {
			ctx += "Errors:\n"
			for _, e := range validationErrors {
				ctx += "- " + e + "\n"
			}
			ctx += "\n"
		}
		if rawResponse != "" {
			ctx += "Previous LLM response (truncated):\n" + rawResponse[:min(len(rawResponse), 2000)] + "\n\n"
		}
		ctx += "Please fix the issues above and return a corrected JSON task array.\n\n"
	}

	// On retry: add corrective instruction only — the system prompt (plan-format.md) already
	// specifies the full schema; repeating it here creates conflicting authority.
	if len(existingTasks) > 0 {
		ctx += "Return a corrected JSON task array that resolves all errors listed above.\n\n"
	}

	messages = append(messages, m31types.Message{Role: "user", Content: ctx})

	return messages
}
