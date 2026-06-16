package workflow

import (
	"context"
	"fmt"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/bisect"
	"github.com/eshanized/M31A/pkg/session"
)

// runVerify checks task outputs for correctness and triggers self-heal/bisect on failure.
func (e *Engine) runVerify(ctx context.Context, goal string) (*PhaseResult, error) {
	ctx, cancel := e.verifyTaskContext(ctx)
	defer cancel()
	e.logger.Info("verify phase starting", "goal", goal)

	// Emit intermediate progress
	e.emit(IntermediateProgressMsg{
		Phase:   "verify",
		Message: "Running acceptance checks...",
	})

	// 1. Load tasks
	tasks, err := e.sessionMgr.LoadTasks(e.sessionID)
	if err != nil {
		return nil, fmt.Errorf("load tasks: %w", err)
	}

	if len(tasks) == 0 {
		return &PhaseResult{
			Phase:   m31types.PhaseVerify,
			Success: true,
		}, nil
	}

	// 2. Verify each completed task
	var skippedWithFiles []string
	for i, task := range tasks {
		if task.Status == m31types.StatusSkipped && len(task.Files) > 0 {
			skippedWithFiles = append(skippedWithFiles, fmt.Sprintf("task %d (%s)", task.ID, task.Description))
		}
		if task.Status != m31types.StatusDone {
			continue
		}

		result := e.verifyTask(ctx, task)

		if result.FilesExist && result.SyntaxOK && result.TestsOK {
			e.logger.Info("task verified", "id", task.ID)
			continue
		}

		// Verification failed — attempt self-heal
		e.logger.Warn("task verification failed", "id", task.ID, "errors", result.Errors)

		if task.HealsAttempted >= m31types.MaxHealAttempts {
			tasks[i].Status = m31types.StatusUnrecoverable
			e.logger.Warn("task unrecoverable — max heal attempts exceeded", "id", task.ID)
			continue
		}

		// Self-heal
		failure := fmt.Sprintf("verification failed: %v", result.Errors)
		tasks[i].HealsAttempted++
		e.emit(SelfHealStartMsg{
			TaskID:  task.ID,
			Attempt: tasks[i].HealsAttempted,
			Max:     m31types.MaxHealAttempts,
		})
		healResult := e.healTask(ctx, task, failure, goal)
		e.emit(SelfHealCompleteMsg{
			TaskID:  task.ID,
			Attempt: tasks[i].HealsAttempted,
			Max:     m31types.MaxHealAttempts,
			Success: healResult.Success,
			Error:   healResult.Error,
		})

		if healResult.Success {
			// Re-verify
			newResult := e.verifyTask(ctx, task)
			if newResult.FilesExist && newResult.SyntaxOK && newResult.TestsOK {
				tasks[i].Status = m31types.StatusDone
				e.logger.Info("task healed and verified", "id", task.ID)
			} else {
				e.logger.Warn("heal did not fix task", "id", task.ID)
				// Try bisect as fallback before giving up
				if healed := e.tryBisectHeal(ctx, &tasks[i], task, result, goal); healed {
					continue
				}
				tasks[i].Status = m31types.StatusFailed
			}
		} else {
			e.logger.Warn("self-heal failed", "id", task.ID, "error", healResult.Error)
			// Try bisect as fallback before marking unrecoverable
			if healed := e.tryBisectHeal(ctx, &tasks[i], task, result, goal); healed {
				continue
			}
			if tasks[i].HealsAttempted >= m31types.MaxHealAttempts {
				tasks[i].Status = m31types.StatusUnrecoverable
			}
		}
	}

	// 3. Save updated TASKS.md
	if err := e.sessionMgr.SaveTasks(e.sessionID, tasks); err != nil {
		e.logger.Warn("save tasks failed", "error", err)
	}

	// 4. Save checkpoint
	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseVerify,
		Timestamp: time.Now(),
	}); err != nil {
		e.logger.Warn("save checkpoint failed", "error", err)
	}

	// 5. Write STATE.md
	if err := e.sessionMgr.SaveState(e.sessionID, m31types.PhaseVerify, "verification complete", "verify done"); err != nil {
		e.logger.Warn("save state failed", "error", err)
	}

	// 6. Check if all passed or skipped
	allOK := true
	var failedTasks []string
	for _, task := range tasks {
		if task.Status == m31types.StatusFailed || task.Status == m31types.StatusUnrecoverable {
			allOK = false
			failedTasks = append(failedTasks, fmt.Sprintf("task %d: %s", task.ID, task.Status))
		}
	}

	e.logger.Info("verify phase complete", "all_ok", allOK)

	// Load manual verification steps from the plan
	var manualSteps []string
	planMarkdown, _ := e.sessionMgr.LoadPlan(e.sessionID)
	if planMarkdown != "" {
		if plan, parseErr := ParsePlan(planMarkdown); parseErr == nil && plan != nil {
			manualSteps = plan.Verification.Manual
		}
	}

	result := &PhaseResult{
		Phase:                   m31types.PhaseVerify,
		Success:                 allOK,
		Tasks:                   tasks,
		ManualVerificationSteps: manualSteps,
	}
	if !allOK && len(failedTasks) > 0 {
		errMsg := strings.Join(failedTasks, "; ")
		if len(skippedWithFiles) > 0 {
			errMsg += "; skipped with undelivered files: " + strings.Join(skippedWithFiles, "; ")
		}
		result.Error = errMsg
		return result, fmt.Errorf("%w: %s", m31errors.ErrTaskFailed, result.Error)
	}
	if len(skippedWithFiles) > 0 {
		result.Error = "skipped tasks with undelivered files: " + strings.Join(skippedWithFiles, "; ")
	}
	return result, nil
}

// findRootCommit runs git rev-list --max-parents=0 HEAD to find the
// root commit of the repository. Used as a fallback when sessionStartHash
// is empty during bisect setup.
func (e *Engine) findRootCommit() (string, error) {
	if e.git == nil {
		return "", m31errors.ErrGitNotInitialized
	}
	out, err := e.git.Run("rev-list", "--max-parents=0", "HEAD")
	if err != nil {
		return "", fmt.Errorf("git rev-list: %w", err)
	}
	hash := strings.TrimSpace(out)
	if hash == "" {
		return "", fmt.Errorf("empty root commit hash")
	}
	// rev-list may return multiple hashes (root commits); take the first
	if idx := strings.IndexByte(hash, '\n'); idx > 0 {
		hash = hash[:idx]
	}
	return hash, nil
}

// tryBisectHeal attempts to heal a failed task using git bisect to find
// the offending commit, then re-healing with that context. Returns true
// if the task was successfully healed and re-verified.
func (e *Engine) tryBisectHeal(ctx context.Context, taskEntry *m31types.Task, task m31types.Task, verifyResult VerificationResult, goal string) bool {
	if e.git == nil {
		return false
	}

	headHash, err := e.git.HeadHash()
	if err != nil {
		e.logger.Warn("bisect fallback: cannot get HEAD hash", "error", err)
		return false
	}

	goodHash := e.sessionStartHash
	if goodHash == "" {
		if rootHash, rootErr := e.findRootCommit(); rootErr == nil {
			goodHash = rootHash
		} else {
			// Cannot determine a good commit for bisect
			e.logger.Warn("bisect fallback: cannot determine good commit for bisect")
			return false
		}
	}

	b := bisect.New(e.workDir, e.logger)
	b.SetGit(e.git)
	checkFn := func() bool {
		vr := e.verifyTask(ctx, task)
		return vr.FilesExist && vr.SyntaxOK && vr.TestsOK
	}
	bisectResult, err := b.Run(goodHash, headHash, checkFn)
	if err != nil || bisectResult == nil {
		e.logger.Warn("bisect fallback failed", "error", err)
		return false
	}

	e.logger.Info("bisect found offending commit",
		"commit", bisectResult.OffendingCommit.ShortHash)

	// Targeted heal using bisect context
	failure := fmt.Sprintf("bisect identified commit %s as introducing the failure:\n%s\n\nVerification errors: %v",
		bisectResult.OffendingCommit.ShortHash, bisectResult.Diff, verifyResult.Errors)
	healResult := e.healTask(ctx, task, failure, goal)

	if !healResult.Success {
		e.logger.Warn("post-bisect heal failed", "id", task.ID, "error", healResult.Error)
		if taskEntry.HealsAttempted >= m31types.MaxHealAttempts {
			taskEntry.Status = m31types.StatusUnrecoverable
		}
		return false
	}

	// Re-verify after bisect heal
	newResult := e.verifyTask(ctx, task)
	if newResult.FilesExist && newResult.SyntaxOK && newResult.TestsOK {
		taskEntry.Status = m31types.StatusDone
		e.logger.Info("task healed after bisect", "id", task.ID)
		return true
	}

	e.logger.Warn("post-bisect heal did not fix task", "id", task.ID)
	if taskEntry.HealsAttempted >= m31types.MaxHealAttempts {
		taskEntry.Status = m31types.StatusUnrecoverable
	}
	return false
}
