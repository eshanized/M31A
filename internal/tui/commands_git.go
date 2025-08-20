package tui

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/pkg/ledger"
)

// handleLedger shows session history from the learning ledger.
func handleLedger(args []string, ctx CommandContext) CommandResult {
	if ctx.Ledger == nil {
		return CommandResult{Success: false, Message: "Ledger not available."}
	}

	if len(args) > 0 && args[0] == "stats" {
		stats := ctx.Ledger.Stats()
		var b strings.Builder
		b.WriteString(fmt.Sprintf("Total sessions:      %d\n", stats.TotalSessions))
		b.WriteString(fmt.Sprintf("Avg tasks/session:   %.1f\n", stats.AvgTaskCount))
		b.WriteString(fmt.Sprintf("Avg cost:            $%.4f\n", stats.AvgCost))
		b.WriteString(fmt.Sprintf("Avg duration:        %.0f min\n", stats.AvgDurationMinutes))
		b.WriteString(fmt.Sprintf("Total failed tasks:  %d\n", stats.TotalFailedTasks))
		if len(stats.TopFrameworks) > 0 {
			b.WriteString(fmt.Sprintf("Top frameworks:      %s\n", strings.Join(stats.TopFrameworks, ", ")))
		}
		if len(stats.TopFailures) > 0 {
			b.WriteString(fmt.Sprintf("Top failures:        %s", strings.Join(stats.TopFailures, ", ")))
		}
		return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
	}

	if len(args) > 0 {
		entries := ctx.Ledger.EntriesFiltered(args[0], nil, 5)
		return formatLedgerEntries(entries, fmt.Sprintf("Recent sessions (type: %s)", args[0]))
	}

	entries := ctx.Ledger.Entries()
	return formatLedgerEntries(entries, "Recent sessions")
}

func formatLedgerEntries(entries []ledger.LedgerEntry, title string) CommandResult {
	if len(entries) == 0 {
		return CommandResult{Success: true, Message: fmt.Sprintf("%s: none", title)}
	}

	if len(entries) > 5 {
		entries = entries[:5]
	}

	var b strings.Builder
	b.WriteString(title + ":\n")
	for i, e := range entries {
		b.WriteString(fmt.Sprintf("  %d. %s | %s/%s | %d msgs | %s\n",
			i+1, e.SessionID, e.Provider, e.Model, e.TaskCount, e.Timestamp.Format("Jan 02 15:04")))
	}
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleRollback shows the commit chain or performs a rollback.
// Subcommands:
//
//	/rollback                          — show commit chain
//	/rollback <hash>                   — soft reset to hash (keeps changes staged)
//	/rollback --hard <hash> --confirm  — hard reset (discards uncommitted changes)
//	/rollback --safe <hash> --confirm  — hard reset then pop stash (preserves uncommitted changes)
//	/rollback --preview <hash>         — show diff between hash and HEAD without resetting
//	/rollback --head                   — show current HEAD hash
func handleRollback(args []string, ctx CommandContext) CommandResult {
	if len(args) == 0 {
		if ctx.Rollback != nil {
			entries, err := ctx.Rollback.Chain(10)
			if err != nil {
				return CommandResult{Success: false, Message: fmt.Sprintf("Failed to get commit chain: %v", err)}
			}
			commits := make([]git.CommitInfo, len(entries))
			for i, e := range entries {
				commits[i] = e.CommitInfo
			}
			return formatCommitChain(commits)
		}

		if ctx.Git != nil {
			commits, err := ctx.Git.Log(false, "")
			if err != nil {
				return CommandResult{Success: false, Message: fmt.Sprintf("Failed to get git log: %v", err)}
			}
			if len(commits) > 10 {
				commits = commits[:10]
			}
			return formatCommitChain(commits)
		}

		return CommandResult{Success: false, Message: "No git repository available."}
	}

	if ctx.Rollback == nil {
		return CommandResult{Success: false, Message: "Rollback not available."}
	}

	// /rollback --head — show current HEAD hash
	if args[0] == "--head" {
		head, err := ctx.Rollback.CurrentHead()
		if err != nil {
			return CommandResult{Success: false, Message: fmt.Sprintf("Failed to read HEAD: %v", err)}
		}
		return CommandResult{Success: true, Message: fmt.Sprintf("HEAD: %s", head)}
	}

	// /rollback --preview <hash> — show diff without modifying the tree
	if args[0] == "--preview" {
		if len(args) < 2 {
			return CommandResult{Success: false, Message: "Usage: /rollback --preview <hash>"}
		}
		diff, err := ctx.Rollback.Preview(args[1])
		if err != nil {
			return CommandResult{Success: false, Message: fmt.Sprintf("Preview failed: %v", err)}
		}
		if diff == "" {
			return CommandResult{Success: true, Message: fmt.Sprintf("No diff between %s and HEAD.", args[1])}
		}
		screen := ScreenDiff
		return CommandResult{
			Success: true,
			Screen:  &screen,
			Message: fmt.Sprintf("Rollback preview: %s..HEAD", args[1]),
			Cmd: func() tea.Msg {
				return DiffScreenMsg{
					Diff:  diff,
					Title: fmt.Sprintf("Rollback preview: %s..HEAD", args[1]),
				}
			},
		}
	}

	// /rollback --safe <hash> [--confirm] — hard reset then pop stash (preserves uncommitted changes)
	if args[0] == "--safe" && len(args) > 1 {
		commitRef := args[1]
		hasConfirm := false
		for _, arg := range args[2:] {
			if arg == "--confirm" {
				hasConfirm = true
			}
		}
		if !hasConfirm {
			return CommandResult{
				Success: false,
				Message: fmt.Sprintf("Safe reset preserves uncommitted changes via stash pop. Use /rollback --safe %s --confirm to proceed.", commitRef),
			}
		}
		result, err := ctx.Rollback.SafeReset(commitRef)
		if err != nil {
			return CommandResult{Success: false, Message: fmt.Sprintf("Safe reset failed: %v", err)}
		}
		return CommandResult{Success: true, Message: result.Message}
	}

	if args[0] == "--hard" && len(args) > 1 {
		// Check for --confirm flag
		hasConfirm := false
		commitRef := args[1]
		for _, arg := range args[2:] {
			if arg == "--confirm" {
				hasConfirm = true
			}
		}
		if !hasConfirm {
			return CommandResult{
				Success: false,
				Message: fmt.Sprintf("This will permanently discard uncommitted changes and reset to %s. Use /rollback --hard %s --confirm to proceed.", commitRef, commitRef),
			}
		}
		result, err := ctx.Rollback.HardReset(commitRef)
		if err != nil {
			return CommandResult{Success: false, Message: fmt.Sprintf("Hard reset failed: %v", err)}
		}
		return CommandResult{Success: true, Message: result.Message}
	}

	result, err := ctx.Rollback.SoftReset(args[0], nil)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Soft reset failed: %v", err)}
	}
	return CommandResult{Success: true, Message: result.Message}
}

func formatCommitChain(commits []git.CommitInfo) CommandResult {
	if len(commits) == 0 {
		return CommandResult{Success: true, Message: "No commits in this repository."}
	}

	var b strings.Builder
	b.WriteString("Recent commits:\n")
	for i, c := range commits {
		marker := ""
		if i == 0 {
			marker = " [HEAD]"
		}
		shortHash := c.ShortHash
		if shortHash == "" && len(c.Hash) > 7 {
			shortHash = c.Hash[:7]
		} else if shortHash == "" {
			shortHash = c.Hash
		}
		b.WriteString(fmt.Sprintf("  %s %s%s\n", shortHash, c.Message, marker))
	}
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleDiff shows git diff of changes.
func handleDiff(args []string, ctx CommandContext) CommandResult {
	if ctx.Git == nil {
		return CommandResult{Success: false, Message: "No git repository available."}
	}

	if len(args) > 0 && args[0] == "--stat" {
		diff, err := ctx.Git.Diff("", "")
		if err != nil {
			return CommandResult{Success: false, Message: fmt.Sprintf("Failed to get diff: %v", err)}
		}
		if diff == "" {
			return CommandResult{Success: true, Message: "No changes."}
		}
		status, err := ctx.Git.StatusPorcelain()
		if err != nil {
			return CommandResult{Success: false, Message: fmt.Sprintf("Failed to get status: %v", err)}
		}
		var b strings.Builder
		b.WriteString("Diff Stat:\n")
		totalAdd, totalDel := 0, 0
		for _, s := range status {
			b.WriteString(fmt.Sprintf("  %s  %s", s.Status, s.Path))
			if s.Additions > 0 || s.Deletions > 0 {
				b.WriteString(fmt.Sprintf("  (+%d, -%d)", s.Additions, s.Deletions))
				totalAdd += s.Additions
				totalDel += s.Deletions
			}
			b.WriteString("\n")
		}
		b.WriteString(fmt.Sprintf("\n%d files changed, %d insertions(+), %d deletions(-)",
			len(status), totalAdd, totalDel))
		return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
	}

	var diff string
	var err error
	var title string

	if len(args) > 0 && args[0] == "--staged" {
		diff, err = ctx.Git.DiffStaged()
		title = "git diff --staged"
	} else if len(args) > 0 && !strings.HasPrefix(args[0], "-") {
		diff, err = ctx.Git.Diff(args[0], "HEAD")
		title = fmt.Sprintf("git diff %s..HEAD", args[0])
	} else {
		diff, err = ctx.Git.Diff("", "")
		title = "git diff"
	}

	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to get diff: %v", err)}
	}

	if diff == "" {
		if len(args) > 0 && args[0] == "--staged" {
			return CommandResult{Success: true, Message: "No staged changes."}
		}
		return CommandResult{Success: true, Message: "No uncommitted changes."}
	}

	screen := ScreenDiff
	return CommandResult{
		Success: true,
		Screen:  &screen,
		Message: title,
		Cmd: func() tea.Msg {
			return DiffScreenMsg{
				Diff:  diff,
				Title: title,
			}
		},
	}
}

// handleLog shows recent log entries.
func handleLog(args []string, ctx CommandContext) CommandResult {
	home, err := os.UserHomeDir()
	if err != nil {
		return CommandResult{Success: false, Message: "Cannot determine home directory for log path."}
	}
	logPath := filepath.Join(home, ".m31a", "m31a.log")

	entries, err := os.ReadFile(logPath)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Cannot read log file: %v", err)}
	}

	lines := strings.Split(string(entries), "\n")
	n := 20
	if len(args) > 0 {
		if count, err := strconv.Atoi(args[0]); err == nil && count > 0 {
			n = count
		}
	}

	if len(lines) > n {
		lines = lines[len(lines)-n:]
	}

	var filtered []string
	for _, line := range lines {
		if strings.TrimSpace(line) != "" {
			filtered = append(filtered, line)
		}
	}

	if len(filtered) == 0 {
		return CommandResult{Success: true, Message: "No log entries found."}
	}

	return CommandResult{Success: true, Message: strings.Join(filtered, "\n")}
}
