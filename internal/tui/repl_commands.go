package tui

import (
	"context"
	"os/exec"
	"strings"
	"time"

	tea "github.com/charmbracelet/bubbletea"
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
		ctx, cancel := context.WithTimeout(ctx, 30*time.Second)
		defer cancel()

		out, err := exec.CommandContext(ctx, "sh", "-c", cmd).CombinedOutput()
		var result string
		if err != nil {
			result = "Error: " + err.Error()
			if len(out) > 0 {
				result += "\n" + string(out)
			}
		} else {
			result = string(out)
		}

		if len(result) > 4000 {
			result = result[:4000] + "\n...(truncated)"
		}
		return SlashCommandMsg{Command: "!result:" + result}
	}
}
