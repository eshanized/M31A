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

// runPlan generates a rich implementation plan and task list to accomplish the goal.
// On refinement (when e.refineFeedback is non-empty), the previous plan is injected
// into the LLM context along with the user's feedback for revision.
func (e *Engine) runPlan(ctx context.Context, goal string) (*PhaseResult, error) {
	e.logger.Info("plan phase starting", "version", e.planVersion+1)

	var tasks []m31types.Task
	var planMarkdown string
	var lastErr error
	var rawResponse string
	var valErrs []string
	var allValErrs []string

	isRefinement := e.refineFeedback != ""
	if isRefinement {
		e.emit(IntermediateProgressMsg{
			Phase:   "plan",
			Message: fmt.Sprintf("Refining plan (v%d)...", e.planVersion+1),
		})
	}

	for attempt := 0; attempt < m31types.MaxPlanRetries; attempt++ {
		if !isRefinement {
			e.emit(IntermediateProgressMsg{
				Phase:   "plan",
				Message: fmt.Sprintf("Creating implementation plan... (attempt %d)", attempt+1),
			})
		}

		messages := e.buildPlanContext(goal, tasks, valErrs, rawResponse)

		content, err := e.streamLLM(ctx, messages, false)
		if err != nil {
			lastErr = err
			e.logger.Warn("LLM error in plan phase", "attempt", attempt, "error", err)
			valErrs = []string{err.Error()}
			allValErrs = append(allValErrs, fmt.Sprintf("attempt %d LLM error: %s", attempt+1, err.Error()))
			rawResponse = ""
			continue
		}

		plan, parseErr := ParsePlan(content)
		if parseErr != nil {
			parsed, jsonErr := parseTasksFromJSON(content)
			if jsonErr != nil {
				lastErr = fmt.Errorf("parse error: %w", parseErr)
				e.logger.Warn("plan parse error", "attempt", attempt, "error", parseErr)
				valErrs = []string{lastErr.Error()}
				allValErrs = append(allValErrs, fmt.Sprintf("attempt %d parse error: %s", attempt+1, lastErr.Error()))
				rawResponse = content
				continue
			}
			tasks = parsed
			planMarkdown = content
		} else {
			tasks = plan.Tasks
			planMarkdown = content

			if len(tasks) == 0 {
				parsed, jsonErr := parseTasksFromJSON(content)
				if jsonErr == nil {
					tasks = parsed
				} else {
					lastErr = fmt.Errorf("no tasks found in plan: %w", jsonErr)
					valErrs = []string{"no tasks found in plan output"}
					allValErrs = append(allValErrs, fmt.Sprintf("attempt %d: no tasks found", attempt+1))
					rawResponse = content
					continue
				}
			}
		}

		valErrs = validateTasks(tasks)
		if len(valErrs) > 0 {
			lastErr = fmt.Errorf("validation errors: %s", strings.Join(valErrs, "; "))
			e.logger.Warn("task validation errors", "attempt", attempt, "errors", valErrs)
			allValErrs = append(allValErrs, fmt.Sprintf("attempt %d validation: %s", attempt+1, strings.Join(valErrs, "; ")))
			rawResponse = content
			tasks = nil
			continue
		}

		break
	}

	if len(tasks) == 0 {
		e.logger.Error("failed to generate valid plan after retries", "retries", m31types.MaxPlanRetries)
		combinedErrs := strings.Join(allValErrs, "\n")
		return &PhaseResult{
			Phase:               m31types.PhasePlan,
			Success:             false,
			Error:               fmt.Sprintf("Plan generation failed after %d attempts:\n%s", m31types.MaxPlanRetries, combinedErrs),
			RequiresManualInput: true,
		}, lastErr
	}

	for i := range tasks {
		tasks[i].Status = m31types.StatusPending
	}

	e.planMarkdown = planMarkdown
	if !isRefinement {
		e.planVersion = 1
	}

	if err := e.sessionMgr.SavePlan(e.sessionID, e.planVersion, planMarkdown); err != nil {
		e.logger.Warn("save plan.md failed", "error", err)
	}

	if err := e.sessionMgr.SaveTasks(e.sessionID, tasks); err != nil {
		return nil, fmt.Errorf("save tasks: %w", err)
	}

	if err := e.sessionMgr.SaveTasksCheckbox(e.sessionID, tasks); err != nil {
		e.logger.Warn("save checkbox tasks.md failed", "error", err)
	}

	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
		Phase:     m31types.PhasePlan,
		Timestamp: time.Now(),
	}); err != nil {
		e.logger.Warn("checkpoint save failed", "error", err)
	}

	if err := e.sessionMgr.SaveState(e.sessionID, m31types.PhasePlan, fmt.Sprintf("%d tasks generated (v%d)", len(tasks), e.planVersion), "plan complete"); err != nil {
		return nil, fmt.Errorf("save state: %w", err)
	}

	e.refineFeedback = ""

	e.logger.Info("plan phase complete", "task_count", len(tasks), "version", e.planVersion)

	return &PhaseResult{
		Phase:   m31types.PhasePlan,
		Success: true,
		Tasks:   tasks,
	}, nil
}

// buildPlanContext creates messages for the plan phase.
// On refinement, the previous plan and user feedback are injected.
func (e *Engine) buildPlanContext(goal string, existingTasks []m31types.Task, validationErrors []string, rawResponse string) []m31types.Message {
	var messages []m31types.Message
	messages = append(messages, m31types.Message{Role: "system", Content: e.buildSystemPrompt(e.prompts.ToolUse, e.prompts.PlanFormat)})

	project, projErr := e.sessionMgr.LoadProject(e.sessionID)
	if projErr != nil {
		e.logger.Warn("plan context: failed to load project", "error", projErr)
	}
	projectType := "unknown"
	framework := ""
	if project != nil {
		projectType = project.ProjectType
		framework = project.Framework
	}

	ctx := fmt.Sprintf("Goal: %s\nProject Type: %s\nFramework: %s\n\n", goal, projectType, framework)

	sessionDir := filepath.Dir(e.planningDir)
	memPath := filepath.Join(sessionDir, "MEMORY.md")
	if mem, err := os.ReadFile(memPath); err == nil {
		ctx += "## Cross-Session Memory\n" + string(mem) + "\n\n"
	}

	fileSchema := listCwdFiles(e.workDir)
	if fileSchema != "" {
		ctx += "Existing files:\n" + fileSchema + "\n\n"
	}

	if project != nil && len(project.Answers) > 0 {
		ctx += "User answers from Discuss phase:\n"
		for q, a := range project.Answers {
			ctx += fmt.Sprintf("- Q: %s → A: %s\n", q, a)
		}
		ctx += "\n"
	}

	if e.refineFeedback != "" && e.planMarkdown != "" {
		prevPlan := e.planMarkdown
		if len(prevPlan) > 4000 {
			prevPlan = prevPlan[:4000] + "\n... (truncated)"
		}
		ctx += "## Previous Plan (v" + fmt.Sprintf("%d", e.planVersion) + ")\n" + prevPlan + "\n\n"
		ctx += "## User Refinement Feedback\n" + e.refineFeedback + "\n\n"
		ctx += "Please revise the plan above based on the user's feedback. Return the complete revised plan in the same format.\n\n"
	}

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
		ctx += "Please fix the issues above and return a corrected plan.\n\n"
	}

	messages = append(messages, m31types.Message{Role: "user", Content: ctx})

	return messages
}
