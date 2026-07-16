package workflow

import (
	"context"
	"fmt"
	"strings"

	m31types "github.com/eshanized/M31A/pkg/types"
)

// PlanCheckResult holds the outcome of the plan checker review.
type PlanCheckResult struct {
	Passed    bool
	Issues    []PlanIssue
	Iteration int
}

// PlanIssue represents a single issue found by the plan checker or coverage gates.
type PlanIssue struct {
	Severity string // "blocker" or "warning"
	Category string // granularity, acceptance, dependency, coverage, alignment, concreteness, security, gap
	Message  string
	TaskID   int // 0 if not task-specific
}

// maxPlanRevisions returns the maximum number of revision iterations.
func (e *Engine) maxPlanRevisions() int {
	if e.cfg != nil && e.cfg.Features.PlanCheckMaxIter > 0 {
		return e.cfg.Features.PlanCheckMaxIter
	}
	return 3
}

// checkPlan runs the plan checker — a separate LLM call that reviews plan quality.
func (e *Engine) checkPlan(ctx context.Context, plan *m31types.Plan, goal string) (*PlanCheckResult, error) {
	e.emit(IntermediateProgressMsg{
		Phase:   "plan",
		Message: "Checking plan quality...",
	})

	messages := e.buildCheckContext(plan, goal)

	content, err := e.streamLLM(ctx, messages, false)
	if err != nil {
		return nil, fmt.Errorf("plan check LLM call failed: %w", err)
	}

	result := parseCheckResult(content)
	return result, nil
}

// revisePlan re-invokes the planner with checker feedback for targeted fixes.
func (e *Engine) revisePlan(ctx context.Context, plan *m31types.Plan, issues []PlanIssue, goal string) (*m31types.Plan, error) {
	e.emit(IntermediateProgressMsg{
		Phase:   "plan",
		Message: fmt.Sprintf("Revising plan (%d issues)...", len(issues)),
	})

	messages := e.buildRevisionContext(plan, issues, goal)

	content, err := e.streamLLM(ctx, messages, false)
	if err != nil {
		return nil, fmt.Errorf("plan revision LLM call failed: %w", err)
	}

	revised, parseErr := ParsePlan(content)
	if parseErr != nil {
		// Try JSON fallback
		tasks, jsonErr := parseTasksFromJSON(content)
		if jsonErr != nil {
			return nil, fmt.Errorf("revision parse failed: %w (json fallback: %v)", parseErr, jsonErr)
		}
		revised = &m31types.Plan{
			RawMarkdown: content,
			Tasks:       tasks,
			Version:     plan.Version + 1,
		}
	} else {
		revised.Version = plan.Version + 1
	}

	return revised, nil
}

// buildCheckContext assembles messages for the plan checker.
func (e *Engine) buildCheckContext(plan *m31types.Plan, goal string) []m31types.Message {
	var messages []m31types.Message

	systemPrompt := e.buildSystemPrompt(e.promptOrGet("plan-check"))
	messages = append(messages, m31types.Message{Role: "system", Content: systemPrompt})

	var userCtx strings.Builder
	fmt.Fprintf(&userCtx, "## Goal\n%s\n\n", goal)
	userCtx.WriteString("## Plan to Review\n\n")
	userCtx.WriteString(plan.RawMarkdown)
	userCtx.WriteString("\n\n## Task List\n")
	userCtx.WriteString(formatTaskSummary(plan.Tasks))

	messages = append(messages, m31types.Message{Role: "user", Content: userCtx.String()})
	return messages
}

// buildRevisionContext assembles messages for plan revision.
func (e *Engine) buildRevisionContext(plan *m31types.Plan, issues []PlanIssue, goal string) []m31types.Message {
	var messages []m31types.Message

	systemPrompt := e.buildSystemPrompt(e.promptOrGet("plan-check"), e.promptOrGet("plan-revise"))
	messages = append(messages, m31types.Message{Role: "system", Content: systemPrompt})

	var userCtx strings.Builder
	fmt.Fprintf(&userCtx, "## Goal\n%s\n\n", goal)

	userCtx.WriteString("## Checker Issues to Fix\n\n")
	for _, issue := range issues {
		taskRef := ""
		if issue.TaskID > 0 {
			taskRef = fmt.Sprintf("Task %d: ", issue.TaskID)
		}
		fmt.Fprintf(&userCtx, "- [%s] %s%s — %s\n",
			issue.Severity, taskRef, issue.Category, issue.Message)
	}

	userCtx.WriteString("\n## Current Plan\n\n")
	userCtx.WriteString(plan.RawMarkdown)

	messages = append(messages, m31types.Message{Role: "user", Content: userCtx.String()})
	return messages
}

// parseCheckResult extracts a PlanCheckResult from the checker's LLM response.
func parseCheckResult(content string) *PlanCheckResult {
	result := &PlanCheckResult{}

	if strings.Contains(content, "PLAN CHECK PASSED") || strings.Contains(content, "No issues found") {
		result.Passed = true
		return result
	}

	// Parse blocker and warning lines
	lines := strings.Split(content, "\n")
	for _, line := range lines {
		trimmed := strings.TrimSpace(line)
		if !strings.HasPrefix(trimmed, "- [") {
			continue
		}

		issue := PlanIssue{}
		if strings.Contains(trimmed, "[B") {
			issue.Severity = "blocker"
		} else if strings.Contains(trimmed, "[W") {
			issue.Severity = "warning"
		} else {
			continue
		}

		// Extract category and message from format: - [B1] Task {id}: {category} — {message}
		afterBracket := trimmed[strings.Index(trimmed, "]")+1:]
		afterBracket = strings.TrimSpace(afterBracket)

		if strings.HasPrefix(afterBracket, "Task ") {
			// Try to extract task ID
			parts := strings.SplitN(afterBracket, ":", 2)
			if len(parts) >= 2 {
				idStr := strings.TrimPrefix(parts[0], "Task ")
				idStr = strings.TrimSpace(idStr)
				var taskID int
				if _, err := fmt.Sscanf(idStr, "%d", &taskID); err == nil {
					issue.TaskID = taskID
				}
				rest := strings.TrimSpace(parts[1])
				catParts := strings.SplitN(rest, "—", 2)
				if len(catParts) == 2 {
					issue.Category = strings.TrimSpace(catParts[0])
					issue.Message = strings.TrimSpace(catParts[1])
				} else {
					issue.Message = rest
				}
			}
		} else {
			// No task reference
			catParts := strings.SplitN(afterBracket, "—", 2)
			if len(catParts) == 2 {
				issue.Category = strings.TrimSpace(catParts[0])
				issue.Message = strings.TrimSpace(catParts[1])
			} else {
				issue.Message = afterBracket
			}
		}

		result.Issues = append(result.Issues, issue)
	}

	blockers := 0
	for _, i := range result.Issues {
		if i.Severity == "blocker" {
			blockers++
		}
	}

	result.Passed = blockers == 0
	return result
}

// countIssuesByType returns blocker and warning counts from a list of issues.
func countIssuesByType(issues []PlanIssue) (blockers, warnings int) {
	for _, i := range issues {
		if i.Severity == "blocker" {
			blockers++
		} else {
			warnings++
		}
	}
	return
}

// isPlanCheckStalled returns true when the issue count is not decreasing
// across consecutive iterations, indicating the revision loop is stuck.
func isPlanCheckStalled(current, previous int) bool {
	return previous > 0 && current >= previous
}
