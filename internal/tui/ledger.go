package tui

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/bubbles/spinner"
	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/pkg/ledger"
)

// ledgerEntryItem wraps a ledger entry for display in a list.
type ledgerEntryItem struct {
	entry ledger.LedgerEntry
}

func (i ledgerEntryItem) Title() string {
	return i.entry.SessionID
}

func (i ledgerEntryItem) Description() string {
	model := i.entry.Model
	if model == "" {
		model = "unknown"
	}
	provider := i.entry.Provider
	if provider == "" {
		provider = "unknown"
	}
	return fmt.Sprintf("%s · %s · %d tasks · $%.4f", provider, model, i.entry.TaskCount, i.entry.CostEstimate)
}

func (i ledgerEntryItem) FilterValue() string {
	return i.entry.SessionID + " " + i.entry.Model + " " + i.entry.Provider + " " + i.entry.ProjectType
}

// LedgerModel provides a filterable ledger browser.
type LedgerModel struct {
	theme       theme.Theme
	width       int
	height      int
	entries     []ledger.LedgerEntry
	filtered    []ledger.LedgerEntry
	stats       ledger.LedgerStats
	filter      string
	searchInput textinput.Model
	showStats   bool
	ledger      *ledger.Ledger
	selected    int
	loaded      bool
	errMsg      string
	spinner     spinner.Model
}

// NewLedgerModel creates a LedgerModel with the given theme and ledger.
func NewLedgerModel(t theme.Theme, l *ledger.Ledger) *LedgerModel {
	ti := textinput.New()
	ti.Placeholder = "Filter entries..."
	ti.PromptStyle = lipgloss.NewStyle().Foreground(t.Brand)
	ti.TextStyle = lipgloss.NewStyle().Foreground(t.TextPrimary)
	ti.Cursor.Style = lipgloss.NewStyle().Foreground(t.Brand)
	ti.CharLimit = 80
	ti.Width = 40

	sp := spinner.New()
	sp.Spinner = spinner.Dot
	sp.Style = lipgloss.NewStyle().Foreground(t.Brand)

	return &LedgerModel{
		theme:       t,
		ledger:      l,
		searchInput: ti,
		spinner:     sp,
	}
}

// Init returns nil (no spinner needed on startup).
func (m *LedgerModel) Init() tea.Cmd {
	return nil
}

// LoadEntries reloads entries from the ledger file.
func (m *LedgerModel) LoadEntries() {
	if m.ledger == nil {
		m.errMsg = "Ledger not available"
		return
	}
	m.entries = m.ledger.Entries()
	m.stats = m.ledger.Stats()
	m.applyFilter()
	m.loaded = true
	m.errMsg = ""
}

// applyFilter filters entries by the current search text.
func (m *LedgerModel) applyFilter() {
	query := strings.ToLower(strings.TrimSpace(m.filter))
	if query == "" {
		m.filtered = make([]ledger.LedgerEntry, len(m.entries))
		copy(m.filtered, m.entries)
		return
	}
	var result []ledger.LedgerEntry
	for _, e := range m.entries {
		haystack := strings.ToLower(e.SessionID + " " + e.Model + " " + e.Provider + " " + e.ProjectType)
		if strings.Contains(haystack, query) {
			result = append(result, e)
		}
	}
	m.filtered = result
}

// Update handles messages for the ledger screen.
func (m *LedgerModel) Update(msg tea.Msg) ([]tea.Cmd, *AppMsg) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return nil, nil

	case spinner.TickMsg:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		return []tea.Cmd{cmd}, nil

	case tea.KeyMsg:
		if m.searchInput.Focused() {
			switch msg.String() {
			case "enter":
				m.searchInput.Blur()
				m.applyFilter()
				return nil, nil
			case "esc":
				m.filter = ""
				m.searchInput.SetValue("")
				m.applyFilter()
				m.searchInput.Blur()
				return nil, nil
			}
			var cmd tea.Cmd
			m.searchInput, cmd = m.searchInput.Update(msg)
			m.filter = m.searchInput.Value()
			m.applyFilter()
			return []tea.Cmd{cmd}, nil
		}

		switch msg.String() {
		case "/", "f":
			m.searchInput.Focus()
			return nil, nil
		case "s":
			m.showStats = !m.showStats
			return nil, nil
		case "up", "k":
			if m.selected > 0 {
				m.selected--
			}
			return nil, nil
		case "down", "j":
			if m.selected < len(m.filtered)-1 {
				m.selected++
			}
			return nil, nil
		case "esc":
			return nil, &AppMsg{Screen: ScreenREPL}
		}
	}

	return nil, nil
}

// View renders the ledger screen.
func (m *LedgerModel) View() string {
	if !m.loaded {
		return lipgloss.Place(m.width, m.height, lipgloss.Center, lipgloss.Center,
			m.spinner.View()+" Loading ledger...")
	}

	var parts []string

	// Header
	headerStyle := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true).
		Padding(0, 1)
	parts = append(parts, headerStyle.Render("/ledger — Learning Journal"))

	// Stats panel (if toggled)
	if m.showStats {
		parts = append(parts, m.renderStats())
		parts = append(parts, "")
	}

	// Entry list
	if len(m.filtered) == 0 {
		emptyStyle := lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Italic(true).
			Padding(2, 4)
		parts = append(parts, emptyStyle.Render("No entries found"))
	} else {
		for i, entry := range m.filtered {
			card := m.renderEntry(i, entry)
			parts = append(parts, card)
		}
	}

	// Error message
	if m.errMsg != "" {
		errStyle := lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Error))
		parts = append(parts, "", errStyle.Render(m.errMsg))
	}

	// Search bar at bottom
	searchBar := m.renderSearchBar()
	parts = append(parts, "", searchBar)

	// Footer hints
	footerStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)
	footer := footerStyle.Render("j/k: navigate  |  /: filter  |  s: toggle stats  |  Esc: back")
	parts = append(parts, footer)

	return strings.Join(parts, "\n")
}

// renderStats renders the aggregate statistics panel.
func (m *LedgerModel) renderStats() string {
	var lines []string
	titleStyle := lipgloss.NewStyle().Foreground(m.theme.Brand).Bold(true)
	labelStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)
	valueStyle := lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Bold(true)

	lines = append(lines, titleStyle.Render("Aggregate Statistics"))
	lines = append(lines, fmt.Sprintf("  %s %s", labelStyle.Render("Sessions:"), valueStyle.Render(fmt.Sprintf("%d", m.stats.TotalSessions))))
	lines = append(lines, fmt.Sprintf("  %s %s", labelStyle.Render("Avg tasks/session:"), valueStyle.Render(fmt.Sprintf("%.1f", m.stats.AvgTaskCount))))
	lines = append(lines, fmt.Sprintf("  %s %s", labelStyle.Render("Avg cost:"), valueStyle.Render(fmt.Sprintf("$%.4f", m.stats.AvgCost))))
	lines = append(lines, fmt.Sprintf("  %s %s", labelStyle.Render("Avg duration:"), valueStyle.Render(fmt.Sprintf("%.0f min", m.stats.AvgDurationMinutes))))
	lines = append(lines, fmt.Sprintf("  %s %s", labelStyle.Render("Total failed:"), valueStyle.Render(fmt.Sprintf("%d", m.stats.TotalFailedTasks))))

	if len(m.stats.TopFrameworks) > 0 {
		lines = append(lines, fmt.Sprintf("  %s %s", labelStyle.Render("Top frameworks:"), valueStyle.Render(strings.Join(m.stats.TopFrameworks, ", "))))
	}
	if len(m.stats.TopFailures) > 0 {
		lines = append(lines, fmt.Sprintf("  %s %s", labelStyle.Render("Top failures:"), valueStyle.Render(strings.Join(m.stats.TopFailures, ", "))))
	}

	boxStyle := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Border).
		Padding(0, 1).
		Width(min(m.width-4, 60))

	return boxStyle.Render(strings.Join(lines, "\n"))
}

// renderEntry renders a single ledger entry as a card.
func (m *LedgerModel) renderEntry(idx int, entry ledger.LedgerEntry) string {
	var lines []string

	// Selection indicator
	indicator := "  "
	if idx == m.selected {
		indicator = lipgloss.NewStyle().Foreground(m.theme.Brand).Render("▶ ")
	}

	// Line 1: ID, model/provider, cost
	model := entry.Model
	if model == "" {
		model = "unknown"
	}
	provider := entry.Provider
	if provider == "" {
		provider = "unknown"
	}

	line1 := fmt.Sprintf("%s%s  %s · %s",
		indicator,
		lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Bold(true).Render(entry.SessionID),
		lipgloss.NewStyle().Foreground(m.theme.TextPrimary).Render(model),
		lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(provider),
	)
	lines = append(lines, line1)

	// Line 2: project type, tasks, cost, duration
	projectType := entry.ProjectType
	if projectType == "" {
		projectType = "general"
	}
	line2 := fmt.Sprintf("   %s · %d tasks · $%.4f · %d min",
		lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Render(projectType),
		entry.TaskCount,
		entry.CostEstimate,
		entry.DurationMinutes,
	)
	if entry.FailedTasks > 0 {
		line2 += fmt.Sprintf(" · %s", lipgloss.NewStyle().Foreground(lipgloss.Color(m.theme.Error)).Render(fmt.Sprintf("%d failed", entry.FailedTasks)))
	}
	lines = append(lines, line2)

	// Line 3: timestamp
	line3 := fmt.Sprintf("   %s",
		lipgloss.NewStyle().Foreground(m.theme.TextMuted).Render(entry.Timestamp.Format("Jan 02 15:04")),
	)
	lines = append(lines, line3)

	content := strings.Join(lines, "\n")

	// Highlight selected entry
	if idx == m.selected {
		cardStyle := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(m.theme.Brand).
			Padding(0, 1).
			Width(m.width - 4)
		return cardStyle.Render(content)
	}

	return content
}

// renderSearchBar renders the filter input.
func (m *LedgerModel) renderSearchBar() string {
	label := lipgloss.NewStyle().
		Foreground(m.theme.Brand).
		Bold(true).
		Render("🔍 ")

	searchRow := lipgloss.JoinHorizontal(lipgloss.Center, label, m.searchInput.View())

	style := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Padding(0, 1).
		Width(m.width - 2)

	return style.Render(searchRow)
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
