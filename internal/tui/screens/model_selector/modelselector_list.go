package model_selector

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/tuitypes"
	"github.com/eshanized/M31A/internal/types"
)

// modelselector_list.go — model list rendering for the Model Selector.

// renderModelList renders the scrollable list of models as a table with aligned columns.
func (ms *ModelSelector) renderModelList() string {
	if ms.loading {
		spin := ms.spinner.Peek()
		return lipgloss.NewStyle().
			Foreground(ms.theme.Spinner.GetForeground()).
			PaddingLeft(2).
			Render(spin + " Loading models...")
	}
	if len(ms.filtered) == 0 {
		return lipgloss.NewStyle().
			Foreground(ms.theme.TextMuted).
			PaddingLeft(2).
			Render("No models found.")
	}

	listH := ms.visibleRows()
	end := ms.offset + listH
	if end > len(ms.filtered) {
		end = len(ms.filtered)
	}
	visible := ms.filtered[ms.offset:end]

	// Compute column widths for alignment.
	nameWidth, ctxWidth, priceWidth := ms.computeColumnWidths(visible)

	var rows []string
	for i, m := range visible {
		globalIdx := ms.offset + i
		selected := globalIdx == ms.cursor
		rows = append(rows, ms.renderModelRow(m, selected, nameWidth, ctxWidth, priceWidth))
	}
	return strings.Join(rows, "\n")
}

// computeColumnWidths calculates the max width for each table column.
func (ms *ModelSelector) computeColumnWidths(visible []types.ModelInfo) (nameW, ctxW, priceW int) {
	for _, m := range visible {
		displayName := m.Name
		if displayName == "" {
			displayName = m.ID
		}
		n := len(displayName + " [" + tuitypes.ProviderShortName(m.Provider) + "]")
		if n > nameW {
			nameW = n
		}
		if m.ContextLength > 0 {
			c := len(fmt.Sprintf("%dK ctx", m.ContextLength/1000))
			if c > ctxW {
				ctxW = c
			}
		}
		if m.Pricing.InputPerMToken > 0 || m.Pricing.OutputPerMToken > 0 {
			p := len(formatModelPricing(m.Pricing.InputPerMToken, m.Pricing.OutputPerMToken))
			if p > priceW {
				priceW = p
			}
		}
	}
	return
}

// renderModelRow renders a single table row with aligned columns.
func (ms *ModelSelector) renderModelRow(m types.ModelInfo, selected bool, nameW, ctxW, priceW int) string {
	t := ms.theme
	avail := ms.width - 10
	if avail < 40 {
		avail = 40
	}

	// Name + provider badge
	displayName := m.Name
	if displayName == "" {
		displayName = m.ID
	}
	nameCol := displayName + " [" + tuitypes.ProviderShortName(m.Provider) + "]"

	// Context length
	ctxCol := ""
	if m.ContextLength > 0 {
		ctxCol = fmt.Sprintf("%dK ctx", m.ContextLength/1000)
	}

	// Pricing
	priceCol := ""
	if m.Pricing.InputPerMToken > 0 || m.Pricing.OutputPerMToken > 0 {
		priceCol = formatModelPricing(m.Pricing.InputPerMToken, m.Pricing.OutputPerMToken)
	}

	// Capability badges
	var capBadges []string
	if m.Capabilities.Reasoning {
		capBadges = append(capBadges, "⚡")
	}
	if m.Capabilities.Vision {
		capBadges = append(capBadges, "👁")
	}

	// Pad columns to aligned widths.
	paddedName := padRight(nameCol, nameW)
	paddedCtx := padRight(ctxCol, ctxW)
	paddedPrice := padRight(priceCol, priceW)

	rowContent := paddedName + "  " + paddedCtx + "  " + paddedPrice
	if len(capBadges) > 0 {
		rowContent += "  " + strings.Join(capBadges, " ")
	}

	// Truncate to available width.
	if len(rowContent) > avail {
		rowContent = tuitypes.TruncateEnd(rowContent, avail)
	}

	var nameStyle lipgloss.Style
	if selected {
		nameStyle = lipgloss.NewStyle().Foreground(t.Text)
	} else {
		nameStyle = lipgloss.NewStyle().Foreground(t.TextMuted)
	}

	prefix := "  "
	if selected {
		prefix = lipgloss.NewStyle().Foreground(t.Brand).Render("▶ ")
	}

	return prefix + nameStyle.Render(rowContent)
}

// padRight pads a string to the given width with spaces.
func padRight(s string, width int) string {
	if len(s) >= width {
		return s
	}
	return s + strings.Repeat(" ", width-len(s))
}

// renderProviderTabs renders provider filter pills at the top.
func (ms *ModelSelector) renderProviderTabs() string {
	t := ms.theme
	all := append([]string{""}, ms.providers...)
	var parts []string
	for _, p := range all {
		label := p
		if label == "" {
			label = "All"
		}
		var s lipgloss.Style
		if p == ms.activeProvider {
			s = lipgloss.NewStyle().
				Foreground(t.Background).
				Background(t.Brand).
				Padding(0, 1).
				Bold(true)
		} else {
			s = lipgloss.NewStyle().
				Foreground(t.TextMuted).
				Padding(0, 1)
		}
		parts = append(parts, s.Render(label))
	}
	return strings.Join(parts, " ")
}

// formatModelPricing formats pricing per M tokens into a compact string.
func formatModelPricing(inputPM, outputPM float64) string {
	if inputPM == 0 && outputPM == 0 {
		return "free"
	}
	return fmt.Sprintf("$%.2f/%.2f/M", inputPM, outputPM)
}
