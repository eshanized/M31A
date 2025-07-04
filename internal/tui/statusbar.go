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
	PromptTokens  int
	TotalTokens   int
	Cost          float64
	ShowCost      bool
	WhichKey      string   // which-key hint text to show
	LeaderActive  bool     // if true, show leader prompt instead of normal content
	AgentName     string   // agent/model name for metadata row
	ModelName     string   // model name
	ProviderName  string   // provider short name
	IsStreaming   bool     // currently streaming
	IsThinking    bool     // currently in thinking mode
	KeyboardHints []string // e.g. ["ctrl+p commands", "ctrl+b sidebar"]
}

func RenderStatusBar(t theme.Theme, operation string, lastActivity time.Time, width int, info *StatusBarInfo) string {
	if width < 10 {
		return ""
	}

	// Build left side: operation status or streaming indicator
	var leftText string
	if info != nil && info.LeaderActive {
		leftText = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("ctrl+x") +
			lipgloss.NewStyle().Foreground(t.TextMuted).Render(" \u2500 waiting \u2500")
	} else if info != nil && info.IsStreaming {
		spinner := t.Spinner.Render("\u22EF")
		leftText = spinner + " " + lipgloss.NewStyle().Foreground(t.TextMuted).Render("building...")
		if info.IsThinking {
			leftText = t.Spinner.Render("\u22EF") + " " + lipgloss.NewStyle().Foreground(t.Thinking).Italic(true).Render("thinking...")
		}
	} else if operation == "" {
		leftText = lipgloss.NewStyle().Foreground(t.TextMuted).Render("Ready")
	} else {
		leftText = lipgloss.NewStyle().Foreground(t.TextMuted).Render(operation)
	}

	// Append which-key hints to left side if available
	if info != nil && info.WhichKey != "" {
		leftText = leftText + "  " + lipgloss.NewStyle().Foreground(t.TextMuted).Render(info.WhichKey)
	}

	// Build right side: keyboard hints + usage
	var rightParts []string
	if info != nil && len(info.KeyboardHints) > 0 {
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
		// Drop right side entirely if no room
		rightText = ""
		rightWidth = 0
		padding = width - leftWidth
		if padding < 0 {
			maxOpWidth := width - 4
			if maxOpWidth < 0 {
				maxOpWidth = 0
			}
			shortOp := operation
			if len(shortOp) > maxOpWidth {
				if maxOpWidth > 3 {
					shortOp = shortOp[:maxOpWidth-3] + "..."
				} else {
					shortOp = ""
				}
			}
			if shortOp == "" {
				leftText = ""
			} else {
				leftText = lipgloss.NewStyle().Foreground(t.TextMuted).Render(shortOp)
			}
			leftWidth = lipgloss.Width(leftText)
			padding = width - leftWidth
			rightText = ""
			rightWidth = 0
		}
	}

	if rightWidth > 0 {
		return leftText + strings.Repeat(" ", padding) + rightText
	}
	return leftText + strings.Repeat(" ", padding)
}

// RenderPromptMetadata renders the agent · model · provider row inside the prompt area.
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

	separator := lipgloss.NewStyle().Foreground(t.TextMuted).Render("\u00B7")
	var result strings.Builder
	for i, p := range parts {
		if i > 0 {
			result.WriteString(" ")
			result.WriteString(separator)
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

// RenderPromptBottomBorder renders the bottom border continuation line (╹▀▀▀...).
func RenderPromptBottomBorder(color lipgloss.Color, width int) string {
	if width < 1 {
		return ""
	}
	left := lipgloss.NewStyle().Foreground(color).Render("\u2579")                          // ╹
	fill := lipgloss.NewStyle().Foreground(color).Render(strings.Repeat("\u2580", width-1)) // ▀
	return left + fill
}

func formatTokenCount(n int) string {
	if n >= 1000 {
		return fmt.Sprintf("%.1fK ctx", float64(n)/1000)
	}
	return fmt.Sprintf("%d ctx", n)
}
