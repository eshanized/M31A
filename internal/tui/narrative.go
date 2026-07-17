package tui

import (
	"fmt"

	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/narrative"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// NarrativeBubbleMsg is the Bubble Tea message carrying a processed narrative.
type NarrativeBubbleMsg struct {
	Narrative narrative.NarrativeObject
}

// NarrativeState holds the current narrative display state in the TUI.
type NarrativeState struct {
	Active  narrative.NarrativeObject   // currently displayed narrative
	History []narrative.NarrativeObject // recent narratives (ring buffer, max 5)
	MaxHist int
}

// NewNarrativeState creates a new narrative state.
func NewNarrativeState() *NarrativeState {
	return &NarrativeState{
		MaxHist: 5,
	}
}

// Update replaces the active narrative and pushes the old one to history.
func (ns *NarrativeState) Update(n narrative.NarrativeObject) {
	if ns.Active.Type != "" {
		ns.History = append(ns.History, ns.Active)
		if len(ns.History) > ns.MaxHist {
			ns.History = ns.History[1:]
		}
	}
	ns.Active = n
}

// Clear resets the active narrative.
func (ns *NarrativeState) Clear() {
	if ns.Active.Type != "" {
		ns.History = append(ns.History, ns.Active)
		if len(ns.History) > ns.MaxHist {
			ns.History = ns.History[1:]
		}
	}
	ns.Active = narrative.NarrativeObject{}
}

// renderNarrativeSidebar renders the narrative state for the sidebar.
func RenderNarrativeSidebar(ns *NarrativeState, t theme.Theme, width int) []string {
	if ns == nil {
		return nil
	}

	var lines []string

	// Active narrative (if any)
	if ns.Active.Type != "" {
		lines = append(lines, renderActiveNarrative(ns.Active, t, width))
	}

	// Recent history (last 2-3 items, faded)
	historyStart := 0
	if len(ns.History) > 3 {
		historyStart = len(ns.History) - 3
	}
	for _, h := range ns.History[historyStart:] {
		lines = append(lines, renderHistoricalNarrative(h, t, width))
	}

	return lines
}

// renderActiveNarrative renders the current active narrative.
func renderActiveNarrative(n narrative.NarrativeObject, t theme.Theme, width int) string {
	icon := narrativeIcon(n)
	style := lipgloss.NewStyle().
		Foreground(narrativeColor(n, t)).
		Width(width - 2)

	text := n.Text
	if len(text) > width-4 {
		text = text[:width-7] + "..."
	}

	return style.Render(fmt.Sprintf("%s %s", icon, text))
}

// renderHistoricalNarrative renders a historical narrative with reduced emphasis.
func renderHistoricalNarrative(n narrative.NarrativeObject, t theme.Theme, width int) string {
	style := lipgloss.NewStyle().
		Foreground(t.TextMuted).
		Faint(true).
		Width(width - 2)

	text := n.Text
	if len(text) > width-6 {
		text = text[:width-9] + "..."
	}

	return style.Render("  " + text)
}

// narrativeIcon returns an icon for the narrative category.
func narrativeIcon(n narrative.NarrativeObject) string {
	switch n.Category {
	case narrative.CategoryOrienting:
		return "\u25c6" // diamond
	case narrative.CategoryUnderstanding:
		return "\u25cf" // filled circle
	case narrative.CategoryDiscussing:
		return "\u25c7" // open diamond
	case narrative.CategoryPlanning:
		return "\u25a1" // open square
	case narrative.CategoryResearching:
		return "\u25c8" // diamond with dot
	case narrative.CategoryExecuting:
		return "\u25b6" // play triangle
	case narrative.CategoryVerifying:
		return "\u2713" // checkmark
	case narrative.CategoryRecovering:
		return "\u21ba" // circular arrow
	case narrative.CategoryShipping:
		return "\u2192" // right arrow
	case narrative.CategoryLearning:
		return "\u2605" // star
	case narrative.CategoryAlerting:
		return "\u26a0" // warning triangle
	default:
		return "\u25cf" // filled circle
	}
}

// narrativeColor returns the theme color for the narrative category.
func narrativeColor(n narrative.NarrativeObject, t theme.Theme) lipgloss.TerminalColor {
	switch n.Category {
	case narrative.CategoryAlerting:
		return t.Error
	case narrative.CategoryRecovering:
		return t.Warning
	case narrative.CategoryExecuting:
		return t.Info
	case narrative.CategoryShipping:
		return t.Success
	case narrative.CategoryPlanning:
		return t.Info
	case narrative.CategoryDiscussing:
		return t.Info
	default:
		return t.Text
	}
}

// renderNarrativeFooter renders a compact narrative status line for the footer.
func renderNarrativeFooter(ns *NarrativeState, t theme.Theme, width int) string {
	if ns == nil || ns.Active.Type == "" {
		return ""
	}

	icon := narrativeIcon(ns.Active)
	color := narrativeColor(ns.Active, t)

	style := lipgloss.NewStyle().
		Foreground(color).
		Width(width)

	text := ns.Active.Text
	maxText := width - 4
	if maxText < 10 {
		maxText = 10
	}
	if len(text) > maxText {
		text = text[:maxText-3] + "..."
	}

	return style.Render(fmt.Sprintf("%s %s", icon, text))
}

// narrativeCategoryLabel returns a short label for the category.
func narrativeCategoryLabel(cat narrative.Category) string {
	switch cat {
	case narrative.CategoryOrienting:
		return "init"
	case narrative.CategoryUnderstanding:
		return "reading"
	case narrative.CategoryDiscussing:
		return "discuss"
	case narrative.CategoryPlanning:
		return "plan"
	case narrative.CategoryResearching:
		return "research"
	case narrative.CategoryExecuting:
		return "exec"
	case narrative.CategoryVerifying:
		return "verify"
	case narrative.CategoryRecovering:
		return "heal"
	case narrative.CategoryShipping:
		return "ship"
	case narrative.CategoryLearning:
		return "context"
	case narrative.CategoryAlerting:
		return "alert"
	default:
		return ""
	}
}
