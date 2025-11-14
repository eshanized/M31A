package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// modelselector_view.go — view rendering for the Model Selector.

// renderView is the full View() implementation for ModelSelector.
func (ms *ModelSelector) renderView() string {
	t := ms.theme
	w := ms.width
	if w < 40 {
		w = 80
	}

	// ── Header ──────────────────────────────────────────────────────────────
	header := lipgloss.NewStyle().
		Foreground(t.Brand).
		Bold(true).
		PaddingLeft(1).
		Render("  Model Selector")

	divider := lipgloss.NewStyle().Foreground(t.Border).
		Render(strings.Repeat("─", w))

	// ── Provider filter pills ────────────────────────────────────────────────
	tabs := "  " + ms.renderProviderTabs()

	// ── Search box with ⌕ prefix ─────────────────────────────────────────────
	searchBox := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		PaddingLeft(2).
		Render("⌕ " + ms.searchInput.View())

	// ── Model list ───────────────────────────────────────────────────────────
	list := ms.renderModelList()

	// ── Detail pane ──────────────────────────────────────────────────────────
	detailPane := ms.renderDetailPane()

	// ── Error ────────────────────────────────────────────────────────────────
	errLine := ""
	if ms.errMsg != "" {
		errLine = lipgloss.NewStyle().Foreground(t.Error).PaddingLeft(2).
			Render("! " + ms.errMsg)
	}

	// ── Scroll indicator ─────────────────────────────────────────────────────
	scrollInfo := ""
	if len(ms.filtered) > 0 {
		scrollInfo = lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
			Render(fmt.Sprintf("%d/%d", ms.cursor+1, len(ms.filtered)))
	}

	// ── Footer ───────────────────────────────────────────────────────────────
	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("↵ select  tab filter  / search  esc back")

	// ── Layout: list + detail side by side if wide enough ────────────────────
	var mainContent string
	if detailPane != "" && w > 120 {
		// Split screen: list on left, detail on right
		listWidth := w * 3 / 5
		detailWidth := w * 2 / 5
		listPane := lipgloss.NewStyle().Width(listWidth).Render(
			lipgloss.JoinVertical(lipgloss.Left,
				tabs,
				searchBox,
				divider,
				list,
			))
		detailPane = lipgloss.NewStyle().Width(detailWidth).PaddingLeft(1).Render(detailPane)
		mainContent = lipgloss.JoinHorizontal(lipgloss.Top, listPane, detailPane)
	} else {
		mainContent = lipgloss.JoinVertical(lipgloss.Left,
			tabs,
			searchBox,
			divider,
			list,
		)
		if detailPane != "" {
			mainContent = lipgloss.JoinVertical(lipgloss.Left,
				mainContent,
				"",
				detailPane,
			)
		}
	}

	parts := []string{
		header,
		divider,
		mainContent,
	}
	if errLine != "" {
		parts = append(parts, errLine)
	}
	if scrollInfo != "" {
		parts = append(parts, scrollInfo)
	}
	parts = append(parts, divider, footer)

	return lipgloss.JoinVertical(lipgloss.Left, parts...)
}

// renderDetailPane renders a detail panel for the currently selected model.
func (ms *ModelSelector) renderDetailPane() string {
	if len(ms.filtered) == 0 {
		return ""
	}
	m := ms.filtered[ms.cursor]
	t := ms.theme

	var lines []string
	truncatedName := TruncateMiddle(m.Name, 40)
	lines = append(lines, lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(truncatedName))
	if m.Description != "" {
		lines = append(lines, "")
		lines = append(lines, lipgloss.NewStyle().Foreground(t.TextSecondary).Render(m.Description))
	}
	lines = append(lines, "")
	lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).Render("Provider: ")+
		lipgloss.NewStyle().Foreground(t.Text).Render(m.Provider))
	if m.ContextLength > 0 {
		lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).Render("Context: ")+
			lipgloss.NewStyle().Foreground(t.Text).Render(fmt.Sprintf("%dK tokens", m.ContextLength/1000)))
	}
	if m.Architecture.TokenizerFamily != "" {
		lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).Render("Tokenizer: ")+
			lipgloss.NewStyle().Foreground(t.Text).Render(m.Architecture.TokenizerFamily))
	}
	lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).Render("Pricing: ")+
		lipgloss.NewStyle().Foreground(t.Text).Render(fmt.Sprintf("$%.2f/$%.2f per M tokens",
			m.Pricing.InputPerMToken, m.Pricing.OutputPerMToken)))

	// Capabilities
	var caps []string
	if m.Capabilities.Tools {
		caps = append(caps, "tools")
	}
	if m.Capabilities.Reasoning {
		caps = append(caps, "reasoning")
	}
	if m.Capabilities.Vision {
		caps = append(caps, "vision")
	}
	if len(caps) > 0 {
		lines = append(lines, lipgloss.NewStyle().Foreground(t.TextMuted).Render("Capabilities: ")+
			lipgloss.NewStyle().Foreground(t.Text).Render(strings.Join(caps, ", ")))
	}

	return lipgloss.JoinVertical(lipgloss.Left, lines...)
}
