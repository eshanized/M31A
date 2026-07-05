package tui

import (
	"strings"
	"time"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ─── View ─────────────────────────────────────────────────────────────────────

func (m *ConfigModel) View() string {
	t := m.theme

	// ── Top: section tabs ────────────────────────────────────────────────────
	tabs := m.renderTabs()

	// ── Middle: field list ───────────────────────────────────────────────────
	fieldArea := m.renderFields()

	// ── Bottom: edit box or hint + status ────────────────────────────────────
	var bottom string
	if m.editing {
		bottom = m.renderEditBox()
	} else {
		bottom = m.renderHint()
	}

	// ── Status bar ───────────────────────────────────────────────────────────
	status := m.renderStatus()

	// ── Keybind footer ────────────────────────────────────────────────────────
	footerParts := []string{
		lipgloss.NewStyle().Foreground(t.Brand).Render("↑↓"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" navigate  "),
		lipgloss.NewStyle().Foreground(t.Brand).Render("↵/e"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" edit  "),
		lipgloss.NewStyle().Foreground(t.Brand).Render("tab"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" section  "),
		lipgloss.NewStyle().Foreground(t.Brand).Render("s"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" save  "),
		lipgloss.NewStyle().Foreground(t.Brand).Render("L"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" save local  "),
		lipgloss.NewStyle().Foreground(t.Brand).Render("q"),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render(" close"),
	}
	if m.dirty {
		footerParts = append(footerParts,
			lipgloss.NewStyle().Foreground(t.Warning).Bold(true).Render("  ● unsaved changes"),
		)
	}
	footer := lipgloss.NewStyle().PaddingLeft(2).Render(strings.Join(footerParts, ""))

	content := lipgloss.JoinVertical(lipgloss.Left,
		tabs,
		fieldArea,
		bottom,
		status,
		footer,
	)

	if m.confirmingExit {
		prompt := lipgloss.NewStyle().
			Border(lipgloss.NormalBorder()).
			BorderForeground(t.Warning).
			Padding(0, 1).
			Render("Unsaved changes. Discard? (y/n)")
		overlay := lipgloss.Place(
			m.width, 3,
			lipgloss.Center, lipgloss.Center,
			prompt,
		)
		return lipgloss.JoinVertical(lipgloss.Left, content, overlay)
	}

	return content
}

func (m *ConfigModel) renderTabs() string {
	t := m.theme
	var parts []string
	for i, sec := range m.sections {
		if i == m.sectionIdx {
			parts = append(parts, lipgloss.NewStyle().
				Foreground(t.Background).Background(t.Brand).
				Bold(true).Padding(0, 2).
				Render(sec.title))
		} else {
			parts = append(parts, lipgloss.NewStyle().
				Foreground(t.TextMuted).Padding(0, 2).
				Render(sec.title))
		}
	}
	bar := strings.Join(parts, " ")
	return lipgloss.NewStyle().
		BorderBottom(true).
		BorderStyle(lipgloss.NormalBorder()).
		BorderForeground(t.Border).
		Width(max(10, m.width-4)).
		PaddingLeft(1).
		Render(bar) + "\n"
}

func (m *ConfigModel) renderFields() string {
	if len(m.sections) == 0 {
		return ""
	}
	sec := m.sections[m.sectionIdx]
	t := m.theme

	// Available height for the field viewport
	vpH := m.height - 10
	if vpH < 4 {
		vpH = 4
	}

	var rows []string
	for i, f := range sec.fields {
		rows = append(rows, m.renderFieldRow(f, i))
	}
	content := strings.Join(rows, "\n")

	cContentW := max(10, m.width-4)
	m.viewport.Width = cContentW
	m.viewport.Height = vpH
	m.viewport.SetContent(content)
	m.scrollToField()

	// Wrap viewport in a subtle border
	return lipgloss.NewStyle().
		Border(theme.ThinBorder).
		BorderForeground(t.Border).
		Width(max(10, m.width-2)).
		Margin(0, 1).
		Render(m.viewport.View())
}

func (m *ConfigModel) renderFieldRow(f cfgField, idx int) string {
	t := m.theme
	val := m.getFieldValue(f)
	selected := idx == m.fieldIdx

	cursor := "  "
	if selected {
		cursor = lipgloss.NewStyle().Foreground(t.Brand).Render("▸ ")
	}

	// Label width: ideal 34, scaled down on narrow terminals
	labelW := 34
	avail := m.width - 10 // cursor(2) + borders(4) + value margin(4)
	if avail < labelW {
		labelW = max(10, avail/2)
	}
	labelStyle := lipgloss.NewStyle().Foreground(t.TextMuted).Width(labelW)
	valStyle := lipgloss.NewStyle().Foreground(t.Text)
	if selected {
		labelStyle = labelStyle.Foreground(t.Text).Bold(true)
		valStyle = valStyle.Foreground(t.Brand).Bold(true)
	}

	var valDisplay string
	switch f.fieldType {
	case cfgBool:
		icon, col := "○ no", t.TextMuted
		if val == "yes" {
			icon, col = "● yes", t.Success
		}
		if selected {
			col = t.Brand
		}
		valDisplay = lipgloss.NewStyle().Foreground(col).Render(icon)

	case cfgChoice:
		var opts []string
		for _, c := range f.choices {
			if c == val {
				opts = append(opts, lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render("●"+c))
			} else {
				opts = append(opts, lipgloss.NewStyle().Foreground(t.TextMuted).Render("○"+c))
			}
		}
		valDisplay = strings.Join(opts, "  ")

	case cfgPassword:
		valDisplay = lipgloss.NewStyle().Foreground(t.TextMuted).Render(maskedKey(val))

	default:
		if val == "" {
			valDisplay = lipgloss.NewStyle().Foreground(t.TextMuted).Italic(true).Render("(not set)")
		} else {
			valDisplay = valStyle.Render(val)
		}
	}

	return cursor + labelStyle.Render(f.label) + valDisplay
}

func (m *ConfigModel) renderEditBox() string {
	t := m.theme
	sec := m.sections[m.sectionIdx]
	f := sec.fields[m.fieldIdx]

	typeHint := ""
	switch f.fieldType {
	case cfgNumber:
		typeHint = "integer"
	case cfgFloat:
		typeHint = "decimal"
	case cfgPassword:
		typeHint = "password"
	default:
		typeHint = "text"
	}

	inner := lipgloss.JoinVertical(lipgloss.Left,
		lipgloss.NewStyle().Foreground(t.TextMuted).Render("Editing: "+f.label+" ("+typeHint+")"),
		m.editInput.View(),
		lipgloss.NewStyle().Foreground(t.TextMuted).Render("↵ confirm   esc cancel"),
	)
	// Edit box width: clamped to terminal, max 60, min 30
	editW := min(60, max(30, m.width-8))
	return lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(0, 2).
		Width(editW).
		MarginLeft(2).
		Render(inner)
}

func (m *ConfigModel) renderHint() string {
	t := m.theme
	if len(m.sections) == 0 {
		return ""
	}
	sec := m.sections[m.sectionIdx]
	if m.fieldIdx >= len(sec.fields) {
		return ""
	}
	hint := sec.fields[m.fieldIdx].hint
	if hint == "" {
		return ""
	}
	return lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Italic(true).
		PaddingLeft(4).
		Render("ℹ  " + hint)
}

func (m *ConfigModel) renderStatus() string {
	if m.statusMsg == "" || time.Since(m.statusTime) > 6*time.Second {
		return ""
	}
	t := m.theme
	col := t.Success
	if strings.HasPrefix(m.statusMsg, "✗") {
		col = t.Error
	} else if strings.HasPrefix(m.statusMsg, "✎") || strings.HasPrefix(m.statusMsg, "↺") {
		col = t.Warning
	}
	return lipgloss.NewStyle().
		Foreground(col).
		PaddingLeft(2).
		Bold(true).
		Render(m.statusMsg)
}

// scrollToField ensures the viewport is scrolled so the active field is visible.
func (m *ConfigModel) scrollToField() {
	// Each row is 1 line; estimate scroll offset
	offset := m.fieldIdx
	vpH := m.viewport.Height
	if vpH < 1 {
		return
	}
	if offset < m.viewport.YOffset {
		m.viewport.SetYOffset(offset)
	} else if offset >= m.viewport.YOffset+vpH {
		m.viewport.SetYOffset(offset - vpH + 1)
	}
}

// buildContent is kept for backward compatibility with AppState which calls it after SettingsSavedMsg.
func (m *ConfigModel) buildContent() {
	// No-op: content is now rendered dynamically from cfg in View().
}
