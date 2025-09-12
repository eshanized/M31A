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

	if m.showDetail && m.detailModel != nil {
		// Split view: list on left, detail on right
		listView := m.list.View()
		detailView := m.detailView()

		listWidth := m.width/2 - 2
		detailWidth := m.width/2 - 2

		listStyled := lipgloss.NewStyle().
			Width(listWidth).
			Render(listView)
		detailStyled := lipgloss.NewStyle().
			Width(detailWidth).
			Render(detailView)

		mainContent := lipgloss.JoinHorizontal(lipgloss.Top, listStyled, detailStyled)
		parts = append(parts, mainContent)
	} else {
		listView := m.list.View()
		parts = append(parts, listView)
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

	// Build detail pane with horizontal bar charts
	var b strings.Builder

	// Title
	titleStyle := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true)
	b.WriteString(titleStyle.Render(fmt.Sprintf("DETAIL: %s", d.ID)))
	b.WriteString("\n\n")

	// Provider info
	infoStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)
	b.WriteString(infoStyle.Render(fmt.Sprintf("Provider: %s", d.Provider)))
	b.WriteString("\n")
	if d.Variant != nil {
		b.WriteString(infoStyle.Render(fmt.Sprintf("Variant: %s", *d.Variant)))
		b.WriteString("\n")
	}

	// Context Window bar chart
	barWidth := 24
	filledStyle := lipgloss.NewStyle().Foreground(m.theme.Brand)
	emptyStyle := lipgloss.NewStyle().Foreground(m.theme.TextMuted)

	ctxFilled := int(float64(d.ContextLength) / 200000.0 * float64(barWidth))
	if ctxFilled > barWidth {
		ctxFilled = barWidth
	}
	ctxBar := lipgloss.NewStyle().Foreground(m.theme.Brand).Render(strings.Repeat("█", ctxFilled)) +
		emptyStyle.Render(strings.Repeat("░", barWidth-ctxFilled))
	b.WriteString(fmt.Sprintf("  Context Window  %s  %dK\n", ctxBar, d.ContextLength/1024))

	// Input Cost bar chart
	inputMax := 30.0
	inputFilled := int(d.Pricing.InputPerMToken / inputMax * float64(barWidth))
	if inputFilled > barWidth {
		inputFilled = barWidth
	}
	inputBar := filledStyle.Render(strings.Repeat("█", inputFilled)) +
		emptyStyle.Render(strings.Repeat("░", barWidth-inputFilled))
	b.WriteString(fmt.Sprintf("  Input Cost      %s  $%.2f / M tkn\n", inputBar, d.Pricing.InputPerMToken))

	// Output Cost bar chart
	outputMax := 60.0
	outputFilled := int(d.Pricing.OutputPerMToken / outputMax * float64(barWidth))
	if outputFilled > barWidth {
		outputFilled = barWidth
	}
	outputBar := filledStyle.Render(strings.Repeat("█", outputFilled)) +
		emptyStyle.Render(strings.Repeat("░", barWidth-outputFilled))
	b.WriteString(fmt.Sprintf("  Output Cost     %s  $%.2f / M tkn\n", outputBar, d.Pricing.OutputPerMToken))

	// Capabilities
	b.WriteString("\n")
	caps := capabilityBadges(d.Capabilities)
	if caps != "" {
		b.WriteString(fmt.Sprintf("  Capabilities: %s\n", caps))
	} else {
		b.WriteString("  Capabilities: basic\n")
	}

	// Architecture
	b.WriteString(fmt.Sprintf("  Architecture: %s\n", d.Architecture.TokenizerFamily))

	// Favorite status
	if m.manager != nil {
		isFav := m.manager.IsFavorite(d.ID)
		if isFav {
			b.WriteString(favoriteStarStyle.Render("  ★ Favorite"))
			b.WriteString("\n")
		}
	}

	return lipgloss.NewStyle().
		Padding(0, 1).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Brand).
		Width(m.width/2 - 4).
		Render(b.String())
}
