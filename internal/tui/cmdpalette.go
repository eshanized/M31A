package tui

import (
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// CommandCategory groups slash commands in the palette.
type CommandCategory string

const (
	CatCore     CommandCategory = "Core"
	CatAI       CommandCategory = "AI"
	CatConfig   CommandCategory = "Config"
	CatSession  CommandCategory = "Session"
	CatGit      CommandCategory = "Git"
	CatWorkflow CommandCategory = "Workflow"
)

// paletteEntry wraps a CommandInfo with its category and optional shortcut.
type paletteEntry struct {
	cmd      CommandInfo
	category CommandCategory
	shortcut string // e.g. "ctrl+b" or ""
}

// CommandPaletteModel is the command palette overlay.
type CommandPaletteModel struct {
	theme    theme.Theme
	entries  []paletteEntry
	filtered []paletteEntry
	selected int
	query    string
	visible  bool
	width    int
	height   int
}

// NewCommandPalette creates a new command palette model.
func NewCommandPalette(registry *CommandRegistry, t theme.Theme) *CommandPaletteModel {
	entries := buildPaletteEntries(registry)
	return &CommandPaletteModel{
		theme:    t,
		entries:  entries,
		filtered: entries,
	}
}

// buildPaletteEntries categorizes all registered commands and assigns known shortcuts.
func buildPaletteEntries(registry *CommandRegistry) []paletteEntry {
	if registry == nil {
		return nil
	}
	cmds := registry.AllCommandsWithExecute()
	entries := make([]paletteEntry, 0, len(cmds))

	// Shortcut map for common key bindings
	shortcuts := map[string]string{
		"help":       "?",
		"model":      "ctrl+m",
		"settings":   "ctrl+s",
		"sidebar":    "ctrl+b",
		"discuss":    "ctrl+d",
		"new session": "ctrl+n",
	}

	// Category map
	catMap := make(map[string]CommandCategory, len(cmds))
	for _, cmd := range cmds {
		switch cmd.Name {
		// Core
		case "help", "clear", "status", "reset", "quit", "undo", "history", "health", "tools":
			catMap[cmd.Name] = CatCore
		// AI / model
		case "compress", "optimize", "model", "models", "fallback", "provider":
			catMap[cmd.Name] = CatAI
		// Config
		case "settings", "config", "theme", "cost", "log", "key", "tokens":
			catMap[cmd.Name] = CatConfig
		// Session
		case "sessions", "fork", "prev", "next", "save", "goal", "resume", "ledger":
			catMap[cmd.Name] = CatSession
		// Git
		case "diff", "rollback":
			catMap[cmd.Name] = CatGit
		// Workflow
		case "workflow", "plan", "execute", "verify", "ship", "phase", "pause", "metrics", "resume-task":
			catMap[cmd.Name] = CatWorkflow
		default:
			catMap[cmd.Name] = CatCore
		}
	}

	for _, cmd := range cmds {
		cat := catMap[cmd.Name]
		if cat == "" {
			cat = CatCore
		}
		entries = append(entries, paletteEntry{
			cmd:      cmd,
			category: cat,
			shortcut: shortcuts[cmd.Name],
		})
	}

	return entries
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
	cp.filtered = cp.entries
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
				entry := cp.filtered[cp.selected]
				cp.Close()
				if entry.cmd.Execute != nil {
					return cp, entry.cmd.Execute()
				}
				return cp, func() tea.Msg {
					return SlashCommandMsg{Command: entry.cmd.Slash}
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

// filterCommands filters entries based on the current query.
func (cp *CommandPaletteModel) filterCommands() {
	cp.selected = 0
	if cp.query == "" {
		cp.filtered = cp.entries
		return
	}
	q := strings.ToLower(cp.query)
	var filtered []paletteEntry
	for _, e := range cp.entries {
		if strings.Contains(strings.ToLower(e.cmd.Name), q) ||
			strings.Contains(strings.ToLower(e.cmd.Description), q) ||
			strings.Contains(strings.ToLower(string(e.category)), q) {
			filtered = append(filtered, e)
		}
	}
	cp.filtered = filtered
}

// View renders the command palette overlay, bottom-anchored above the status bar.
func (cp *CommandPaletteModel) View() string {
	if !cp.visible {
		return ""
	}
	t := cp.theme

	// Palette box (bottom of terminal — ensure room for status bar)
	paletteWidth := 60
	if cp.width > 0 && cp.width < paletteWidth+4 {
		paletteWidth = cp.width - 4
	}

	// ── Search bar ───────────────────────────────────────────────────────────
	searchLabel := lipgloss.NewStyle().Foreground(t.TextMuted).Render("> ")
	searchText := cp.renderHighlightedQuery(t)
	cursor := lipgloss.NewStyle().Foreground(t.Brand).Render("█")
	searchBar := searchLabel + searchText + cursor

	// ── Commands grouped by category ─────────────────────────────────────────
	maxItems := 12
	var items []string
	start := 0
	if cp.selected >= maxItems {
		start = cp.selected - maxItems + 1
	}

	// Deduplicate category headers
	seenCategories := make(map[CommandCategory]bool)
	itemCount := 0
	entryIdx := 0
	for i := 0; i < len(cp.filtered) && entryIdx < start+maxItems; i++ {
		entry := cp.filtered[i]

		// Insert category header (only before entries, not counted as item)
		if !seenCategories[entry.category] {
			seenCategories[entry.category] = true
			if entryIdx >= start {
				items = append(items, cp.renderCategoryHeader(entry.category))
			}
		}

		if entryIdx >= start && itemCount < maxItems {
			items = append(items, cp.renderEntry(i, entry, paletteWidth))
			itemCount++
		}
		entryIdx++
	}

	if len(cp.filtered) == 0 {
		items = append(items, lipgloss.NewStyle().Foreground(t.TextMuted).Italic(true).Render("  No commands match"))
	}

	// ── Footer hint ──────────────────────────────────────────────────────────
	footerHint := lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true).
		Render("  ↑↓ navigate  ↵ select  esc close")

	// ── Assemble palette ─────────────────────────────────────────────────────
	// Use SectionDivider between search and results
	divider := components.SectionDivider{Width: paletteWidth, Theme: t}.Render()

	var paletteParts []string
	paletteParts = append(paletteParts, "  "+searchBar)
	if len(items) > 0 {
		paletteParts = append(paletteParts, divider)
		paletteParts = append(paletteParts, strings.Join(items, "\n"))
	}
	paletteParts = append(paletteParts, "", footerHint)

	content := strings.Join(paletteParts, "\n")

	palette := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(0, 1).
		Width(paletteWidth).
		Render(content)

	// Bottom-anchored: position above bottom edge with 2-line margin
	// (1 for palette bottom border + 1 for status bar gap)
	paletteHeight := lipgloss.Height(palette)
	gapFromBottom := paletteHeight + 2
	return lipgloss.Place(cp.width, cp.height, lipgloss.Center, lipgloss.Bottom,
		lipgloss.NewStyle().
			MarginBottom(gapFromBottom).
			Render(palette))
}

// renderCategoryHeader renders a category section header.
func (cp *CommandPaletteModel) renderCategoryHeader(cat CommandCategory) string {
	t := cp.theme
	return lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Bold(true).
		PaddingLeft(2).
		PaddingTop(1).
		Render(string(cat))
}

// renderEntry renders a single palette entry with shortcut and search highlighting.
func (cp *CommandPaletteModel) renderEntry(idx int, entry paletteEntry, paletteWidth int) string {
	t := cp.theme
	isSelected := idx == cp.selected

	// Slash command in brand
	var slashPart string
	if isSelected {
		slashPart = lipgloss.NewStyle().
			Foreground(t.Background).
			Render(entry.cmd.Slash)
	} else {
		slashPart = lipgloss.NewStyle().
			Foreground(t.Brand).
			Render(entry.cmd.Slash)
	}

	// Description in muted (or background if selected)
	var descPart string
	descText := "  " + entry.cmd.Description
	if isSelected {
		descPart = lipgloss.NewStyle().
			Foreground(t.Background).
			Render(descText)
	} else {
		descPart = lipgloss.NewStyle().
			Foreground(t.TextMuted).
			Render(descText)
	}

	// Shortcut if available (right-aligned with spacer)
	shortcutPart := ""
	if entry.shortcut != "" {
		shortcutText := "  " + entry.shortcut + "  "
		if isSelected {
			shortcutPart = lipgloss.NewStyle().
				Foreground(t.Background).
				Render(shortcutText)
		} else {
			shortcutPart = lipgloss.NewStyle().
				Foreground(t.TextMuted).
				Faint(true).
				Render(shortcutText)
		}
	}

	line := "    " + slashPart + " " + descPart + shortcutPart
	if lipgloss.Width(line) > paletteWidth {
		line = TruncateWithEllipsis(line, paletteWidth)
	}

	if isSelected {
		return lipgloss.NewStyle().
			Background(t.Brand).
			Foreground(t.Background).
			Width(paletteWidth).
			Render(line)
	}
	return line
}

// renderHighlightedQuery renders the query with matched characters highlighted.
func (cp *CommandPaletteModel) renderHighlightedQuery(t theme.Theme) string {
	if cp.query == "" {
		return ""
	}

	// For simplicity, just render the query in primary color — full
	// character-level highlighting is complex and may flicker during rapid typing.
	return lipgloss.NewStyle().Foreground(t.Text).Render(cp.query)
}
