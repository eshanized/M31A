package workflow

import (
	"context"
	"crypto/md5"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
	"github.com/eshanized/M31A/pkg/taskrunner"
)

// runExecute executes tasks in dependency order with tool dispatch and self-heal.
func (e *Engine) runExecute(ctx context.Context, goal string) (*PhaseResult, error) {
	e.logger.Info("execute phase starting", "goal", goal, "mode", e.workflowMode)

	// 1. Load tasks
	tasks, err := e.sessionMgr.LoadTasks(e.sessionID)
	if err != nil {
		return nil, fmt.Errorf("load tasks: %w", err)
	}

	// In Fast/Direct mode, auto-generate a single task when none exist
	// (Plan phase was skipped, so we create one on the fly).
	if len(tasks) == 0 && e.workflowMode != "" && e.workflowMode != m31types.ModeFull {
		tasks = []m31types.Task{
			{
				ID:                 1,
				Description:        goal,
				Action:             "implement",
				Dependencies:       []int{},
				Files:              []string{},
				AcceptanceCriteria: []string{goal},
				Status:             m31types.StatusPending,
			},
		}
		e.logger.Info("auto-generated task for fast/direct mode", "task_id", 1, "goal", goal)
		if saveErr := e.sessionMgr.SaveTasks(e.sessionID, tasks); saveErr != nil {
			e.logger.Warn("failed to save auto-generated task", "error", saveErr)
		}
	}

	if len(tasks) == 0 {
		return &PhaseResult{
			Phase:   m31types.PhaseExecute,
			Success: true,
		}, nil
	}

	// ── Pre-execution validation ──────────────────────────────────────────
	preflightEnabled := e.cfg != nil && e.cfg.Features.ExecutePreflight
	if preflightEnabled {
		preflight := e.runExecutePreflight(tasks)
		e.emit(ExecutePreflightMsg(preflight))
		if !preflight.Passed {
			e.logger.Warn("execute preflight found issues", "count", len(preflight.Issues))
			for _, issue := range preflight.Issues {
				e.logger.Warn("preflight issue", "issue", issue)
			}
		}
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
	totalToolCalls := 0
	for _, group := range groups {
		execFn := func(ctx context.Context, task m31types.Task) taskrunner.TaskResult {
			// H16 fix: removed pre-task checkpoint — too expensive (10+ read+parse+write
			// cycles per plan). Checkpoints now only at phase boundaries and on heal.

			result := e.executeTaskWithTools(ctx, &task, tasks, goal)
			totalToolCalls += result.ToolCalls
			// Propagate HealsAttempted mutations back to the tasks slice
			for i := range tasks {
				if tasks[i].ID == task.ID {
					tasks[i].HealsAttempted = task.HealsAttempted
					break
				}
			}
			return result
		}

		if err := runner.ExecuteGroup(ctx, group, execFn); err != nil {
			execErrors = append(execErrors, fmt.Sprintf("group %v: %v", group, err))
			e.logger.Error("execute group failed", "error", err)
		}

		// Invalidate code intel so next group gets a fresh index reflecting
		// files written by this group.
		e.codeIntelMu.Lock()
		e.codeIntel = nil
		e.codeIntelBuilt = false
		e.codeIntelMu.Unlock()

		// M38 fix: only save state after group, not tasks (tasks saved once at end)
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
		ToolCalls: totalToolCalls,
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

	// Loop detection tracker for this task
	loopEnabled := e.cfg != nil && e.cfg.Features.ExecuteLoopDetect
	var tracker *ToolCallTracker
	if loopEnabled {
		tracker = NewToolCallTracker(3)
	}

	for task.HealsAttempted < m31types.MaxHealAttempts {
		// Build context
		messages := e.buildExecuteContext(ctx, *task, allTasks, goal)

		// Stream LLM with native tool calling
		content, toolCalls, err := e.streamLLMWithTools(ctx, messages)
		if err != nil {
			// Context overflow must NOT enter self-heal — healTask() adds MORE context
			// (git diff, file state, failure description) which guarantees repeat failure.
			if errors.Is(err, m31errors.ErrContextExceeded) {
				return taskrunner.TaskResult{Success: false, Error: fmt.Sprintf("context window exceeded: %v", err)}
			}
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
			if !e.healCreatedExpectedFiles(task) {
				e.logger.Warn("heal succeeded but expected files missing, treating as failure", "task", task.ID)
				return taskrunner.TaskResult{
					Success: false,
					Error:   fmt.Sprintf("heal succeeded but expected files not created for task %d", task.ID),
				}
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
				if !e.healCreatedExpectedFiles(task) {
					e.logger.Warn("heal succeeded but expected files missing, retrying", "task", task.ID)
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

		// Execute tool calls in parallel with bounded concurrency.
		// The dispatcher has its own rate limiter, and goroutines are bounded
		// by the semaphore to prevent resource exhaustion.
		const maxToolConcurrency = 4
		toolExecResults = make([]struct {
			call     m31types.ToolCall
			result   m31types.ToolResult
			err      error
			duration int64
		}, len(toolCalls))

		var wg sync.WaitGroup
		sem := make(chan struct{}, maxToolConcurrency)
		for i, tc := range toolCalls {
			wg.Add(1)
			go func(idx int, call m31types.ToolCall) {
				defer wg.Done()
				defer func() {
					if r := recover(); r != nil {
						panicErr := fmt.Errorf("tool %s panicked: %v", call.Name, r)
						toolExecResults[idx] = struct {
							call     m31types.ToolCall
							result   m31types.ToolResult
							err      error
							duration int64
						}{call: call, err: panicErr}
						e.emit(ToolCompleteMsg{
							ToolName: call.Name,
							Success:  false,
							Error:    panicErr.Error(),
						})
					}
				}()
				sem <- struct{}{}        // acquire
				defer func() { <-sem }() // release

				// Loop detection: check if this tool call repeats the same pattern
				if tracker != nil {
					if tracker.Record(call.Name, string(call.Input)) {
						e.emit(ExecuteLoopDetectMsg{
							TaskID:   task.ID,
							ToolName: call.Name,
							Count:    tracker.Count(),
						})
						e.logger.Warn("tool call loop detected",
							"task_id", task.ID, "tool", call.Name, "count", tracker.Count())
					}
				}

				e.emit(ToolStartMsg{
					ToolName:    call.Name,
					Description: fmt.Sprintf("Executing %s", call.Name),
				})

				toolStart := time.Now()
				result, err := e.dispatcher.Execute(ctx, call)
				toolDuration := time.Since(toolStart).Milliseconds()

				toolExecResults[idx] = struct {
					call     m31types.ToolCall
					result   m31types.ToolResult
					err      error
					duration int64
				}{
					call:     call,
					result:   result,
					err:      err,
					duration: toolDuration,
				}

				// Extract the file path from the tool call input for file-writing tools
				// so the TUI can track recently changed files in the sidebar.
				var affectedPath string
				switch call.Name {
				case "FileWrite", "Edit", "FileDelete", "FileMove":
					var params struct {
						Path string `json:"path"`
					}
					if unmarshalErr := json.Unmarshal(call.Input, &params); unmarshalErr == nil {
						affectedPath = params.Path
					}
				}

				if err != nil {
					e.emit(ToolCompleteMsg{
						ToolName:   call.Name,
						Success:    false,
						DurationMs: toolDuration,
						Error:      err.Error(),
						FilePath:   affectedPath,
					})
				} else {
					e.emit(ToolCompleteMsg{
						ToolName:   call.Name,
						Success:    true,
						DurationMs: toolDuration,
						FilePath:   affectedPath,
					})
				}
			}(i, tc)
		}
		wg.Wait()

		// Feed ALL tool results back to the LLM, including errors
		var toolErrMessages []string
		for _, tr := range toolExecResults {
			if tr.err != nil {
				toolErrMessages = append(toolErrMessages, fmt.Sprintf("Tool: %s\nInput: %s\nError: %v",
					tr.call.Name, summarizeInput(tr.call.Input, 200), tr.err))
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
			if !e.healCreatedExpectedFiles(task) {
				e.logger.Warn("heal succeeded but expected files missing, treating as failure", "task", task.ID)
				return taskrunner.TaskResult{
					Success: false,
					Error:   fmt.Sprintf("heal succeeded but expected files not created for task %d", task.ID),
				}
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

		// ── Per-task quality gate ───────────────────────────────────────────
		qualityEnabled := e.cfg != nil && e.cfg.Features.ExecuteQualityGate
		if qualityEnabled && len(task.AcceptanceCriteria) > 0 {
			qgResult := e.checkAcceptanceCriteria(*task)
			e.emit(ExecuteQualityGateMsg{
				TaskID:  task.ID,
				Passed:  qgResult.Passed,
				Checked: qgResult.Checked,
				Failed:  qgResult.Failed,
			})
			if !qgResult.Passed {
				e.logger.Warn("quality gate failed for task",
					"task_id", task.ID, "failed", qgResult.Failed, "checked", qgResult.Checked)
				for _, detail := range qgResult.Details {
					if strings.HasPrefix(detail, "FAIL:") {
						e.logger.Warn("quality gate detail", "detail", detail)
					}
				}
				// Feed quality gate failures back as a heal trigger
				if task.HealsAttempted < m31types.MaxHealAttempts {
					failureReason := fmt.Sprintf("Quality gate failed: %s",
						strings.Join(qgResult.Details, "; "))
					task.HealsAttempted++
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
			}
		}

		// Guard: fail file-changing tasks that produced no tool calls and no commit
		if toolCallCount == 0 && len(task.Files) > 0 && commitHash == "" {
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
func (e *Engine) buildExecuteContext(ctx context.Context, task m31types.Task, tasks []m31types.Task, goal string) []m31types.Message {
	var messages []m31types.Message
	systemPrompt := e.buildSystemPrompt(e.prompts.ToolUse, e.prompts.ExecuteTask, e.prompts.ContextAwareness, e.prompts.CodeQuality, e.prompts.CodeIntelligence)
	if goal != "" {
		systemPrompt += "\n\n## Original Goal\n" + goal
	}
	messages = append(messages, m31types.Message{Role: "system", Content: systemPrompt})

	// Load PROJECT.md for project context (H15 fix: cache on first load per session)
	var project *m31types.ProjectState
	if e.cachedProjectID == e.sessionID {
		project = e.cachedProject
	} else {
		var projErr error
		project, projErr = e.sessionMgr.LoadProject(e.sessionID)
		if projErr != nil {
			e.logger.Warn("execute context: failed to load project", "error", projErr)
		}
		e.cachedProject = project
		e.cachedProjectID = e.sessionID
	}
	projectCtx := ""
	if project != nil {
		projectCtx = fmt.Sprintf("## Project Context\nGoal: %s\nType: %s\nFramework: %s\n\n",
			project.Goal, project.ProjectType, project.Framework)
	}

	// Load plan narrative for implementation context (H15 fix: cache parsed plan)
	planCtx := ""
	planMarkdown, _ := e.sessionMgr.LoadPlan(e.sessionID)
	if planMarkdown == "" {
		planMarkdown = e.planMarkdown
	}
	if planMarkdown != "" {
		var plan *m31types.Plan
		planHash := fmt.Sprintf("%x", md5.Sum([]byte(planMarkdown)))
		if e.cachedPlan != nil && e.cachedPlanMD5 == planHash {
			plan = e.cachedPlan
		} else {
			var parseErr error
			plan, parseErr = ParsePlan(planMarkdown)
			if parseErr != nil {
				e.logger.Warn("failed to parse plan for execute context", "error", parseErr)
			}
			e.cachedPlan = plan
			e.cachedPlanMD5 = planHash
		}
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

	// Codebase intelligence — inject relevant file context automatically
	if ci := e.getCodeIntel(ctx); ci != nil {
		ciCtx := ci.FormatContext(task.Files, task.Description, 10, 4000)
		if ciCtx != "" {
			messages = append(messages, m31types.Message{
				Role:    "user",
				Content: ciCtx,
			})
		}
	}

	// Current file state — re-read from disk so the LLM sees post-heal content
	// on every iteration of the heal loop. Capped to avoid ballooning context.
	if fileCtx := e.readTaskFiles(task.Files); fileCtx != "" {
		const maxFileCtx = 32000
		if len(fileCtx) > maxFileCtx {
			fileCtx = fileCtx[:maxFileCtx] + "\n... (truncated)"
		}
		messages = append(messages, m31types.Message{
			Role:    "user",
			Content: "## Current File State (fresh from disk)\n" + fileCtx,
		})
	}

	return messages
}

// healTask attempts to fix a failed task via LLM.
func (e *Engine) healTask(ctx context.Context, task m31types.Task, failure string, goal string) taskrunner.TaskResult {
	start := time.Now()

	healPrompt := e.buildSystemPrompt(e.prompts.ToolUse, e.prompts.SelfHeal, e.prompts.CodeQuality)
	if goal != "" {
		healPrompt += "\n\n## Original Goal\n" + goal
	}

	// Build enhanced heal context with acceptance criteria and diagnostic info
	healCtx := fmt.Sprintf(
		"## Self-Heal Request\n\nTask %d failed with the following error:\n\n%s\n\n"+
			"## Task Specification\n"+
			"- Action: %s\n"+
			"- Description: %s\n"+
			"- Files: %v\n"+
			"- Acceptance Criteria: %s\n"+
			"- Heal Attempts: %d/%d\n\n"+
			"## Current File State\n\n%s\n",
		task.ID, failure,
		task.Action, task.Description, task.Files,
		strings.Join(task.AcceptanceCriteria, "; "),
		task.HealsAttempted, m31types.MaxHealAttempts,
		e.readTaskFiles(task.Files),
	)

	// Add git diff of recent changes if available
	if e.git != nil {
		if diff, diffErr := e.git.Run("diff", "--stat", "HEAD~3..HEAD"); diffErr == nil && strings.TrimSpace(diff) != "" {
			healCtx += "\n## Recent Changes (last 3 commits)\n\n```\n" + diff + "\n```\n"
		}
	}

	// Add codebase intelligence context for the task files
	if ci := e.getCodeIntel(ctx); ci != nil {
		ciCtx := ci.FormatContext(task.Files, task.Description, 5, 2000)
		if ciCtx != "" {
			healCtx += "\n" + ciCtx + "\n"
		}
	}

	healCtx += "\nDiagnose the root cause using the diagnostic steps in your instructions, then apply a fix using your available tools." +
		" Prefer Edit for targeted changes; use FileWrite only when rewriting a file entirely."

	messages := []m31types.Message{
		{Role: "system", Content: healPrompt},
		{Role: "user", Content: healCtx},
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

func summarizeInput(input json.RawMessage, maxChars int) string {
	s := string(input)
	if len(s) <= maxChars {
		return s
	}
	return s[:maxChars] + "...(truncated)"
}

func (e *Engine) healCreatedExpectedFiles(task *m31types.Task) bool {
	if len(task.Files) == 0 {
		return true
	}
	for _, f := range task.Files {
		if _, err := os.Stat(filepath.Join(e.workDir, f)); os.IsNotExist(err) {
			return false
		}
	}
	return true
}
