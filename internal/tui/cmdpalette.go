package tui

import (
	"sort"
	"strings"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// CommandInfo represents a command available in the palette.
type CommandInfo struct {
	Name        string
	Description string
	Slash       string
	Execute     func() tea.Cmd
}

// CommandPaletteModel provides a fuzzy-searchable command palette overlay.
type CommandPaletteModel struct {
	input    textinput.Model
	commands []CommandInfo
	matches  []CommandInfo
	selected int
	width    int
	height   int
	open     bool
	theme    theme.Theme
}

func NewCommandPaletteModel(t theme.Theme) *CommandPaletteModel {
	ti := textinput.New()
	ti.Placeholder = "Search commands..."
	ti.CharLimit = 64
	ti.Prompt = "> "
	ti.TextStyle = lipgloss.NewStyle().Foreground(t.Text)
	ti.Cursor.Style = lipgloss.NewStyle().Foreground(t.Brand)

	return &CommandPaletteModel{
		input:    ti,
		selected: 0,
		theme:    t,
	}
}

// Init satisfies tea.Model.
func (m *CommandPaletteModel) Init() tea.Cmd {
	return nil
}

// SetCommands populates the palette with available commands.
func (m *CommandPaletteModel) SetCommands(cmds []CommandInfo) {
	m.commands = cmds
	m.updateMatches()
}

// Open shows the palette and focuses the input.
func (m *CommandPaletteModel) Open() {
	m.open = true
	m.input.Focus()
	m.input.SetValue("")
	m.selected = 0
	m.updateMatches()
}

// Close hides the palette.
func (m *CommandPaletteModel) Close() {
	m.open = false
	m.input.Blur()
}

// IsOpen returns whether the palette is visible.
func (m *CommandPaletteModel) IsOpen() bool {
	return m.open
}

// SelectedCommand returns the currently highlighted command.
func (m *CommandPaletteModel) SelectedCommand() *CommandInfo {
	if m.selected < 0 || m.selected >= len(m.matches) {
		return nil
	}
	return &m.matches[m.selected]
}

// Update handles keyboard input for the palette.
func (m *CommandPaletteModel) Update(msg tea.Msg) tea.Cmd {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
	case tea.KeyMsg:
		switch msg.Type {
		case tea.KeyEsc:
			m.Close()
			return nil
		case tea.KeyUp:
			if m.selected > 0 {
				m.selected--
			}
			return nil
		case tea.KeyDown:
			if m.selected < len(m.matches)-1 {
				m.selected++
			}
			return nil
		case tea.KeyEnter:
			return nil
		}
	}

	var cmd tea.Cmd
	m.input, cmd = m.input.Update(msg)
	m.updateMatches()
	return cmd
}

// View renders the command palette overlay.
func (m *CommandPaletteModel) View() string {
	if !m.open {
		return ""
	}

	paletteWidth := m.width * 4 / 5
	if paletteWidth > 80 {
		paletteWidth = 80
	}
	if paletteWidth < 40 {
		paletteWidth = 40
	}

	paletteHeight := m.height * 3 / 5
	if paletteHeight > 20 {
		paletteHeight = 20
	}
	if paletteHeight < 6 {
		paletteHeight = 6
	}

	// Build content
	var lines []string

	// Title
	titleStyle := lipgloss.NewStyle().
		Bold(true).
		Foreground(m.theme.Brand).
		Padding(0, 1)
	lines = append(lines, titleStyle.Render("Command Palette"))

	// Separator
	sep := lipgloss.NewStyle().
		Foreground(m.theme.Border).
		Render(strings.Repeat("─", paletteWidth-2))
	lines = append(lines, sep)

	// Search input
	inputLine := m.input.View()
	lines = append(lines, lipgloss.NewStyle().Padding(0, 1).Render(inputLine))

	// Separator
	lines = append(lines, sep)

	// Match results
	maxResults := paletteHeight - 5 // title + 2 seps + input + padding
	if maxResults < 1 {
		maxResults = 1
	}

	for i := 0; i < len(m.matches) && i < maxResults; i++ {
		cmd := m.matches[i]
		style := lipgloss.NewStyle().Padding(0, 1)
		if i == m.selected {
			style = lipgloss.NewStyle().
				Background(m.theme.Surface).
				Foreground(m.theme.Text).
				Padding(0, 1)
		}

		entry := cmd.Name
		if cmd.Slash != "" {
			entry = cmd.Slash + "  " + cmd.Name
		}
		if cmd.Description != "" {
			entry += "    " + cmd.Description
		}

		// Truncate to palette width
		entryWidth := paletteWidth - 2
		maxLen := entryWidth - 3
		if maxLen < 10 {
			maxLen = 10
		}
		runes := []rune(entry)
		if len(runes) > maxLen {
			entry = string(runes[:maxLen]) + "..."
		}

		lines = append(lines, style.Render(entry))
	}

	if len(m.matches) == 0 {
		lines = append(lines, lipgloss.NewStyle().
			Foreground(m.theme.TextSecondary).
			Padding(0, 1).
			Render("No matching commands"))
	}

	// Hint
	hintStyle := lipgloss.NewStyle().
		Foreground(m.theme.TextSecondary).
		Padding(0, 1)
	lines = append(lines, "")
	lines = append(lines, hintStyle.Render("↑↓ navigate · enter execute · esc close"))

	content := lipgloss.JoinVertical(lipgloss.Top, lines...)

	panel := lipgloss.NewStyle().
		Width(paletteWidth).
		Height(paletteHeight).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(m.theme.Brand).
		Background(m.theme.Background).
		Render(content)

	// Center the panel
	topPad := (m.height - paletteHeight) / 2
	leftPad := (m.width - paletteWidth) / 2

	var result strings.Builder
	for i := 0; i < topPad; i++ {
		result.WriteString("\n")
	}
	if leftPad > 0 {
		padding := strings.Repeat(" ", leftPad)
		for _, line := range strings.Split(panel, "\n") {
			result.WriteString(padding)
			result.WriteString(line)
			result.WriteString("\n")
		}
	} else {
		result.WriteString(panel)
		result.WriteString("\n")
	}

	return result.String()
}

func (m *CommandPaletteModel) updateMatches() {
	query := strings.TrimSpace(m.input.Value())
	if query == "" {
		m.matches = make([]CommandInfo, len(m.commands))
		copy(m.matches, m.commands)
		m.selected = 0
		return
	}

	q := strings.ToLower(query)

	// Score and sort commands by match quality
	type scoredCmd struct {
		cmd   CommandInfo
		score int
	}
	var scored []scoredCmd
	for _, cmd := range m.commands {
		kw := strings.ToLower(cmd.Name + " " + cmd.Description + " " + cmd.Slash)
		if strings.Contains(kw, q) {
			// Higher score for matches at the start
			score := 1
			if strings.HasPrefix(kw, q) {
				score = 10
			}
			if strings.HasPrefix(strings.ToLower(cmd.Slash), q) {
				score = 20
			}
			scored = append(scored, scoredCmd{cmd: cmd, score: score})
		}
	}

	sort.Slice(scored, func(i, j int) bool {
		return scored[i].score > scored[j].score
	})

	m.matches = make([]CommandInfo, 0, len(scored))
	for _, s := range scored {
		m.matches = append(m.matches, s.cmd)
	}
	m.selected = 0
}
