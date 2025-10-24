package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// renderView is the full View() implementation for ModelSelector.
func (ms *ModelSelector) renderView() string {
	t := ms.theme
	w := ms.width
	if w < 20 {
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

	// ── Provider Tabs ────────────────────────────────────────────────────────
	tabs := "  " + ms.renderProviderTabs()

	// ── Search box ──────────────────────────────────────────────────────────
	searchBox := lipgloss.NewStyle().
		Foreground(t.Border).
		PaddingLeft(2).
		Render("🔍 " + ms.searchInput.View())

	// ── Column headers ───────────────────────────────────────────────────────
	colW := ms.columnWidths()
	colHeader := "  " +
		lipgloss.NewStyle().Foreground(t.TextMuted).Width(colW[0]).Bold(true).Render("Model") +
		lipgloss.NewStyle().Foreground(t.TextMuted).Width(colW[1]).Bold(true).Render("Pricing $/M")

	// ── Model list ───────────────────────────────────────────────────────────
	list := ms.renderModelList()

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
		Render("↵ select  ↑↓/jk scroll  tab cycle provider  / search  esc back")

	parts := []string{
		header,
		divider,
		tabs,
		searchBox,
		divider,
		colHeader,
		list,
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
