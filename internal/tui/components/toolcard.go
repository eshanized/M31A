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

// ToolIcons maps tool names to icon characters for inline rendering.
var ToolIcons = map[string]string{
	"Bash":      "$",
	"Edit":      "\u2190",
	"FileRead":  "\u2192",
	"FileWrite": "\u2190",
	"Glob":      "\u2731",
	"Grep":      "\u2731",
	"TodoWrite": "\u2699",
	"Question":  "?",
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
	input := renderer.RenderInput(call, 0)
	output := ""
	truncated := false
	isBinary := false
	if result != nil {
		output = result.Output
		truncated = result.Truncated
		if isBinaryContent(output) {
			isBinary = true
			output = "[binary content]"
		} else {
			output = SanitizeOutput(output)
		}
	}

	tc := &ToolCard{
		toolName:  call.Name,
		input:     input,
		output:    output,
		state:     state,
		truncated: truncated,
		theme:     t,
		collapsed: isBinary,
		renderer:  renderer,
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

	// Collapsed or running without output: render inline single-line
	if c.collapsed || (c.output == "" && c.state == ToolRunning) {
		return c.renderInline(cardWidth)
	}

	// Expanded with output: render as left-bordered block
	return c.renderBlock(cardWidth)
}

// renderInline renders a tool as a single-line inline element with paddingLeft=3.
func (c *ToolCard) renderInline(width int) string {
	icon := ToolIcons[c.toolName]
	if icon == "" {
		icon = "\u2022"
	}

	var desc string
	switch c.state {
	case ToolRunning:
		desc = lipgloss.NewStyle().Foreground(c.theme.TextMuted).Render("running...")
	case ToolSuccess:
		if c.truncated {
			desc = lipgloss.NewStyle().Foreground(c.theme.Warning).Render("completed (truncated)")
		} else {
			desc = lipgloss.NewStyle().Foreground(c.theme.TextMuted).Render("completed")
		}
	case ToolError:
		desc = lipgloss.NewStyle().Foreground(c.theme.Error).Render("failed")
	}

	parts := []string{
		lipgloss.NewStyle().Foreground(c.theme.Text).Render(icon),
		lipgloss.NewStyle().Foreground(c.theme.Text).Render(c.toolName),
	}
	if c.input != "" {
		short := c.input
		if len(short) > 60 {
			short = short[:57] + "..."
		}
		short = strings.ReplaceAll(short, "\n", " ")
		parts = append(parts, lipgloss.NewStyle().Foreground(c.theme.TextMuted).Render(short))
	}
	parts = append(parts, desc)

	line := lipgloss.JoinHorizontal(lipgloss.Top, parts...)

	return lipgloss.NewStyle().
		PaddingLeft(3).
		Width(width).
		Render(line)
}

// renderBlock renders a tool as a left-bordered block with background.
func (c *ToolCard) renderBlock(width int) string {
	header := c.renderer.RenderHeader(width)

	var contentParts []string
	contentParts = append(contentParts, header)

	if c.input != "" {
		inputStyle := lipgloss.NewStyle().
			Foreground(c.theme.Text).
			Width(width).
			PaddingLeft(2)
		contentParts = append(contentParts, inputStyle.Render(c.input))
	}

	var result *types.ToolResult
	if c.output != "" || c.state != ToolRunning {
		result = &types.ToolResult{
			Output:     c.output,
			DurationMs: c.durationMs,
		}
	}

	outputBlock := c.renderer.RenderOutput(result, c.state, c.durationMs, c.truncated, c.collapsed, width)
	if outputBlock != "" {
		contentParts = append(contentParts, outputBlock)
	}

	content := lipgloss.JoinVertical(lipgloss.Top, contentParts...)

	blockStyle := lipgloss.NewStyle().
		Border(theme.SplitBorder, true, false, false, false).
		BorderForeground(c.theme.Border).
		Background(c.theme.BackgroundPanel).
		Padding(1, 2).
		MarginTop(1).
		Width(width + 4)

	return blockStyle.Render(content)
}

func (c *ToolCard) Toggle() {
	c.collapsed = !c.collapsed
}

// SetCollapsed sets the collapsed state of the tool card.
// M-17: used to honor AutoCollapseTools config flag.
func (c *ToolCard) SetCollapsed(v bool) {
	c.collapsed = v
}

func (c *ToolCard) IsCollapsed() bool {
	return c.collapsed
}
