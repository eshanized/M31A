package tui

import (
	"fmt"
	"strconv"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
)

// handleUndo loads the latest checkpoint and reports its details.
func handleUndo(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session. Start a session first."}
	}

	cp, err := ctx.SessionManager.LatestCheckpoint(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("No checkpoint available: %v", err)}
	}

	return CommandResult{
		Success: true,
		Message: fmt.Sprintf("Latest checkpoint: phase=%s, timestamp=%s, messages=%d, tasks=%d",
			cp.Phase, cp.Timestamp.Format("2006-01-02 15:04:05"), cp.MessageCount, cp.TaskCount),
	}
}

// handleFork creates a child session copying the current session's messages.
func handleFork(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{
			Success: false,
			Message: "No active session. Use /fork from within an active session.",
		}
	}

	child, err := ctx.SessionManager.ForkSession(ctx.SessionID)
	if err != nil {
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Fork failed: %v", err),
		}
	}

	newID := child.ID
	return CommandResult{
		Success:   true,
		SessionID: &newID,
		Message:   fmt.Sprintf("Session forked: %s (child of %s). Use /prev or /next to navigate siblings.", newID, ctx.SessionID),
	}
}

// handlePrev switches to the previous sibling session in the fork tree.
func handlePrev(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{
			Success: false,
			Message: "No active session.",
		}
	}

	siblings, idx, err := ctx.SessionManager.SiblingSessions(ctx.SessionID)
	if err != nil {
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Sibling lookup failed: %v", err),
		}
	}

	if len(siblings) == 0 || idx <= 0 {
		return CommandResult{
			Success: false,
			Message: "Already at first sibling.",
		}
	}

	prevID := siblings[idx-1].ID
	return CommandResult{
		Success:   true,
		SessionID: &prevID,
		Message:   fmt.Sprintf("Switched to sibling: %s", prevID),
	}
}

// handleNext switches to the next sibling session in the fork tree.
func handleNext(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{
			Success: false,
			Message: "No active session.",
		}
	}

	siblings, idx, err := ctx.SessionManager.SiblingSessions(ctx.SessionID)
	if err != nil {
		return CommandResult{
			Success: false,
			Message: fmt.Sprintf("Sibling lookup failed: %v", err),
		}
	}

	if idx < 0 || idx >= len(siblings)-1 {
		return CommandResult{
			Success: false,
			Message: "Already at last sibling.",
		}
	}

	nextID := siblings[idx+1].ID
	return CommandResult{
		Success:   true,
		SessionID: &nextID,
		Message:   fmt.Sprintf("Switched to sibling: %s", nextID),
	}
}

// handleHistory shows the conversation message history.
func handleHistory(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session."}
	}

	s, err := ctx.SessionManager.LoadSession(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to load session: %v", err)}
	}

	limit := 10
	if len(args) > 0 {
		if n, err := strconv.Atoi(args[0]); err == nil && n > 0 {
			limit = n
		}
	}

	msgs := s.Messages
	if len(msgs) == 0 {
		return CommandResult{Success: true, Message: "No messages in this session."}
	}

	if len(msgs) > limit {
		msgs = msgs[len(msgs)-limit:]
	}

	var b strings.Builder
	b.WriteString(fmt.Sprintf("Recent messages (last %d of %d):\n", len(msgs), len(s.Messages)))
	for i, m := range msgs {
		preview := m.Content
		if len(preview) > 80 {
			preview = preview[:77] + "..."
		}
		b.WriteString(fmt.Sprintf("  %d. [%s] %s\n", i+1, m.Role, preview))
	}
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleSessions lists recent sessions from the session manager.
func handleSessions(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil {
		return CommandResult{Success: false, Message: "Session manager not available."}
	}

	sessions, err := ctx.SessionManager.ListSessions()
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to list sessions: %v", err)}
	}

	if len(sessions) == 0 {
		return CommandResult{Success: true, Message: "No sessions available."}
	}

	var b strings.Builder
	b.WriteString("Sessions:\n")
	for i, s := range sessions {
		corrupt := ""
		if s.Corrupted {
			corrupt = " [!CORRUPT]"
		}
		b.WriteString(fmt.Sprintf("  %d. %s | %s/%s | %d msgs%s\n",
			i+1, s.ID, s.Provider, s.Model, s.MessageCount, corrupt))
	}
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleSave saves the current session and conversation state.
func handleSave(args []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session to save."}
	}

	if err := ctx.SessionManager.SaveState(ctx.SessionID, "", "manual save", "user requested /save"); err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to save session: %v", err)}
	}

	return CommandResult{
		Success: true,
		Message: "Session saved successfully.",
		Cmd:     func() tea.Msg { return SettingsSavedMsg{} },
	}
}
