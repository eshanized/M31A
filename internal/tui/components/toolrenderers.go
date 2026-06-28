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
	styles   theme.SemanticStyles
}

func (b *BaseRenderer) Name() string {
	return b.toolName
}

func (b *BaseRenderer) RenderHeader(width int) string {
	labelStyle, ok := b.theme.ToolLabel[b.toolName]
	if !ok {
		labelStyle = b.styles.ToolLabel
	}
	label := labelStyle.Render(fmt.Sprintf(" %s ", b.toolName))
	return lipgloss.NewStyle().Width(width).Render(label)
}

func (b *BaseRenderer) RenderStatus(state ToolState, durationMs int64, width int, errMsg string) string {
	s := b.styles
	var badge string
	switch state {
	case ToolRunning:
		badge = s.Spinner.Render("[..] Running...")
	case ToolSuccess:
		dur := fmt.Sprintf("%.2fs", float64(durationMs)/1000.0)
		badge = lipgloss.JoinHorizontal(lipgloss.Top,
			s.BadgeSuccess.Render(" OK "),
			s.SecondaryText.Render(fmt.Sprintf(" Completed in %s", dur)),
		)
	case ToolError:
		badge = s.BadgeError.Render(" ERR ")
	}
	status := lipgloss.NewStyle().Width(width).Padding(0, 1).Render(badge)

	// Show error message inline below badge, truncated to 3 lines
	if state == ToolError && errMsg != "" {
		errLines := strings.Split(errMsg, "\n")
		maxLines := 3
		if len(errLines) > maxLines {
			errLines = errLines[:maxLines]
			truncatedMsg := strings.Join(errLines, "\n") + "\n..."
			status += "\n" + s.ErrorText.Width(width).Padding(0, 1).Render(truncatedMsg)
		} else {
			status += "\n" + s.ErrorText.Width(width).Padding(0, 1).Render(errMsg)
		}
	}

	return status
}

func (b *BaseRenderer) RenderGenericOutput(output string, truncated bool, collapsed bool, width int) string {
	if output == "" {
		return ""
	}
	s := b.styles
	if collapsed {
		lineCount := strings.Count(output, "\n") + 1
		hidden := lineCount - 3
		if hidden < 1 {
			hidden = 1
		}
		return s.SecondaryText.Italic(true).
			Render(fmt.Sprintf("[+%d lines hidden — Space to expand]", hidden))
	}
	if truncated {
		output += "\n" + s.WarningText.Italic(true).
			Render("[... output truncated, full output in session log]")
	}
	return s.SecondaryText.Width(width).Padding(0, 1).Render(output)
}

func RendererForTool(toolName string, t theme.Theme) ToolRenderer {
	s := theme.BuildSemanticStyles(t)
	switch toolName {
	case "Bash":
		return &BashRenderer{BaseRenderer: BaseRenderer{toolName: "Bash", theme: t, styles: s}}
	case "Edit":
		return &EditRenderer{BaseRenderer: BaseRenderer{toolName: "Edit", theme: t, styles: s}}
	case "FileRead":
		return &FileReadRenderer{BaseRenderer: BaseRenderer{toolName: "FileRead", theme: t, styles: s}}
	case "FileWrite":
		return &FileWriteRenderer{BaseRenderer: BaseRenderer{toolName: "FileWrite", theme: t, styles: s}}
	case "TodoWrite":
		return &TodoWriteRenderer{BaseRenderer: BaseRenderer{toolName: "TodoWrite", theme: t, styles: s}}
	case "Grep":
		return &GrepRenderer{BaseRenderer: BaseRenderer{toolName: "Grep", theme: t, styles: s}}
	case "Glob":
		return &GlobRenderer{BaseRenderer: BaseRenderer{toolName: "Glob", theme: t, styles: s}}
	case "WebFetch":
		return &WebFetchRenderer{BaseRenderer: BaseRenderer{toolName: "WebFetch", theme: t, styles: s}}
	case "AskUserQuestion":
		return &AskUserQuestionRenderer{BaseRenderer: BaseRenderer{toolName: "AskUserQuestion", theme: t, styles: s}}
	default:
		return &GenericRenderer{BaseRenderer: BaseRenderer{toolName: toolName, theme: t, styles: s}}
	}
}
