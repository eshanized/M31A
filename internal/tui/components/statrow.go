package components

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// StatRow renders a compact label-value pair with optional icon.
type StatRow struct {
	Label    string
	Value    string
	Icon     string // optional emoji or symbol
	Width    int    // total width, 0 for auto
	Align    lipgloss.Position
}

// Render returns the stat row as a string.
func (s StatRow) Render() string {
	t := theme.Default()

	labelStyle := lipgloss.NewStyle().
		Foreground(t.TextSecondary)

	valueStyle := lipgloss.NewStyle().
		Foreground(t.TextPrimary)

	icon := ""
	if s.Icon != "" {
		icon = s.Icon + " "
	}

	label := icon + s.Label + ":"
	left := labelStyle.Render(label)
	right := valueStyle.Render(s.Value)

	row := lipgloss.JoinHorizontal(lipgloss.Top, left, " ", right)

	if s.Width > 0 {
		row = lipgloss.NewStyle().
			Width(s.Width).
			Align(s.Align).
			Render(row)
	}

	return row
}

// StatGroup renders multiple stat rows with optional separator.
type StatGroup struct {
	Stats     []StatRow
	Separator bool
	Width     int
}

// Render returns the stat group as a string.
func (s StatGroup) Render() string {
	if len(s.Stats) == 0 {
		return ""
	}

	rows := make([]string, 0, len(s.Stats)*2) // *2 if separators included

	for i, stat := range s.Stats {
		stat.Width = s.Width
		rows = append(rows, stat.Render())

		if s.Separator && i < len(s.Stats)-1 {
			t := theme.Default()
			sep := lipgloss.NewStyle().
				Foreground(t.Border).
				Render(strings.Repeat("─", s.Width))
			rows = append(rows, sep)
		}
	}

	return strings.Join(rows, "\n")
}

// KeyValue renders a key-value pair in a styled box.
type KeyValue struct {
	Key   string
	Value string
	Width int
}

// Render returns the key-value pair as a string.
func (kv KeyValue) Render() string {
	t := theme.Default()

	keyStyle := lipgloss.NewStyle().
		Foreground(t.TextSecondary)

	valueStyle := lipgloss.NewStyle().
		Foreground(t.TextPrimary).
		Bold(true)

	key := keyStyle.Render(kv.Key)
	value := valueStyle.Render(kv.Value)

	result := fmt.Sprintf("%s: %s", key, value)

	if kv.Width > 0 {
		result = lipgloss.NewStyle().Width(kv.Width).Render(result)
	}

	return result
}

// KeyValueGrid renders multiple key-value pairs with alternating backgrounds.
type KeyValueGrid struct {
	Pairs []KeyValue
	Width int
}

// Render returns the key-value grid as a string.
func (g KeyValueGrid) Render() string {
	if len(g.Pairs) == 0 {
		return ""
	}

	t := theme.Default()
	rows := make([]string, len(g.Pairs))

	for i, pair := range g.Pairs {
		keyStyle := lipgloss.NewStyle().
			Foreground(t.TextSecondary)

		valueStyle := lipgloss.NewStyle().
			Foreground(t.TextPrimary)

		row := fmt.Sprintf("%s: %s",
			keyStyle.Render(pair.Key),
			valueStyle.Render(pair.Value))

		// Alternate background every other row
		if i%2 == 0 {
			row = lipgloss.NewStyle().
				Background(t.Surface).
				Padding(0, 1).
				Render(row)
		}

		if g.Width > 0 {
			row = lipgloss.NewStyle().Width(g.Width).Render(row)
		}

		rows[i] = row
	}

	return strings.Join(rows, "\n")
}
