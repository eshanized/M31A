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
	e.logger.Info("execute phase starting")

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
			// H-15: Save checkpoint before each task for rollback on heal failure.
			if err := e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
				Phase:     m31types.PhaseExecute,
				Timestamp: time.Now(),
				TaskCount: task.ID,
			}); err != nil {
				e.logger.Warn("pre-task checkpoint failed", "task", task.ID, "error", err)
			}

			result := e.executeTaskWithTools(ctx, task, tasks)
			// Count tool calls from task result
			if result.ToolCalls > 0 {
				toolCallCount += result.ToolCalls
			}
			return result
		}

		if err := runner.ExecuteGroup(group, execFn); err != nil {
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
func (e *Engine) executeTaskWithTools(ctx context.Context, task m31types.Task, allTasks []m31types.Task) taskrunner.TaskResult {
	start := time.Now()

	for task.HealsAttempted < m31types.MaxHealAttempts {
		// Build context
		messages := e.buildExecuteContext(task, allTasks)

		// Stream LLM
		content, err := e.streamLLM(ctx, messages, true)
		if err != nil {
			failureReason := fmt.Sprintf("LLM stream failed: %v", err)
			if task.HealsAttempted >= m31types.MaxHealAttempts {
				return taskrunner.TaskResult{Success: false, Error: failureReason}
			}
			task.HealsAttempted++
			e.logger.Info("self-healing task after LLM failure", "task", task.ID, "attempt", task.HealsAttempted)
			healResult := e.healTask(ctx, task, failureReason)
			if !healResult.Success {
				return healResult
			}
			continue
		}

		// Parse tool calls
		toolCalls, _ := e.parseToolCalls(content)

		// Dispatch tool calls
		toolErr := false
		var toolErrMsg error
		var toolErrName string
		toolCallCount := len(toolCalls)
		for _, tc := range toolCalls {
			// Emit tool start message
			e.emit(ToolStartMsg{
				ToolName:    tc.Name,
				Description: fmt.Sprintf("Executing %s", tc.Name),
			})

			toolStart := time.Now()
			result, err := e.dispatcher.Execute(ctx, tc)
			toolDuration := time.Since(toolStart).Milliseconds()

			if err != nil {
				// Emit tool failure
				e.emit(ToolCompleteMsg{
					ToolName:   tc.Name,
					Success:    false,
					DurationMs: toolDuration,
					Error:      err.Error(),
				})
				toolErr = true
				toolErrMsg = err
				toolErrName = tc.Name
				break
			}

			// Emit tool success
			e.emit(ToolCompleteMsg{
				ToolName:   tc.Name,
				Success:    true,
				DurationMs: toolDuration,
			})

			// Feed tool result back
			messages = append(messages, m31types.Message{
				Role:      "assistant",
				Content:   content,
				ToolCalls: []m31types.ToolCall{tc},
			})
			messages = append(messages, m31types.Message{
				Role:    "tool",
				Content: result.Output,
			})
		}

		if toolErr {
			if task.HealsAttempted >= m31types.MaxHealAttempts {
				return taskrunner.TaskResult{
					Success: false,
					Error:   fmt.Sprintf("tool %s: %v", toolErrName, toolErrMsg),
				}
			}
			failureReason := fmt.Sprintf("tool %s failed: %v", toolErrName, toolErrMsg)
			task.HealsAttempted++
			e.logger.Info("self-healing task after tool failure", "task", task.ID, "attempt", task.HealsAttempted)
			healResult := e.healTask(ctx, task, failureReason)
			if !healResult.Success {
				return healResult
			}
			continue
		}

		// Commit changes
		var commitHash string
		if len(task.Files) > 0 {
			if err := e.git.AddAll(); err != nil {
				e.logger.Warn("git add failed", "task", task.ID, "error", err)
			}
			if err := e.git.Commit(fmt.Sprintf("feat: %s", task.Description)); err != nil {
				e.logger.Warn("commit failed", "task", task.ID, "error", err)
			} else {
				commitHash, _ = e.git.HeadHash()
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
func (e *Engine) buildExecuteContext(task m31types.Task, tasks []m31types.Task) []m31types.Message {
	var messages []m31types.Message
	messages = append(messages, m31types.Message{Role: "system", Content: e.buildSystemPrompt(e.prompts.ToolUse, e.prompts.ExecuteTask)})

	// Load PROJECT.md for project context
	project, _ := e.sessionMgr.LoadProject(e.sessionID)
	projectCtx := ""
	if project != nil {
		projectCtx = fmt.Sprintf("## Project Context\nGoal: %s\nType: %s\nFramework: %s\n\n",
			project.Goal, project.ProjectType, project.Framework)
	}

	// Task list
	taskSummary := formatTaskSummary(tasks)
	messages = append(messages, m31types.Message{
		Role:    "user",
		Content: projectCtx + "Task list:\n" + taskSummary,
	})

	// Current task spec
	taskSpec := fmt.Sprintf("Execute task %d: %d\nAction: %s\nDescription: %s\nFiles: %v\nDependencies: %v",
		task.ID, task.ID, task.Action, task.Description, task.Files, task.Dependencies)
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
func (e *Engine) healTask(ctx context.Context, task m31types.Task, failure string) taskrunner.TaskResult {
	start := time.Now()

	messages := []m31types.Message{
		{Role: "system", Content: e.buildSystemPrompt(e.prompts.SelfHeal)},
		{Role: "user", Content: fmt.Sprintf("Task %d failed: %s\n\nCurrent file state:\n%s\n\nFix the issue and use tools to apply the fix.",
			task.ID, failure, e.readTaskFiles(task.Files))},
	}

	content, err := e.streamLLM(ctx, messages, true)
	if err != nil {
		return taskrunner.TaskResult{Success: false, Error: err.Error(), DurationMs: time.Since(start).Milliseconds()}
	}

	// Dispatch tool calls for fix
	toolCalls, _ := e.parseToolCalls(content)
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

	// Commit fix
	var commitHash string
	if len(task.Files) > 0 {
		if err := e.git.AddAll(); err != nil {
			e.logger.Warn("git add failed during heal", "task", task.ID, "error", err)
		}
		if err := e.git.Commit(fmt.Sprintf("fix: %s", task.Description)); err != nil {
			e.logger.Warn("heal commit failed", "task", task.ID, "error", err)
		} else {
			commitHash, _ = e.git.HeadHash()
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
