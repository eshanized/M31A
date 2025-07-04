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
	e.logger.Info("verify phase starting")

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
	for i, task := range tasks {
		if task.Status != m31types.StatusDone {
			continue
		}

		result := e.verifyTask(task)

		if result.FilesExist && result.SyntaxOK && result.TestsOK {
			e.logger.Info("task verified", "id", task.ID)
			continue
		}

		// Verification failed — attempt self-heal
		e.logger.Warn("task verification failed", "id", task.ID, "errors", result.Errors)

		if task.HealsAttempted >= m31types.MaxHealAttempts {
			tasks[i].Status = m31types.StatusUnrecoverable
			e.logger.Warn("task unrecoverable", "id", task.ID)

			// Trigger bisect — use the commit before session started as 'good'
			headHash, _ := e.git.HeadHash()
			goodHash := e.sessionStartHash
			if goodHash == "" {
				// Fallback: use HEAD~50 if sessionStartHash not captured
				goodHash = "HEAD~50"
			}
			b := bisect.New(e.workDir, e.logger)
			checkFn := func() bool {
				vr := e.verifyTask(task)
				return vr.FilesExist && vr.SyntaxOK && vr.TestsOK
			}
			bisectResult, err := b.Run(goodHash, headHash, checkFn)
			if err == nil && bisectResult != nil {
				e.logger.Info("bisect found offending commit",
					"commit", bisectResult.OffendingCommit.ShortHash)

				// Targeted heal using bisect context
				failure := fmt.Sprintf("bisect identified commit %s as introducing the failure:\n%s\n\nVerification errors: %v",
					bisectResult.OffendingCommit.ShortHash, bisectResult.Diff, result.Errors)
				healResult := e.healTask(ctx, task, failure)
				tasks[i].HealsAttempted++

				if healResult.Success {
					// Re-verify
					newResult := e.verifyTask(task)
					if newResult.FilesExist && newResult.SyntaxOK && newResult.TestsOK {
						tasks[i].Status = m31types.StatusDone
						e.logger.Info("task healed after bisect", "id", task.ID)
					} else {
						e.logger.Warn("post-bisect heal did not fix task", "id", task.ID)
						tasks[i].Status = m31types.StatusUnrecoverable
					}
				} else {
					e.logger.Warn("post-bisect heal failed", "id", task.ID, "error", healResult.Error)
					tasks[i].Status = m31types.StatusUnrecoverable
				}
			}
			continue
		}

		// Self-heal
		failure := fmt.Sprintf("verification failed: %v", result.Errors)
		healResult := e.healTask(ctx, task, failure)
		tasks[i].HealsAttempted++

		if healResult.Success {
			// Re-verify
			newResult := e.verifyTask(task)
			if newResult.FilesExist && newResult.SyntaxOK && newResult.TestsOK {
				tasks[i].Status = m31types.StatusDone
				e.logger.Info("task healed and verified", "id", task.ID)
			} else {
				e.logger.Warn("heal did not fix task", "id", task.ID)
				tasks[i].Status = m31types.StatusFailed
			}
		} else {
			e.logger.Warn("self-heal failed", "id", task.ID, "error", healResult.Error)
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

	result := &PhaseResult{
		Phase:   m31types.PhaseVerify,
		Success: allOK,
		Tasks:   tasks,
	}
	if !allOK && len(failedTasks) > 0 {
		result.Error = strings.Join(failedTasks, "; ")
		return result, fmt.Errorf("%w: %s", m31errors.ErrTaskFailed, result.Error)
	}
	return result, nil
}
