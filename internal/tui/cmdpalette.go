package tui

import (
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// CommandPaletteModel is the command palette overlay.
// It shows a filtered list of slash commands and runs the selected one.
type CommandPaletteModel struct {
	theme    theme.Theme
	commands []CommandInfo
	filtered []CommandInfo
	selected int
	query    string
	visible  bool
	width    int
	height   int
}

// NewCommandPalette creates a new command palette model.
func NewCommandPalette(registry *CommandRegistry, t theme.Theme) *CommandPaletteModel {
	cmds := []CommandInfo{}
	if registry != nil {
		cmds = registry.AllCommandsWithExecute()
	}
	return &CommandPaletteModel{
		theme:    t,
		commands: cmds,
		filtered: cmds,
	}
}

// SetTheme updates the command palette theme.
func (cp *CommandPaletteModel) SetTheme(t theme.Theme) {
	cp.theme = t
}

// SetDimensions updates the display dimensions.
func (cp *CommandPaletteModel) SetDimensions(w, h int) {
	cp.width = w
	cp.height = h
}

// Open shows the command palette.
func (cp *CommandPaletteModel) Open() {
	cp.visible = true
	cp.query = ""
	cp.selected = 0
	cp.filtered = cp.commands
}

// Close hides the command palette.
func (cp *CommandPaletteModel) Close() {
	cp.visible = false
	cp.query = ""
}

// IsOpen returns true when the palette is visible.
func (cp *CommandPaletteModel) IsOpen() bool {
	return cp.visible
}

// Update handles key events inside the command palette.
func (cp *CommandPaletteModel) Update(msg tea.Msg) (*CommandPaletteModel, tea.Cmd) {
	if !cp.visible {
		return cp, nil
	}

	switch msg := msg.(type) {
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "ctrl+p":
			cp.Close()
		case "up":
			if cp.selected > 0 {
				cp.selected--
			}
		case "down":
			if cp.selected < len(cp.filtered)-1 {
				cp.selected++
			}
		case "enter":
			if cp.selected < len(cp.filtered) {
				cmd := cp.filtered[cp.selected]
				cp.Close()
				if cmd.Execute != nil {
					return cp, cmd.Execute()
				}
				// Fall back to emitting SlashCommandMsg
				return cp, func() tea.Msg {
					return SlashCommandMsg{Command: cmd.Slash}
				}
			}
		case "backspace":
			if len(cp.query) > 0 {
				cp.query = cp.query[:len(cp.query)-1]
				cp.filterCommands()
			}
		default:
			if len(msg.Runes) > 0 {
				cp.query += string(msg.Runes)
				cp.filterCommands()
			}
		}
	}
	return cp, nil
}

// filterCommands filters the command list based on the current query.
func (cp *CommandPaletteModel) filterCommands() {
	cp.selected = 0
	if cp.query == "" {
		cp.filtered = cp.commands
		return
	}
	q := strings.ToLower(cp.query)
	var filtered []CommandInfo
	for _, cmd := range cp.commands {
		if strings.Contains(strings.ToLower(cmd.Name), q) ||
			strings.Contains(strings.ToLower(cmd.Description), q) {
			filtered = append(filtered, cmd)
		}
	}
	cp.filtered = filtered
}

// View renders the command palette overlay.
func (cp *CommandPaletteModel) View() string {
	if !cp.visible {
		return ""
	}
	t := cp.theme

	// Palette box
	paletteWidth := 60
	if cp.width > 0 && cp.width < paletteWidth+4 {
		paletteWidth = cp.width - 4
	}

	// Title
	title := lipgloss.NewStyle().
		Foreground(t.Brand).
		Bold(true).
		Render("⌘ Command Palette")

	// Search bar
	searchLabel := lipgloss.NewStyle().Foreground(t.TextMuted).Render("> ")
	searchText := lipgloss.NewStyle().Foreground(t.Text).Render(cp.query)
	cursor := lipgloss.NewStyle().Foreground(t.Brand).Render("█")
	searchBar := searchLabel + searchText + cursor

	divider := strings.Repeat("─", paletteWidth)

	// Command list
	maxItems := 10
	var items []string
	start := 0
	if cp.selected >= maxItems {
		start = cp.selected - maxItems + 1
	}
	for i := start; i < len(cp.filtered) && i < start+maxItems; i++ {
		cmd := cp.filtered[i]
		nameStyle := lipgloss.NewStyle().Foreground(t.Text)
		descStyle := lipgloss.NewStyle().Foreground(t.TextMuted)
		if i == cp.selected {
			nameStyle = nameStyle.Background(t.Brand).Foreground(t.Background)
			descStyle = descStyle.Background(t.Brand).Foreground(t.Background)
		}
		slash := lipgloss.NewStyle().Foreground(t.Brand).Render(cmd.Slash)
		name := nameStyle.Render(" " + cmd.Name)
		desc := descStyle.Render("  " + cmd.Description)
		_ = slash
		line := name + desc
		if lipgloss.Width(line) > paletteWidth {
			line = TruncateWithEllipsis(line, paletteWidth)
		}
		items = append(items, line)
	}

	if len(cp.filtered) == 0 {
		items = append(items, lipgloss.NewStyle().Foreground(t.TextMuted).Italic(true).Render("  No commands match"))
	}

	content := lipgloss.JoinVertical(lipgloss.Left,
		title,
		"",
		searchBar,
		divider,
		strings.Join(items, "\n"),
	)

	palette := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(1, 2).
		Width(paletteWidth).
		Render(content)

	// Center in terminal
	return centerScreen(palette, cp.width, cp.height)
}
