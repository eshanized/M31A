package workflow

import (
	"context"
	"fmt"
	"time"

	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/pkg/session"
	"github.com/eshanized/M31A/pkg/taskrunner"
	m31types "github.com/eshanized/M31A/internal/types"
)

// ShipSummary holds the session completion summary.
type ShipSummary struct {
	TaskDone    int
	TaskTotal   int
	TaskFailed  int
	TaskSkipped int
	Commits     []git.CommitInfo
	Duration    time.Duration
	SessionID   string
}

// runShip finalizes the session, writes ledger entry, and archives.
func (e *Engine) runShip(ctx context.Context, goal string) (*PhaseResult, error) {
	e.logger.Info("ship phase starting")

	// 1. Load tasks for summary
	tasks, err := e.sessionMgr.LoadTasks(e.sessionID)
	if err != nil {
		tasks = []m31types.Task{}
	}

	// Create runner to get summary
	runner := taskrunner.New(tasks)
	total, done, failed, skipped := runner.Summary()

	// 2. Final git commit
	e.git.Commit(fmt.Sprintf("chore(ship): complete session %s", e.sessionID))

	// 3. Build summary
	duration := time.Since(e.startTime)

	commits, _ := e.git.Log(true, e.startTime.Format(time.RFC3339))

	_ = ShipSummary{
		TaskDone:    done,
		TaskTotal:   total,
		TaskFailed:  failed,
		TaskSkipped: skipped,
		Commits:     commits,
		Duration:    duration,
		SessionID:   e.sessionID,
	}

	// 4. Update ledger
	e.appendLedgerEntry(e.modelID, e.provider.Name(), goal, done, total, duration)

	// 5. Archive session
	e.sessionMgr.ArchiveSession(e.sessionID)

	// 6. Write final STATE.md
	e.sessionMgr.SaveState(e.sessionID, m31types.PhaseShip, "complete", "session shipped")

	// 7. Save checkpoint
	e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseShip,
		Timestamp: time.Now(),
	})

	e.logger.Info("ship phase complete", "duration", duration, "tasks", fmt.Sprintf("%d/%d", done, total))

	return &PhaseResult{
		Phase:   m31types.PhaseShip,
		Success: true,
	}, nil
}

// BuildSummary creates a ShipSummary from the current session state.
func (e *Engine) BuildSummary() ShipSummary {
	tasks, _ := e.sessionMgr.LoadTasks(e.sessionID)
	runner := taskrunner.New(tasks)
	total, done, failed, skipped := runner.Summary()

	commits, _ := e.git.Log(true, e.startTime.Format(time.RFC3339))

	return ShipSummary{
		TaskDone:    done,
		TaskTotal:   total,
		TaskFailed:  failed,
		TaskSkipped: skipped,
		Commits:     commits,
		Duration:    time.Since(e.startTime),
		SessionID:   e.sessionID,
	}
}
