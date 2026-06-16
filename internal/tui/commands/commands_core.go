package commands

import (
	"fmt"
	"runtime"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
)

// handleHelp navigates to the Help screen with scrollable keybinding reference.
func handleHelp(_ []string, _ CommandContext) CommandResult {
	screen := tuitypes.ScreenHelp
	return CommandResult{Success: true, Screen: &screen}
}

// handleClear clears the current conversation messages.
func handleClear(_ []string, ctx CommandContext) CommandResult {
	return CommandResult{
		Success:         true,
		ConfirmRequired: true,
		ConfirmPrompt:   "Clear entire conversation? This cannot be undone.",
		Cmd: func() tea.Msg {
			if ctx.ClearMessages != nil {
				ctx.ClearMessages()
			}
			return tuitypes.ToastMsg{Text: "Conversation cleared", Duration: 3 * time.Second, Type: "success"}
		},
	}
}

// handleStatus shows the current session information.
func handleStatus(_ []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session."}
	}
	sess, err := ctx.SessionManager.LoadSession(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: "Failed to load session."}
	}

	provider := "unknown"
	model := "unknown"
	phase := "idle"
	msgCount := 0
	if ctx.Config != nil {
		provider = ctx.Config.Provider.Default
		model = ctx.Config.Model.Default
	}
	if sess != nil {
		if sess.Provider != "" {
			provider = sess.Provider
		}
		if sess.Model != "" {
			model = sess.Model
		}
		phase = string(sess.WorkflowPhase)
		msgCount = sess.MessageCount
	}

	msg := fmt.Sprintf(
		"**Session:** %s\n**Provider:** %s\n**Model:** %s\n**Phase:** %s\n**Messages:** %d",
		ctx.SessionID, provider, model, phase, msgCount,
	)
	return CommandResult{Success: true, Message: msg}
}

// handleReset navigates to the first-run screen, resetting the UI to initial state.
func handleReset(_ []string, _ CommandContext) CommandResult {
	screen := tuitypes.ScreenFirstRun
	return CommandResult{
		Success:         true,
		ConfirmRequired: true,
		ConfirmPrompt:   "Reset to first-run screen? Current session state will be lost.",
		Screen:          &screen,
	}
}

// handleQuit exits the application.
func handleQuit(_ []string, _ CommandContext) CommandResult {
	return CommandResult{
		Success: true,
		Message: "Goodbye!",
		Cmd: func() tea.Msg {
			return tea.QuitMsg{}
		},
	}
}

// handleUndo shows the latest checkpoint info for the current session.
func handleUndo(_ []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session."}
	}
	checkpoint, err := ctx.SessionManager.LatestCheckpoint(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: "No checkpoint found."}
	}
	msg := fmt.Sprintf(
		"**Latest checkpoint:**\n  Phase: %s\n  Time: %s",
		checkpoint.Phase,
		checkpoint.Timestamp.Format("2006-01-02 15:04:05"),
	)
	return CommandResult{Success: true, Message: msg}
}

// handleHistory opens the chat history table browser.
func handleHistory(_ []string, _ CommandContext) CommandResult {
	screen := tuitypes.ScreenChatHistory
	return CommandResult{
		Success: true,
		Screen:  &screen,
		Message: "Opening chat history...",
	}
}

// handlePromptHistory shows recent prompt history from the frecency tracker.
func handlePromptHistory(_ []string, ctx CommandContext) CommandResult {
	if ctx.FrecentHistory == nil {
		return CommandResult{Success: false, Message: "Prompt history not available."}
	}
	entries := ctx.FrecentHistory.Search("", 20)
	if len(entries) == 0 {
		return CommandResult{Success: true, Message: "No prompt history yet."}
	}
	var sb strings.Builder
	fmt.Fprintf(&sb, "**Prompt history** (%d entries):\n\n", len(entries))
	for i, e := range entries {
		text := e.Text
		if len(text) > 60 {
			text = text[:57] + "..."
		}
		// Replace newlines for single-line display
		text = strings.ReplaceAll(text, "\n", " ")
		fmt.Fprintf(&sb, "  %2d. %s\n", i+1, text)
	}
	return CommandResult{Success: true, Message: sb.String()}
}

// handleHealth shows the system health status.
func handleHealth(_ []string, ctx CommandContext) CommandResult {
	if ctx.Registry == nil {
		return CommandResult{Success: false, Message: "Provider registry not available."}
	}
	active := ctx.Registry.Active()
	p, err := ctx.Registry.Get(active)
	if err != nil || p == nil {
		return CommandResult{Success: false, Message: "No active provider to check health."}
	}
	return CommandResult{
		Success: true,
		Message: fmt.Sprintf("Running health check for provider **%s**...", active),
		Cmd: func() tea.Msg {
			return tuitypes.HealthCheckTickMsg{}
		},
	}
}

// handleTools lists all registered tools in the dispatcher.
func handleTools(_ []string, ctx CommandContext) CommandResult {
	if ctx.Dispatcher == nil {
		return CommandResult{Success: false, Message: "Tool dispatcher not available."}
	}
	names := ctx.Dispatcher.List()
	if len(names) == 0 {
		return CommandResult{Success: true, Message: "No tools registered."}
	}
	var sb strings.Builder
	sb.WriteString("**Available tools:**\n\n")
	for _, name := range names {
		tool, ok := ctx.Dispatcher.GetTool(name)
		if ok {
			fmt.Fprintf(&sb, "  %-20s — %s\n", name, tool.Description())
		} else {
			fmt.Fprintf(&sb, "  %s\n", name)
		}
	}
	return CommandResult{Success: true, Message: sb.String()}
}

// handleCopyError copies the last error message to the clipboard.
func handleCopyError(_ []string, ctx CommandContext) CommandResult {
	if ctx.CopyError != nil {
		return CommandResult{
			Success: true,
			Cmd:     ctx.CopyError(),
		}
	}
	return CommandResult{Success: false, Message: "Copy error not available."}
}

// handleExit is an alias for /quit.
func handleExit(_ []string, _ CommandContext) CommandResult {
	return handleQuit(nil, CommandContext{})
}

// handleChat starts a new plain chat session by clearing messages and cancelling
// any running agent, without entering workflow mode.
func handleChat(_ []string, ctx CommandContext) CommandResult {
	return CommandResult{
		Success:         true,
		ConfirmRequired: true,
		ConfirmPrompt:   "Start a new chat session? Current messages will be cleared.",
		Cmd: func() tea.Msg {
			if ctx.CancelAgent != nil {
				ctx.CancelAgent()
			}
			if ctx.ClearMessages != nil {
				ctx.ClearMessages()
			}
			return tuitypes.ToastMsg{Text: "New chat session started", Duration: 3 * time.Second, Type: "success"}
		},
	}
}

// handleFlush clears the screen and resets the view to the REPL without
// clearing conversation messages.
func handleFlush(_ []string, _ CommandContext) CommandResult {
	screen := tuitypes.ScreenREPL
	return CommandResult{
		Success: true,
		Screen:  &screen,
		Message: "Screen flushed.",
	}
}

// handleSearch searches conversation messages for a query string and returns
// matching messages with their role and a content snippet.
func handleSearch(args []string, ctx CommandContext) CommandResult {
	if len(args) == 0 {
		return CommandResult{Success: false, Message: "Usage: `/search <query>`"}
	}
	query := strings.ToLower(strings.Join(args, " "))

	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session."}
	}
	sess, err := ctx.SessionManager.LoadSession(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: "Failed to load session."}
	}

	var sb strings.Builder
	matches := 0
	for _, msg := range sess.Messages {
		if strings.Contains(strings.ToLower(msg.Content), query) {
			matches++
			snippet := msg.Content
			if len(snippet) > 120 {
				snippet = snippet[:117] + "..."
			}
			snippet = strings.ReplaceAll(snippet, "\n", " ")
			fmt.Fprintf(&sb, "  **%s:** %s\n", msg.Role, snippet)
		}
	}

	if matches == 0 {
		return CommandResult{Success: true, Message: fmt.Sprintf("No messages matching %q.", query)}
	}
	return CommandResult{
		Success: true,
		Message: fmt.Sprintf("**%d match(es)** for %q:\n\n%s", matches, query, sb.String()),
	}
}

// handleAbout shows version and system information.
func handleAbout(_ []string, ctx CommandContext) CommandResult {
	version := ctx.Version
	if version == "" {
		version = "dev"
	}

	provider := "not configured"
	model := "not configured"
	if ctx.Config != nil {
		if ctx.Config.Provider.Default != "" {
			provider = ctx.Config.Provider.Default
		}
		if ctx.Config.Model.Default != "" {
			model = ctx.Config.Model.Default
		}
	}

	msgCount := 0
	if ctx.SessionManager != nil && ctx.SessionID != "" {
		if sess, err := ctx.SessionManager.LoadSession(ctx.SessionID); err == nil && sess != nil {
			msgCount = sess.MessageCount
		}
	}

	msg := fmt.Sprintf(
		"**M31A** v%s\n\n"+
			"**Provider:** %s\n"+
			"**Model:** %s\n"+
			"**Session:** %s (%d messages)\n"+
			"**Platform:** %s/%s",
		version, provider, model, ctx.SessionID, msgCount,
		runtime.GOOS, runtime.GOARCH,
	)
	return CommandResult{Success: true, Message: msg}
}
