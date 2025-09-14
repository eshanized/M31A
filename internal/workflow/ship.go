package workflow

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/git"
	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/ledger"
	"github.com/eshanized/M31A/pkg/session"
	"github.com/eshanized/M31A/pkg/taskrunner"
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

	// Emit intermediate progress
	e.emit(IntermediateProgressMsg{
		Phase:   "ship",
		Message: "Creating final commit...",
	})

	// 1. Load tasks for summary
	tasks, err := e.sessionMgr.LoadTasks(e.sessionID)
	if err != nil {
		e.logger.Warn("failed to load tasks for ship summary, using empty list", "error", err)
		tasks = []m31types.Task{}
	}

	// Create runner to get summary
	runner := taskrunner.New(tasks)
	total, done, failed, skipped := runner.Summary()

	// 2. Final git commit
	if e.git != nil {
		if err := e.git.AddAll(); err != nil {
			e.logger.Warn("git add all before ship commit failed", "error", err)
		}
		if err := e.git.Commit(fmt.Sprintf("%s: ship %s", e.gitConfig().ShipPrefix, e.sessionID)); err != nil {
			return nil, fmt.Errorf("ship commit: %w", err)
		}
	}

	// 3. Build summary
	duration := time.Since(e.startTime)

	var commits []git.CommitInfo
	if e.git != nil {
		var err error
		commits, err = e.git.Log(true, e.startTime.Format(time.RFC3339))
		if err != nil {
			e.logger.Warn("git log failed during ship summary", "error", err)
		}
	}

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
		home, err := os.UserHomeDir()
		if err != nil {
			e.logger.Warn("ledger update skipped: cannot determine home dir", "error", err)
		} else {
			ledgerPath := filepath.Join(home, ".m31a", "LEDGER.md")
			l := ledger.New(ledgerPath)
			entry := ledger.NewEntry(sess.Session, total, failed, skipped, len(commits), 0)
			if err := l.Append(entry); err != nil {
				e.logger.Warn("ledger update failed", "error", err)
			}
		}
	} else {
		e.logger.Warn("ledger update skipped: cannot load session", "error", err)
	}

	// 5. Write final STATE.md before archiving (CR-10: fix write ordering)
	if err := e.sessionMgr.SaveState(e.sessionID, m31types.PhaseShip, "complete", "session shipped"); err != nil {
		e.logger.Warn("save state failed", "error", err)
	}

	// 6. Archive session (after state is persisted)
	if err := e.sessionMgr.ArchiveSession(e.sessionID); err != nil {
		e.logger.Warn("archive session failed", "error", err)
	}

	// 7. Save checkpoint
	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseShip,
		Timestamp: time.Now(),
	}); err != nil {
		e.logger.Warn("save checkpoint failed", "error", err)
	}

	e.logger.Info("ship phase complete", "duration", duration, "tasks", fmt.Sprintf("%d/%d", done, total))

	// Collect git diff stats
	var diffStats DiffStats
	if e.git != nil {
		diffStats = e.collectDiffStats()
	}

	result := &PhaseResult{
		Phase:      m31types.PhaseShip,
		Success:    true,
		Commits:    commits,
		DiffStats:  diffStats,
		DurationMs: duration.Milliseconds(),
	}
	if failed > 0 {
		result.Error = fmt.Sprintf("%d tasks failed", failed)
		result.Success = false
		return result, fmt.Errorf("%w: %s", m31errors.ErrTaskFailed, result.Error)
	}
	return result, nil
}

// collectDiffStats collects file change statistics from git diff.
func (e *Engine) collectDiffStats() DiffStats {
	stats := DiffStats{}

	if e.git == nil {
		return stats
	}

	// Use git diff --numstat to get per-file stats
	diffOutput, err := e.git.Diff("HEAD", "")
	if err != nil {
		return stats
	}

	// Parse numstat format: <additions>\t<deletions>\t<filepath>
	lines := strings.Split(diffOutput, "\n")
	for _, line := range lines {
		if line == "" {
			continue
		}
		parts := strings.SplitN(line, "\t", 3)
		if len(parts) < 3 {
			continue
		}

		adds := 0
		dels := 0
		if parts[0] != "-" {
			fmt.Sscanf(parts[0], "%d", &adds)
		}
		if parts[1] != "-" {
			fmt.Sscanf(parts[1], "%d", &dels)
		}

		stats.Insertions += adds
		stats.Deletions += dels

		// Count file changes
		filepath := parts[2]
		if strings.HasPrefix(filepath, "a/") || strings.HasPrefix(filepath, "b/") {
			filepath = strings.TrimPrefix(filepath, "a/")
			filepath = strings.TrimPrefix(filepath, "b/")
		}

		// Simple heuristic: new files have all additions and no deletions
		if adds > 0 && dels == 0 && parts[0] != "0" {
			stats.FilesAdded++
		} else if adds == 0 && dels > 0 {
			stats.FilesDeleted++
		} else {
			stats.FilesModified++
		}
	}

	return stats
}

// BuildSummary creates a ShipSummary from the current session state.
func (e *Engine) BuildSummary() ShipSummary {
	tasks, _ := e.sessionMgr.LoadTasks(e.sessionID)
	runner := taskrunner.New(tasks)
	total, done, failed, skipped := runner.Summary()

	var commits []git.CommitInfo
	if e.git != nil {
		var err error
		commits, err = e.git.Log(true, e.startTime.Format(time.RFC3339))
		if err != nil {
			e.logger.Warn("git log failed during ship summary", "error", err)
		}
	}

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
