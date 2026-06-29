package commands

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
)

// handleDiff shows the current git diff.
func handleDiff(args []string, ctx CommandContext) CommandResult {
	if ctx.Git == nil {
		return CommandResult{Success: false, Message: "Git not available in this context."}
	}

	if !ctx.Git.IsRepo() {
		return CommandResult{Success: false, Message: "Not inside a git repository."}
	}

	return CommandResult{
		Success: true,
		Message: "Loading git diff...",
		Cmd: func() tea.Msg {
			var diff string
			var err error

			if len(args) > 0 {
				// diff between two refs
				if len(args) >= 2 {
					diff, err = ctx.Git.Diff(args[0], args[1])
				} else {
					diff, err = ctx.Git.Diff(args[0], "HEAD")
				}
			} else {
				// unstaged working-tree diff
				diff, err = ctx.Git.Diff("", "")
				if diff == "" && err == nil {
					// fallback: staged changes
					diff, err = ctx.Git.DiffStaged()
				}
			}

			if err != nil {
				return tuitypes.ToastMsg{
					Text: fmt.Sprintf("git diff failed: %v", err),
					Type: "error",
				}
			}

			if strings.TrimSpace(diff) == "" {
				return tuitypes.ToastMsg{
					Text: "No changes in working tree.",
					Type: "info",
				}
			}

			lines := strings.Split(diff, "\n")
			return tuitypes.DiffScreenMsg{
				Diff:  diff,
				Title: "git diff",
				Lines: lines,
			}
		},
	}
}

// handleRollback opens the rollback (commit time machine) screen or performs a reset.
func handleRollback(args []string, ctx CommandContext) CommandResult {
	if ctx.Rollback == nil {
		return CommandResult{Success: false, Message: "Rollback not available in this context."}
	}

	if len(args) == 0 {
		// Open rollback browser screen
		screen := tuitypes.ScreenRollback
		return CommandResult{
			Success: true,
			Screen:  &screen,
			Message: "Opening commit time machine...",
		}
	}

	// /rollback file <hash> [path...] — revert specific files from a commit
	if args[0] == "file" {
		return handleRollbackFile(args[1:], ctx)
	}

	// Perform soft reset to the given commit hash
	hash := args[0]
	return CommandResult{
		Success:         true,
		ConfirmRequired: true,
		ConfirmPrompt:   fmt.Sprintf("Soft reset to commit %s? This cannot be undone.", hash),
		Message:         fmt.Sprintf("Rolling back to commit **%s**...", hash),
		Cmd: func() tea.Msg {
			result, err := ctx.Rollback.SoftReset(hash, nil)
			if err != nil {
				return tuitypes.ToastMsg{
					Text: fmt.Sprintf("Rollback failed: %v", err),
					Type: "error",
				}
			}
			return tuitypes.ToastMsg{
				Text: result.Message,
				Type: "success",
			}
		},
	}
}

// handleRollbackFile reverts specific files from a commit.
// Usage: /rollback file <hash> [path...]
func handleRollbackFile(args []string, ctx CommandContext) CommandResult {
	if ctx.Rollback == nil {
		return CommandResult{Success: false, Message: "Rollback not available in this context."}
	}

	if len(args) < 1 {
		return CommandResult{
			Success: false,
			Message: "Usage: /rollback file <commit-hash> [file-path...]",
		}
	}

	hash := args[0]
	paths := args[1:]

	// If no paths specified, list changed files in the commit
	if len(paths) == 0 {
		files, err := ctx.Rollback.ChangedFiles(hash)
		if err != nil {
			return CommandResult{
				Success: false,
				Message: fmt.Sprintf("Failed to list changed files: %v", err),
			}
		}
		if len(files) == 0 {
			return CommandResult{
				Success: false,
				Message: fmt.Sprintf("No files changed in commit %s", hash[:min(7, len(hash))]),
			}
		}
		var b strings.Builder
		fmt.Fprintf(&b, "Files changed in %s:\n\n", hash[:min(7, len(hash))])
		for _, f := range files {
			fmt.Fprintf(&b, "  %s\n", f)
		}
		b.WriteString("\nUsage: /rollback file <hash> <file-path> [file-path...]")
		return CommandResult{Success: true, Message: b.String()}
	}

	// Revert the specified files
	return CommandResult{
		Success:         true,
		ConfirmRequired: true,
		ConfirmPrompt:   fmt.Sprintf("Revert %d file(s) from commit %s?", len(paths), hash[:min(7, len(hash))]),
		Message:         fmt.Sprintf("Reverting files from commit **%s**...", hash[:min(7, len(hash))]),
		Cmd: func() tea.Msg {
			if err := ctx.Rollback.RevertFiles(hash, paths); err != nil {
				return tuitypes.ToastMsg{
					Text: fmt.Sprintf("File revert failed: %v", err),
					Type: "error",
				}
			}
			return tuitypes.ToastMsg{
				Text: fmt.Sprintf("Reverted %d file(s) from commit %s", len(paths), hash[:min(7, len(hash))]),
				Type: "success",
			}
		},
	}
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

// handleBisect provides git bisect information or starts interactive bisect.
func handleBisect(args []string, ctx CommandContext) CommandResult {
	if ctx.Git == nil {
		return CommandResult{Success: false, Message: "Git not available in this context."}
	}
	if !ctx.Git.IsRepo() {
		return CommandResult{Success: false, Message: "Not inside a git repository."}
	}

	// /bisect with args: start <good> <bad>
	if len(args) >= 2 {
		goodRef := args[0]
		badRef := args[1]

		screen := tuitypes.ScreenBisect
		return CommandResult{
			Success: true,
			Screen:  &screen,
			Message: fmt.Sprintf("Starting bisect: good=%s, bad=%s", goodRef, badRef),
			Cmd: func() tea.Msg {
				return tuitypes.BisectStartMsg{
					GoodCommit: goodRef,
					BadCommit:  badRef,
				}
			},
		}
	}

	// /bisect with no args: show last 20 commits in the bisect screen
	screen := tuitypes.ScreenBisect
	return CommandResult{
		Success: true,
		Screen:  &screen,
		Message: "Opening bisect screen with recent commits...",
		Cmd: func() tea.Msg {
			commits, err := ctx.Git.Log(20)
			if err != nil || len(commits) == 0 {
				return tuitypes.ToastMsg{
					Text: fmt.Sprintf("Could not load commits: %v", err),
					Type: "error",
				}
			}
			return tuitypes.BisectStartMsg{
				BadCommit:  commits[0].Hash,
				GoodCommit: commits[len(commits)-1].Hash,
			}
		},
	}
}
