package components

import (
	"encoding/json"
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ToolRenderer interface for specialized per-tool rendering.
type ToolRenderer interface {
	Name() string
	RenderHeader(width int) string
	RenderInput(call types.ToolCall, width int) string
	RenderOutput(result *types.ToolResult, state ToolState, durationMs int64, truncated bool, collapsed bool, width int) string
	RenderStatus(state ToolState, durationMs int64, width int) string
}

// BaseRenderer provides shared header/status rendering for all tools.
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

func (b *BaseRenderer) RenderStatus(state ToolState, durationMs int64, width int) string {
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
	return lipgloss.NewStyle().Width(width).Padding(0, 1).Render(badge)
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
			Render(fmt.Sprintf("[+%d lines hidden]", hidden))
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

// BashRenderer renders command execution with collapsible output.
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

// EditRenderer renders file edits with path, line range, and inline diff.
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
		return r.RenderStatus(state, durationMs, width)
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
			r.RenderStatus(state, durationMs, width),
		)
	}

	// Render inline diff: lines starting with + are green, - are red
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
		r.RenderStatus(state, durationMs, width),
	)
}

// FileReadRenderer renders file read operations.
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
	parts = append(parts, r.RenderStatus(state, durationMs, width))
	return lipgloss.JoinVertical(lipgloss.Top, parts...)
}

// FileWriteRenderer renders file write with path and byte count.
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
		return r.RenderStatus(state, durationMs, width)
	}

	// Show byte count if available
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
		r.RenderStatus(state, durationMs, width),
	)
}

// TodoWriteRenderer renders todo progress with a simple bar.
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
	return r.RenderStatus(state, durationMs, width)
}

// GrepRenderer renders pattern search with match count.
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
		return r.RenderStatus(state, durationMs, width)
	}

	if collapsed {
		// Count matches from output lines
		matchCount := strings.Count(result.Output, "\n") + 1
		return lipgloss.JoinVertical(lipgloss.Top,
			lipgloss.NewStyle().
				Foreground(r.theme.TextSecondary).
				Italic(true).
				Render(fmt.Sprintf("[%d matches]", matchCount)),
			r.RenderStatus(state, durationMs, width),
		)
	}

	return lipgloss.JoinVertical(lipgloss.Top,
		r.RenderGenericOutput(result.Output, truncated, false, width),
		r.RenderStatus(state, durationMs, width),
	)
}

// GlobRenderer renders file pattern matching with file count.
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
		return r.RenderStatus(state, durationMs, width)
	}

	if collapsed {
		fileCount := strings.Count(result.Output, "\n") + 1
		return lipgloss.JoinVertical(lipgloss.Top,
			lipgloss.NewStyle().
				Foreground(r.theme.TextSecondary).
				Italic(true).
				Render(fmt.Sprintf("[%d files]", fileCount)),
			r.RenderStatus(state, durationMs, width),
		)
	}

	return lipgloss.JoinVertical(lipgloss.Top,
		r.RenderGenericOutput(result.Output, truncated, false, width),
		r.RenderStatus(state, durationMs, width),
	)
}

// GenericRenderer is the fallback for unknown tool types.
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
	// Show first few params as key=value
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

// RendererForTool returns the appropriate ToolRenderer for the given tool name.
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
