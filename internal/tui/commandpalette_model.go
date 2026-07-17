package tui

import (
	"fmt"
	"sort"
	"strings"

	"github.com/charmbracelet/bubbles/viewport"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// CommandPaletteScreenModel is a dedicated full-screen command palette with
// a split-panel layout: searchable command list on the left, detail panel on the right.
type CommandPaletteScreenModel struct {
	theme    theme.Theme
	entries  []PaletteEntry
	filtered []PaletteEntry
	selected int
	query    string
	width    int
	height   int

	listViewport viewport.Model
}

// NewCommandPaletteScreenModel creates a new dedicated command palette screen.
func NewCommandPaletteScreenModel(registry *CommandRegistry, t theme.Theme, w, h int) *CommandPaletteScreenModel {
	entries := BuildPaletteEntries(registry)
	listH := h - 8
	if listH < 3 {
		listH = 3
	}
	listW := w * 2 / 5
	if listW < 20 {
		listW = 20
	}
	return &CommandPaletteScreenModel{
		theme:        t,
		entries:      entries,
		filtered:     entries,
		width:        w,
		height:       h,
		listViewport: viewport.New(listW, listH),
	}
}

// SetTheme updates the theme.
func (m *CommandPaletteScreenModel) SetTheme(t theme.Theme) {
	m.theme = t
}

// SetDimensions updates the model dimensions.
func (m *CommandPaletteScreenModel) SetDimensions(w, h int) {
	m.width = w
	m.height = h
	listW := w * 2 / 5
	if listW < 20 {
		listW = 20
	}
	listH := h - 8
	if listH < 3 {
		listH = 3
	}
	m.listViewport = viewport.New(listW, listH)
	m.listViewport.SetContent(m.renderListContent())
}

// Init implements tea.Model.
func (m *CommandPaletteScreenModel) Init() tea.Cmd {
	m.listViewport.SetContent(m.renderListContent())
	return nil
}

// Update implements Screenable.
func (m *CommandPaletteScreenModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.SetDimensions(msg.Width, msg.Height)
		return m, nil
	case tea.MouseMsg:
		if msg.Action == tea.MouseActionPress {
			switch msg.Button {
			case tea.MouseButtonWheelUp:
				if m.selected > 0 {
					m.selected--
					m.clampScroll()
					m.listViewport.SetContent(m.renderListContent())
				}
				return m, nil
			case tea.MouseButtonWheelDown:
				if m.selected < len(m.filtered)-1 {
					m.selected++
					m.clampScroll()
					m.listViewport.SetContent(m.renderListContent())
				}
				return m, nil
			}
		}
		return m, nil
	case tea.KeyMsg:
		switch msg.String() {
		case "esc":
			return m, func() tea.Msg {
				return PopScreenMsg{}
			}
		case "up", "k":
			if m.selected > 0 {
				m.selected--
				m.clampScroll()
				m.listViewport.SetContent(m.renderListContent())
			}
		case "down", "j":
			if m.selected < len(m.filtered)-1 {
				m.selected++
				m.clampScroll()
				m.listViewport.SetContent(m.renderListContent())
			}
		case "g":
			m.selected = 0
			m.clampScroll()
			m.listViewport.SetContent(m.renderListContent())
		case "G":
			if len(m.filtered) > 0 {
				m.selected = len(m.filtered) - 1
				m.clampScroll()
				m.listViewport.SetContent(m.renderListContent())
			}
		case "enter":
			if m.selected < len(m.filtered) {
				entry := m.filtered[m.selected]
				if entry.Cmd.Execute != nil {
					return m, entry.Cmd.Execute()
				}
				return m, func() tea.Msg {
					return SlashCommandMsg{Command: entry.Cmd.Slash}
				}
			}
		case "backspace":
			if len(m.query) > 0 {
				m.query = m.query[:len(m.query)-1]
				m.filterCommands()
				m.listViewport.SetContent(m.renderListContent())
			}
		default:
			if len(msg.Runes) > 0 {
				m.query += string(msg.Runes)
				m.filterCommands()
				m.listViewport.SetContent(m.renderListContent())
			}
		}
	}
	return m, nil
}

// View implements tea.Model — content only, chrome handled by PageLayout.
func (m *CommandPaletteScreenModel) View() string {
	return m.renderScreen()
}

// clampScroll ensures the selected item is visible in the list viewport.
func (m *CommandPaletteScreenModel) clampScroll() {
	listH := m.listViewport.Height
	if listH < 1 {
		return
	}
	// Calculate the rendered row index of the selected item
	rowIdx := m.selectedRow()
	if rowIdx < m.listViewport.YOffset {
		m.listViewport.YOffset = rowIdx
	}
	if rowIdx >= m.listViewport.YOffset+listH {
		m.listViewport.YOffset = rowIdx - listH + 1
	}
}

// selectedRow returns the rendered row index of the selected entry,
// accounting for category headers.
func (m *CommandPaletteScreenModel) selectedRow() int {
	seen := make(map[CommandCategory]bool)
	row := 0
	for i, e := range m.filtered {
		if !seen[e.Category] {
			seen[e.Category] = true
			row++ // category header
		}
		if i == m.selected {
			return row
		}
		row++
	}
	return row
}

// filterCommands filters entries based on the current query using fuzzy matching.
func (m *CommandPaletteScreenModel) filterCommands() {
	m.selected = 0
	if m.query == "" {
		m.filtered = m.entries
		return
	}
	q := strings.ToLower(m.query)
	type scoredEntry struct {
		entry PaletteEntry
		score int
	}
	var scored []scoredEntry
	for _, e := range m.entries {
		name := strings.ToLower(e.Cmd.Name)
		desc := strings.ToLower(e.Cmd.Description)
		cat := strings.ToLower(string(e.Category))
		nameScore, nameMatch := FuzzyScore(name, q)
		descScore, descMatch := FuzzyScore(desc, q)
		catScore, catMatch := FuzzyScore(cat, q)
		if nameMatch || descMatch || catMatch {
			best := nameScore
			if descScore > best {
				best = descScore
			}
			if catScore > best {
				best = catScore
			}
			scored = append(scored, scoredEntry{entry: e, score: best})
		}
	}
	sort.Slice(scored, func(i, j int) bool {
		return scored[i].score > scored[j].score
	})
	m.filtered = nil
	for _, s := range scored {
		m.filtered = append(m.filtered, s.entry)
	}
}

// renderScreen renders the full split-panel command palette.
func (m *CommandPaletteScreenModel) renderScreen() string {
	t := m.theme
	w := m.width
	h := m.height

	// ── Search bar (top, spanning full width) ────────────────────────────────────
	searchBar := m.renderSearchBar(w)

	// ── Split panels ─────────────────────────────────────────────────────────────
	listW := w * 2 / 5
	if listW < 20 {
		listW = 20
	}
	detailW := w - listW - 3 // 3 for divider column
	if detailW < 20 {
		detailW = 20
	}
	panelH := h - 6 // search bar (2) + divider (1) + footer (2) + padding (1)
	if panelH < 3 {
		panelH = 3
	}

	// Update viewport dimensions
	m.listViewport.Width = listW
	m.listViewport.Height = panelH
	m.listViewport.SetContent(m.renderListContent())

	listPanel := m.renderListPanel(listW, panelH)
	detailPanel := m.renderDetailPanel(detailW, panelH)

	// Divider column
	dividerStyle := lipgloss.NewStyle().Foreground(t.Border)
	var dividerLines []string
	for i := 0; i < panelH; i++ {
		dividerLines = append(dividerLines, dividerStyle.Render("│"))
	}
	dividerCol := strings.Join(dividerLines, "\n")

	panels := lipgloss.JoinHorizontal(lipgloss.Top, listPanel, dividerCol, detailPanel)

	// ── Footer ───────────────────────────────────────────────────────────────────
	footer := m.renderFooter(w)

	return lipgloss.JoinVertical(lipgloss.Left, searchBar, panels, footer)
}

// renderSearchBar renders the search input at the top.
func (m *CommandPaletteScreenModel) renderSearchBar(width int) string {
	t := m.theme

	label := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(" > ")
	queryText := m.renderHighlightedQuery()
	cursor := lipgloss.NewStyle().Foreground(t.Brand).Render("█")

	count := lipgloss.NewStyle().Foreground(t.TextMuted).Render(
		lipgloss.PlaceHorizontal(width-30, lipgloss.Right,
			fmt.Sprintf("%d/%d commands", len(m.filtered), len(m.entries))))

	searchContent := label + queryText + cursor + count
	if lipgloss.Width(searchContent) > width {
		searchContent = TruncateWithEllipsis(searchContent, width)
	}

	return lipgloss.NewStyle().
		Border(lipgloss.NormalBorder(), false, false, true, false).
		BorderForeground(t.Border).
		Width(width).
		Padding(0, 1).
		Render(searchContent)
}

// renderListPanel renders the left panel with the scrollable command list.
func (m *CommandPaletteScreenModel) renderListPanel(w, h int) string {
	content := m.listViewport.View()

	return lipgloss.NewStyle().
		Width(w).
		Height(h).
		Render(content)
}

// renderListContent renders all list items (category headers + commands).
func (m *CommandPaletteScreenModel) renderListContent() string {
	t := m.theme
	w := m.listViewport.Width
	if w <= 0 {
		w = 30
	}

	var lines []string
	seenCategories := make(map[CommandCategory]bool)

	for i, entry := range m.filtered {
		// Category header
		if !seenCategories[entry.Category] {
			seenCategories[entry.Category] = true
			catHeader := lipgloss.NewStyle().
				Foreground(t.TextSecondary).
				Bold(true).
				PaddingLeft(1).
				PaddingTop(1).
				Render(string(entry.Category))
			lines = append(lines, catHeader)
		}

		// Command entry
		isSelected := i == m.selected
		var line string

		descText := "  " + entry.Cmd.Description

		if isSelected {
			indicator := lipgloss.NewStyle().Foreground(t.Brand).Render("▸ ")
			slashPart := lipgloss.NewStyle().Foreground(t.Background).Bold(true).Render(entry.Cmd.Slash)
			descPart := lipgloss.NewStyle().Foreground(t.Background).Render(descText)
			line = indicator + slashPart + descPart
			line = lipgloss.NewStyle().
				Background(t.Brand).
				Foreground(t.Background).
				Width(w).
				Render(line)
		} else {
			slashPart := lipgloss.NewStyle().Foreground(t.Brand).Render(entry.Cmd.Slash)
			descPart := lipgloss.NewStyle().Foreground(t.TextMuted).Render(descText)
			line = "  " + slashPart + descPart
		}

		if lipgloss.Width(line) > w {
			line = TruncateWithEllipsis(line, w)
		}
		lines = append(lines, line)
	}

	if len(m.filtered) == 0 {
		lines = append(lines, lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Italic(true).
			PaddingLeft(2).
			Render("No commands match"))
	}

	return strings.Join(lines, "\n")
}

// renderDetailPanel renders the right panel with command details.
func (m *CommandPaletteScreenModel) renderDetailPanel(w, h int) string {
	t := m.theme

	if len(m.filtered) == 0 || m.selected >= len(m.filtered) {
		emptyMsg := lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Italic(true).
			Padding(2, 3).
			Render("Select a command to see details")
		return lipgloss.NewStyle().
			Width(w).
			Height(h).
			Render(emptyMsg)
	}

	entry := m.filtered[m.selected]

	// Command name (large, brand-colored)
	cmdName := lipgloss.NewStyle().
		Foreground(t.Brand).
		Bold(true).
		Render(entry.Cmd.Slash)

	// Divider
	divider := lipgloss.NewStyle().
		Foreground(t.Border).
		Render(strings.Repeat("─", w-6))

	// Category badge
	catBadge := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Background(t.SurfaceElevated).
		Padding(0, 1).
		Render(string(entry.Category))

	// Shortcut
	shortcutLine := ""
	if entry.Shortcut != "" {
		shortcutLabel := lipgloss.NewStyle().Foreground(t.TextMuted).Render("Shortcut  ")
		shortcutVal := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(entry.Shortcut)
		shortcutLine = shortcutLabel + shortcutVal
	}

	// Description
	descLabel := lipgloss.NewStyle().Foreground(t.TextMuted).Render("Description")
	descVal := lipgloss.NewStyle().Foreground(t.Text).PaddingLeft(2).Render(entry.Cmd.Description)

	// Slash format
	slashLabel := lipgloss.NewStyle().Foreground(t.TextMuted).Render("Command")
	slashVal := lipgloss.NewStyle().Foreground(t.Text).PaddingLeft(2).Render(entry.Cmd.Slash)

	// Name label
	nameLabel := lipgloss.NewStyle().Foreground(t.TextMuted).Render("Name")
	nameVal := lipgloss.NewStyle().Foreground(t.Text).PaddingLeft(2).Render(entry.Cmd.Name)

	// Build detail content
	var parts []string
	parts = append(parts, "  "+cmdName)
	parts = append(parts, "  "+divider)
	parts = append(parts, "")
	parts = append(parts, "  "+nameLabel)
	parts = append(parts, "  "+nameVal)
	parts = append(parts, "")
	parts = append(parts, "  "+catBadge)
	if shortcutLine != "" {
		parts = append(parts, "")
		parts = append(parts, "  "+shortcutLine)
	}
	parts = append(parts, "")
	parts = append(parts, "  "+descLabel)
	parts = append(parts, "  "+descVal)
	parts = append(parts, "")
	parts = append(parts, "  "+slashLabel)
	parts = append(parts, "  "+slashVal)

	content := strings.Join(parts, "\n")

	return lipgloss.NewStyle().
		Background(t.SurfaceElevated).
		Width(w).
		Height(h).
		Padding(1, 1).
		Render(content)
}

// renderFooter renders the bottom hint bar.
func (m *CommandPaletteScreenModel) renderFooter(width int) string {
	t := m.theme

	hints := []struct {
		key  string
		desc string
	}{
		{"↑↓", "navigate"},
		{"enter", "execute"},
		{"g/G", "top/bottom"},
		{"esc", "close"},
	}

	var parts []string
	for _, h := range hints {
		key := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(h.key)
		desc := lipgloss.NewStyle().Foreground(t.TextMuted).Render(h.desc)
		parts = append(parts, key+" "+desc)
	}

	footerContent := "  " + strings.Join(parts, "    ")

	return lipgloss.NewStyle().
		Border(lipgloss.NormalBorder(), true, false, false, false).
		BorderForeground(t.Border).
		Width(width).
		Padding(0, 1).
		Render(footerContent)
}

// renderHighlightedQuery renders the query with matched characters highlighted.
func (m *CommandPaletteScreenModel) renderHighlightedQuery() string {
	t := m.theme
	if m.query == "" {
		return ""
	}
	if len(m.filtered) == 0 {
		return lipgloss.NewStyle().Foreground(t.Error).Render(m.query)
	}
	bestTarget := strings.ToLower(m.filtered[0].Cmd.Name)
	query := strings.ToLower(m.query)
	var sb strings.Builder
	qi := 0
	for i := 0; i < len(m.query); i++ {
		ch := string(m.query[i])
		if qi < len(bestTarget) && qi < len(query) {
			idx := strings.IndexByte(bestTarget[qi:], query[qi])
			if idx == 0 {
				sb.WriteString(lipgloss.NewStyle().Foreground(t.Brand).Bold(true).Render(ch))
				qi++
				continue
			}
		}
		sb.WriteString(lipgloss.NewStyle().Foreground(t.Text).Render(ch))
	}
	return sb.String()
}
