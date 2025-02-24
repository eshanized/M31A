package tui

import (
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

func RenderStatusBar(t theme.Theme, operation string, lastActivity time.Time, width int) string {
	if width < 10 {
		return ""
	}

	var leftText string
	if operation == "" {
		leftText = lipgloss.NewStyle().Foreground(t.TextSecondary).Render("Ready")
	} else {
		leftText = t.ModelBadge.Render(operation)
	}

	var rightText string
	if !lastActivity.IsZero() {
		rightText = lipgloss.NewStyle().Foreground(t.TextSecondary).Render(fmt.Sprintf("Last activity: %s", lastActivity.Format("15:04:05")))
	}

	leftWidth := lipgloss.Width(leftText)
	rightWidth := lipgloss.Width(rightText)

	padding := width - leftWidth - rightWidth
	if padding < 0 {
		maxOpWidth := width - rightWidth - 4
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
		padding = width - leftWidth - rightWidth
		if padding < 0 {
			padding = 0
		}
	}

	return leftText + strings.Repeat(" ", padding) + rightText
}
