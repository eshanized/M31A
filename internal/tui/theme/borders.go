package theme

import "github.com/charmbracelet/lipgloss"

// Gradient borders for focused elements
var (
	BrandGradientBorder = lipgloss.Border{
		Top:         "─",
		Bottom:      "─",
		Left:        "│",
		Right:       "│",
		TopLeft:     "╭",
		TopRight:    "╮",
		BottomLeft:  "╰",
		BottomRight: "╯",
	}

	ThinkingGradientBorder = lipgloss.Border{
		Top:         "─",
		Bottom:      "─",
		Left:        "│",
		Right:       "│",
		TopLeft:     "╭",
		TopRight:    "╮",
		BottomLeft:  "╰",
		BottomRight: "╯",
	}
)

// Custom border definitions
var (
	HeavyBorder = lipgloss.Border{
		Top:         "━",
		Bottom:      "━",
		Left:        "┃",
		Right:       "┃",
		TopLeft:     "┏",
		TopRight:    "┓",
		BottomLeft:  "┗",
		BottomRight: "┛",
	}

	DashedBorder = lipgloss.Border{
		Top:         "╌",
		Bottom:      "╌",
		Left:        "╎",
		Right:       "╎",
		TopLeft:     "┌",
		TopRight:    "┐",
		BottomLeft:  "└",
		BottomRight: "┘",
	}

	ShadowBorder = lipgloss.Border{
		Top:         "─",
		Bottom:      "▄",
		Left:        "│",
		Right:       "▐",
		TopLeft:     "╭",
		TopRight:    "╮",
		BottomLeft:  "╘",
		BottomRight: "╛",
	}
)

// BrandGradientStyle returns a style with a 3-color gradient border
func BrandGradientStyle() lipgloss.Style {
	return lipgloss.NewStyle().
		Border(BrandGradientBorder).
		BorderForeground(
			lipgloss.Color("#D77757"),
			lipgloss.Color("#E8A87C"),
			lipgloss.Color("#D77757"),
		)
}

// ThinkingGradientStyle returns a style with a 2-color gradient border
func ThinkingGradientStyle() lipgloss.Style {
	return lipgloss.NewStyle().
		Border(ThinkingGradientBorder).
		BorderForeground(
			lipgloss.Color("#8AB4F8"),
			lipgloss.Color("#6B9BD2"),
		)
}

// BorderByName returns a border by name
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
	case "shadow":
		return ShadowBorder
	case "none":
		return lipgloss.HiddenBorder()
	default:
		return NormalBorder
	}
}
