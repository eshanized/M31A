package phase_model_picker

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"
	"github.com/eshanized/M31A/internal/types"
)

// renderView renders the dual-panel model picker.
func (m *PhaseModelPickerModel) renderView() string {
	t := m.theme
	w := m.width
	if w < 40 {
		w = 80
	}

	// ── Title ──────────────────────────────────────────────────────────────────
	titleStyle := lipgloss.NewStyle().
		Bold(true).
		Foreground(t.Brand).
		MarginBottom(1)
	title := titleStyle.Render("  ◈  Choose Models for This Workflow")

	subtitleStyle := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		MarginBottom(1)
	subtitle := subtitleStyle.Render("  Select a Planning model and a Coding model, or press esc to use defaults.")

	// ── Panel widths ────────────────────────────────────────────────────────────
	gap := 2
	panelW := (w - gap) / 2
	if panelW < 20 {
		panelW = 20
	}

	leftPanel := m.renderPanel(0, panelW, t)
	rightPanel := m.renderPanel(1, panelW, t)

	panels := lipgloss.JoinHorizontal(lipgloss.Top,
		leftPanel,
		strings.Repeat(" ", gap),
		rightPanel,
	)

	// ── Footer ──────────────────────────────────────────────────────────────────
	hintStyle := lipgloss.NewStyle().Foreground(t.TextMuted)
	footer := hintStyle.Render(
		"  ↑/↓ navigate  ↵ select  tab switch panel  ctrl+enter confirm  esc skip",
	)

	return lipgloss.JoinVertical(lipgloss.Left,
		"",
		title,
		subtitle,
		"",
		panels,
		"",
		footer,
	)
}

// renderPanel renders a single picker panel (left or right).
func (m *PhaseModelPickerModel) renderPanel(idx, width int, t theme.Theme) string {
	p := &m.panels[idx]
	isFocused := m.focus == idx

	// Border color: brand when focused, muted when not.
	borderColor := t.TextMuted
	if isFocused {
		borderColor = t.Brand
	}

	borderStyle := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(borderColor).
		Width(width-2).
		Padding(0, 1)

	var sb strings.Builder

	// Panel header
	labelStyle := lipgloss.NewStyle().Bold(true).Foreground(t.Text)
	descStyle := lipgloss.NewStyle().Foreground(t.TextMuted).Italic(true)
	sb.WriteString(labelStyle.Render(p.label) + "\n")
	sb.WriteString(descStyle.Render(p.description) + "\n")

	// Selected badge
	if p.selected != nil {
		name := p.selected.Name
		if name == "" {
			name = p.selected.ID
		}
		badge := lipgloss.NewStyle().
			Foreground(t.Success).
			Bold(true).
			Render("✓ " + tuitypes.TruncateWithEllipsis(name, width-6))
		sb.WriteString(badge + "\n")
	} else {
		sb.WriteString(lipgloss.NewStyle().Foreground(t.TextMuted).Render("(using default)") + "\n")
	}

	sb.WriteString("\n")

	// Search input
	searchPrefix := lipgloss.NewStyle().Foreground(t.TextMuted).Render("  ")
	sb.WriteString(searchPrefix + p.searchInput.View() + "\n\n")

	// Model list or loading spinner
	if p.loading {
		spinner := lipgloss.NewStyle().Foreground(t.Brand).Render(string(p.spinner.Peek()))
		sb.WriteString("  " + spinner + " Loading models...\n")
	} else if len(p.filtered) == 0 {
		sb.WriteString(lipgloss.NewStyle().Foreground(t.TextMuted).Render("  No models found.") + "\n")
	} else {
		visible := m.visibleRows()
		start := p.offset
		end := start + visible
		if end > len(p.filtered) {
			end = len(p.filtered)
		}
		for i := start; i < end; i++ {
			model := p.filtered[i]
			sb.WriteString(renderModelRow(model, i == p.cursor && isFocused, width-4, t))
			sb.WriteString("\n")
		}
		// Scroll indicator
		if len(p.filtered) > visible {
			indicator := fmt.Sprintf("  %d/%d", p.cursor+1, len(p.filtered))
			sb.WriteString(lipgloss.NewStyle().Foreground(t.TextMuted).Render(indicator) + "\n")
		}
	}

	return borderStyle.Render(sb.String())
}

// renderModelRow renders a single model entry in the list.
func renderModelRow(m types.ModelInfo, selected bool, maxW int, t theme.Theme) string {
	name := m.Name
	if name == "" {
		name = m.ID
	}

	cursor := "  "
	textStyle := lipgloss.NewStyle().Foreground(t.Text)

	if selected {
		cursor = lipgloss.NewStyle().Foreground(t.Brand).Render("▸ ")
		textStyle = textStyle.Bold(true).Foreground(t.Brand)
	}

	nameStr := tuitypes.TruncateWithEllipsis(name, maxW-18)
	provStr := lipgloss.NewStyle().Foreground(t.TextMuted).Render(tuitypes.TruncateWithEllipsis(m.Provider, 12))

	return cursor + textStyle.Render(nameStr) + "  " + provStr
}
