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
	e.logger.Info("ship phase starting", "goal", goal)

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

	// 2. Final git commit — ship commits files touched by the workflow.
	// Collect file paths from tasks to scope the commit.
	if e.git != nil {
		// Collect files touched by tasks
		var taskFiles []string
		for _, task := range tasks {
			taskFiles = append(taskFiles, task.Files...)
		}

		if dirty, _ := e.git.HasUncommittedChanges(); dirty {
			statusOut, _ := e.git.StatusPorcelain()
			if len(statusOut) > 0 {
				// Log unrelated dirty files as a warning
				e.logger.Warn("ship commit: found uncommitted changes in working tree",
					"dirty_files", len(statusOut))
				for _, fs := range statusOut {
					related := false
					for _, tf := range taskFiles {
						if fs.Path == tf {
							related = true
							break
						}
					}
					if !related {
						e.logger.Warn("ship commit: file not related to any task", "path", fs.Path, "status", fs.Status)
					}
				}
			}
		}

		if len(taskFiles) > 0 {
			if err := e.git.Add(taskFiles...); err != nil {
				e.logger.Warn("git add task files failed, falling back to add all", "error", err)
				if err := e.git.AddAll(); err != nil {
					e.logger.Warn("git add all before ship commit failed", "error", err)
				}
			}
		} else {
			if err := e.git.AddAll(); err != nil {
				e.logger.Warn("git add all before ship commit failed", "error", err)
			}
		}

		// Guard: skip the commit when nothing is staged. DiffStaged checks
		// the index against HEAD, which correctly detects staged changes even
		// when the working tree is clean after AddAll (BUG-10).
		staged, _ := e.git.DiffStaged()
		if strings.TrimSpace(staged) == "" {
			e.logger.Info("ship: no staged changes — skipping commit")
		} else if _, err := e.git.CommitStaged(fmt.Sprintf("%s: ship %s", e.gitConfig().ShipPrefix, e.sessionID)); err != nil {
			return nil, fmt.Errorf("ship commit: %w", err)
		}
	}

	// 3. Build summary
	duration := time.Since(e.startTime)

	var commits []git.CommitInfo
	if e.git != nil {
		var err error
		commits, err = e.git.LogSince(e.startTime)
		if err != nil {
			e.logger.Warn("git log failed during ship summary", "error", err)
		}
		// W-17: Truncate commit log to last 50 lines
		if len(commits) > 50 {
			commits = commits[len(commits)-50:]
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

	// 5. Write final STATE.md and checkpoint before archiving (fix write ordering)
	if err := e.sessionMgr.SaveState(e.sessionID, m31types.PhaseShip, "complete", "session shipped"); err != nil {
		e.logger.Warn("save state failed", "error", err)
	}

	// 6. Save checkpoint before archiving (checkpoint requires the session directory to exist)
	if err := e.sessionMgr.SaveCheckpoint(e.sessionID, session.Checkpoint{
		Phase:     m31types.PhaseShip,
		Timestamp: time.Now(),
	}); err != nil {
		e.logger.Warn("save checkpoint failed", "error", err)
	}

	// 7. Generate demonstration document via LLM
	var demonstration string
	e.emit(IntermediateProgressMsg{
		Phase:   "ship",
		Message: "Generating walkthrough...",
	})
	demonstration = e.generateDemonstration(ctx, goal, tasks, done, failed, commits)
	if demonstration != "" {
		if err := e.sessionMgr.SaveDemonstration(e.sessionID, demonstration); err != nil {
			e.logger.Warn("save demonstration failed", "error", err)
		}
		e.emit(DemonstrationReadyMsg{Content: demonstration})
	}

	// Append session learnings to MEMORY.md for cross-session recall
	sessionDir := filepath.Dir(e.planningDir)
	memPath := filepath.Join(sessionDir, "MEMORY.md")
	project, _ := e.sessionMgr.LoadProject(e.sessionID)
	projectType, framework := "unknown", ""
	if project != nil {
		projectType = project.ProjectType
		framework = project.Framework
	}
	memEntry := fmt.Sprintf(
		"\n## Session %s (%s)\n- Goal: %s\n- Tasks done: %d/%d\n- Project: %s (%s)\n- Model: %s\n",
		e.sessionID, time.Now().Format("2006-01-02"),
		goal, done, total, projectType, framework, e.modelForPhase(m31types.PhaseExecute),
	)

	// Record failed tasks with their root causes for cross-session learning
	if failed > 0 {
		memEntry += "- Failed tasks:\n"
		for _, t := range tasks {
			if t.Status == m31types.StatusFailed || t.Status == m31types.StatusUnrecoverable {
				memEntry += fmt.Sprintf("  - Task %d (%s): %s (heals attempted: %d)\n",
					t.ID, t.Status, t.Description, t.HealsAttempted)
			}
		}
	}

	// Record key patterns discovered (file types touched, frameworks used)
	fileExts := make(map[string]int)
	for _, t := range tasks {
		for _, f := range t.Files {
			ext := filepath.Ext(f)
			if ext != "" {
				fileExts[ext]++
			}
		}
	}
	if len(fileExts) > 0 {
		memEntry += "- File patterns: "
		first := true
		for ext, count := range fileExts {
			if !first {
				memEntry += ", "
			}
			memEntry += fmt.Sprintf("%s(%d)", ext, count)
			first = false
		}
		memEntry += "\n"
	}

	// Record duration and efficiency
	memEntry += fmt.Sprintf("- Duration: %s\n", duration.Round(time.Second))

	if memFile, openErr := os.OpenFile(memPath, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0644); openErr == nil {
		if _, writeErr := memFile.WriteString(memEntry); writeErr != nil {
			e.logger.Warn("memory write failed", "error", writeErr)
		}
		memFile.Close()
	}

	// Update checkbox tasks.md with final statuses
	if err := e.sessionMgr.SaveTasksCheckbox(e.sessionID, tasks); err != nil {
		e.logger.Warn("save checkbox tasks.md failed", "error", err)
	}

	e.logger.Info("ship phase complete", "duration", duration, "tasks", fmt.Sprintf("%d/%d", done, total))

	// Collect git diff stats
	var diffStats DiffStats
	if e.git != nil {
		diffStats = e.collectDiffStats()
	}

	result := &PhaseResult{
		Phase:         m31types.PhaseShip,
		Success:       true,
		Commits:       commits,
		DiffStats:     diffStats,
		DurationMs:    duration.Milliseconds(),
		Demonstration: demonstration,
	}
	if failed > 0 {
		result.Error = fmt.Sprintf("%d tasks failed", failed)
		result.Success = false
		return result, fmt.Errorf("%w: %s", m31errors.ErrTaskFailed, result.Error)
	}
	return result, nil
}

// collectDiffStats collects file change statistics from git diff.
// Diffs HEAD against the session start (or HEAD^ as fallback) so the stats
// reflect changes made by this session, not unrelated uncommitted files.
// Uses git status --porcelain on the range for accurate add/modify/delete
// classification instead of a numstat-based heuristic (BUG-15).
func (e *Engine) collectDiffStats() DiffStats {
	stats := DiffStats{}

	if e.git == nil {
		return stats
	}

	// Pick a base ref: prefer session start hash; fall back to HEAD^.
	// If neither is available (root commit, no session start), use plain
	// diff which compares working tree against HEAD (least useful but safe).
	baseRef := ""
	switch {
	case e.sessionStartHash != "":
		baseRef = e.sessionStartHash
	default:
		// Try HEAD^ — if the repo has no parent commit, this will fail
		// and we'll fall through to plain diff.
		if _, err := e.git.RevParse("HEAD^"); err == nil {
			baseRef = "HEAD^"
		}
	}

	var numstatOutput string
	var err error
	if baseRef != "" {
		numstatOutput, err = e.git.Run("diff", "--numstat", baseRef+"..HEAD")
	} else {
		// Last resort: compare working tree against HEAD (not numstat-equivalent
		// but preserves insertions/deletions accounting when no range is available).
		numstatOutput, err = e.git.Run("diff", "--numstat")
	}
	if err != nil {
		return stats
	}

	// Parse numstat format: <additions>\t<deletions>\t<filepath>
	lines := strings.Split(numstatOutput, "\n")
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
		stats.FilesModified++
	}

	// Refine classification using git log --diff-filter on the same range.
	// This accurately distinguishes added/deleted/modified files, fixing the
	// numstat-based heuristic that misclassified add-only modifications.
	if baseRef != "" {
		if addedOut, err := e.git.Run("diff", "--name-only", "--diff-filter=A", baseRef+"..HEAD"); err == nil {
			stats.FilesAdded = countNonEmptyLines(addedOut)
		}
		if deletedOut, err := e.git.Run("diff", "--name-only", "--diff-filter=D", baseRef+"..HEAD"); err == nil {
			stats.FilesDeleted = countNonEmptyLines(deletedOut)
		}
		// FilesModified currently counts ALL changed files (numstat lines).
		// Subtract added+deleted to get true modification count.
		total := stats.FilesModified
		stats.FilesModified = total - stats.FilesAdded - stats.FilesDeleted
		if stats.FilesModified < 0 {
			stats.FilesModified = 0
		}
	}

	return stats
}

// countNonEmptyLines counts non-empty lines in s.
func countNonEmptyLines(s string) int {
	n := 0
	for _, line := range strings.Split(s, "\n") {
		if strings.TrimSpace(line) != "" {
			n++
		}
	}
	return n
}

// BuildSummary creates a ShipSummary from the current session state.
func (e *Engine) BuildSummary() ShipSummary {
	tasks, _ := e.sessionMgr.LoadTasks(e.sessionID)
	runner := taskrunner.New(tasks)
	total, done, failed, skipped := runner.Summary()

	var commits []git.CommitInfo
	if e.git != nil {
		var err error
		commits, err = e.git.LogSince(e.startTime)
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

// generateDemonstration creates a post-completion walkthrough document using the LLM.
// It combines the implementation plan, completed tasks, and git diff stats to produce
// a narrative describing what was built and how to run it.
func (e *Engine) generateDemonstration(ctx context.Context, goal string, tasks []m31types.Task, done, failed int, commits []git.CommitInfo) string {
	planMarkdown, _ := e.sessionMgr.LoadPlan(e.sessionID)
	if planMarkdown == "" {
		planMarkdown = e.planMarkdown
	}

	var sb strings.Builder
	if goal != "" {
		sb.WriteString("## Original Goal\n")
		sb.WriteString(goal)
		sb.WriteString("\n\n")
	}
	sb.WriteString("## Implementation Plan\n")
	if planMarkdown != "" {
		summary := planMarkdown
		if len(summary) > 3000 {
			summary = summary[:3000] + "\n... (truncated)"
		}
		sb.WriteString(summary)
	}
	sb.WriteString("\n\n## Completed Tasks\n")
	sb.WriteString(formatTaskSummary(tasks))

	if len(commits) > 0 {
		sb.WriteString("\n## Commits\n")
		for _, c := range commits {
			sb.WriteString(fmt.Sprintf("- %s: %s\n", c.Hash[:min(len(c.Hash), 7)], c.Message))
		}
	}

	sb.WriteString(fmt.Sprintf("\n## Summary\n- Tasks completed: %d/%d\n- Tasks failed: %d\n", done, len(tasks), failed))

	if e.git != nil && e.sessionStartHash != "" {
		if diffOut, err := e.git.DiffRefs(e.sessionStartHash, "HEAD"); err == nil && diffOut != "" {
			lines := strings.Split(diffOut, "\n")
			if len(lines) > 20 {
				lines = lines[:20]
			}
			sb.WriteString("\n## File Changes\n")
			for _, line := range lines {
				if line != "" {
					sb.WriteString("- " + line + "\n")
				}
			}
		}
	}

	messages := []m31types.Message{
		{Role: "system", Content: e.buildSystemPrompt(e.prompts.Demonstration)},
		{Role: "user", Content: sb.String()},
	}

	content, err := e.streamLLM(ctx, messages, false)
	if err != nil {
		e.logger.Warn("demonstration generation failed", "error", err)
		return ""
	}
	return content
}
