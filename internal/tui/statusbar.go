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
	WhichKey      string // which-key hint text to show
	LeaderActive  bool   // if true, show leader prompt instead of normal content
}

func RenderStatusBar(t theme.Theme, operation string, lastActivity time.Time, width int, info *StatusBarInfo) string {
	if width < 10 {
		return ""
	}

	var leftText string
	if info != nil && info.LeaderActive {
		leftText = lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("ctrl+x") +
			lipgloss.NewStyle().Foreground(t.TextSecondary).Render(" ─ waiting ─")
	} else if operation == "" {
		leftText = lipgloss.NewStyle().Foreground(t.TextSecondary).Render("Ready")
	} else {
		leftText = t.ModelBadge.Render(operation)
	}

	// Append which-key hints to left side if available
	if info != nil && info.WhichKey != "" {
		leftText = leftText + "  " + lipgloss.NewStyle().Foreground(t.TextSecondary).Render(info.WhichKey)
	}

	var rightParts []string
	if info != nil && info.ShowCost && info.TotalTokens > 0 {
		tokStr := formatTokenCount(info.TotalTokens)
		rightParts = append(rightParts, lipgloss.NewStyle().Foreground(t.TextSecondary).Render(tokStr))
		if info.Cost > 0 {
			costStr := fmt.Sprintf("$%.4f", info.Cost)
			rightParts = append(rightParts, lipgloss.NewStyle().Foreground(t.Warning).Render(costStr))
		}
	}
	if !lastActivity.IsZero() {
		rightParts = append(rightParts, lipgloss.NewStyle().Foreground(t.TextSecondary).Render(fmt.Sprintf("Last: %s", lastActivity.Format("15:04:05"))))
	}

	rightText := strings.Join(rightParts, "  ")

	leftWidth := lipgloss.Width(leftText)
	rightWidth := lipgloss.Width(rightText)

	padding := width - leftWidth - rightWidth
	if padding < 0 {
		// Drop right side entirely if no room
		if width-leftWidth-4 > 0 {
			rightText = ""
			rightWidth = 0
			padding = width - leftWidth
		} else {
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
				leftText = t.ModelBadge.Render(shortOp)
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

func formatTokenCount(n int) string {
	if n >= 1000 {
		return fmt.Sprintf("%.1fK ctx", float64(n)/1000)
	}
	return fmt.Sprintf("%d ctx", n)
}
