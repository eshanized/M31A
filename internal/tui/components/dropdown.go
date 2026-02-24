package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// DropdownItem represents an option in a dropdown.
type DropdownItem struct {
	Label string
	Value string
}

// Dropdown renders a selection dropdown.
type Dropdown struct {
	Items  []DropdownItem
	Cursor int
	Open   bool
	Theme  theme.Theme
	Width  int
}

// Selected returns the currently selected item.
func (dd *Dropdown) Selected() *DropdownItem {
	if dd.Cursor >= 0 && dd.Cursor < len(dd.Items) {
		return &dd.Items[dd.Cursor]
	}
	return nil
}

// MoveUp moves the cursor up.
func (dd *Dropdown) MoveUp() {
	if dd.Cursor > 0 {
		dd.Cursor--
	}
}

// MoveDown moves the cursor down.
func (dd *Dropdown) MoveDown() {
	if dd.Cursor < len(dd.Items)-1 {
		dd.Cursor++
	}
}

// Toggle opens/closes the dropdown.
func (dd *Dropdown) Toggle() {
	dd.Open = !dd.Open
}

// View renders the dropdown.
func (dd *Dropdown) View() string {
	t := dd.Theme

	if len(dd.Items) == 0 {
		return ""
	}

	selected := dd.Items[dd.Cursor].Label
	header := lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(t.Brand).
		Padding(0, 1).
		Width(dd.Width).
		Render(selected + " ▾")

	if !dd.Open {
		return header
	}

	var options []string
	for i, item := range dd.Items {
		if i == dd.Cursor {
			options = append(options, lipgloss.NewStyle().
				Foreground(t.Brand).Bold(true).
				Width(dd.Width).
				Render("▶ "+item.Label))
		} else {
			options = append(options, lipgloss.NewStyle().
				Foreground(t.Text).
				Width(dd.Width).
				Render("  "+item.Label))
		}
	}

	return header + "\n" + strings.Join(options, "\n")
}
