package tui

import (
	"fmt"
	"strings"
	"time"

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
}

// RenderStatusBar renders the status bar line at the bottom of the terminal.
func RenderStatusBar(t theme.Theme, operation string, lastActivity time.Time, width int, info *StatusBarInfo) string {
	if width < 10 {
		return ""
	}

	// Build left side
	var leftText string
	if info != nil && info.LeaderActive {
		leftText = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("ctrl+x") +
			lipgloss.NewStyle().Foreground(t.TextMuted).Render(" ─ waiting ─")
	} else if info != nil && info.IsThinking {
		leftText = t.Spinner.Render("⋯") + " " +
			lipgloss.NewStyle().Foreground(t.Thinking).Italic(true).Render("thinking...")
	} else if info != nil && info.IsStreaming {
		leftText = t.Spinner.Render("⋯") + " " +
			lipgloss.NewStyle().Foreground(t.TextMuted).Render("building...")
	} else if info != nil && info.WorkflowPhase != "" {
		phaseText := "▸ " + info.WorkflowPhase
		if info.QuestionProgress != "" {
			phaseText += " · " + info.QuestionProgress
		}
		leftText = lipgloss.NewStyle().Foreground(t.Brand).Render(phaseText)
	} else if operation == "" {
		leftText = lipgloss.NewStyle().Foreground(t.TextMuted).Render("Ready")
	} else {
		leftText = lipgloss.NewStyle().Foreground(t.TextMuted).Render(operation)
	}

	if info != nil && info.WhichKey != "" {
		leftText = leftText + "  " + lipgloss.NewStyle().Foreground(t.TextMuted).Render(info.WhichKey)
	}

	// Build right side: keyboard hints + usage
	var rightParts []string
	if info != nil {
		for _, hint := range info.KeyboardHints {
			rightParts = append(rightParts, lipgloss.NewStyle().Foreground(t.TextMuted).Render(hint))
		}
	}
	if info != nil && info.ShowCost && info.TotalTokens > 0 {
		tokStr := formatTokenCount(info.TotalTokens)
		rightParts = append(rightParts, lipgloss.NewStyle().Foreground(t.TextMuted).Render(tokStr))
		if info.Cost > 0 {
			costStr := fmt.Sprintf("$%.4f", info.Cost)
			rightParts = append(rightParts, lipgloss.NewStyle().Foreground(t.Warning).Render(costStr))
		}
	}

	rightText := strings.Join(rightParts, "  ")

	leftWidth := lipgloss.Width(leftText)
	rightWidth := lipgloss.Width(rightText)
	padding := width - leftWidth - rightWidth

	if padding < 0 {
		rightText = ""
		rightWidth = 0
		padding = width - leftWidth
		if padding < 0 {
			maxLeft := width - 2
			if maxLeft < 0 {
				maxLeft = 0
			}
			leftText = TruncateWithEllipsis(leftText, maxLeft)
			leftWidth = lipgloss.Width(leftText)
			padding = width - leftWidth
			if padding < 0 {
				padding = 0
			}
		}
	}

	if rightWidth > 0 {
		return leftText + strings.Repeat(" ", padding) + rightText
	}
	return leftText + strings.Repeat(" ", padding)
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
