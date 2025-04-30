package components

import (
	"encoding/json"
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/tui/theme"
)

type ToolState int

const (
	ToolRunning ToolState = iota
	ToolSuccess
	ToolError
)

type ToolCard struct {
	toolName  string
	input     string
	output    string
	state     ToolState
	durationMs int64
	truncated bool
	theme     theme.Theme
	collapsed bool
}

func NewToolCard(call types.ToolCall, result *types.ToolResult, state ToolState, t theme.Theme) *ToolCard {
	input := formatToolInput(call.Name, call.Input)
	output := ""
	truncated := false
	if result != nil {
		output = result.Output
		truncated = result.Truncated
	}

	tc := &ToolCard{
		toolName:   call.Name,
		input:      input,
		output:     output,
		state:      state,
		truncated:  truncated,
		theme:      t,
		collapsed:  false,
	}
	if result != nil {
		tc.durationMs = result.DurationMs
	}

	lineCount := strings.Count(output, "\n") + 1
	if lineCount > 20 {
		tc.collapsed = true
	}

	if len(output) > types.MaxToolOutputChars {
		tc.output = output[:types.MaxToolOutputChars]
		tc.truncated = true
	}

	if isBinaryContent(output) {
		tc.output = fmt.Sprintf("[binary file, %d bytes]", len(output))
		tc.collapsed = true
	}

	return tc
}

func formatToolInput(name string, input []byte) string {
	if len(input) == 0 {
		return ""
	}
	raw := string(input)

	var params map[string]any
	if err := json.Unmarshal(input, &params); err != nil {
		return raw
	}

	switch name {
	case "Bash":
		if cmd, ok := params["command"].(string); ok {
			return "$ " + cmd
		}
	case "FileRead":
		if path, ok := params["path"].(string); ok {
			return "reading " + path
		}
	case "FileWrite":
		if path, ok := params["path"].(string); ok {
			return "writing " + path
		}
	case "Edit":
		if path, ok := params["path"].(string); ok {
			if startLine, ok := params["start_line"].(float64); ok {
				if endLine, ok := params["end_line"].(float64); ok {
					return fmt.Sprintf("editing %s lines %.0f-%0.f", path, startLine, endLine)
				}
			}
			return "editing " + path
		}
	case "TodoWrite":
		if todos, ok := params["todos"].([]any); ok {
			pending, completed := 0, 0
			for _, t := range todos {
				if m, ok := t.(map[string]any); ok {
					switch m["status"] {
					case "completed":
						completed++
					case "pending", "in_progress":
						pending++
					}
				}
			}
			return fmt.Sprintf("todos: %d pending, %d completed", pending, completed)
		}
	case "WebFetch":
		if u, ok := params["url"].(string); ok {
			return "fetch " + u
		}
	case "AskUserQuestion":
		if q, ok := params["question"].(string); ok {
			return "? " + q
		}
	case "Glob":
		if pattern, ok := params["pattern"].(string); ok {
			return "glob " + pattern
		}
	case "Grep":
		if pattern, ok := params["pattern"].(string); ok {
			return "grep " + pattern
		}
	}

	return raw
}

func isBinaryContent(s string) bool {
	if len(s) == 0 {
		return false
	}
	nullBytes := 0
	checkLen := len(s)
	if checkLen > 1024 {
		checkLen = 1024
	}
	for i := 0; i < checkLen; i++ {
		if s[i] == 0 {
			nullBytes++
		}
	}
	return nullBytes > 0
}

func (c *ToolCard) Render(width int) string {
	cardWidth := width - 4
	if cardWidth < 20 {
		cardWidth = 20
	}

	header := c.renderHeader(cardWidth)
	input := c.renderInput(c.input, cardWidth)

	parts := []string{header, input}

	if !c.collapsed && c.output != "" {
		parts = append(parts, c.renderOutput(c.output, cardWidth))
	}

	status := c.renderStatus(cardWidth)
	parts = append(parts, status)

	content := lipgloss.JoinVertical(lipgloss.Top, parts...)

	return c.theme.ToolCard.
		Width(cardWidth + 4).
		Render(content)
}

func (c *ToolCard) Toggle() {
	c.collapsed = !c.collapsed
}

func (c *ToolCard) IsCollapsed() bool {
	return c.collapsed
}

func (c *ToolCard) renderHeader(width int) string {
	labelStyle, ok := c.theme.ToolLabel[c.toolName]
	if !ok {
		labelStyle = lipgloss.NewStyle().
			Background(c.theme.TextSecondary).
			Foreground(lipgloss.Color("#000000")).
			Padding(0, 1).
			Bold(true)
	}

	label := labelStyle.Render(fmt.Sprintf(" %s ", c.toolName))
	return lipgloss.NewStyle().Width(width).Render(label)
}

func (c *ToolCard) renderInput(input string, width int) string {
	if input == "" {
		return ""
	}
	return lipgloss.NewStyle().
		Foreground(c.theme.TextPrimary).
		Width(width).
		Padding(0, 1).
		Render(input)
}

func (c *ToolCard) renderOutput(output string, width int) string {
	var b strings.Builder

	if c.collapsed {
		lineCount := strings.Count(output, "\n") + 1
		hidden := lineCount - 3
		if hidden < 1 {
			hidden = 1
		}
		b.WriteString(lipgloss.NewStyle().
			Foreground(c.theme.TextSecondary).
			Italic(true).
			Render(fmt.Sprintf("[+%d lines hidden]", hidden)))
		return b.String()
	}

	if c.truncated {
		output += "\n" + lipgloss.NewStyle().
			Foreground(c.theme.Warning).
			Italic(true).
			Render("[... output truncated, full output in session log]")
	}

	b.WriteString(lipgloss.NewStyle().
		Foreground(c.theme.TextSecondary).
		Width(width).
		Padding(0, 1).
		Render(output))

	return b.String()
}

func (c *ToolCard) renderStatus(width int) string {
	var badge string
	switch c.state {
	case ToolRunning:
		badge = c.theme.Spinner.Render("[..] Running...")
	case ToolSuccess:
		dur := fmt.Sprintf("%.2fs", float64(c.durationMs)/1000.0)
		badge = lipgloss.JoinHorizontal(lipgloss.Top,
			c.theme.SuccessBadge.Render(" OK "),
			lipgloss.NewStyle().
				Foreground(c.theme.TextSecondary).
				Render(fmt.Sprintf(" Completed in %s", dur)),
		)
	case ToolError:
		badge = c.theme.ErrorBadge.Render(" ERR ")
	}

	return lipgloss.NewStyle().Width(width).Padding(0, 1).Render(badge)
}
