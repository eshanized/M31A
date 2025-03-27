package workflow

import (
	"context"
	"fmt"
	"time"

	"github.com/eshanized/M31A/pkg/bisect"
	"github.com/eshanized/M31A/pkg/session"
	m31types "github.com/eshanized/M31A/internal/types"
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

			// Trigger bisect
			sessionStart, err := e.sessionMgr.LoadSession(e.sessionID)
			if err == nil && sessionStart != nil {
				headHash, _ := e.git.HeadHash()
				b := bisect.New(e.workDir, e.logger)
				// Bisect with a check function that runs verification
				checkFn := func() bool {
					vr := e.verifyTask(task)
					return vr.FilesExist && vr.SyntaxOK && vr.TestsOK
				}
				bisectResult, err := b.Run(sessionStart.ID, headHash, checkFn)
				if err == nil && bisectResult != nil {
					e.logger.Info("bisect found offending commit",
						"commit", bisectResult.OffendingCommit.ShortHash)
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
				e.logger.Info("task healed and verified", "id", task.ID)
			} else {
				e.logger.Warn("heal did not fix task", "id", task.ID)
			}
		} else {
			e.logger.Warn("self-heal failed", "id", task.ID, "error", healResult.Error)
			if tasks[i].HealsAttempted >= m31types.MaxHealAttempts {
				tasks[i].Status = m31types.StatusUnrecoverable
			}
		}
	}

	// 3. Save updated TASKS.md
	e.sessionMgr.SaveTasks(e.sessionID, tasks)

	// 4. Save checkpoint
	e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseVerify,
		Timestamp: time.Now(),
	})

	// 5. Write STATE.md
	e.sessionMgr.SaveState(e.sessionID, m31types.PhaseVerify, "verification complete", "verify done")

	// 6. Check if all passed or skipped
	allOK := true
	for _, task := range tasks {
		if task.Status == m31types.StatusFailed || task.Status == m31types.StatusUnrecoverable {
			allOK = false
			break
		}
	}

	e.logger.Info("verify phase complete", "all_ok", allOK)

	return &PhaseResult{
		Phase:   m31types.PhaseVerify,
		Success: allOK,
		Tasks:   tasks,
	}, nil
}
