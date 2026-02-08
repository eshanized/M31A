package tui

import (
	"fmt"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
)

// handleHelp navigates to the Help screen with scrollable keybinding reference.
func handleHelp(_ []string, _ CommandContext) CommandResult {
	screen := ScreenHelp
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
			return ToastMsg{Text: "Conversation cleared", Duration: 3 * time.Second, Type: "success"}
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
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to load session: %v", err)}
	}

	provider := "unknown"
	model := "unknown"
	phase := "idle"
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
	}

	msg := fmt.Sprintf(
		"**Session:** %s\n**Provider:** %s\n**Model:** %s\n**Phase:** %s\n**Messages:** %d",
		ctx.SessionID, provider, model, phase, sess.MessageCount,
	)
	return CommandResult{Success: true, Message: msg}
}

// handleReset navigates to the first-run screen, resetting the UI to initial state.
func handleReset(_ []string, _ CommandContext) CommandResult {
	screen := ScreenFirstRun
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
		return CommandResult{Success: false, Message: fmt.Sprintf("No checkpoint found: %v", err)}
	}
	msg := fmt.Sprintf(
		"**Latest checkpoint:**\n  Phase: %s\n  Time: %s",
		checkpoint.Phase,
		checkpoint.Timestamp.Format("2006-01-02 15:04:05"),
	)
	return CommandResult{Success: true, Message: msg}
}

// handleHistory shows conversation history entry count.
func handleHistory(_ []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session."}
	}
	sess, err := ctx.SessionManager.LoadSession(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to load session: %v", err)}
	}
	count := 0
	if sess != nil {
		count = sess.MessageCount
	}
	return CommandResult{
		Success: true,
		Message: fmt.Sprintf("Session **%s** has **%d** messages in history.", ctx.SessionID, count),
	}
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
			return HealthCheckTickMsg{}
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
			sb.WriteString(fmt.Sprintf("  %-20s — %s\n", name, tool.Description()))
		} else {
			sb.WriteString(fmt.Sprintf("  %s\n", name))
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
