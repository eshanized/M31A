package tui

import (
	"context"
	"os/exec"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
)

// executeShellCommand runs a shell command (prefixed with !) and adds the result to messages.
func (m *ReplModel) executeShellCommand(input string) tea.Cmd {
	cmd := strings.TrimPrefix(input, "!")
	cmd = strings.TrimSpace(cmd)
	if cmd == "" {
		return nil
	}

	return func() tea.Msg {
		ctx, cancel := context.WithTimeout(context.Background(), 30*1e9) // 30s
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

// expandFileRefs resolves @filepath mentions in the input and returns
// the expanded string with file contents inline.
func (m *ReplModel) expandFileRefs(input string) string {
	// Simple implementation: find @word tokens and read file contents
	words := strings.Fields(input)
	for i, word := range words {
		if strings.HasPrefix(word, "@") {
			path := strings.TrimPrefix(word, "@")
			// Skip special @ mentions like @conversation
			if strings.HasPrefix(path, "conversation") {
				continue
			}
			// Resolve relative to cwd
			if m.cwd != "" && !strings.HasPrefix(path, "/") {
				path = m.cwd + "/" + path
			}
			words[i] = word // keep original if file not readable
		}
	}
	return strings.Join(words, " ")
}
