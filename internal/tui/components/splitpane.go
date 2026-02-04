package components

import (
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// SplitPane renders a side-by-side layout for wide terminals.
type SplitPane struct {
	Left     string
	Right    string
	Ratio    float64 // 0.0 to 1.0, portion for left pane
	Theme    theme.Theme
	Width    int
	Height   int
	Divider  bool
}

// View renders the split pane.
func (sp SplitPane) View() string {
	t := sp.Theme
	w := sp.Width
	leftW := int(float64(w) * sp.Ratio)
	rightW := w - leftW

	if sp.Divider {
		rightW -= 1
	}

	leftStyled := lipgloss.NewStyle().Width(leftW).Height(sp.Height).Render(sp.Left)

	if sp.Divider {
		dividerStr := lipgloss.NewStyle().
			Foreground(t.Border).
			Width(1).
			Height(sp.Height).
			Render("")
		rightStyled := lipgloss.NewStyle().Width(rightW).Height(sp.Height).Render(sp.Right)
		return lipgloss.JoinHorizontal(lipgloss.Top, leftStyled, dividerStr, rightStyled)
	}

	rightStyled := lipgloss.NewStyle().Width(rightW).Height(sp.Height).Render(sp.Right)
	return lipgloss.JoinHorizontal(lipgloss.Top, leftStyled, rightStyled)
}
