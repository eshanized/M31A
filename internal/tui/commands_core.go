package tui

import (
	"context"
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
)

// handleHelp lists all registered commands with their descriptions, grouped by category.
func handleHelp(args []string, ctx CommandContext) CommandResult {
	// Per-command help
	if len(args) > 0 {
		cmd := strings.TrimPrefix(args[0], "/")
		r := DefaultCommands()
		if desc, ok := r.descriptions[cmd]; ok {
			return CommandResult{Success: true, Message: fmt.Sprintf("/%s — %s", cmd, desc)}
		}
		return CommandResult{Success: true, Message: fmt.Sprintf("No additional help available for /%s", cmd)}
	}

	r := DefaultCommands()

	categories := []struct {
		name    string
		commands []string
	}{
		{"Session", []string{"sessions", "fork", "prev", "next", "save", "clear", "undo"}},
		{"Workflow", []string{"workflow", "plan", "execute", "verify", "ship", "goal", "phase", "pause", "resume-task"}},
		{"Config", []string{"settings", "provider", "model", "models", "fallback", "theme", "key", "config"}},
		{"Git", []string{"rollback", "diff", "log", "ledger"}},
		{"AI", []string{"compress", "optimize", "cost", "tokens"}},
		{"System", []string{"help", "status", "health", "tools", "history", "quit", "reset"}},
	}

	var b strings.Builder
	for _, cat := range categories {
		b.WriteString(fmt.Sprintf("\n%s:\n", cat.name))
		for _, name := range cat.commands {
			if desc, ok := r.descriptions[name]; ok {
				b.WriteString(fmt.Sprintf("  /%-18s %s\n", name, desc))
			}
		}
	}

	b.WriteString("\nShell Mode:\n")
	b.WriteString("  !<command>       Run a shell command directly (e.g., !ls -la)\n")

	b.WriteString("\nNote: command chaining with ';' is not supported. Use each command separately.")
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleSettings switches to the settings screen.
func handleSettings(args []string, ctx CommandContext) CommandResult {
	screen := ScreenSettings
	return CommandResult{Success: true, Screen: &screen}
}

// handleClear clears the conversation context (message history).
func handleClear(args []string, ctx CommandContext) CommandResult {
	if ctx.ClearMessages != nil {
		ctx.ClearMessages()
		return CommandResult{Success: true, Message: "Context cleared."}
	}
	return CommandResult{Success: true, Message: "No conversation to clear."}
}

// handleStatus returns current session information.
func handleStatus(args []string, ctx CommandContext) CommandResult {
	var b strings.Builder
	b.WriteString("Session Info:\n")

	if ctx.SessionID != "" {
		b.WriteString(fmt.Sprintf("  Session ID:  %s\n", ctx.SessionID))
	}

	if ctx.Registry != nil {
		active := ctx.Registry.Active()
		b.WriteString(fmt.Sprintf("  Provider:    %s\n", active))

		if ap := ctx.Registry.ActiveProvider(); ap != nil {
			models, err := ap.FetchModels(context.Background())
			if err == nil && len(models) > 0 {
				b.WriteString(fmt.Sprintf("  Models:      %d cached\n", len(models)))
			}
		}
	}

	if ctx.SessionManager != nil && ctx.SessionID != "" {
		s, err := ctx.SessionManager.LoadSession(ctx.SessionID)
		if err == nil {
			b.WriteString(fmt.Sprintf("  Phase:       %s\n", s.WorkflowPhase))
			b.WriteString(fmt.Sprintf("  Messages:    %d\n", s.MessageCount))
		}
	}

	if ctx.Config != nil {
		b.WriteString(fmt.Sprintf("  Model:       %s\n", ctx.Config.Model.Default))
	}

	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleQuit returns a goodbye message.
func handleQuit(args []string, ctx CommandContext) CommandResult {
	return CommandResult{Success: true, Message: "Goodbye!"}
}

// handleTheme switches between dark and light themes.
func handleTheme(args []string, ctx CommandContext) CommandResult {
	if ctx.Config == nil {
		return CommandResult{Success: false, Message: "Config not available."}
	}

	if len(args) == 0 {
		return CommandResult{Success: true, Message: fmt.Sprintf("Current theme: %s. Use /theme dark or /theme light to switch.", ctx.Config.UI.Theme)}
	}

	switch args[0] {
	case "dark", "light":
		ctx.Config.UI.Theme = args[0]
		ctx.Config.Save(ctx.ConfigPath)
		return CommandResult{Success: true, Message: fmt.Sprintf("Theme switched to %s.", args[0]), Cmd: func() tea.Msg {
			return ThemeChangedMsg{Theme: args[0]}
		}}
	default:
		return CommandResult{Success: false, Message: "Invalid theme. Use 'dark' or 'light'."}
	}
}
