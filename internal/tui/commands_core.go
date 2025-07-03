package tui

import (
	"context"
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
)

// handleHelp lists all registered commands with their descriptions.
func handleHelp(args []string, ctx CommandContext) CommandResult {
	r := DefaultCommands()
	names := r.List()

	var b strings.Builder
	b.WriteString("Available commands:\n")
	for _, name := range names {
		desc := r.descriptions[name]
		b.WriteString(fmt.Sprintf("  /%s — %s\n", name, desc))
	}
	b.WriteString("\nNote: command chaining with ';' is not supported. Use each command separately.")
	return CommandResult{Success: true, Message: strings.TrimRight(b.String(), "\n")}
}

// handleSettings switches to the settings screen.
func handleSettings(args []string, ctx CommandContext) CommandResult {
	screen := ScreenSettings
	return CommandResult{Success: true, Screen: &screen}
}

// handleClear returns a confirmation that the context was cleared.
func handleClear(args []string, ctx CommandContext) CommandResult {
	return CommandResult{Success: true, Message: "Context cleared."}
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
