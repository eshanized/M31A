package tui

import (
	"fmt"
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
	KeyboardHints    []string
	WorkflowPhase    string
	QuestionProgress string
	CwdName          string // basename of working directory
	GitBranch        string // current git branch
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
	switch {
	case info.LeaderActive:
		centerText = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("ctrl+x") +
			lipgloss.NewStyle().Foreground(t.TextMuted).Render(" ─ waiting ─")
	case info.IsThinking:
		centerText = t.Spinner.Render("⋯") + " " +
			lipgloss.NewStyle().Foreground(t.Thinking).Italic(true).Render("thinking...")
	case info.IsStreaming:
		centerText = t.Spinner.Render("⋯") + " " +
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
			costStr := fmt.Sprintf("$%.4f", info.Cost)
			rightParts = append(rightParts,
				lipgloss.NewStyle().Foreground(t.Warning).Render(costStr))
		}
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
		rightText = ""
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
		centerText = ""
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
