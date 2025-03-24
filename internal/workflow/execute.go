package workflow

import (
	"context"
	"fmt"
	"strings"
	"time"

	"github.com/eshanized/M31A/pkg/session"
	"github.com/eshanized/M31A/pkg/taskrunner"
	m31types "github.com/eshanized/M31A/internal/types"
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

	// 3. Schedule
	groups, err := runner.Schedule()
	if err != nil {
		return nil, fmt.Errorf("schedule: %w", err)
	}

	// 4. Execute each group sequentially
	for _, group := range groups {
		execFn := func(ctx context.Context, task m31types.Task) taskrunner.TaskResult {
			return e.executeTaskWithTools(ctx, task, tasks)
		}

		if err := runner.ExecuteGroup(group, execFn); err != nil {
			e.logger.Error("execute group failed", "error", err)
		}

		// Update TASKS.md and STATE.md after each group
		updatedTasks := runner.Tasks()
		e.sessionMgr.SaveTasks(e.sessionID, updatedTasks)
		e.sessionMgr.SaveState(e.sessionID, m31types.PhaseExecute,
			fmt.Sprintf("executing tasks"), "group complete")
	}

	// 5. Save checkpoint
	e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseExecute,
		Timestamp: time.Now(),
	})

	// 6. Final state
	updatedTasks := runner.Tasks()
	e.sessionMgr.SaveTasks(e.sessionID, updatedTasks)

	total, done, failed, skipped := runner.Summary()
	allDone := done+skipped == total

	e.logger.Info("execute phase complete", "total", total, "done", done, "failed", failed, "skipped", skipped)

	return &PhaseResult{
		Phase:   m31types.PhaseExecute,
		Success: allDone,
		Tasks:   updatedTasks,
	}, nil
}

// executeTaskWithTools runs a single task with tool dispatch and self-heal.
func (e *Engine) executeTaskWithTools(ctx context.Context, task m31types.Task, allTasks []m31types.Task) taskrunner.TaskResult {
	start := time.Now()

	// Build context
	messages := e.buildExecuteContext(task, allTasks)

	// Stream LLM
	content, err := e.streamLLM(ctx, messages, true)
	if err != nil {
		return taskrunner.TaskResult{Success: false, Error: err.Error(), DurationMs: time.Since(start).Milliseconds()}
	}

	// Parse tool calls
	toolCalls := parseToolCalls(content)

	// Dispatch tool calls
	for _, tc := range toolCalls {
		result, err := e.dispatcher.Execute(ctx, tc)
		if err != nil {
			return taskrunner.TaskResult{
				Success:    false,
				Error:      fmt.Sprintf("tool %s: %v", tc.Name, err),
				DurationMs: time.Since(start).Milliseconds(),
			}
		}

		// Feed tool result back
		messages = append(messages, m31types.Message{
			Role:    "assistant",
			Content: content,
			ToolCalls: []m31types.ToolCall{tc},
		})
		messages = append(messages, m31types.Message{
			Role:    "tool",
			Content: result.Output,
		})
	}

	// Commit changes
	var commitHash string
	if len(task.Files) > 0 {
		hash, err := e.git.CommitWithFiles(
			fmt.Sprintf("feat(task %d): %s", task.ID, task.Description),
			task.Files...,
		)
		if err != nil {
			e.logger.Warn("commit failed", "task", task.ID, "error", err)
		} else {
			commitHash = hash
		}
	}

	return taskrunner.TaskResult{
		Success:    true,
		Output:     content,
		CommitHash: commitHash,
		DurationMs: time.Since(start).Milliseconds(),
	}
}

// buildExecuteContext creates messages for task execution.
func (e *Engine) buildExecuteContext(task m31types.Task, tasks []m31types.Task) []m31types.Message {
	var messages []m31types.Message
	messages = append(messages, m31types.Message{Role: "system", Content: systemPrompt})

	// Task summary
	taskSummary := formatTaskSummary(tasks)
	messages = append(messages, m31types.Message{
		Role:    "user",
		Content: "Task list:\n" + taskSummary,
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
		{Role: "system", Content: systemPrompt},
		{Role: "user", Content: fmt.Sprintf("Task %d failed: %s\n\nCurrent file state:\n%s\n\nFix the issue and use tools to apply the fix.",
			task.ID, failure, e.readTaskFiles(task.Files))},
	}

	content, err := e.streamLLM(ctx, messages, true)
	if err != nil {
		return taskrunner.TaskResult{Success: false, Error: err.Error(), DurationMs: time.Since(start).Milliseconds()}
	}

	// Dispatch tool calls for fix
	toolCalls := parseToolCalls(content)
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
		hash, _ := e.git.CommitWithFiles(
			fmt.Sprintf("fix(task %d): %s", task.ID, task.Description),
			task.Files...,
		)
		commitHash = hash
	}

	return taskrunner.TaskResult{
		Success:    true,
		Output:     content,
		CommitHash: commitHash,
		DurationMs: time.Since(start).Milliseconds(),
	}
}
