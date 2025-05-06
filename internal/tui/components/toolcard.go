package components

import (
	"regexp"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/tui/theme"
)

var (
	ansiEscapeRe  = regexp.MustCompile(`\x1b\[[0-9;]*[a-zA-Z]`)
	controlCharRe = regexp.MustCompile(`[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]`)
)

// SanitizeOutput strips ANSI escape sequences and non-printable control
// characters from tool output to prevent terminal injection attacks.
func SanitizeOutput(s string) string {
	s = ansiEscapeRe.ReplaceAllString(s, "")
	s = controlCharRe.ReplaceAllString(s, "")
	return s
}

type ToolState int

const (
	ToolRunning ToolState = iota
	ToolSuccess
	ToolError
)

type ToolCard struct {
	toolName   string
	input      string
	output     string
	state      ToolState
	durationMs int64
	truncated  bool
	theme      theme.Theme
	collapsed  bool
	renderer   ToolRenderer
}

func NewToolCard(call types.ToolCall, result *types.ToolResult, state ToolState, t theme.Theme) *ToolCard {
	renderer := RendererForTool(call.Name, t)
	input := renderer.RenderInput(call, 0) // width doesn't matter for input extraction
	output := ""
	truncated := false
	isBinary := false
	if result != nil {
		output = result.Output
		truncated = result.Truncated
		// Check for binary before sanitizing (SanitizeOutput strips null bytes)
		if isBinaryContent(output) {
			output = "[binary content]"
		} else {
			output = SanitizeOutput(output)
		}
	}

	tc := &ToolCard{
		toolName:   call.Name,
		input:      input,
		output:     output,
		state:      state,
		truncated:  truncated,
		theme:      t,
		collapsed:  isBinary,
		renderer:   renderer,
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

	return tc
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

	header := c.renderer.RenderHeader(cardWidth)

	var contentParts []string
	contentParts = append(contentParts, header)

	if c.input != "" {
		inputStyle := lipgloss.NewStyle().
			Foreground(c.theme.TextPrimary).
			Width(cardWidth).
			Padding(0, 1)
		contentParts = append(contentParts, inputStyle.Render(c.input))
	}

	// Build a synthetic result for the renderer
	var result *types.ToolResult
	if c.output != "" || c.state != ToolRunning {
		result = &types.ToolResult{
			Output:     c.output,
			DurationMs: c.durationMs,
		}
	}

	outputBlock := c.renderer.RenderOutput(result, c.state, c.durationMs, c.truncated, c.collapsed, cardWidth)
	if outputBlock != "" {
		contentParts = append(contentParts, outputBlock)
	}

	content := lipgloss.JoinVertical(lipgloss.Top, contentParts...)

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

