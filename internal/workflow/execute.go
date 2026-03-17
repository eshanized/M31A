package workflow

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
	"github.com/eshanized/M31A/pkg/taskrunner"
)

// runExecute executes tasks in dependency order with tool dispatch and self-heal.
func (e *Engine) runExecute(ctx context.Context, goal string) (*PhaseResult, error) {
	e.logger.Info("execute phase starting", "goal", goal)

	// 1. Load tasks
	tasks, err := e.sessionMgr.LoadTasks(e.sessionID)
	if err != nil {
		return nil, fmt.Errorf("load tasks: %w", err)
	}

	if len(tasks) == 0 {
		return &PhaseResult{
			Phase:   m31types.PhaseExecute,
			Success: true,
		}, nil
	}

	// 2. Create runner
	runner := taskrunner.New(tasks)

	// Wire task lifecycle callbacks to emit messages to the TUI.
	runner.OnTaskStart = func(task m31types.Task) {
		e.emit(TaskStartMsg{Task: task})
	}
	runner.OnTaskUpdate = func(task m31types.Task, status string) {
		e.emit(TaskUpdateMsg{Task: task, Status: status})
	}

	// 3. Schedule
	groups, err := runner.Schedule()
	if err != nil {
		return nil, fmt.Errorf("schedule: %w", err)
	}

	// 4. Execute each group sequentially
	var execErrors []string
	toolCallCount := 0
	for _, group := range groups {
		execFn := func(ctx context.Context, task m31types.Task) taskrunner.TaskResult {
			// Save checkpoint before each task for rollback on heal failure.
			if err := e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
				Phase:     m31types.PhaseExecute,
				Timestamp: time.Now(),
				TaskCount: task.ID,
			}); err != nil {
				e.logger.Warn("pre-task checkpoint failed", "task", task.ID, "error", err)
			}

			result := e.executeTaskWithTools(ctx, &task, tasks, goal)
			// Count tool calls from task result
			if result.ToolCalls > 0 {
				toolCallCount += result.ToolCalls
			}
			return result
		}

		if err := runner.ExecuteGroup(ctx, group, execFn); err != nil {
			execErrors = append(execErrors, fmt.Sprintf("group %v: %v", group, err))
			e.logger.Error("execute group failed", "error", err)
		}

		// Update TASKS.md and STATE.md after each group
		updatedTasks := runner.Tasks()
		if err := e.sessionMgr.SaveTasks(e.sessionID, updatedTasks); err != nil {
			e.logger.Warn("save tasks failed", "error", err)
		}
		if err := e.sessionMgr.SaveState(e.sessionID, m31types.PhaseExecute,
			"executing tasks", "group complete"); err != nil {
			e.logger.Warn("save state failed", "error", err)
		}
	}

	// 5. Save checkpoint
	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseExecute,
		Timestamp: time.Now(),
	}); err != nil {
		e.logger.Warn("save checkpoint failed", "error", err)
	}

	// 6. Final state
	updatedTasks := runner.Tasks()
	if err := e.sessionMgr.SaveTasks(e.sessionID, updatedTasks); err != nil {
		e.logger.Warn("save tasks failed", "error", err)
	}

	total, done, failed, skipped := runner.Summary()
	allDone := done+skipped == total

	e.logger.Info("execute phase complete", "total", total, "done", done, "failed", failed, "skipped", skipped)

	result := &PhaseResult{
		Phase:     m31types.PhaseExecute,
		Success:   allDone,
		Tasks:     updatedTasks,
		ToolCalls: toolCallCount,
	}
	if len(execErrors) > 0 {
		result.Error = strings.Join(execErrors, "; ")
		return result, fmt.Errorf("%w: %s", m31errors.ErrTaskFailed, result.Error)
	}
	return result, nil
}

// executeTaskWithTools runs a single task with tool dispatch and self-heal.
func (e *Engine) executeTaskWithTools(ctx context.Context, task *m31types.Task, allTasks []m31types.Task, goal string) taskrunner.TaskResult {
	start := time.Now()

	for task.HealsAttempted < m31types.MaxHealAttempts {
		// Build context
		messages := e.buildExecuteContext(*task, allTasks, goal)

		// Stream LLM with native tool calling
		content, toolCalls, err := e.streamLLMWithTools(ctx, messages)
		if err != nil {
			failureReason := fmt.Sprintf("LLM stream failed: %v", err)
			if task.HealsAttempted >= m31types.MaxHealAttempts {
				return taskrunner.TaskResult{Success: false, Error: failureReason}
			}
			task.HealsAttempted++
			e.logger.Info("self-healing task after LLM failure", "task", task.ID, "attempt", task.HealsAttempted)
			e.emit(SelfHealStartMsg{
				TaskID:  task.ID,
				Attempt: task.HealsAttempted,
				Max:     m31types.MaxHealAttempts,
			})
			healResult := e.healTask(ctx, *task, failureReason, goal)
			e.emit(SelfHealCompleteMsg{
				TaskID:  task.ID,
				Attempt: task.HealsAttempted,
				Max:     m31types.MaxHealAttempts,
				Success: healResult.Success,
				Error:   healResult.Error,
			})
			if !healResult.Success {
				return healResult
			}
			continue
		}

		// Fall back to text-based parsing if no native tool calls
		if len(toolCalls) == 0 {
			parsedCalls, parseErr := e.parseToolCalls(content)
			if parseErr != nil {
				failureReason := fmt.Sprintf("tool call parsing failed: %v", parseErr)
				if task.HealsAttempted >= m31types.MaxHealAttempts {
					return taskrunner.TaskResult{Success: false, Error: failureReason}
				}
				task.HealsAttempted++
				e.logger.Info("self-healing task after parse failure", "task", task.ID, "attempt", task.HealsAttempted)
				e.emit(SelfHealStartMsg{
					TaskID:  task.ID,
					Attempt: task.HealsAttempted,
					Max:     m31types.MaxHealAttempts,
				})
				healResult := e.healTask(ctx, *task, failureReason, goal)
				e.emit(SelfHealCompleteMsg{
					TaskID:  task.ID,
					Attempt: task.HealsAttempted,
					Max:     m31types.MaxHealAttempts,
					Success: healResult.Success,
					Error:   healResult.Error,
				})
				if !healResult.Success {
					return healResult
				}
				continue
			}
			toolCalls = parsedCalls
		}

		// Dispatch tool calls — C-6: execute ALL tool calls and collect results
		var toolExecResults []struct {
			call     m31types.ToolCall
			result   m31types.ToolResult
			err      error
			duration int64
		}

		// CR-05 fix: add ONE assistant message outside the tool-call loop
		// to prevent N duplicate assistant messages for N tool calls.
		messages = append(messages, m31types.Message{
			Role:      "assistant",
			Content:   content,
			ToolCalls: toolCalls,
		})

		for _, tc := range toolCalls {
			// Emit tool start message
			e.emit(ToolStartMsg{
				ToolName:    tc.Name,
				Description: fmt.Sprintf("Executing %s", tc.Name),
			})

			toolStart := time.Now()
			result, err := e.dispatcher.Execute(ctx, tc)
			toolDuration := time.Since(toolStart).Milliseconds()

			toolExecResults = append(toolExecResults, struct {
				call     m31types.ToolCall
				result   m31types.ToolResult
				err      error
				duration int64
			}{
				call:     tc,
				result:   result,
				err:      err,
				duration: toolDuration,
			})

			if err != nil {
				e.emit(ToolCompleteMsg{
					ToolName:   tc.Name,
					Success:    false,
					DurationMs: toolDuration,
					Error:      err.Error(),
				})
			} else {
				e.emit(ToolCompleteMsg{
					ToolName:   tc.Name,
					Success:    true,
					DurationMs: toolDuration,
				})
			}
		}

		// Feed ALL tool results back to the LLM, including errors
		var toolErrMessages []string
		for _, tr := range toolExecResults {
			if tr.err != nil {
				toolErrMessages = append(toolErrMessages, fmt.Sprintf("tool %s failed: %v", tr.call.Name, tr.err))
				messages = append(messages, m31types.Message{
					Role:       "tool",
					Content:    fmt.Sprintf("Error: %v", tr.err),
					ToolCallID: tr.call.ID,
				})
			} else {
				messages = append(messages, m31types.Message{
					Role:       "tool",
					Content:    tr.result.Output,
					ToolCallID: tr.call.ID,
				})
			}
		}

		toolErr := len(toolErrMessages) > 0
		toolCallCount := len(toolExecResults)

		if toolErr {
			if task.HealsAttempted >= m31types.MaxHealAttempts {
				return taskrunner.TaskResult{
					Success: false,
					Error:   strings.Join(toolErrMessages, "; "),
				}
			}
			failureReason := strings.Join(toolErrMessages, "; ")
			task.HealsAttempted++
			e.logger.Info("self-healing task after tool failure", "task", task.ID, "attempt", task.HealsAttempted)
			e.emit(SelfHealStartMsg{
				TaskID:  task.ID,
				Attempt: task.HealsAttempted,
				Max:     m31types.MaxHealAttempts,
			})
			healResult := e.healTask(ctx, *task, failureReason, goal)
			e.emit(SelfHealCompleteMsg{
				TaskID:  task.ID,
				Attempt: task.HealsAttempted,
				Max:     m31types.MaxHealAttempts,
				Success: healResult.Success,
				Error:   healResult.Error,
			})
			if !healResult.Success {
				return healResult
			}
			continue
		}

		// Commit changes scoped to task files only
		var commitHash string
		if len(task.Files) > 0 && e.git != nil {
			hash, err := e.git.CommitWithFiles(
				fmt.Sprintf("%s: %s", e.gitConfig().CommitPrefix, task.Description),
				task.Files...,
			)
			if err != nil {
				e.logger.Warn("commit failed", "task", task.ID, "error", err)
			} else {
				commitHash = hash
			}
		}

		// Guard: fail file-changing tasks that produced no tool calls
		if toolCallCount == 0 && len(task.Files) > 0 {
			return taskrunner.TaskResult{
				Success:    false,
				Error:      "no tool calls produced for file-changing task",
				DurationMs: time.Since(start).Milliseconds(),
				ToolCalls:  toolCallCount,
			}
		}

		return taskrunner.TaskResult{
			Success:    true,
			Output:     content,
			CommitHash: commitHash,
			DurationMs: time.Since(start).Milliseconds(),
			ToolCalls:  toolCallCount,
		}
	}

	return taskrunner.TaskResult{
		Success: false,
		Error:   fmt.Sprintf("max heal attempts exceeded (%d)", m31types.MaxHealAttempts),
	}
}

// buildExecuteContext creates messages for task execution.
func (e *Engine) buildExecuteContext(task m31types.Task, tasks []m31types.Task, goal string) []m31types.Message {
	var messages []m31types.Message
	systemPrompt := e.buildSystemPrompt(e.prompts.ToolUse, e.prompts.ExecuteTask)
	if goal != "" {
		systemPrompt += "\n\n## Original Goal\n" + goal
	}
	messages = append(messages, m31types.Message{Role: "system", Content: systemPrompt})

	// Load PROJECT.md for project context
	project, _ := e.sessionMgr.LoadProject(e.sessionID)
	projectCtx := ""
	if project != nil {
		projectCtx = fmt.Sprintf("## Project Context\nGoal: %s\nType: %s\nFramework: %s\n\n",
			project.Goal, project.ProjectType, project.Framework)
	}

	// Load plan narrative for implementation context
	planCtx := ""
	planMarkdown, _ := e.sessionMgr.LoadPlan(e.sessionID)
	if planMarkdown == "" {
		planMarkdown = e.planMarkdown
	}
	if planMarkdown != "" {
		plan, _ := ParsePlan(planMarkdown)
		if plan != nil {
			planCtx = "## Implementation Plan Context\n"
			if plan.Summary != "" {
				planCtx += plan.Summary + "\n\n"
			}
			for _, group := range plan.ProposedChanges {
				planCtx += fmt.Sprintf("### %s\n", group.Category)
				for _, change := range group.Changes {
					planCtx += fmt.Sprintf("- [%s] %s: %s\n", change.Action, change.File, change.Description)
				}
				planCtx += "\n"
			}
		}
	}

	// Task list
	taskSummary := formatTaskSummary(tasks)
	messages = append(messages, m31types.Message{
		Role:    "user",
		Content: projectCtx + planCtx + "Task list:\n" + taskSummary,
	})

	// Current task spec
	taskSpec := fmt.Sprintf("## Current Task\nID: %d\nAction: %s\nDescription: %s\nFiles to create/modify: %v\nDepends on task IDs: %v",
		task.ID, task.Action, task.Description, task.Files, task.Dependencies)
	if len(task.AcceptanceCriteria) > 0 {
		taskSpec += "\nAcceptance criteria: " + strings.Join(task.AcceptanceCriteria, "; ")
	}
	messages = append(messages, m31types.Message{
		Role:    "user",
		Content: taskSpec,
	})

	return messages
}

// healTask attempts to fix a failed task via LLM.
func (e *Engine) healTask(ctx context.Context, task m31types.Task, failure string, goal string) taskrunner.TaskResult {
	start := time.Now()

	healPrompt := e.buildSystemPrompt(e.prompts.SelfHeal)
	if goal != "" {
		healPrompt += "\n\n## Original Goal\n" + goal
	}

	messages := []m31types.Message{
		{Role: "system", Content: healPrompt},
		{Role: "user", Content: fmt.Sprintf(
			"## Self-Heal Request\n\nTask %d failed with the following error:\n\n%s\n\n## Current File State\n\n%s\n\n"+
				"Diagnose the root cause using the diagnostic steps in your instructions, then apply a fix using your available tools."+
				" Prefer Edit for targeted changes; use FileWrite only when rewriting a file entirely.",
			task.ID, failure, e.readTaskFiles(task.Files))},
	}

	content, toolCalls, err := e.streamLLMWithTools(ctx, messages)
	if err != nil {
		return taskrunner.TaskResult{Success: false, Error: err.Error(), DurationMs: time.Since(start).Milliseconds()}
	}

	// Fall back to text-based parsing if no native tool calls
	if len(toolCalls) == 0 {
		toolCalls, err = e.parseToolCalls(content)
		if err != nil {
			return taskrunner.TaskResult{
				Success:    false,
				Error:      fmt.Sprintf("heal: tool call parsing failed: %v", err),
				DurationMs: time.Since(start).Milliseconds(),
			}
		}
	}
	if len(toolCalls) == 0 {
		return taskrunner.TaskResult{
			Success:    false,
			Error:      "heal: no tool calls generated",
			DurationMs: time.Since(start).Milliseconds(),
		}
	}
	for _, tc := range toolCalls {
		_, err := e.dispatcher.Execute(ctx, tc)
		if err != nil {
			return taskrunner.TaskResult{
				Success:    false,
				Error:      fmt.Sprintf("heal tool %s: %v", tc.Name, err),
				DurationMs: time.Since(start).Milliseconds(),
			}
		}
	}

	// Commit fix scoped to task files
	var commitHash string
	if len(task.Files) > 0 && e.git != nil {
		hash, err := e.git.CommitWithFiles(
			fmt.Sprintf("%s: %s", e.gitConfig().FixPrefix, task.Description),
			task.Files...,
		)
		if err != nil {
			e.logger.Warn("heal commit failed", "task", task.ID, "error", err)
		} else {
			commitHash = hash
		}
	}

	// Verify the fix was actually applied by re-checking task files
	for _, f := range task.Files {
		path := filepath.Join(e.workDir, f)
		if _, err := os.Stat(path); os.IsNotExist(err) {
			e.logger.Warn("heal did not create expected file", "task", task.ID, "file", f)
		}
	}

	return taskrunner.TaskResult{
		Success:    true,
		Output:     content,
		CommitHash: commitHash,
		DurationMs: time.Since(start).Milliseconds(),
	}
}
