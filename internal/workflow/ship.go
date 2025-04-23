package workflow

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/pkg/ledger"
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
	if err := e.git.Commit(fmt.Sprintf("chore: ship %s", e.sessionID)); err != nil {
		return nil, fmt.Errorf("ship commit: %w", err)
	}

	// 3. Build summary
	duration := time.Since(e.startTime)

	commits, _ := e.git.Log(true, e.startTime.Format(time.RFC3339))

	summary := ShipSummary{
		TaskDone:    done,
		TaskTotal:   total,
		TaskFailed:  failed,
		TaskSkipped: skipped,
		Commits:     commits,
		Duration:    duration,
		SessionID:   e.sessionID,
	}

	e.logger.Info("session shipped",
		"session_id", summary.SessionID,
		"tasks", fmt.Sprintf("%d/%d done", summary.TaskDone, summary.TaskTotal),
		"failed", summary.TaskFailed,
		"skipped", summary.TaskSkipped,
		"commits", len(summary.Commits),
		"duration", summary.Duration,
	)

	// 4. Update ledger
	sess, err := e.sessionMgr.LoadSession(e.sessionID)
	if err == nil {
		home := os.Getenv("HOME")
		if home == "" {
			home = os.Getenv("USERPROFILE")
		}
		if home != "" {
			ledgerPath := filepath.Join(home, ".m31a", "LEDGER.md")
			l := ledger.New(ledgerPath)
			entry := ledger.NewEntry(sess.Session, total, done, skipped, len(commits), 0)
			if err := l.Append(entry); err != nil {
				e.logger.Warn("ledger update failed", "error", err)
			}
		}
	} else {
		e.logger.Warn("ledger update skipped: cannot load session", "error", err)
	}

	// 5. Archive session
	if err := e.sessionMgr.ArchiveSession(e.sessionID); err != nil {
		e.logger.Warn("archive session failed", "error", err)
	}

	// 6. Write final STATE.md
	if err := e.sessionMgr.SaveState(e.sessionID, m31types.PhaseShip, "complete", "session shipped"); err != nil {
		e.logger.Warn("save state failed", "error", err)
	}

	// 7. Save checkpoint
	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseShip,
		Timestamp: time.Now(),
	}); err != nil {
		e.logger.Warn("save checkpoint failed", "error", err)
	}

	e.logger.Info("ship phase complete", "duration", duration, "tasks", fmt.Sprintf("%d/%d", done, total))

	result := &PhaseResult{
		Phase:   m31types.PhaseShip,
		Success: true,
	}
	if failed > 0 {
		result.Error = fmt.Sprintf("%d tasks failed", failed)
		return result, fmt.Errorf("%w: %s", m31errors.ErrTaskFailed, result.Error)
	}
	return result, nil
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
