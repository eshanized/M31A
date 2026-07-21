package components

import (
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"
)

// Breadcrumb renders a navigation breadcrumb trail.
type Breadcrumb struct {
	Parts []string
	Theme theme.Theme
	Width int
}

// BuildFromStack creates breadcrumb items from a screen stack and current screen.
// The stack provides the path history; the current screen is the final breadcrumb.
func BuildFromStack(screens []tuitypes.Screen, current tuitypes.Screen) []BreadcrumbItem {
	var items []BreadcrumbItem
	for _, s := range screens {
		items = append(items, BreadcrumbItem{
			Label:  screenLabel(s),
			Screen: s,
		})
	}
	// Add the current screen if not already the last item
	if len(items) == 0 || items[len(items)-1].Screen != current {
		items = append(items, BreadcrumbItem{
			Label:  screenLabel(current),
			Screen: current,
		})
	}
	return items
}

// BreadcrumbItem represents a single breadcrumb with a label and screen reference.
type BreadcrumbItem struct {
	Label  string
	Screen tuitypes.Screen
}

// screenLabel returns a human-readable label for a screen constant.
func screenLabel(s tuitypes.Screen) string {
	return s.Label()
}

// View renders the breadcrumb.
func (bc Breadcrumb) View() string {
	t := bc.Theme
	if len(bc.Parts) == 0 {
		return ""
	}

	var parts []string
	for i, part := range bc.Parts {
		if i == len(bc.Parts)-1 {
			parts = append(parts, lipgloss.NewStyle().
				Foreground(t.Brand).Bold(true).Render(part))
		} else {
			parts = append(parts, lipgloss.NewStyle().
				Foreground(t.TextMuted).Render(part))
		}
	}

	sep := lipgloss.NewStyle().Foreground(t.TextMuted).Render(" › ")
	return strings.Join(parts, sep)
}
