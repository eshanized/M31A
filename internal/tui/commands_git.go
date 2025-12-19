package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
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
				return ToastMsg{
					Text: fmt.Sprintf("git diff failed: %v", err),
					Type: "error",
				}
			}

			if strings.TrimSpace(diff) == "" {
				return ToastMsg{
					Text: "No changes in working tree.",
					Type: "info",
				}
			}

			lines := strings.Split(diff, "\n")
			return DiffScreenMsg{
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
		screen := ScreenRollback
		return CommandResult{
			Success: true,
			Screen:  &screen,
			Message: "Opening commit time machine...",
		}
	}

	// Perform soft reset to the given commit hash
	hash := args[0]
	return CommandResult{
		Success: true,
		Message: fmt.Sprintf("Rolling back to commit **%s**...", hash),
		Cmd: func() tea.Msg {
			result, err := ctx.Rollback.SoftReset(hash, nil)
			if err != nil {
				return ToastMsg{
					Text: fmt.Sprintf("Rollback failed: %v", err),
					Type: "error",
				}
			}
			return ToastMsg{
				Text: result.Message,
				Type: "success",
			}
		},
	}
}

// handleBisect provides git bisect information.
func handleBisect(_ []string, ctx CommandContext) CommandResult {
	if ctx.Git == nil {
		return CommandResult{Success: false, Message: "Git not available in this context."}
	}
	if !ctx.Git.IsRepo() {
		return CommandResult{Success: false, Message: "Not inside a git repository."}
	}
	return CommandResult{
		Success: true,
		Message: "**Git bisect** requires a known good and bad commit.\n\nUsage:\n  `git bisect start`\n  `git bisect bad HEAD`\n  `git bisect good <good-commit>`\n\nThen run your test and mark each commit as `good` or `bad`.",
	}
}
