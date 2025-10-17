package tui

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
)

// diskUsageFormatted returns a human-friendly disk usage string for the given path.
func diskUsageFormatted(path string) string {
	n, err := diskUsageBytes(path)
	if err != nil {
		return "unknown"
	}
	switch {
	case n >= 1<<30:
		return fmt.Sprintf("%.1f GB", float64(n)/(1<<30))
	case n >= 1<<20:
		return fmt.Sprintf("%.1f MB", float64(n)/(1<<20))
	case n >= 1<<10:
		return fmt.Sprintf("%.1f KB", float64(n)/(1<<10))
	default:
		return fmt.Sprintf("%d B", n)
	}
}

// handleSettings opens the settings editor screen.
func handleSettings(_ []string, _ CommandContext) CommandResult {
	screen := ScreenSettings
	return CommandResult{Success: true, Screen: &screen, Message: "Opening settings..."}
}

// handleConfig shows or describes the current config state.
func handleConfig(args []string, ctx CommandContext) CommandResult {
	if ctx.Config == nil {
		return CommandResult{Success: false, Message: "Config not available."}
	}
	cfg := ctx.Config
	if len(args) == 0 {
		// Show summary
		theme := cfg.UI.Theme
		if theme == "" {
			theme = "auto"
		}
		msg := fmt.Sprintf(
			"**Config:** %s\n**Provider:** %s\n**Model:** %s\n**Theme:** %s\n**Session limit:** %d\n**Auto-fallback:** %v",
			ctx.ConfigPath,
			cfg.Provider.Default,
			cfg.Model.Default,
			theme,
			cfg.UI.SessionListLimit,
			cfg.Provider.AutoFallback,
		)
		return CommandResult{Success: true, Message: msg}
	}
	return CommandResult{Success: true, Message: fmt.Sprintf("Config path: %s", ctx.ConfigPath)}
}

// handleTheme switches between dark and light themes.
func handleTheme(args []string, ctx CommandContext) CommandResult {
	theme := "dark"
	if len(args) > 0 {
		t := strings.ToLower(args[0])
		if t == "dark" || t == "light" || t == "auto" {
			theme = t
		} else {
			return CommandResult{
				Success: false,
				Message: fmt.Sprintf("Unknown theme %q. Valid values: dark, light, auto.", args[0]),
			}
		}
	} else if ctx.Config != nil {
		// Toggle
		if ctx.Config.UI.Theme == "dark" {
			theme = "light"
		} else {
			theme = "dark"
		}
	}

	return CommandResult{
		Success: true,
		Message: fmt.Sprintf("Switching to **%s** theme.", theme),
		Cmd: func() tea.Msg {
			return ThemeChangedMsg{Theme: theme}
		},
	}
}

// handleCost toggles the cost estimate display.
func handleCost(_ []string, ctx CommandContext) CommandResult {
	if ctx.Config == nil {
		return CommandResult{Success: false, Message: "Config not available."}
	}
	current := ctx.Config.UI.ShowCostEstimate
	if current {
		return CommandResult{Success: true, Message: "Cost display is currently **enabled**. Use settings to toggle."}
	}
	return CommandResult{Success: true, Message: "Cost display is currently **disabled**. Use settings to toggle."}
}

// handleLog shows recent log entries from the m31a log file.
func handleLog(args []string, ctx CommandContext) CommandResult {
	lines := 20
	if ctx.Config != nil && ctx.Config.UI.DefaultLogLines > 0 {
		lines = ctx.Config.UI.DefaultLogLines
	}
	if len(args) > 0 {
		n := 0
		fmt.Sscanf(args[0], "%d", &n)
		if n > 0 {
			lines = n
		}
	}

	homeDir, err := os.UserHomeDir()
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Cannot determine home dir: %v", err)}
	}
	logPath := filepath.Join(homeDir, ".m31a", "m31a.log")
	data, err := os.ReadFile(logPath)
	if err != nil {
		if os.IsNotExist(err) {
			return CommandResult{Success: true, Message: "No log file found yet."}
		}
		return CommandResult{Success: false, Message: fmt.Sprintf("Cannot read log: %v", err)}
	}

	logLines := strings.Split(strings.TrimSpace(string(data)), "\n")
	if len(logLines) > lines {
		logLines = logLines[len(logLines)-lines:]
	}

	return CommandResult{
		Success: true,
		Message: fmt.Sprintf("**Last %d log lines:**\n```\n%s\n```", lines, strings.Join(logLines, "\n")),
	}
}

// handleKey shows the API key status for configured providers.
func handleKey(_ []string, ctx CommandContext) CommandResult {
	if ctx.Config == nil {
		return CommandResult{Success: false, Message: "Config not available."}
	}
	cfg := ctx.Config

	var sb strings.Builder
	sb.WriteString("**API Key Status:**\n\n")

	orKey := cfg.Provider.OpenRouter.APIKey
	if orKey == "" {
		sb.WriteString("  OpenRouter: ❌ not set\n")
	} else {
		masked := "sk-or-..." + orKey[max(0, len(orKey)-4):]
		sb.WriteString(fmt.Sprintf("  OpenRouter: ✅ set (%s)\n", masked))
	}

	zenKey := cfg.Provider.Zen.APIKey
	if zenKey == "" {
		sb.WriteString("  Zen:        ❌ not set\n")
	} else {
		masked := "***" + zenKey[max(0, len(zenKey)-4):]
		sb.WriteString(fmt.Sprintf("  Zen:        ✅ set (%s)\n", masked))
	}

	return CommandResult{Success: true, Message: sb.String()}
}

// handleTokens estimates the token count for session messages.
func handleTokens(_ []string, ctx CommandContext) CommandResult {
	if ctx.SessionManager == nil || ctx.SessionID == "" {
		return CommandResult{Success: false, Message: "No active session."}
	}
	sess, err := ctx.SessionManager.LoadSession(ctx.SessionID)
	if err != nil {
		return CommandResult{Success: false, Message: fmt.Sprintf("Failed to load session: %v", err)}
	}

	totalChars := 0
	for _, msg := range sess.Messages {
		totalChars += len(msg.Content)
	}
	// Rough estimate: ~4 chars per token
	estTokens := totalChars / 4

	sessDir := ""
	if ctx.SessionManager != nil {
		sessDir = filepath.Join(ctx.SessionManager.BaseDir(), ctx.SessionID)
	}
	usage := diskUsageFormatted(sessDir)

	return CommandResult{
		Success: true,
		Message: fmt.Sprintf(
			"**Token estimate:** ~%d tokens (%d messages)\n**Session disk usage:** %s",
			estTokens, sess.MessageCount, usage,
		),
	}
}


