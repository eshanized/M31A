package tui

import (
	"strings"

	tea "github.com/charmbracelet/bubbletea"
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

// DiffModel is a bubbletea model for viewing git diffs.
type DiffModel struct {
	diff      string
	lines     []DiffLine
	width     int
	height    int
	scrollPos int
	theme     theme.Theme
	title     string
	splitView bool
}

// NewDiffModel creates a new DiffModel with the given theme.
func NewDiffModel(th theme.Theme, width, height int) DiffModel {
	return DiffModel{
		lines:  []DiffLine{},
		theme:  th,
		width:  width,
		height: height,
	}
}

// Init implements tea.Model.Init.
func (m DiffModel) Init() tea.Cmd {
	return nil
}

// Update implements tea.Model.Update.
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
		case "v", "V":
			m.splitView = !m.splitView
		case "esc", "q":
			return m, func() tea.Msg { return DiffCloseMsg{} }
		case "enter":
			return m, func() tea.Msg { return DiffCloseMsg{} }
		}
	}

	return m, nil
}

// parseDiff classifies each line of a git diff output into typed DiffLine entries.
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
		case strings.HasPrefix(line, "index "):
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

// computeStats computes file change statistics from parsed diff lines.
func (m *DiffModel) computeStats() (insertions, deletions, files int) {
	inDiff := false
	for _, line := range m.lines {
		switch line.Type {
		case DiffHeader:
			if strings.HasPrefix(line.Content, "diff --git") {
				files++
				inDiff = true
			}
		case DiffAdded:
			if inDiff {
				insertions++
			}
		case DiffDeleted:
			if inDiff {
				deletions++
			}
		}
	}
	return
}
