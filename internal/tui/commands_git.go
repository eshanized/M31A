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
		Success:         true,
		ConfirmRequired: true,
		ConfirmPrompt:   fmt.Sprintf("Soft reset to commit %s? This cannot be undone.", hash),
		Message:         fmt.Sprintf("Rolling back to commit **%s**...", hash),
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

		screen := ScreenBisect
		return CommandResult{
			Success: true,
			Screen:  &screen,
			Message: fmt.Sprintf("Starting bisect: good=%s, bad=%s", goodRef, badRef),
			Cmd: func() tea.Msg {
				return BisectStartMsg{
					GoodCommit: goodRef,
					BadCommit:  badRef,
				}
			},
		}
	}

	// /bisect with no args: show last 20 commits in the bisect screen
	screen := ScreenBisect
	return CommandResult{
		Success: true,
		Screen:  &screen,
		Message: "Opening bisect screen with recent commits...",
		Cmd: func() tea.Msg {
			commits, err := ctx.Git.Log(20)
			if err != nil || len(commits) == 0 {
				return ToastMsg{
					Text: fmt.Sprintf("Could not load commits: %v", err),
					Type: "error",
				}
			}
			return BisectStartMsg{
				BadCommit:  commits[0].Hash,
				GoodCommit: commits[len(commits)-1].Hash,
			}
		},
	}
}
