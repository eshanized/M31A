package components

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
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

func (b *BaseRenderer) RenderInput(call types.ToolCall, width int) string {
	s := b.styles
	input := string(call.Input)
	if len(input) > 200 {
		input = input[:200] + "..."
	}
	return s.Muted.Render(input)
}

func (b *BaseRenderer) RenderOutput(result *types.ToolResult, state ToolState, durationMs int64, truncated bool, collapsed bool, width int) string {
	if result == nil {
		return ""
	}
	return b.RenderGenericOutput(result.Output, truncated, collapsed, width)
}

func RendererForTool(toolName string, t theme.Theme) ToolRenderer {
	s := theme.BuildSemanticStyles(t)
	return &BaseRenderer{toolName: toolName, theme: t, styles: s}
}
