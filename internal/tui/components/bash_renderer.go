package components

import (
	"encoding/json"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

type BashRenderer struct {
	BaseRenderer
}

func NewBashRenderer(t theme.Theme) *BashRenderer {
	return &BashRenderer{BaseRenderer: BaseRenderer{toolName: "Bash", theme: t}}
}

func (r *BashRenderer) RenderInput(call types.ToolCall, width int) string {
	var params map[string]any
	if err := json.Unmarshal(call.Input, &params); err != nil {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
	}
	if cmd, ok := params["command"].(string); ok {
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render("$ " + cmd)
	}
	return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
}

func (r *BashRenderer) RenderOutput(result *types.ToolResult, state ToolState, durationMs int64, truncated bool, collapsed bool, width int) string {
	var output string
	if result != nil {
		output = result.Output
	}
	parts := []string{}
	if !collapsed && output != "" {
		parts = append(parts, r.RenderGenericOutput(output, truncated, false, width))
	}
	parts = append(parts, r.RenderStatus(state, durationMs, width))
	return lipgloss.JoinVertical(lipgloss.Top, parts...)
}
