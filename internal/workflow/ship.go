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
	phaseStart := time.Now()
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

	// ── Pre-ship checklist ────────────────────────────────────────────────
	preflightEnabled := e.cfg != nil && e.cfg.Features.ShipPreflight
	if preflightEnabled {
		preflight := e.runShipPreflight(tasks)
		e.emit(ShipPreflightMsg(preflight))
		if !preflight.Passed {
			e.logger.Warn("ship preflight found blocking issues", "count", len(preflight.Issues))
			for _, issue := range preflight.Issues {
				e.logger.Warn("preflight issue", "issue", issue)
			}
		} else if len(preflight.Issues) > 0 {
			e.logger.Info("ship preflight: warnings only", "count", len(preflight.Issues))
		}
	}

	// 2. Final git commit — ship commits files touched by the workflow.
	// Collect file paths from tasks to scope the commit.
	if e.git != nil {
		// Collect files touched by tasks
		var taskFiles []string
		for _, task := range tasks {
			taskFiles = append(taskFiles, task.Files...)
		}

		dirty, dirtyErr := e.git.HasUncommittedChanges()
		if dirtyErr != nil {
			e.logger.Warn("ship: failed to check uncommitted changes, assuming dirty", "error", dirtyErr)
			dirty = true
		}
		if dirty {
			statusOut, statusErr := e.git.StatusPorcelain()
			if statusErr != nil {
				e.logger.Warn("ship: failed to get status porcelain", "error", statusErr)
			} else if len(statusOut) > 0 {
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
			if addErr := e.git.Add(taskFiles...); addErr != nil {
				e.logger.Warn("git add task files failed, falling back to add all", "error", addErr)
				if addAllErr := e.git.AddAll(); addAllErr != nil {
					e.logger.Warn("git add all before ship commit failed", "error", addAllErr)
				}
			}
		} else {
			e.logger.Warn("ship: no task-to-file mapping found — skipping commit to avoid committing unrelated files")
		}

		// Guard: skip the commit when nothing is staged. DiffStaged checks
		// the index against HEAD, which correctly detects staged changes even
		// when the working tree is clean after AddAll (BUG-10).
		staged, stagedErr := e.git.DiffStaged()
		if stagedErr != nil {
			e.logger.Warn("ship: failed to check staged changes, attempting commit anyway", "error", stagedErr)
		} else if strings.TrimSpace(staged) == "" {
			e.logger.Info("ship: no staged changes — skipping commit")
		} else if _, commitErr := e.git.CommitStaged(fmt.Sprintf("%s: ship %s", e.gitConfig().ShipPrefix, e.sessionID)); commitErr != nil {
			return nil, fmt.Errorf("ship commit: %w", commitErr)
		}
	}

	// 3. Post-ship validation
	if e.git != nil {
		hash, hashErr := e.git.HeadHash()
		if hashErr != nil {
			e.logger.Warn("post-ship: cannot verify commit", "error", hashErr)
		} else {
			e.logger.Info("post-ship: commit verified", "hash", hash[:min(len(hash), 7)])
		}
		dirty, dirtyErr := e.git.HasUncommittedChanges()
		if dirtyErr != nil {
			e.logger.Warn("post-ship: failed to check working tree", "error", dirtyErr)
		} else if dirty {
			e.logger.Warn("post-ship: working tree has uncommitted changes after ship")
		}
	}

	// 4. Build summary
	duration := time.Since(e.startTime)

	var commits []git.CommitInfo
	if e.git != nil {
		var logErr error
		commits, logErr = e.git.LogSince(e.startTime)
		if logErr != nil {
			e.logger.Warn("git log failed during ship summary", "error", logErr)
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
		if e.ledger != nil {
			entry := ledger.NewEntry(sess.Session, total, failed, skipped, len(commits), 0)
			if appendErr := e.ledger.Append(entry); appendErr != nil {
				e.logger.Warn("ledger update failed", "error", appendErr)
			}
		} else {
			e.logger.Warn("ledger update skipped: no ledger instance configured on engine")
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

	// ── Changelog generation ──────────────────────────────────────────────
	changelogEnabled := e.cfg != nil && e.cfg.Features.ShipChangelog
	if changelogEnabled {
		changelogContent := e.generateChangelog(tasks, commits)
		if changelogContent != "" {
			changelogPath := filepath.Join(filepath.Dir(e.planningDir), "CHANGELOG_SESSION.md")
			if writeErr := os.WriteFile(changelogPath, []byte(changelogContent), m31types.FilePermission); writeErr != nil {
				e.logger.Warn("failed to write changelog", "error", writeErr)
			} else {
				e.logger.Info("changelog written", "path", changelogPath)
			}
			e.emit(ShipChangelogMsg{
				Content: changelogContent,
				Entries: len(tasks),
			})
		}
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

	// Rotate MEMORY.md if it exceeds max size (512KB) to prevent unbounded growth
	const maxMemorySize = 512 * 1024
	if info, statErr := os.Stat(memPath); statErr == nil && info.Size() > maxMemorySize {
		e.rotateMemoryFile(memPath)
	}

	if memFile, openErr := os.OpenFile(memPath, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0600); openErr == nil {
		// Use flock to prevent interleaved writes from concurrent sessions.
		if writeErr := flockWrite(memFile, memEntry); writeErr != nil {
			e.logger.Warn("memory write failed", "error", writeErr)
		}
		_ = memFile.Close()
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
		DurationMs:    time.Since(phaseStart).Milliseconds(),
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
			_, _ = fmt.Sscanf(parts[0], "%d", &adds)
		}
		if parts[1] != "-" {
			_, _ = fmt.Sscanf(parts[1], "%d", &dels)
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
	demoTasks := tasks
	if len(demoTasks) > 20 {
		demoTasks = demoTasks[:20]
	}
	sb.WriteString(formatTaskSummary(demoTasks))

	if len(commits) > 0 {
		sb.WriteString("\n## Commits\n")
		for _, c := range commits {
			fmt.Fprintf(&sb, "- %s: %s\n", c.Hash[:min(len(c.Hash), 7)], c.Message)
		}
	}

	fmt.Fprintf(&sb, "\n## Summary\n- Tasks completed: %d/%d\n- Tasks failed: %d\n", done, len(tasks), failed)

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

// rotateMemoryFile trims MEMORY.md to its most recent entries when it
// exceeds maxMemorySize, preventing unbounded growth across sessions.
func (e *Engine) rotateMemoryFile(memPath string) {
	data, err := os.ReadFile(memPath)
	if err != nil {
		e.logger.Warn("failed to read MEMORY.md for rotation", "error", err)
		return
	}
	content := string(data)
	lines := strings.Split(content, "\n")

	// Find all session headers (## Session ...) and keep only the last 20
	type sessionBlock struct {
		start, end int
	}
	var blocks []sessionBlock
	for i, line := range lines {
		if strings.HasPrefix(line, "## Session ") {
			if len(blocks) > 0 {
				blocks[len(blocks)-1].end = i
			}
			blocks = append(blocks, sessionBlock{start: i, end: len(lines)})
		}
	}
	if len(blocks) <= 20 {
		return // not enough entries to rotate
	}

	// Keep only the last 20 session blocks
	keep := blocks[len(blocks)-20:]
	rotated := lines[keep[0].start:]
	rotatedContent := strings.Join(rotated, "\n")

	if writeErr := os.WriteFile(memPath, []byte(rotatedContent), 0600); writeErr != nil {
		e.logger.Warn("failed to write rotated MEMORY.md", "error", writeErr)
	} else {
		e.logger.Info("rotated MEMORY.md", "removed_sessions", len(blocks)-20, "new_size", len(rotatedContent))
	}
}
