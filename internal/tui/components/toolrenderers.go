package components

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

type ToolRenderer interface {
	Name() string
	RenderHeader(width int) string
	RenderInput(call types.ToolCall, width int) string
	RenderOutput(result *types.ToolResult, state ToolState, durationMs int64, truncated bool, collapsed bool, width int) string
	RenderStatus(state ToolState, durationMs int64, width int, errMsg string) string
}

type BaseRenderer struct {
	toolName string
	theme    theme.Theme
}

func (b *BaseRenderer) Name() string {
	return b.toolName
}

func (b *BaseRenderer) RenderHeader(width int) string {
	labelStyle, ok := b.theme.ToolLabel[b.toolName]
	if !ok {
		labelStyle = lipgloss.NewStyle().
			Background(b.theme.TextSecondary).
			Foreground(lipgloss.Color("#000000")).
			Padding(0, 1).
			Bold(true)
	}
	label := labelStyle.Render(fmt.Sprintf(" %s ", b.toolName))
	return lipgloss.NewStyle().Width(width).Render(label)
}

func (b *BaseRenderer) RenderStatus(state ToolState, durationMs int64, width int, errMsg string) string {
	var badge string
	switch state {
	case ToolRunning:
		badge = b.theme.Spinner.Render("[..] Running...")
	case ToolSuccess:
		dur := fmt.Sprintf("%.2fs", float64(durationMs)/1000.0)
		badge = lipgloss.JoinHorizontal(lipgloss.Top,
			b.theme.SuccessBadge.Render(" OK "),
			lipgloss.NewStyle().
				Foreground(b.theme.TextSecondary).
				Render(fmt.Sprintf(" Completed in %s", dur)),
		)
	case ToolError:
		badge = b.theme.ErrorBadge.Render(" ERR ")
	}
	status := lipgloss.NewStyle().Width(width).Padding(0, 1).Render(badge)

	// Show error message inline below badge, truncated to 3 lines
	if state == ToolError && errMsg != "" {
		errLines := strings.Split(errMsg, "\n")
		maxLines := 3
		if len(errLines) > maxLines {
			errLines = errLines[:maxLines]
			truncatedMsg := strings.Join(errLines, "\n") + "\n..."
			status += "\n" + lipgloss.NewStyle().
				Foreground(b.theme.Error).
				Width(width).
				Padding(0, 1).
				Render(truncatedMsg)
		} else {
			status += "\n" + lipgloss.NewStyle().
				Foreground(b.theme.Error).
				Width(width).
				Padding(0, 1).
				Render(errMsg)
		}
	}

	return status
}

func (b *BaseRenderer) RenderGenericOutput(output string, truncated bool, collapsed bool, width int) string {
	if output == "" {
		return ""
	}
	if collapsed {
		lineCount := strings.Count(output, "\n") + 1
		hidden := lineCount - 3
		if hidden < 1 {
			hidden = 1
		}
		return lipgloss.NewStyle().
			Foreground(b.theme.TextSecondary).
			Italic(true).
			Render(fmt.Sprintf("[+%d lines hidden — Space to expand]", hidden))
	}
	if truncated {
		output += "\n" + lipgloss.NewStyle().
			Foreground(b.theme.Warning).
			Italic(true).
			Render("[... output truncated, full output in session log]")
	}
	return lipgloss.NewStyle().
		Foreground(b.theme.TextSecondary).
		Width(width).
		Padding(0, 1).
		Render(output)
}

func RendererForTool(toolName string, t theme.Theme) ToolRenderer {
	switch toolName {
	case "Bash":
		return NewBashRenderer(t)
	case "Edit":
		return NewEditRenderer(t)
	case "FileRead":
		return NewFileReadRenderer(t)
	case "FileWrite":
		return NewFileWriteRenderer(t)
	case "TodoWrite":
		return NewTodoWriteRenderer(t)
	case "Grep":
		return NewGrepRenderer(t)
	case "Glob":
		return NewGlobRenderer(t)
	default:
		return NewGenericRenderer(toolName, t)
	}
}
