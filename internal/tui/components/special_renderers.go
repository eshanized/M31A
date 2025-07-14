package components

import (
	"encoding/json"
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

type TodoWriteRenderer struct {
	BaseRenderer
}

func NewTodoWriteRenderer(t theme.Theme) *TodoWriteRenderer {
	return &TodoWriteRenderer{BaseRenderer: BaseRenderer{toolName: "TodoWrite", theme: t}}
}

func (r *TodoWriteRenderer) RenderInput(call types.ToolCall, width int) string {
	var params map[string]any
	if err := json.Unmarshal(call.Input, &params); err != nil {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
	}
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
		total := pending + completed
		barWidth := width - 20
		if barWidth < 10 {
			barWidth = 10
		}
		filled := 0
		if total > 0 {
			filled = (completed * barWidth) / total
		}
		bar := strings.Repeat("█", filled) + strings.Repeat("░", barWidth-filled)
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(
			fmt.Sprintf("todos: %d/%d completed  [%s]", completed, total, bar))
	}
	return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
}

func (r *TodoWriteRenderer) RenderOutput(result *types.ToolResult, state ToolState, durationMs int64, truncated bool, collapsed bool, width int) string {
	return r.RenderStatus(state, durationMs, width, "")
}

type GrepRenderer struct {
	BaseRenderer
}

func NewGrepRenderer(t theme.Theme) *GrepRenderer {
	return &GrepRenderer{BaseRenderer: BaseRenderer{toolName: "Grep", theme: t}}
}

func (r *GrepRenderer) RenderInput(call types.ToolCall, width int) string {
	var params map[string]any
	if err := json.Unmarshal(call.Input, &params); err != nil {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
	}
	if pattern, ok := params["pattern"].(string); ok {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render("grep " + pattern)
	}
	return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
}

func (r *GrepRenderer) RenderOutput(result *types.ToolResult, state ToolState, durationMs int64, truncated bool, collapsed bool, width int) string {
	if result == nil || result.Output == "" {
		return r.RenderStatus(state, durationMs, width, "")
	}

	if collapsed {
		matchCount := strings.Count(result.Output, "\n") + 1
		return lipgloss.JoinVertical(lipgloss.Top,
			lipgloss.NewStyle().
				Foreground(r.theme.TextSecondary).
				Italic(true).
				Render(fmt.Sprintf("[%d matches]", matchCount)),
			r.RenderStatus(state, durationMs, width, ""),
		)
	}

	return lipgloss.JoinVertical(lipgloss.Top,
		r.RenderGenericOutput(result.Output, truncated, false, width),
		r.RenderStatus(state, durationMs, width, ""),
	)
}

type GlobRenderer struct {
	BaseRenderer
}

func NewGlobRenderer(t theme.Theme) *GlobRenderer {
	return &GlobRenderer{BaseRenderer: BaseRenderer{toolName: "Glob", theme: t}}
}

func (r *GlobRenderer) RenderInput(call types.ToolCall, width int) string {
	var params map[string]any
	if err := json.Unmarshal(call.Input, &params); err != nil {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
	}
	if pattern, ok := params["pattern"].(string); ok {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render("glob " + pattern)
	}
	return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
}

func (r *GlobRenderer) RenderOutput(result *types.ToolResult, state ToolState, durationMs int64, truncated bool, collapsed bool, width int) string {
	if result == nil || result.Output == "" {
		return r.RenderStatus(state, durationMs, width, "")
	}

	if collapsed {
		fileCount := strings.Count(result.Output, "\n") + 1
		return lipgloss.JoinVertical(lipgloss.Top,
			lipgloss.NewStyle().
				Foreground(r.theme.TextSecondary).
				Italic(true).
				Render(fmt.Sprintf("[%d files]", fileCount)),
			r.RenderStatus(state, durationMs, width, ""),
		)
	}

	return lipgloss.JoinVertical(lipgloss.Top,
		r.RenderGenericOutput(result.Output, truncated, false, width),
		r.RenderStatus(state, durationMs, width, ""),
	)
}

type GenericRenderer struct {
	BaseRenderer
}

func NewGenericRenderer(toolName string, t theme.Theme) *GenericRenderer {
	return &GenericRenderer{BaseRenderer: BaseRenderer{toolName: toolName, theme: t}}
}

func (r *GenericRenderer) RenderInput(call types.ToolCall, width int) string {
	var params map[string]any
	if err := json.Unmarshal(call.Input, &params); err != nil {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
	}
	var parts []string
	for k, v := range params {
		parts = append(parts, fmt.Sprintf("%s=%v", k, v))
		if len(parts) >= 3 {
			break
		}
	}
	if len(parts) == 0 {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
	}
	return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(strings.Join(parts, ", "))
}

func (r *GenericRenderer) RenderOutput(result *types.ToolResult, state ToolState, durationMs int64, truncated bool, collapsed bool, width int) string {
	var output string
	var errMsg string
	if result != nil {
		output = result.Output
		if state == ToolError && result.Error != "" {
			errMsg = result.Error
		} else if state == ToolError && output != "" {
			errMsg = output
		}
	}
	parts := []string{}
	if !collapsed && output != "" {
		parts = append(parts, r.RenderGenericOutput(output, truncated, false, width))
	}
	parts = append(parts, r.RenderStatus(state, durationMs, width, errMsg))
	return lipgloss.JoinVertical(lipgloss.Top, parts...)
}
