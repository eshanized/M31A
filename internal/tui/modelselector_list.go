package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/types"
)

// modelselector_list.go — model list rendering for the Model Selector.

// renderModelList renders the scrollable list of models with compact rows.
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

	var rows []string
	for i, m := range visible {
		globalIdx := ms.offset + i
		selected := globalIdx == ms.cursor
		rows = append(rows, ms.renderModelRow(m, selected))
	}
	return strings.Join(rows, "\n")
}

// renderModelRow renders a single compact model row with name, pricing, context, and capabilities.
func (ms *ModelSelector) renderModelRow(m types.ModelInfo, selected bool) string {
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
	provBadge := " [" + ProviderShortName(m.Provider) + "]"

	// Context length
	ctxStr := ""
	if m.ContextLength > 0 {
		ctxStr = fmt.Sprintf("%dK ctx", m.ContextLength/1000)
	}

	// Pricing
	pricingStr := ""
	if m.Pricing.InputPerMToken > 0 || m.Pricing.OutputPerMToken > 0 {
		pricingStr = formatModelPricing(m.Pricing.InputPerMToken, m.Pricing.OutputPerMToken)
	}

	// Capability badges
	var capBadges []string
	if m.Capabilities.Reasoning {
		capBadges = append(capBadges, "⚡")
	}
	if m.Capabilities.Vision {
		capBadges = append(capBadges, "👁")
	}

	// Build row parts
	var parts []string
	parts = append(parts, displayName+provBadge)
	if ctxStr != "" {
		parts = append(parts, ctxStr)
	}
	if pricingStr != "" {
		parts = append(parts, pricingStr)
	}
	if len(capBadges) > 0 {
		parts = append(parts, strings.Join(capBadges, " "))
	}

	rowContent := strings.Join(parts, "  ")

	// Truncate to available width
	if len(rowContent) > avail {
		rowContent = rowContent[:avail-3] + "..."
	}

	var nameStyle lipgloss.Style
	if selected {
		nameStyle = lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
	} else {
		nameStyle = lipgloss.NewStyle().Foreground(t.Text)
	}

	prefix := "  "
	if selected {
		prefix = lipgloss.NewStyle().Foreground(t.Brand).Render("▶ ")
	}

	return prefix + nameStyle.Render(rowContent)
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
