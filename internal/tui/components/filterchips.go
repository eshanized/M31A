package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// FilterChip represents a single filter toggle.
type FilterChip struct {
	Label  string
	Active bool
	Icon   string // optional icon when active
}

// FilterChips renders a horizontal row of filter toggles.
type FilterChips struct {
	Chips    []FilterChip
	Selected int // index of focused chip
	Theme    theme.Theme
}

// Render returns the filter chips as a string.
func (f FilterChips) Render() string {
	if len(f.Chips) == 0 {
		return ""
	}

	t := f.Theme
	if t.Text == "" {
		t = theme.Default()
	}
	parts := make([]string, len(f.Chips))

	for i, chip := range f.Chips {
		label := chip.Label
		if chip.Icon != "" && chip.Active {
			label = chip.Icon + " " + label
		}

		var style lipgloss.Style
		if i == f.Selected {
			// Focused chip
			if chip.Active {
				style = lipgloss.NewStyle().
					Background(t.Brand).
					Foreground(lipgloss.Color("#000000")).
					Bold(true).
					Padding(0, 1)
			} else {
				style = lipgloss.NewStyle().
					Border(lipgloss.ThickBorder()).
					BorderForeground(t.Brand).
					Foreground(t.TextPrimary).
					Padding(0, 1)
			}
		} else if chip.Active {
			// Active but not focused
			style = lipgloss.NewStyle().
				Background(t.SurfaceElevated).
				Foreground(t.Brand).
				Bold(true).
				Padding(0, 1)
		} else {
			// Inactive
			style = lipgloss.NewStyle().
				Foreground(t.TextSecondary).
				Padding(0, 1)
		}

		parts[i] = style.Render(" " + label + " ")
	}

	return strings.Join(parts, " ")
}

// ActiveCount returns the number of active chips.
func (f FilterChips) ActiveCount() int {
	count := 0
	for _, chip := range f.Chips {
		if chip.Active {
			count++
		}
	}
	return count
}

// ToggleChip toggles the active state of a chip by index.
func (f *FilterChips) ToggleChip(index int) {
	if index >= 0 && index < len(f.Chips) {
		f.Chips[index].Active = !f.Chips[index].Active
	}
}

// ChipGroup renders a labeled group of filter chips.
type ChipGroup struct {
	Label     string
	Chips     FilterChips
	Separator bool
	Theme     theme.Theme
}

// Render returns the chip group as a string.
func (g ChipGroup) Render() string {
	t := g.Theme
	if t.Text == "" {
		t = theme.Default()
	}

	labelStyle := lipgloss.NewStyle().
		Foreground(t.TextSecondary).
		Bold(true)

	label := labelStyle.Render(g.Label + ":")
	chips := g.Chips.Render()

	result := lipgloss.JoinHorizontal(lipgloss.Top, label, " ", chips)

	if g.Separator {
		sep := lipgloss.NewStyle().
			Foreground(t.Border).
			Render(strings.Repeat("─", lipgloss.Width(result)))
		result = result + "\n" + sep
	}

	return result
}
