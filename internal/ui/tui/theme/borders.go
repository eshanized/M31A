package theme

import "github.com/charmbracelet/lipgloss"

// HeavyBorder is a bold double-line border for important modals.
var HeavyBorder = lipgloss.Border{
	Top:         "━",
	Bottom:      "━",
	Left:        "┃",
	Right:       "┃",
	TopLeft:     "┏",
	TopRight:    "┓",
	BottomLeft:  "┗",
	BottomRight: "┛",
}

// DashedBorder is a dashed border for compact elements.
var DashedBorder = lipgloss.Border{
	Top:         "╌",
	Bottom:      "╌",
	Left:        "╎",
	Right:       "╎",
	TopLeft:     "┌",
	TopRight:    "┐",
	BottomLeft:  "└",
	BottomRight: "┘",
}

// BorderByName returns a border by semantic name.
func BorderByName(name string) lipgloss.Border {
	switch name {
	case "thin":
		return ThinBorder
	case "double":
		return DoubleBorder
	case "heavy":
		return HeavyBorder
	case "dashed":
		return DashedBorder
	case "none":
		return lipgloss.HiddenBorder()
	default:
		return NormalBorder
	}
}
