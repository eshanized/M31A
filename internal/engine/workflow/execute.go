package workflow

import (
	"context"
	"crypto/md5"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	m31types "github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/engine/decision"
	"github.com/eshanized/M31A/internal/engine/session"
	"github.com/eshanized/M31A/internal/engine/taskrunner"
)

// runExecute executes tasks in dependency order with tool dispatch and self-heal.
func (e *Engine) runExecute(ctx context.Context, goal string) (*PhaseResult, error) {
	e.logger.Info("execute phase starting", "goal", goal, "mode", e.WorkflowMode())

	// 1. Load tasks
	tasks, err := e.sessionMgr.LoadTasks(e.sessionID)
	if err != nil {
		return nil, fmt.Errorf("load tasks: %w", err)
	}

	// In Fast/Direct mode, auto-generate a single task when none exist
	// (Plan phase was skipped, so we create one on the fly).
	mode := e.WorkflowMode()
	if len(tasks) == 0 && mode != "" && mode != m31types.ModeFull {
		tasks = []m31types.Task{
			{
				ID:           1,
				Description:  goal,
				Action:       "implement",
				Dependencies: []int{},
				Files:        []string{"main.go"},
				AcceptanceCriteria: []string{
					"code compiles with go build",
					"all required files are created using FileWrite tool",
				},
				Status: m31types.StatusPending,
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
	if e.cfg != nil && e.cfg.Features.MaxParallelTasks > 0 {
		runner.MaxParallel = e.cfg.Features.MaxParallelTasks
	}

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
	var totalToolCalls atomic.Int64
	for _, group := range groups {
		// Check for pause before executing each group
		e.waitForPause(ctx)

		var groupToolCalls atomic.Int64
		execFn := func(ctx context.Context, task m31types.Task) taskrunner.TaskResult {
			// H16 fix: removed pre-task checkpoint — too expensive (10+ read+parse+write
			// cycles per plan). Checkpoints now only at phase boundaries and on heal.

			// Check for pause/skip/cancel while executing
			e.pauseMu.Lock()
			paused := e.pauseCh != nil
			e.pauseMu.Unlock()
			if paused {
				// Wait for resume or skip/cancel commands
				skipID, cancelID, cancelledGroup, ok := e.consumeSkipOrCancel(ctx)
				if !ok {
					// Context cancelled
					return taskrunner.TaskResult{Error: "execution cancelled"}
				}
				if cancelledGroup {
					// Mark remaining tasks in group as skipped
					return taskrunner.TaskResult{Error: "group cancelled"}
				}
				if skipID == task.ID {
					// Mark task as skipped
					task.Status = m31types.StatusSkipped
					return taskrunner.TaskResult{Success: true, Output: "skipped by user"}
				}
				if cancelID == task.ID {
					// Mark task as failed
					task.Status = m31types.StatusFailed
					return taskrunner.TaskResult{Error: "cancelled by user"}
				}
			}

			result := e.executeTaskWithTools(ctx, &task, tasks, goal)
			totalToolCalls.Add(int64(result.ToolCalls))
			groupToolCalls.Add(int64(result.ToolCalls))
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

		// Invalidate code intel only when tool calls were made (files may have changed)
		if groupToolCalls.Load() > 0 {
			e.codeIntelMu.Lock()
			e.codeIntel = nil
			e.codeIntelBuilt = false
			e.codeIntelMu.Unlock()
		}

		// M38 fix: only save state after group, not tasks (tasks saved once at end)
		if err := e.sessionMgr.SaveState(e.sessionID, m31types.PhaseExecute,
			"executing tasks", "group complete"); err != nil {
			e.logger.Warn("save state failed", "error", err)
		}

		// Revoke batch approvals after each task group completes
		// to prevent stale approvals from carrying across groups
		if e.dispatcher != nil {
			e.dispatcher.RevokeBatchApprovals()
		}

		// Auto-sync TODO.md from task runner state after each group
		if syncErr := e.dispatcher.SyncTodoFromTasks(runner.Tasks()); syncErr != nil {
			e.logger.Warn("todo sync after group failed", "error", syncErr)
		}
	}

	// 5. Save checkpoint — critical for crash recovery; retry on failure
	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseExecute,
		Timestamp: time.Now(),
	}); err != nil {
		e.logger.Error("save checkpoint failed (session may not resume after crash)", "error", err)
		// Retry once before giving up
		if retryErr := e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
			Phase:     m31types.PhaseExecute,
			Timestamp: time.Now(),
		}); retryErr != nil {
			return nil, fmt.Errorf("save checkpoint (critical, retry failed): %w", retryErr)
		}
	}

	// 6. Final state
	updatedTasks := runner.Tasks()
	if err := e.sessionMgr.SaveTasks(e.sessionID, updatedTasks); err != nil {
		e.logger.Warn("save tasks failed", "error", err)
	}

	// Final TODO sync from completed task state
	if syncErr := e.dispatcher.SyncTodoFromTasks(updatedTasks); syncErr != nil {
		e.logger.Warn("final todo sync failed", "error", syncErr)
	}

	total, done, failed, skipped := runner.Summary()
	allDone := done+skipped == total

	e.logger.Info("execute phase complete", "total", total, "done", done, "failed", failed, "skipped", skipped)

	result := &PhaseResult{
		Phase:     m31types.PhaseExecute,
		Success:   allDone,
		Tasks:     updatedTasks,
		ToolCalls: int(totalToolCalls.Load()),
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
		loopWindow := 3
		if e.cfg != nil && e.cfg.Tools.LoopDetectWindow > 0 {
			loopWindow = e.cfg.Tools.LoopDetectWindow
		}
		tracker = NewToolCallTracker(loopWindow)
	}

	// Track whether we need to re-check quality gate after a heal
	qualityGatePending := false

	maxHeals := m31types.MaxHealAttempts
	if e.cfg != nil && e.cfg.Features.MaxHealAttempts > 0 {
		maxHeals = e.cfg.Features.MaxHealAttempts
	}
	var messages []m31types.Message
	for task.HealsAttempted < maxHeals {
		// Re-check quality gate after a successful heal before calling LLM again
		if qualityGatePending {
			qualityGatePending = false
			qualityEnabled := e.cfg != nil && e.cfg.Features.ExecuteQualityGate
			if qualityEnabled && len(task.AcceptanceCriteria) > 0 {
				qgResult := e.checkAcceptanceCriteria(*task)
				e.emit(ExecuteQualityGateMsg{
					TaskID:  task.ID,
					Passed:  qgResult.Passed,
					Checked: qgResult.Checked,
					Failed:  qgResult.Failed,
				})
				if qgResult.Passed {
					// Quality gate now passes — commit and return
					if len(task.Files) > 0 && e.git != nil {
						_, err := e.git.CommitWithFiles(
							fmt.Sprintf("%s: %s", e.gitConfig().CommitPrefix, task.Description),
							task.Files...,
						)
						if err != nil {
							e.logger.Warn("commit failed", "task", task.ID, "error", err)
						}
					}
					return taskrunner.TaskResult{
						Success:    true,
						Output:     "quality gate passed after heal",
						DurationMs: time.Since(start).Milliseconds(),
					}
				}
				// Still failing — will re-enter LLM loop for another heal attempt
			}
		}

		// Build context
		messages = e.buildExecuteContext(ctx, *task, allTasks, goal)

		// Wave 2B: proactive compaction during Execute phase — run on every iteration
		// so compacted messages are used for the immediate LLM call.
		e.toolCallsSinceLastCompact = 0
		if e.cfg != nil && e.cfg.Compaction.Proactive {
			messages = e.proactiveCompactCheck(messages)
		}

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
				if e.collector != nil {
					e.collector.RecordHealLoop(m31types.PhaseExecute)
				}
				return taskrunner.TaskResult{Success: false, Error: failureReason}
			}
			task.HealsAttempted++
			e.logger.Info("self-healing task after LLM failure", "task", task.ID, "attempt", task.HealsAttempted)
			e.emit(SelfHealStartMsg{
				TaskID:  task.ID,
				Attempt: task.HealsAttempted,
				Max:     m31types.MaxHealAttempts,
			})
			if e.collector != nil {
				e.collector.RecordHealTrigger(m31types.PhaseExecute)
			}
			healStart := time.Now()
			healResult := e.healTask(ctx, *task, failureReason, goal)
			healDuration := time.Since(healStart).Milliseconds()
			e.emit(SelfHealCompleteMsg{
				TaskID:  task.ID,
				Attempt: task.HealsAttempted,
				Max:     m31types.MaxHealAttempts,
				Success: healResult.Success,
				Error:   healResult.Error,
			})
			if e.collector != nil {
				e.collector.RecordHealOutcome(m31types.PhaseExecute, healResult.Success)
				e.collector.RecordHealDuration(m31types.PhaseExecute, healDuration)
			}
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
			// HARD ENFORCEMENT: Detect code-as-text output from weak models.
			// If the LLM printed code instead of using FileWrite, force a retry
			// with an explicit tool-use prompt.
			if looksLikeCode(content) && task.HealsAttempted < m31types.MaxHealAttempts {
				e.logger.Warn("detected code-as-text output, forcing tool-use retry",
					"task", task.ID, "content_len", len(content))
				task.HealsAttempted++
				e.emit(SelfHealStartMsg{
					TaskID:  task.ID,
					Attempt: task.HealsAttempted,
					Max:     m31types.MaxHealAttempts,
				})
				if e.collector != nil {
					e.collector.RecordHealTrigger(m31types.PhaseExecute)
				}
				// Inject the code-as-text as context and demand FileWrite usage.
				// Clone messages first to avoid aliasing the original slice's backing array.
				forcedMsg := make([]m31types.Message, len(messages), len(messages)+1)
				copy(forcedMsg, messages)
				forcedMsg = append(forcedMsg, m31types.Message{
					Role:    "user",
					Content: "CRITICAL: You output code as plain text. This is WRONG. You MUST use the FileWrite tool to create files. Here is the code you wrote — now create each file using FileWrite tool calls. Do NOT output code as text again. Create the files using FileWrite.",
				})
				forcedContent, forcedToolCalls, forcedErr := e.streamLLMWithTools(ctx, forcedMsg)
				if forcedErr == nil && len(forcedToolCalls) > 0 {
					content = forcedContent
					toolCalls = forcedToolCalls
					e.logger.Info("code-as-text retry produced tool calls", "task", task.ID, "count", len(forcedToolCalls))
				} else {
					e.logger.Warn("code-as-text retry still failed", "task", task.ID, "err", forcedErr, "toolCalls", len(forcedToolCalls))
				}
				e.emit(SelfHealCompleteMsg{
					TaskID:  task.ID,
					Attempt: task.HealsAttempted,
					Max:     m31types.MaxHealAttempts,
					Success: len(toolCalls) > 0,
				})
				if e.collector != nil {
					e.collector.RecordHealOutcome(m31types.PhaseExecute, len(toolCalls) > 0)
				}
			}

			if len(toolCalls) == 0 {
				parsedCalls, parseErr := e.parseToolCalls(content)
				if parseErr != nil {
					failureReason := fmt.Sprintf("tool call parsing failed: %v", parseErr)
					if task.HealsAttempted >= m31types.MaxHealAttempts {
						if e.collector != nil {
							e.collector.RecordHealLoop(m31types.PhaseExecute)
						}
						return taskrunner.TaskResult{Success: false, Error: failureReason}
					}
					task.HealsAttempted++
					e.logger.Info("self-healing task after parse failure", "task", task.ID, "attempt", task.HealsAttempted)
					e.emit(SelfHealStartMsg{
						TaskID:  task.ID,
						Attempt: task.HealsAttempted,
						Max:     m31types.MaxHealAttempts,
					})
					if e.collector != nil {
						e.collector.RecordHealTrigger(m31types.PhaseExecute)
					}
					healStart := time.Now()
					healResult := e.healTask(ctx, *task, failureReason, goal)
					healDuration := time.Since(healStart).Milliseconds()
					e.emit(SelfHealCompleteMsg{
						TaskID:  task.ID,
						Attempt: task.HealsAttempted,
						Max:     m31types.MaxHealAttempts,
						Success: healResult.Success,
						Error:   healResult.Error,
					})
					if e.collector != nil {
						e.collector.RecordHealOutcome(m31types.PhaseExecute, healResult.Success)
						e.collector.RecordHealDuration(m31types.PhaseExecute, healDuration)
					}
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
		maxToolConcurrency := 4
		if e.cfg != nil && e.cfg.Tools.MaxToolConcurrency > 0 {
			maxToolConcurrency = e.cfg.Tools.MaxToolConcurrency
		}
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

				// Log tool execution decision
				e.LogDecision(decision.DecisionReceipt{
					Decision:  fmt.Sprintf("tool:%s", call.Name),
					Rationale: fmt.Sprintf("task:%d", task.ID),
					Category:  decision.CategoryTool,
					Cost: decision.Cost{
						Duration: float64(toolDuration) / 1000.0,
						Attempts: 1,
					},
				})

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
				if e.collector != nil {
					e.collector.RecordHealLoop(m31types.PhaseExecute)
				}
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
			if e.collector != nil {
				e.collector.RecordHealTrigger(m31types.PhaseExecute)
			}
			healStart := time.Now()
			healResult := e.healTask(ctx, *task, failureReason, goal)
			healDuration := time.Since(healStart).Milliseconds()
			e.emit(SelfHealCompleteMsg{
				TaskID:  task.ID,
				Attempt: task.HealsAttempted,
				Max:     m31types.MaxHealAttempts,
				Success: healResult.Success,
				Error:   healResult.Error,
			})
			if e.collector != nil {
				e.collector.RecordHealOutcome(m31types.PhaseExecute, healResult.Success)
				e.collector.RecordHealDuration(m31types.PhaseExecute, healDuration)
			}
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
		var beforeHash string
		if len(task.Files) > 0 && e.git != nil {
			if h, err := e.git.HeadHash(); err == nil {
				beforeHash = h
			}
			hash, err := e.git.CommitWithFiles(
				fmt.Sprintf("%s: %s", e.gitConfig().CommitPrefix, task.Description),
				task.Files...,
			)
			if err != nil {
				e.logger.Warn("commit failed", "task", task.ID, "error", err)
			} else {
				commitHash = hash
				if beforeHash != "" {
					if ds, err := CaptureDiffSummary(e.git, beforeHash, hash); err == nil {
						e.emit(TaskDiffSummaryMsg{
							TaskID:  task.ID,
							Summary: ds,
						})
					}
				}
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
					if e.collector != nil {
						e.collector.RecordHealTrigger(m31types.PhaseExecute)
					}
					healStart := time.Now()
					healResult := e.healTask(ctx, *task, failureReason, goal)
					healDuration := time.Since(healStart).Milliseconds()
					e.emit(SelfHealCompleteMsg{
						TaskID:  task.ID,
						Attempt: task.HealsAttempted,
						Max:     m31types.MaxHealAttempts,
						Success: healResult.Success,
						Error:   healResult.Error,
					})
					if e.collector != nil {
						e.collector.RecordHealOutcome(m31types.PhaseExecute, healResult.Success)
						e.collector.RecordHealDuration(m31types.PhaseExecute, healDuration)
					}
					if !healResult.Success {
						return healResult
					}
					qualityGatePending = true
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

	// All heal attempts exhausted — record heal loop for metrics.
	if e.collector != nil {
		e.collector.RecordHealLoop(m31types.PhaseExecute)
	}

	return taskrunner.TaskResult{
		Success: false,
		Error:   fmt.Sprintf("max heal attempts exceeded (%d)", m31types.MaxHealAttempts),
	}
}

// buildExecuteContext creates messages for task execution.
func (e *Engine) buildExecuteContext(ctx context.Context, task m31types.Task, tasks []m31types.Task, goal string) []m31types.Message {
	var messages []m31types.Message
	extras := []string{e.promptOrGet("tool-use"), e.promptOrGet("execute-task"), e.promptOrGet("context-awareness"), e.promptOrGet("code-quality"), e.promptOrGet("code-intelligence")}
	if e.ScopeIncludes("website") && e.promptOrGet("website-build") != "" {
		extras = append(extras, e.promptOrGet("website-build"))
	}
	systemPrompt := e.buildSystemPrompt(extras...)
	if goal != "" {
		systemPrompt += "\n\n## Original Goal\n" + goal
	}
	// Inject website template path when scope includes "website"
	if e.ScopeIncludes("website") {
		if templateDir, err := e.ExtractWebsiteTemplateTo(); err == nil {
			systemPrompt += fmt.Sprintf("\n\n## Website Template\nBundled Next.js template at: %s\nCopy files from this directory to the working directory before implementing.\n", templateDir)
		} else {
			e.logger.Warn("failed to extract website template", "error", err)
		}
	}
	messages = append(messages, m31types.Message{Role: "system", Content: systemPrompt})

	// Load PROJECT.md for project context — use shared cache
	var project *m31types.ProjectState
	e.cacheMu.RLock()
	cache := e.cache
	e.cacheMu.RUnlock()
	if cached := cache.GetProject(e.sessionID); cached != nil {
		project = cached
	} else {
		project = e.loadProjectCached()
		cache.SetProject(e.sessionID, project)
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
		planMarkdown = e.state.planMarkdown
	}
	if planMarkdown != "" {
		var plan *m31types.Plan
		planHash := fmt.Sprintf("%x", md5.Sum([]byte(planMarkdown)))
		e.cacheMu.RLock()
		cache := e.cache
		e.cacheMu.RUnlock()
		if cached := cache.GetPlan(planHash); cached != nil {
			plan = cached
		} else {
			var parseErr error
			plan, parseErr = ParsePlan(planMarkdown)
			if parseErr != nil {
				e.logger.Warn("failed to parse plan for execute context", "error", parseErr)
			}
			cache.SetPlan(planHash, plan)
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
	// CRITICAL: When no files are specified, explicitly require tool use.
	// Without this, the LLM prints code as text instead of creating files.
	if len(task.Files) == 0 {
		taskSpec += "\n\n**IMPORTANT**: You MUST use the FileWrite tool to create all necessary files. Do NOT output code as text in your response. Each file must be created via a FileWrite tool call."
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
