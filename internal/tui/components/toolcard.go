package components

import (
	"fmt"
	"regexp"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

var (
	ansiEscapeRe  = regexp.MustCompile(`\x1b\[[0-9;]*[a-zA-Z]`)
	mouseEventRe  = regexp.MustCompile(`\x1b\[<\d+;\d+;\d+[Mm]`)
	controlCharRe = regexp.MustCompile(`[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]`)
)

// StripANSI removes all ANSI escape sequences, mouse event codes, and
// non-printable control characters from a string. Use this before passing
// content to Glamour to prevent escape-sequence mangling.
func StripANSI(s string) string {
	s = mouseEventRe.ReplaceAllString(s, "")
	s = ansiEscapeRe.ReplaceAllString(s, "")
	s = controlCharRe.ReplaceAllString(s, "")
	return s
}

// SanitizeOutput strips ANSI escape sequences and non-printable control
// characters from tool output to prevent terminal injection attacks.
func SanitizeOutput(s string) string {
	return StripANSI(s)
}

// ToolIcons maps tool names to icon characters for inline rendering.
var ToolIcons = map[string]string{
	"Bash":            "$",
	"Edit":            "\u2190",
	"FileRead":        "\u2192",
	"FileWrite":       "\u2190",
	"Glob":            "\u2731",
	"Grep":            "\u2731",
	"TodoWrite":       "\u2699",
	"AskUserQuestion": "?",
}

// ToolStatusIcons maps tool state to status prefix characters.
var ToolStatusIcons = map[ToolState]string{
	ToolRunning: "\u27f3", // ⟳
	ToolSuccess: "\u2713", // ✓
	ToolError:   "\u2717", // ✗
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
	lineCount  int // output line count (for header display)

	flashUntil  time.Time      // border flash expiry timestamp
	flashColor  lipgloss.Color // border flash color
	completedAt time.Time      // when the tool completed (for age-based fading)
}

// ToolName returns the tool's identifier (e.g. "FileRead", "Bash").
func (c *ToolCard) ToolName() string { return c.toolName }

// Input returns the rendered input summary the card displays.
func (c *ToolCard) Input() string { return c.input }

// Output returns the rendered tool output body the card displays.
func (c *ToolCard) Output() string { return c.output }

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
			output = fmt.Sprintf("[binary content, %d bytes]", len(output))
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
	if state == ToolSuccess || state == ToolError {
		tc.completedAt = time.Now()
	}

	trimmed := strings.TrimRight(output, "\n")
	lineCount := strings.Count(trimmed, "\n") + 1
	if trimmed == "" {
		lineCount = 0
	}
	tc.lineCount = lineCount
	if lineCount > 50 {
		tc.collapsed = true
	}

	runeCount := utf8.RuneCountInString(output)
	if runeCount > types.MaxToolOutputChars {
		tc.output = string([]rune(output)[:types.MaxToolOutputChars])
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

	// Expanded with output: render as thin-border block
	return c.renderBlock(cardWidth)
}

// renderInline renders a tool as a single-line inline element:
//
//	⟳ [ FileRead ] reading path/to/file.go
func (c *ToolCard) renderInline(width int) string {
	// Tool name badge with per-tool colors
	labelStyle, ok := c.theme.ToolLabel[c.toolName]
	if !ok {
		labelStyle = lipgloss.NewStyle().
			Foreground(c.theme.TextSecondary).
			Background(c.theme.SurfaceElevated).
			Padding(0, 1).
			Bold(true)
	}
	badge := labelStyle.Render(c.toolName)

	// Input description
	inputStr := ""
	if c.input != "" {
		short := c.input
		short = strings.ReplaceAll(short, "\n", " ")
		short = TruncateEnd(short, 50)
		inputStr = lipgloss.NewStyle().Foreground(c.theme.TextMuted).Render(short)
	}

	// Status indicator
	var statusStr string
	switch c.state {
	case ToolRunning:
		statusStr = lipgloss.NewStyle().Foreground(c.theme.Warning).Render("⟳")
	case ToolSuccess:
		if c.truncated {
			statusStr = lipgloss.NewStyle().Foreground(c.theme.Warning).Render("✓ truncated")
		} else if c.collapsed && c.output != "" {
			lineCount := strings.Count(c.output, "\n") + 1
			statusStr = lipgloss.NewStyle().Foreground(c.theme.TextMuted).Render(fmt.Sprintf("✓ +%d lines", lineCount))
		} else {
			statusStr = lipgloss.NewStyle().Foreground(c.theme.Success).Render("✓")
		}
	case ToolError:
		statusStr = lipgloss.NewStyle().Foreground(c.theme.Error).Render("✗ failed")
	}

	// Assemble: badge + input + status
	line := badge
	if inputStr != "" {
		line += " " + inputStr
	}
	line += " " + statusStr

	return lipgloss.NewStyle().
		PaddingLeft(3).
		Width(width).
		Render(line)
}

// renderBlock renders a tool as a thin-border block with status and timing.
// Uses ThinBorder (┌─┐) — opencode-style compact tool cards.
func (c *ToolCard) renderBlock(width int) string {
	// Build single-line header: status icon + tool badge + truncated input + timing
	header := c.renderThinBorderHeader(width)

	// Content body: 2-char left padding, no inner border
	var bodyLines []string

	if c.input != "" {
		inputStr := lipgloss.NewStyle().
			Foreground(c.theme.TextMuted).
			Render(c.input)
		bodyLines = append(bodyLines, inputStr)
	}

	if c.output != "" {
		result := &types.ToolResult{
			Output:     c.output,
			DurationMs: c.durationMs,
			Truncated:  c.truncated,
		}
		outputStr := c.renderer.RenderOutput(result, c.state, c.durationMs, c.truncated, c.collapsed, width-4)
		if outputStr != "" {
			bodyLines = append(bodyLines, outputStr)
		}
	}

	var body string
	if len(bodyLines) > 0 {
		body = lipgloss.JoinVertical(lipgloss.Top, bodyLines...)
		// Indent body with 2-char padding
		body = lipgloss.NewStyle().PaddingLeft(2).Render(body)
	}

	content := lipgloss.JoinVertical(lipgloss.Top, header, body)

	// ThinBorder: ┌─┐ for clean, compact appearance
	blockStyle := lipgloss.NewStyle().
		Border(theme.ThinBorder).
		BorderForeground(c.getBorderColor()).
		Padding(0, 1).
		MarginTop(1).
		Width(width + 4)
	if c.isFaded() {
		blockStyle = blockStyle.Faint(true)
	}

	return blockStyle.Render(content)
}

// renderThinBorderHeader renders a single-line tool card header:
//
//	┌─ Bash ── input ... ──────────────── ✓ 120ms ─┐
//	(tool badge + truncated input on left, status icon + timing on right)
func (c *ToolCard) renderThinBorderHeader(width int) string {
	// Tool label badge (use the pre-existing per-tool label colors)
	labelStyle, ok := c.theme.ToolLabel[c.toolName]
	if !ok {
		labelStyle = lipgloss.NewStyle().
			Background(c.theme.TextSecondary).
			Foreground(c.theme.BadgeForeground).
			Padding(0, 1).
			Bold(true)
	}
	label := labelStyle.Render(fmt.Sprintf(" %s ", c.toolName))

	// Input snippet (truncated for inline display)
	inputSnippet := ""
	if c.input != "" {
		short := c.input
		short = strings.ReplaceAll(short, "\n", " ")
		short = TruncateEnd(short, 50)
		inputSnippet = " " + lipgloss.NewStyle().Foreground(c.theme.TextMuted).Render(short)
	}

	// Status icon (right side)
	statusIcon := ToolStatusIcons[c.state]
	statusColor := c.theme.TextMuted
	switch c.state {
	case ToolSuccess:
		statusColor = c.theme.Success
	case ToolError:
		statusColor = c.theme.Error
	case ToolRunning:
		statusColor = c.theme.Warning
	}
	statusStr := lipgloss.NewStyle().Foreground(statusColor).Render(statusIcon)

	// Right-side info: timing + line count
	var infoParts []string
	if c.durationMs > 0 {
		dur := fmt.Sprintf("%.0fms", float64(c.durationMs))
		infoParts = append(infoParts, dur)
	}
	if c.lineCount > 0 && !c.collapsed {
		infoParts = append(infoParts, fmt.Sprintf("%d lines", c.lineCount))
	}
	if c.truncated {
		infoParts = append(infoParts, "truncated")
	}
	infoStr := ""
	if len(infoParts) > 0 {
		infoStr = lipgloss.NewStyle().Foreground(c.theme.TextMuted).Render(strings.Join(infoParts, " · "))
	}

	// Assemble left side: tool badge + input
	left := lipgloss.JoinHorizontal(lipgloss.Top, label, inputSnippet)
	// Assemble right side: status + timing
	var rightParts []string
	if infoStr != "" {
		rightParts = append(rightParts, infoStr)
	}
	rightParts = append(rightParts, statusStr)
	right := strings.Join(rightParts, " ")

	// Calculate filler to push right side to the edge
	leftWidth := lipgloss.Width(left)
	rightWidth := lipgloss.Width(right)
	filler := width - leftWidth - rightWidth - 4 // -4 for border chars + padding
	if filler < 0 {
		filler = 0
	}

	return lipgloss.JoinHorizontal(lipgloss.Top,
		left,
		strings.Repeat(" ", filler),
		right,
	)
}

// getBorderColor returns the appropriate border color based on tool state.
func (c *ToolCard) getBorderColor() lipgloss.Color {
	if !c.flashUntil.IsZero() && time.Now().Before(c.flashUntil) {
		return c.flashColor
	}
	switch c.state {
	case ToolRunning:
		return c.theme.Warning
	case ToolSuccess:
		return c.theme.Border
	case ToolError:
		return c.theme.Error
	default:
		return c.theme.Border
	}
}

// StartFlash begins a transient border color flash (e.g., green on success, red on error).
func (c *ToolCard) StartFlash(color lipgloss.Color, dur time.Duration) {
	c.flashColor = color
	c.flashUntil = time.Now().Add(dur)
}

// MarkCompleted sets the completion timestamp and applies a flash effect.
func (c *ToolCard) MarkCompleted(state ToolState) {
	c.state = state
	c.completedAt = time.Now()
	switch state {
	case ToolSuccess:
		c.StartFlash(c.theme.Success, 300*time.Millisecond)
	case ToolError:
		c.StartFlash(c.theme.Error, 300*time.Millisecond)
	}
}

// isFaded returns true if the tool card completed more than 30 seconds ago.
func (c *ToolCard) isFaded() bool {
	if c.completedAt.IsZero() {
		return false
	}
	return time.Since(c.completedAt) > 30*time.Second
}

func (c *ToolCard) Toggle() {
	c.collapsed = !c.collapsed
}

func (c *ToolCard) SetCollapsed(v bool) {
	c.collapsed = v
}

func (c *ToolCard) IsCollapsed() bool {
	return c.collapsed
}
