package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

func (m ModelSelector) View() string {
	if !m.ready {
		return lipgloss.Place(m.width, m.height,
			lipgloss.Center, lipgloss.Center,
			m.spinner.View()+" Loading models...",
		)
	}

	var parts []string

	topBar := m.renderTopBar()
	parts = append(parts, topBar)

	searchView := m.renderSearchInput()
	parts = append(parts, searchView)

	listView := m.list.View()
	parts = append(parts, listView)

	if m.showDetail && m.detailModel != nil {
		detailView := m.detailView()
		parts = append(parts, detailView)
	}

	if m.err != "" {
		errStyle := lipgloss.NewStyle().Foreground(m.theme.Error)
		parts = append(parts, errStyle.Render(m.err))
	}

	helpBar := m.renderHelpBar()
	parts = append(parts, helpBar)

	return lipgloss.JoinVertical(lipgloss.Top, parts...)
}

func (m ModelSelector) renderTopBar() string {
	filterText := fmt.Sprintf("Showing: %s", m.filter.String())
	filterStyle := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true)

	hints := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("[P] Filter  [Tab] Details  [F] Favorite  [/] Search  [Enter] Select  [Esc] Back")

	return lipgloss.JoinHorizontal(lipgloss.Top,
		filterStyle.Render(filterText),
		"  ",
		hints,
	)
}

func (m ModelSelector) renderSearchInput() string {
	searchStyle := lipgloss.NewStyle().
		Border(lipgloss.NormalBorder()).
		BorderForeground(m.theme.Border).
		Padding(0, 1)

	searchLabel := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Render("Search:")

	return searchStyle.Render(searchLabel + " " + m.search.View())
}

func (m ModelSelector) renderHelpBar() string {
	return lipgloss.NewStyle().
		Foreground(lipgloss.Color("#9AA0A6")).
		Render("P: cycle provider filter  |  Tab: toggle details  |  F: toggle favorite  |  /: search  |  Enter: select  |  Esc: back")
}

func (m ModelSelector) detailView() string {
	if m.detailModel == nil {
		return ""
	}
	d := m.detailModel

	estCost := (d.Pricing.InputPerMToken * 0.1) + (d.Pricing.OutputPerMToken * 0.05)

	var b strings.Builder
	b.WriteString(fmt.Sprintf("Model: %s\n", d.ID))
	b.WriteString(fmt.Sprintf("Provider: %s\n", d.Provider))
	if d.Variant != nil {
		b.WriteString(fmt.Sprintf("Variant: %s\n", *d.Variant))
	}
	b.WriteString(fmt.Sprintf("Description: %s\n", d.Description))
	b.WriteString(fmt.Sprintf("Context: %d tokens\n", d.ContextLength))
	b.WriteString(fmt.Sprintf("Tokenizer: %s\n", d.Architecture.TokenizerFamily))
	b.WriteString(fmt.Sprintf("Pricing: $%.4f/M in, $%.4f/M out (est. $%.4f/100K+50K)\n",
		d.Pricing.InputPerMToken, d.Pricing.OutputPerMToken, estCost))
	b.WriteString(fmt.Sprintf("Capabilities: %s\n", capabilityString(d.Capabilities)))

	if m.manager != nil {
		isFav := m.manager.IsFavorite(d.ID)
		if isFav {
			b.WriteString(favoriteStarStyle.Render("Favorite: Yes\n"))
		} else {
			b.WriteString("Favorite: No\n")
		}
	}

	return lipgloss.NewStyle().
		Padding(1, 2).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Brand).
		Width(m.width - 6).
		Render(b.String())
}
