package components

import (
	"encoding/json"
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

type EditRenderer struct {
	BaseRenderer
}

func NewEditRenderer(t theme.Theme) *EditRenderer {
	return &EditRenderer{BaseRenderer: BaseRenderer{toolName: "Edit", theme: t}}
}

func (r *EditRenderer) RenderInput(call types.ToolCall, width int) string {
	var params map[string]any
	if err := json.Unmarshal(call.Input, &params); err != nil {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
	}
	if path, ok := params["path"].(string); ok {
		if startLine, ok := params["start_line"].(float64); ok {
			if endLine, ok := params["end_line"].(float64); ok {
				return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(
					fmt.Sprintf("editing %s lines %.0f-%.0f", path, startLine, endLine))
			}
		}
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render("editing " + path)
	}
	return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
}

func (r *EditRenderer) RenderOutput(result *types.ToolResult, state ToolState, durationMs int64, truncated bool, collapsed bool, width int) string {
	if result == nil || result.Output == "" {
		return r.RenderStatus(state, durationMs, width, "")
	}

	output := result.Output
	if truncated {
		output += "\n" + lipgloss.NewStyle().
			Foreground(r.theme.Warning).
			Italic(true).
			Render("[... output truncated, full output in session log]")
	}

	if collapsed {
		lineCount := strings.Count(output, "\n") + 1
		hidden := lineCount - 3
		if hidden < 1 {
			hidden = 1
		}
		return lipgloss.JoinVertical(lipgloss.Top,
			lipgloss.NewStyle().
				Foreground(r.theme.TextSecondary).
				Italic(true).
				Render(fmt.Sprintf("[+%d lines hidden]", hidden)),
			r.RenderStatus(state, durationMs, width, ""),
		)
	}

	lines := strings.Split(output, "\n")
	var diffLines []string
	for _, line := range lines {
		if strings.HasPrefix(line, "+") && !strings.HasPrefix(line, "+++") {
			diffLines = append(diffLines, lipgloss.NewStyle().Foreground(r.theme.Success).Render(line))
		} else if strings.HasPrefix(line, "-") && !strings.HasPrefix(line, "---") {
			diffLines = append(diffLines, lipgloss.NewStyle().Foreground(r.theme.Error).Render(line))
		} else {
			diffLines = append(diffLines, lipgloss.NewStyle().Foreground(r.theme.TextSecondary).Render(line))
		}
	}

	return lipgloss.JoinVertical(lipgloss.Top,
		lipgloss.NewStyle().Width(width).Padding(0, 1).Render(strings.Join(diffLines, "\n")),
		r.RenderStatus(state, durationMs, width, ""),
	)
}

type FileReadRenderer struct {
	BaseRenderer
}

func NewFileReadRenderer(t theme.Theme) *FileReadRenderer {
	return &FileReadRenderer{BaseRenderer: BaseRenderer{toolName: "FileRead", theme: t}}
}

func (r *FileReadRenderer) RenderInput(call types.ToolCall, width int) string {
	var params map[string]any
	if err := json.Unmarshal(call.Input, &params); err != nil {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
	}
	if path, ok := params["path"].(string); ok {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render("reading " + path)
	}
	return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
}

func (r *FileReadRenderer) RenderOutput(result *types.ToolResult, state ToolState, durationMs int64, truncated bool, collapsed bool, width int) string {
	var output string
	if result != nil {
		output = result.Output
	}
	parts := []string{}
	if !collapsed && output != "" {
		parts = append(parts, r.RenderGenericOutput(output, truncated, false, width))
	}
	parts = append(parts, r.RenderStatus(state, durationMs, width, ""))
	return lipgloss.JoinVertical(lipgloss.Top, parts...)
}

type FileWriteRenderer struct {
	BaseRenderer
}

func NewFileWriteRenderer(t theme.Theme) *FileWriteRenderer {
	return &FileWriteRenderer{BaseRenderer: BaseRenderer{toolName: "FileWrite", theme: t}}
}

func (r *FileWriteRenderer) RenderInput(call types.ToolCall, width int) string {
	var params map[string]any
	if err := json.Unmarshal(call.Input, &params); err != nil {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
	}
	if path, ok := params["path"].(string); ok {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render("writing " + path)
	}
	return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
}

func (r *FileWriteRenderer) RenderOutput(result *types.ToolResult, state ToolState, durationMs int64, truncated bool, collapsed bool, width int) string {
	if result == nil || result.Output == "" {
		return r.RenderStatus(state, durationMs, width, "")
	}

	byteCount := len(result.Output)
	statusLine := fmt.Sprintf("wrote %d bytes", byteCount)
	if truncated {
		statusLine += " (truncated)"
	}

	return lipgloss.JoinVertical(lipgloss.Top,
		lipgloss.NewStyle().
			Foreground(r.theme.TextSecondary).
			Italic(true).
			Width(width).
			Padding(0, 1).
			Render(statusLine),
		r.RenderStatus(state, durationMs, width, ""),
	)
}
