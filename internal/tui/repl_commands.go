package tui

import (
	"context"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/shell"
)

// executeShellCommand runs a shell command (prefixed with !) and adds the result to messages.
// Accepts a context that is cancelled on app shutdown to prevent zombie processes.
func (m *ReplModel) executeShellCommand(input string, ctx context.Context) tea.Cmd {
	cmd := strings.TrimPrefix(input, "!")
	cmd = strings.TrimSpace(cmd)
	if cmd == "" {
		return nil
	}

	return func() tea.Msg {
		timeoutCtx, cancel := context.WithTimeout(ctx, 30*time.Second)
		defer cancel()

		out, err := shell.CommandContext(timeoutCtx, cmd).CombinedOutput()
		var result string
		if err != nil {
			result = "Error: " + err.Error()
			if len(out) > 0 {
				result += "\n" + string(out)
			}
		} else {
			result = string(out)
		}

		// PERF-39: Increased from 4KB to 8KB to show more output
		const maxShellOutput = 8192
		if len(result) > maxShellOutput {
			result = result[:maxShellOutput] + "\n...(truncated)"
		}
		return SlashCommandMsg{Command: "!result:" + result}
	}
}
