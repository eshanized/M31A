package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/types"
)

// renderModelList renders the scrollable list of models with pricing columns.
func (ms *ModelSelector) renderModelList() string {
	if ms.loading {
		return lipgloss.NewStyle().
			Foreground(ms.theme.TextMuted).
			PaddingLeft(2).
			Render("Loading models...")
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

	colWidths := ms.columnWidths()
	var rows []string
	for i, m := range visible {
		globalIdx := ms.offset + i
		selected := globalIdx == ms.cursor
		rows = append(rows, ms.renderModelRow(m, selected, colWidths))
	}
	return strings.Join(rows, "\n")
}

// columnWidths returns [nameW, pricingW] based on terminal width.
func (ms *ModelSelector) columnWidths() [2]int {
	avail := ms.width - 6
	if avail < 30 {
		avail = 30
	}
	priceW := 20
	nameW := avail - priceW
	if nameW < 20 {
		nameW = 20
	}
	return [2]int{nameW, priceW}
}

// renderModelRow renders a single model row.
func (ms *ModelSelector) renderModelRow(m types.ModelInfo, selected bool, colWidths [2]int) string {
	t := ms.theme
	nameW := colWidths[0]
	priceW := colWidths[1]

	displayName := m.Name
	if displayName == "" {
		displayName = m.ID
	}
	provBadge := " [" + ProviderShortName(m.Provider) + "]"
	nameCell := TruncateWithEllipsis(displayName+provBadge, nameW)

	pricing := ""
	if m.Pricing.InputPerMToken > 0 || m.Pricing.OutputPerMToken > 0 {
		pricing = lipgloss.NewStyle().Foreground(t.TextMuted).
			Render(formatModelPricing(m.Pricing.InputPerMToken, m.Pricing.OutputPerMToken))
	}
	pricingCell := lipgloss.NewStyle().Width(priceW).Render(pricing)

	var nameStyle lipgloss.Style
	if selected {
		nameStyle = lipgloss.NewStyle().
			Foreground(t.Brand).
			Bold(true).
			Width(nameW)
	} else {
		nameStyle = lipgloss.NewStyle().
			Foreground(t.Text).
			Width(nameW)
	}

	prefix := "  "
	if selected {
		prefix = lipgloss.NewStyle().Foreground(t.Brand).Render("▶ ")
	}

	return prefix + nameStyle.Render(nameCell) + pricingCell
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
	return fmt.Sprintf("$%.2f/%.2f", inputPM, outputPM)
}
