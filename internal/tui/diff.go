package tui

import (
	"fmt"
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// DiffLineType represents a category of diff line content.
type DiffLineType int

const (
	DiffContext DiffLineType = iota
	DiffAdded
	DiffDeleted
	DiffHeader
	DiffHunk
)

// DiffLine represents a single parsed line from a git diff.
type DiffLine struct {
	Type    DiffLineType
	Content string
}

// DiffScreenMsg carries diff content and title to the DiffModel.
type DiffScreenMsg struct {
	Diff  string
	Title string
}

// DiffCloseMsg signals that the diff viewer should close and return to REPL.
type DiffCloseMsg struct{}

// DiffModel is a bubbletea model for viewing git diffs with syntax
// highlighting. It supports split/unified view toggling, keyboard scrolling,
// and theming via the theme package.
type DiffModel struct {
	diff      string
	lines     []DiffLine
	width     int
	height    int
	scrollPos int
	splitView bool
	theme     theme.Theme
	title     string
}

// NewDiffModel creates a new DiffModel with the given theme.
func NewDiffModel(th theme.Theme) DiffModel {
	return DiffModel{
		lines:     []DiffLine{},
		theme:     th,
		splitView: false,
	}
}

// Init implements tea.Model.Init.
func (m DiffModel) Init() tea.Cmd {
	return nil
}

// Update implements tea.Model.Update. It handles window resize, diff content
// messages, and keyboard input for navigation and view toggling.
func (m DiffModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height

	case DiffScreenMsg:
		m.diff = msg.Diff
		m.title = msg.Title
		m.lines = m.parseDiff(msg.Diff)
		m.scrollPos = 0

	case tea.KeyMsg:
		switch msg.String() {
		case "s", "S":
			m.splitView = !m.splitView
		case "up", "k":
			if m.scrollPos > 0 {
				m.scrollPos--
			}
		case "down", "j":
			if m.scrollPos < len(m.lines)-1 {
				m.scrollPos++
			}
		case "pgup", "K":
			half := (m.height - 3) / 2
			if half < 1 {
				half = 1
			}
			m.scrollPos -= half
			if m.scrollPos < 0 {
				m.scrollPos = 0
			}
		case "pgdown", "J":
			half := (m.height - 3) / 2
			if half < 1 {
				half = 1
			}
			m.scrollPos += half
			if m.scrollPos >= len(m.lines) {
				m.scrollPos = len(m.lines) - 1
				if m.scrollPos < 0 {
					m.scrollPos = 0
				}
			}
		case "g", "home":
			m.scrollPos = 0
		case "G", "end":
			m.scrollPos = len(m.lines) - 1
			if m.scrollPos < 0 {
				m.scrollPos = 0
			}
		case "esc", "q":
			return m, func() tea.Msg { return DiffCloseMsg{} }
		case "enter":
			return m, func() tea.Msg { return DiffCloseMsg{} }
		}
	}

	return m, nil
}

// parseDiff classifies each line of a git diff output into typed DiffLine
// entries for styled rendering.
func (m *DiffModel) parseDiff(diffText string) []DiffLine {
	lines := strings.Split(diffText, "\n")
	result := make([]DiffLine, 0, len(lines))

	for _, line := range lines {
		var dt DiffLineType

		switch {
		case strings.HasPrefix(line, "diff --git"):
			dt = DiffHeader
		case strings.HasPrefix(line, "---"):
			dt = DiffHeader
		case strings.HasPrefix(line, "+++"):
			dt = DiffHeader
		case strings.HasPrefix(line, "@@"):
			dt = DiffHunk
		case strings.HasPrefix(line, "+"):
			dt = DiffAdded
		case strings.HasPrefix(line, "-"):
			dt = DiffDeleted
		default:
			dt = DiffContext
		}

		result = append(result, DiffLine{Type: dt, Content: line})
	}

	return result
}

// View implements tea.Model.View. It renders the diff with lipgloss syntax
// highlighting — green for additions, red for deletions, brand for hunk
// headers, muted italic for file headers. Shows a header bar with title,
// view mode, and scroll position, and a help bar at the bottom.
func (m DiffModel) View() string {
	if m.diff == "" {
		return centerText("No diff to display. Run /diff with arguments.", m.width)
	}
	if len(m.lines) == 0 {
		return "No changes in diff."
	}

	// --- title / header bar ---
	var b strings.Builder
	viewMode := "unified"
	if m.splitView {
		viewMode = "split"
	}
	titleStr := fmt.Sprintf(" %s  [%s]  line %d/%d",
		m.title, viewMode, m.scrollPos+1, len(m.lines))
	headerStyle := lipgloss.NewStyle().
		Background(m.theme.Surface).
		Foreground(m.theme.TextSecondary).
		Padding(0, 1)
	if m.width > 0 {
		headerStyle = headerStyle.Width(m.width)
	}
	b.WriteString(headerStyle.Render(titleStr))
	b.WriteString("\n")

	// --- visible lines ---
	viewportHeight := m.height - 3 // header + padding + help bar
	if viewportHeight < 1 {
		viewportHeight = 1
	}
	end := m.scrollPos + viewportHeight
	if end > len(m.lines) {
		end = len(m.lines)
	}
	visible := m.lines[m.scrollPos:end]

	for _, line := range visible {
		var style lipgloss.Style
		switch line.Type {
		case DiffAdded:
			style = lipgloss.NewStyle().Foreground(m.theme.DiffAdded)
		case DiffDeleted:
			style = lipgloss.NewStyle().Foreground(m.theme.DiffRemoved)
		case DiffHunk:
			style = lipgloss.NewStyle().Foreground(m.theme.Brand).Bold(true)
		case DiffHeader:
			style = lipgloss.NewStyle().Foreground(m.theme.TextSecondary).Italic(true)
		case DiffContext:
			style = lipgloss.NewStyle().Foreground(m.theme.Text)
		default:
			style = lipgloss.NewStyle().Foreground(m.theme.Text)
		}
		b.WriteString(style.Render(line.Content))
		b.WriteString("\n")
	}

	// --- help bar ---
	helpBar := "[s] toggle split  [↑/↓] scroll  [g] top  [G] bottom  [esc] back"
	helpStyle := lipgloss.NewStyle().Foreground(m.theme.TextSecondary)
	if m.width > 0 {
		helpStyle = helpStyle.Width(m.width)
	}
	b.WriteString(helpStyle.Render(helpBar))

	return b.String()
}

// centerText centers lines of text within the given width.
func centerText(text string, width int) string {
	if width <= 0 {
		return text
	}
	lines := strings.Split(text, "\n")
	for i, line := range lines {
		if len(line) < width {
			pad := (width - len(line)) / 2
			if pad > 0 {
				lines[i] = strings.Repeat(" ", pad) + line
			}
		}
	}
	return strings.Join(lines, "\n")
}
