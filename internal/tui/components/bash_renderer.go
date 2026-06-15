package components

import (
	"encoding/json"
	"runtime"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// bashPrefix returns the platform-appropriate bash prompt prefix.
func bashPrefix() string {
	if runtime.GOOS == "windows" {
		return "> "
	}
	return "$ "
}

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
		return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(bashPrefix() + cmd)
	}
	return lipgloss.NewStyle().Foreground(r.theme.TextPrimary).Width(width).Padding(0, 1).Render(string(call.Input))
}

func (r *BashRenderer) RenderOutput(result *types.ToolResult, state ToolState, durationMs int64, truncated bool, collapsed bool, width int) string {
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
		lang := DetectLanguage(output)
		highlighted := HighlightCode(output, lang, r.theme)
		if truncated {
			highlighted += "\n" + lipgloss.NewStyle().
				Foreground(r.theme.Warning).
				Italic(true).
				Render("[... output truncated, full output in session log]")
		}
		parts = append(parts, lipgloss.NewStyle().
			Foreground(r.theme.TextSecondary).
			Width(width).
			Padding(0, 1).
			Render(highlighted))
	}
	parts = append(parts, r.RenderStatus(state, durationMs, width, errMsg))
	return lipgloss.JoinVertical(lipgloss.Top, parts...)
}
