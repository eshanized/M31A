package tui

import (
	"fmt"
	"math"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// StatusBarInfo carries optional info to render in the status bar.
type StatusBarInfo struct {
	PromptTokens     int
	TotalTokens      int
	Cost             float64
	ShowCost         bool
	WhichKey         string
	LeaderActive     bool
	AgentName        string
	ModelName        string
	ProviderName     string
	IsStreaming      bool
	IsThinking       bool
	ThinkingDuration int64 // milliseconds of current thinking session
	KeyboardHints    []string
	WorkflowPhase    string
	QuestionProgress string
	CwdName          string // basename of working directory
	GitBranch        string // current git branch
	SpinnerFrame     string // animated spinner frame (empty = use static char)
	ContextUsed      int    // tokens used in context window
	ContextMax       int    // model's max context length (0 = unknown)
}

// RenderStatusBar renders the status bar line at the bottom of the terminal.
// Uses a clean 3-zone layout:
//
//	left (cwd + branch) · center (operation) · right (hints + cost)
//
// No background fill — inherits terminal background.
func RenderStatusBar(t theme.Theme, width int, info *StatusBarInfo) string {
	if width < 10 {
		return ""
	}

	if info == nil {
		info = &StatusBarInfo{}
	}

	// Clone info to avoid mutating the original (narrow terminal adaptations)
	cloned := *info
	info = &cloned

	// ── Compact mode ──────────────────────────────────────────────────────────
	// Narrow terminal (< 80 cols): hide hints, hide cost, shorten cwd.
	if width < 80 {
		info.KeyboardHints = nil
		info.ShowCost = false
		if info.CwdName != "" {
			parts := strings.Split(info.CwdName, "/")
			info.CwdName = parts[len(parts)-1]
		}
	}

	// Ultra-narrow (< 60 cols): hide git branch.
	if width < 60 {
		info.GitBranch = ""
	}

	// ── Left zone: cwd + branch ───────────────────────────────────────────────
	var leftParts []string
	if info.CwdName != "" {
		leftParts = append(leftParts,
			lipgloss.NewStyle().Foreground(t.TextMuted).Render("⌂ "+info.CwdName))
	}
	if info.GitBranch != "" {
		leftParts = append(leftParts,
			lipgloss.NewStyle().Foreground(t.TextSecondary).Render("⎇ "+info.GitBranch))
	}
	leftText := strings.Join(leftParts, "  ")

	// ── Center zone: operation status ─────────────────────────────────────────
	var centerText string
	spinnerChar := info.SpinnerFrame
	if spinnerChar == "" {
		spinnerChar = "⋯"
	}
	switch {
	case info.LeaderActive:
		centerText = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("ctrl+x") +
			lipgloss.NewStyle().Foreground(t.TextMuted).Render(" ─ waiting ─")
	case info.IsThinking:
		thinkingLabel := "thinking..."
		if info.ThinkingDuration > 0 {
			thinkingLabel = "thinking· " + formatDurationMs(info.ThinkingDuration)
		}
		centerText = t.Spinner.Render(spinnerChar) + " " +
			lipgloss.NewStyle().Foreground(t.Thinking).Italic(true).Render(thinkingLabel)
	case info.IsStreaming:
		centerText = t.Spinner.Render(spinnerChar) + " " +
			lipgloss.NewStyle().Foreground(t.TextMuted).Render("responding...")
	case info.WorkflowPhase != "":
		phaseText := "▸ " + info.WorkflowPhase
		if info.QuestionProgress != "" {
			phaseText += " · " + info.QuestionProgress
		}
		centerText = lipgloss.NewStyle().Foreground(t.Brand).Render(phaseText)
	case info.WhichKey != "":
		centerText = lipgloss.NewStyle().Foreground(t.TextMuted).Render(info.WhichKey)
	}
	// When idle and no operation, centerText stays empty (no "Ready" label).

	// ── Right zone: hints + cost/tokens ──────────────────────────────────────
	var rightParts []string
	for _, hint := range info.KeyboardHints {
		rightParts = append(rightParts,
			lipgloss.NewStyle().Foreground(t.TextMuted).Render(hint))
	}
	if info.ShowCost && info.TotalTokens > 0 {
		tokStr := formatTokenCount(info.TotalTokens)
		rightParts = append(rightParts,
			lipgloss.NewStyle().Foreground(t.TextMuted).Render(tokStr))
		if info.Cost > 0 {
			var costStr string
			if info.Cost < 0.01 {
				costStr = "<$0.01"
			} else {
				costStr = fmt.Sprintf("$%.2f", info.Cost)
			}
			rightParts = append(rightParts,
				lipgloss.NewStyle().Foreground(t.Warning).Render(costStr))
		}
	}
	if info.ContextMax > 0 && info.ContextUsed > 0 {
		rightParts = append(rightParts, renderContextRing(info.ContextUsed, info.ContextMax, t))
	}
	rightText := strings.Join(rightParts, "  ")

	// ── Assemble with separators ─────────────────────────────────────────────
	// Only show separators between non-empty zones.
	var displayParts []string
	if leftText != "" {
		displayParts = append(displayParts, leftText)
	}
	if centerText != "" {
		displayParts = append(displayParts, centerText)
	}
	if rightText != "" {
		displayParts = append(displayParts, rightText)
	}

	result := strings.Join(displayParts, " · ")
	resultWidth := lipgloss.Width(result)

	// ── Overflow: drop right zone first, then center, then truncate left ─────
	if resultWidth > width && rightText != "" {
		displayParts = nil
		if leftText != "" {
			displayParts = append(displayParts, leftText)
		}
		if centerText != "" {
			displayParts = append(displayParts, centerText)
		}
		result = strings.Join(displayParts, " · ")
		resultWidth = lipgloss.Width(result)
	}

	if resultWidth > width && centerText != "" {
		displayParts = nil
		if leftText != "" {
			displayParts = append(displayParts, leftText)
		}
		result = strings.Join(displayParts, " · ")
		resultWidth = lipgloss.Width(result)
	}

	if resultWidth > width && leftText != "" {
		maxLeft := width - 1
		if maxLeft < 1 {
			maxLeft = 1
		}
		result = TruncateWithEllipsis(leftText, maxLeft)
		resultWidth = lipgloss.Width(result)
	}

	padding := width - resultWidth
	if padding < 0 {
		padding = 0
	}

	return result + strings.Repeat(" ", padding)
}

// RenderPromptMetadata renders the agent · model · provider row above the textarea.
func RenderPromptMetadata(agentName, modelName, providerName string, t theme.Theme, width int) string {
	var parts []string
	if agentName != "" {
		parts = append(parts, lipgloss.NewStyle().Foreground(t.Text).Render(agentName))
	}
	if modelName != "" {
		parts = append(parts, lipgloss.NewStyle().Foreground(t.TextMuted).Render(modelName))
	}
	if providerName != "" {
		providerShort := ProviderShortName(providerName)
		parts = append(parts, lipgloss.NewStyle().Foreground(t.TextMuted).Render("["+providerShort+"]"))
	}
	if len(parts) == 0 {
		return ""
	}

	sep := lipgloss.NewStyle().Foreground(t.TextMuted).Render("·")
	var result strings.Builder
	for i, p := range parts {
		if i > 0 {
			result.WriteString(" ")
			result.WriteString(sep)
			result.WriteString(" ")
		}
		result.WriteString(p)
	}

	return lipgloss.NewStyle().
		Foreground(t.TextMuted).
		PaddingTop(1).
		Width(width).
		Render(result.String())
}

// RenderPromptBottomBorder renders the border line below the input frame.
func RenderPromptBottomBorder(color lipgloss.Color, width int) string {
	if width < 1 {
		return ""
	}
	left := lipgloss.NewStyle().Foreground(color).Render("╹")
	fill := lipgloss.NewStyle().Foreground(color).Render(strings.Repeat("▀", width-1))
	return left + fill
}

func formatTokenCount(n int) string {
	if n >= 1000 {
		return fmt.Sprintf("%.1fK ctx", float64(n)/1000)
	}
	return fmt.Sprintf("%d ctx", n)
}

// renderContextRing renders a compact context window usage bar:
//
//	ctx [████░░░░] 42%
//
// Color shifts from brand → warning → error as usage increases.
func renderContextRing(used, total int, t theme.Theme) string {
	if total <= 0 {
		return ""
	}
	pct := float64(used) / float64(total)
	if pct > 1.0 {
		pct = 1.0
	}

	var barColor lipgloss.Color
	switch {
	case pct >= 0.9:
		barColor = t.Error
	case pct >= 0.7:
		barColor = t.Warning
	default:
		barColor = t.Brand
	}

	// PERF-43: Increased from 8 to 16 segments for finer granularity
	const segments = 16
	filled := int(math.Round(pct * segments))
	if filled > segments {
		filled = segments
	}
	if filled < 0 {
		filled = 0
	}

	bar := "[" + strings.Repeat("█", filled) + strings.Repeat("░", segments-filled) + "]"

	return lipgloss.NewStyle().Foreground(barColor).Render(
		fmt.Sprintf("ctx %s %d%%", bar, int(pct*100)),
	)
}
