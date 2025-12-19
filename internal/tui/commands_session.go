package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
)

// handleSessions lists recent sessions.
func handleSessions(_ []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil {
		return CommandResult{Success: false, Message: "Session manager not available."}
	}

	sessions, err := ctx.SessionManager.ListSessions()
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to list sessions: %v", err)}
	}

	limit := 10
	if ctx.Config != nil && ctx.Config.UI.SessionListLimit > 0 {
		limit = ctx.Config.UI.SessionListLimit
	}
	if len(sessions) > limit {
		sessions = sessions[:limit]
	}

	if len(sessions) == 0 {
		return CommandResult{Success: true, Message: "No sessions found."}
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("**Recent sessions** (showing %d):\n\n", len(sessions)))
	for _, s := range sessions {
		active := " "
		if s.ID == ctx.SessionID {
			active = "▶"
		}
		corrupted := ""
		if s.Corrupted {
			corrupted = " [corrupted]"
		}
		sb.WriteString(fmt.Sprintf(
			"  %s %s  %-10s  %s  %d msgs%s\n",
			active,
			s.ID,
			s.Provider,
			s.LastModified.Format("Jan 02 15:04"),
			s.MessageCount,
			corrupted,
		))
	}
	return CommandResult{Success: true, Message: sb.String()}
}

// handleFork forks the current session into a child session.
func handleFork(_ []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session to fork."}
	}

	child, err := ctx.SessionManager.ForkSession(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Fork failed: %v", err)}
	}

	id := child.ID
	return CommandResult{
		Success:   true,
		Message:   fmt.Sprintf("Forked session **%s** → **%s**. Switching to new session.", ctx.SessionID, id),
		SessionID: &id,
	}
}

// handlePrev switches to the previous sibling session.
func handlePrev(_ []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session."}
	}

	siblings, idx, err := ctx.SessionManager.SiblingSessions(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to list siblings: %v", err)}
	}
	if siblings == nil || idx <= 0 {
		return CommandResult{Success: false, Message: "No previous sibling session."}
	}

	prevID := siblings[idx-1].ID
	return CommandResult{
		Success:   true,
		Message:   fmt.Sprintf("Switching to previous session **%s**.", prevID),
		SessionID: &prevID,
	}
}

// handleNext switches to the next sibling session.
func handleNext(_ []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session."}
	}

	siblings, idx, err := ctx.SessionManager.SiblingSessions(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to list siblings: %v", err)}
	}
	if siblings == nil || idx < 0 || idx >= len(siblings)-1 {
		return CommandResult{Success: false, Message: "No next sibling session."}
	}

	nextID := siblings[idx+1].ID
	return CommandResult{
		Success:   true,
		Message:   fmt.Sprintf("Switching to next session **%s**.", nextID),
		SessionID: &nextID,
	}
}

// handleSave saves the current session to disk.
func handleSave(_ []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session to save."}
	}

	sess, err := ctx.SessionManager.LoadSession(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to load session for saving: %v", err)}
	}

	if err := ctx.SessionManager.SaveSession(sess); err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Save failed: %v", err)}
	}

	return CommandResult{
		Success: true,
		Message: fmt.Sprintf("Session **%s** saved (%d messages).", ctx.SessionID, sess.MessageCount),
	}
}

// handleGoal sets or shows the current session goal.
func handleGoal(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session."}
	}

	if len(args) == 0 {
		// Show current goal
		goal, _, _, err := ctx.SessionManager.LoadWorkflowState(ctx.SessionID)
		if err != nil {
			return CommandResult{Success: false, Message: fmt.Sprintf("Failed to load workflow state: %v", err)}
		}
		if goal == "" {
			// Open goal input screen
			screen := ScreenGoalInput
			return CommandResult{
				Success: true,
				Screen:  &screen,
				Message: "No goal set. Opening goal input...",
			}
		}
		return CommandResult{Success: true, Message: fmt.Sprintf("**Current goal:** %s", goal)}
	}

	// Set goal from args
	newGoal := strings.Join(args, " ")
	return CommandResult{
		Success:        true,
		Message:        fmt.Sprintf("Goal set: **%s**", newGoal),
		WorkflowResume: false,
		Cmd: func() tea.Msg {
			return GoalSubmittedMsg{Goal: newGoal}
		},
	}
}

// handleResume opens the session browser screen.
func handleResume(_ []string, _ CommandContext) CommandResult {
	screen := ScreenResume
	return CommandResult{
		Success: true,
		Screen:  &screen,
		Message: "Opening session browser...",
	}
}

// handleLedger opens the learning ledger screen or shows summary stats.
func handleLedger(args []string, ctx CommandContext) CommandResult {
	if ctx.Ledger == nil {
		return CommandResult{Success: false, Message: "Ledger not available."}
	}

	if len(args) == 0 {
		// Open ledger screen
		screen := ScreenLedger
		return CommandResult{
			Success: true,
			Screen:  &screen,
			Message: "Opening learning ledger...",
		}
	}

	// Show stats summary
	stats := ctx.Ledger.Stats()
	return CommandResult{
		Success: true,
		Message: fmt.Sprintf(
			"**Ledger stats:**\n  Sessions: %d\n  Avg tasks: %.1f\n  Avg cost: $%.4f\n  Avg duration: %.0f min\n  Failed tasks: %d",
			stats.TotalSessions,
			stats.AvgTaskCount,
			stats.AvgCost,
			stats.AvgDurationMinutes,
			stats.TotalFailedTasks,
		),
	}
}

// handleExport exports the current session to a file.
// Usage: /export [markdown|json] [path]
func handleExport(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session to export."}
	}

	format := "markdown"
	path := ctx.SessionID + ".md"
	if len(args) > 0 {
		format = strings.ToLower(args[0])
	}
	if len(args) > 1 {
		path = args[1]
	}

	var err error
	switch format {
	case "json":
		if !strings.HasSuffix(path, ".json") {
			path = ctx.SessionID + ".json"
		}
		err = ctx.SessionManager.ExportSessionJSON(ctx.SessionID, path)
	case "markdown", "md":
		err = ctx.SessionManager.ExportSessionMarkdown(ctx.SessionID, path)
	default:
		return CommandResult{Success: false, Message: "Unknown format. Use `markdown` or `json`."}
	}
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Export failed: %v", err)}
	}
	return CommandResult{Success: true, Message: fmt.Sprintf("Session exported to **%s**.", path)}
}


