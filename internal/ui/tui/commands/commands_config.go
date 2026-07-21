package commands

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"unicode/utf8"

	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"
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
	screen := tuitypes.ScreenSettings
	return CommandResult{Success: true, Screen: &screen, Message: "Opening settings..."}
}

// handleConfig opens the full config viewer screen.
func handleConfig(_ []string, _ CommandContext) CommandResult {
	screen := tuitypes.ScreenConfig
	return CommandResult{Success: true, Screen: &screen, Message: "Opening full config..."}
}

// handleCost toggles the cost estimate display.
func handleCost(_ []string, ctx CommandContext) CommandResult {
	if ctx.Config == nil {
		return CommandResult{Success: false, Message: "Config not available."}
	}
	current := ctx.Config.UI.ShowCostEstimate
	ctx.Config.UI.ShowCostEstimate = !current
	state := "enabled"
	if !ctx.Config.UI.ShowCostEstimate {
		state = "disabled"
	}
	if ctx.ConfigPath != "" {
		if err := ctx.Config.SaveWithKeychain(ctx.ConfigPath, ctx.Keychain); err != nil {
			return CommandResult{
				Success: true,
				Message: fmt.Sprintf("Cost display **%s** (save failed: %v).", state, err),
			}
		}
	}
	return CommandResult{
		Success: true,
		Message: fmt.Sprintf("Cost display **%s**.", state),
	}
}

// handleLog shows recent log entries from the m31a log file.
// Uses reverse scan to read only the last N lines without loading the entire file.
func handleLog(args []string, ctx CommandContext) CommandResult {
	lines := 20
	if ctx.Config != nil && ctx.Config.UI.DefaultLogLines > 0 {
		lines = ctx.Config.UI.DefaultLogLines
	}
	if len(args) > 0 {
		n := 0
		_, _ = fmt.Sscanf(args[0], "%d", &n)
		if n > 0 {
			lines = n
		}
	}

	homeDir, err := os.UserHomeDir()
	if err != nil {
		return CommandResult{Success: false, Message: "Cannot determine home directory."}
	}
	logPath := filepath.Join(homeDir, ".m31a", "m31a.log")

	tail, err := readTailLines(logPath, lines)
	if err != nil {
		if os.IsNotExist(err) {
			return CommandResult{Success: true, Message: "No log file found yet."}
		}
		return CommandResult{Success: false, Message: "Cannot read log file."}
	}

	return CommandResult{
		Success: true,
		Message: fmt.Sprintf("**Last %d log lines:**\n```\n%s\n```", lines, strings.Join(tail, "\n")),
	}
}

// readTailLines reads the last n lines from path using a reverse scan,
// avoiding loading the entire file into memory.
func readTailLines(path string, n int) ([]string, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close() //nolint:errcheck

	stat, err := f.Stat()
	if err != nil {
		return nil, err
	}

	fileSize := stat.Size()
	if fileSize == 0 {
		return nil, nil
	}

	// Read backwards from end of file collecting complete lines.
	const chunkSize int64 = 8192
	offset := fileSize
	var tail []string
	pending := ""

	for offset > 0 && len(tail) < n {
		readSize := chunkSize
		if readSize > offset {
			readSize = offset
		}
		offset -= readSize

		buf := make([]byte, readSize)
		if _, err := f.ReadAt(buf, offset); err != nil && err != io.EOF {
			return nil, err
		}

		text := string(buf) + pending
		parts := strings.Split(text, "\n")
		// Last element is the partial line before this chunk (carry to next iteration)
		pending = parts[0]
		for i := len(parts) - 1; i >= 1; i-- {
			if len(tail) >= n {
				break
			}
			tail = append(tail, parts[i])
		}
	}

	// If there are still fewer than n lines, the first line may be the remainder
	if len(tail) < n && pending != "" {
		tail = append(tail, pending)
	}

	// Reverse to chronological order
	for i, j := 0, len(tail)-1; i < j; i, j = i+1, j-1 {
		tail[i], tail[j] = tail[j], tail[i]
	}

	// Strip empty trailing lines
	for len(tail) > 0 && tail[len(tail)-1] == "" {
		tail = tail[:len(tail)-1]
	}

	return tail, nil
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
		sb.WriteString("  OpenRouter: ✅ set\n")
	}

	zenKey := cfg.Provider.Zen.APIKey
	if zenKey == "" {
		sb.WriteString("  Zen:        ❌ not set\n")
	} else {
		sb.WriteString("  Zen:        ✅ set\n")
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

	// Use rune count for accurate non-ASCII estimation, then apply ~4 chars/token heuristic.
	// len() counts bytes which underestimates for CJK/emoji; rune count is more accurate.
	totalRunes := 0
	for _, msg := range sess.Messages {
		totalRunes += utf8.RuneCountInString(msg.Content)
	}
	estTokens := totalRunes / 4

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
