package theme

import (
	"os"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// ColorProfile represents terminal color capability.
type ColorProfile int

const (
	ProfileTrueColor ColorProfile = iota
	Profile256
	Profile16
)

// DetectColorProfile determines terminal color capability from environment variables.
func DetectColorProfile() ColorProfile {
	ct := os.Getenv("COLORTERM")
	if ct == "truecolor" || ct == "24bit" {
		return ProfileTrueColor
	}
	term := os.Getenv("TERM")
	if strings.Contains(term, "256") || strings.Contains(term, "256color") {
		return Profile256
	}
	return Profile16
}

// PaletteForProfile returns a Theme suitable for the given color profile.
// TrueColor and 256-color profiles use the full hex-based palette.
// 16-color profiles use ANSI named colors for reliable rendering.
func PaletteForProfile(profile ColorProfile) Theme {
	switch profile {
	case Profile16:
		return ansiPalette()
	default:
		return Default()
	}
}

// ansiPalette returns a Theme that uses ANSI 16-color names (0–7, 0–15)
// for reliable rendering on terminals without 256-color or truecolor support.
func ansiPalette() Theme {
	t := Dark() // start from dark palette
	// Override key color fields with ANSI color codes
	t.Brand = lipgloss.Color("208")  // ANSI orange (256-color index)
	t.Success = lipgloss.Color("2")  // ANSI green
	t.Error = lipgloss.Color("1")    // ANSI red
	t.Warning = lipgloss.Color("3")  // ANSI yellow
	t.Thinking = lipgloss.Color("4") // ANSI blue
	applyThemeStyles(&t)
	return t
}

func Dark() Theme {
	t := Theme{
		Mode:              ModeDark,
		Background:        lipgloss.Color("#0d0f1a"), // deep blue-black
		Surface:           lipgloss.Color("#141520"), // slightly lighter
		SurfaceElevated:   lipgloss.Color("#1f2133"), // elevated panels
		Border:            lipgloss.Color("#3C4043"),
		Brand:             lipgloss.Color("#7C6AF7"), // electric indigo — modern AI aesthetic
		TextPrimary:       lipgloss.Color("#E8EAED"),
		TextSecondary:     lipgloss.Color("#9AA0A6"),
		Thinking:          lipgloss.Color("#5BC8F5"), // electric cyan for thinking
		Success:           lipgloss.Color("#81C995"),
		Error:             lipgloss.Color("#F28B82"),
		Warning:           lipgloss.Color("#FDD663"),
		CodeBG:            lipgloss.Color("#1A1C28"),
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color("#141520"),
		BackgroundElement: lipgloss.Color("#1f2133"),
		Text:              lipgloss.Color("#E8EAED"),
		TextMuted:         lipgloss.Color("#6B7280"),
		BorderActive:      lipgloss.Color("#7C6AF7"), // brand color on active borders
		BorderSubtle:      lipgloss.Color("#2A2D3E"),
		Primary:           lipgloss.Color("#7C6AF7"),
		Secondary:         lipgloss.Color("#5BC8F5"), // electric cyan
		Accent:            lipgloss.Color("#5BC8F5"),
		Info:              lipgloss.Color("#5BC8F5"),
		ThinkingOpacity:   0.7,
		DiffAdded:         lipgloss.Color("#81C995"),
		DiffRemoved:       lipgloss.Color("#F28B82"),
		DiffAddedBg:       lipgloss.Color("#81C99520"),
		DiffRemovedBg:     lipgloss.Color("#F28B8220"),
		DiffContextBg:     lipgloss.Color("#1f2133"),
		BadgeForeground:   lipgloss.Color("#FFFFFF"), // white on indigo badges
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),
		BadgeTextDark:     lipgloss.Color("#000000"),

		DividerChar:  "─",
		HeaderHeight: 1,
		ShadowColor:  lipgloss.Color("#00000060"),
		CompactMode:  false,
		SelectionBg:  lipgloss.Color("#2A2D3E"),
		CardPadding:  1,
		TabWidth:     4,
	}
	applyThemeStyles(&t)

	return t
}

func Light() Theme {
	t := Theme{
		Mode:              ModeLight,
		Background:        lipgloss.Color("#fafaf8"), // warm white
		Surface:           lipgloss.Color("#f5f5f0"), // warm surface
		SurfaceElevated:   lipgloss.Color("#ffffff"), // pure white for elevation
		Border:            lipgloss.Color("#DADCE0"), // was #E0E0E0
		Brand:             lipgloss.Color("#D77757"), // was #7C3AED (same as dark — brand is brand)
		TextPrimary:       lipgloss.Color("#202124"), // was #1E293B
		TextSecondary:     lipgloss.Color("#5F6368"), // was #64748B
		Thinking:          lipgloss.Color("#1A73E8"), // was #7C3AED
		Success:           lipgloss.Color("#137333"), // was #059669
		Error:             lipgloss.Color("#C5221F"), // was #DC2626
		Warning:           lipgloss.Color("#EA8600"), // was #D97706
		CodeBG:            lipgloss.Color("#F1F3F4"), // keep
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color("#F8F9FA"),   // keep
		BackgroundElement: lipgloss.Color("#FFFFFF"),   // keep
		Text:              lipgloss.Color("#202124"),   // was #1E293B
		TextMuted:         lipgloss.Color("#9AA0A6"),   // was #94A3B8
		BorderActive:      lipgloss.Color("#D77757"),   // was #C0C0C0
		BorderSubtle:      lipgloss.Color("#E8E8E8"),   // keep
		Primary:           lipgloss.Color("#D77757"),   // was #7C3AED
		Secondary:         lipgloss.Color("#1A73E8"),   // was #0891B2
		Accent:            lipgloss.Color("#1A73E8"),   // was #0891B2
		Info:              lipgloss.Color("#1A73E8"),   // was #0891B2
		ThinkingOpacity:   0.6,                         // keep
		DiffAdded:         lipgloss.Color("#137333"),   // was #059669
		DiffRemoved:       lipgloss.Color("#C5221F"),   // was #DC2626
		DiffAddedBg:       lipgloss.Color("#13733320"), // was #05966920
		DiffRemovedBg:     lipgloss.Color("#C5221F20"), // was #DC262620
		DiffContextBg:     lipgloss.Color("#F8F9FA"),   // keep
		BadgeForeground:   lipgloss.Color("#000000"),   // keep
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),   // keep
		BadgeTextDark:     lipgloss.Color("#000000"),   // keep

		DividerChar:  "─",
		HeaderHeight: 1,
		ShadowColor:  lipgloss.Color("#00000040"),
		CompactMode:  false,
		SelectionBg:  lipgloss.Color("#DADCE0"),
		CardPadding:  1,
		TabWidth:     4,
	}
	applyThemeStyles(&t)

	return t
}

// buildBadgeStyles sets all badge/label styles using the theme's BadgeForeground color.
func buildBadgeStyles(t *Theme) {
	fg := lipgloss.Color(t.BadgeForeground)
	// Execution tools
	t.ToolLabel["Bash"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Warning)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ToolLabel["Agent"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Secondary)).Foreground(fg).Padding(0, 1).Bold(true)
	// File tools
	t.ToolLabel["FileRead"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Thinking)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ToolLabel["FileWrite"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Brand)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ToolLabel["FileDelete"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Error)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ToolLabel["FileMove"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Warning)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ToolLabel["FileList"] = lipgloss.NewStyle().Background(lipgloss.Color(t.TextSecondary)).Foreground(fg).Padding(0, 1).Bold(true)
	// Code analysis tools
	t.ToolLabel["CodeMap"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Success)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ToolLabel["Edit"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Brand)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ToolLabel["Glob"] = lipgloss.NewStyle().Background(lipgloss.Color(t.TextSecondary)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ToolLabel["Grep"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Thinking)).Foreground(fg).Padding(0, 1).Bold(true)
	// Web tools
	t.ToolLabel["WebFetch"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Thinking)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ToolLabel["WebSearch"] = lipgloss.NewStyle().Background(lipgloss.Color(t.TextSecondary)).Foreground(fg).Padding(0, 1).Bold(true)
	// User interaction tools
	t.ToolLabel["AskUserQuestion"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Brand)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ToolLabel["TodoWrite"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Success)).Foreground(fg).Padding(0, 1).Bold(true)
	t.SuccessBadge = lipgloss.NewStyle().Background(lipgloss.Color(t.Success)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ErrorBadge = lipgloss.NewStyle().Background(lipgloss.Color(t.Error)).Foreground(fg).Padding(0, 1).Bold(true)
	t.WarningBadge = lipgloss.NewStyle().Background(lipgloss.Color(t.Warning)).Foreground(fg).Padding(0, 1).Bold(true)
}

// applyThemeStyles sets all computed styles that depend on base colors.
func applyThemeStyles(t *Theme) {
	t.Header = lipgloss.NewStyle().Bold(true)
	t.ModelBadge = lipgloss.NewStyle().Background(lipgloss.Color(t.Brand)).Foreground(lipgloss.Color(t.BadgeTextLight)).Padding(0, 1).Bold(true)
	t.ContextBar = lipgloss.NewStyle().Background(lipgloss.Color(t.Surface)).Foreground(lipgloss.Color(t.TextSecondary)).Padding(0, 1)
	t.StatusLive = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Success)).Bold(true)
	t.StatusSlow = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Warning)).Bold(true)
	t.StatusOffline = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Error)).Bold(true)
	t.UserBubble = lipgloss.NewStyle().Background(lipgloss.Color(t.SurfaceElevated)).Foreground(lipgloss.Color(t.TextPrimary)).Padding(0, 1).Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color(t.Border))
	t.AssistantBubble = lipgloss.NewStyle().Background(lipgloss.Color(t.Surface)).Foreground(lipgloss.Color(t.TextPrimary)).Padding(0, 1).Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color(t.Border))
	t.InputArea = lipgloss.NewStyle().Background(lipgloss.Color(t.Surface)).Foreground(lipgloss.Color(t.TextPrimary)).Padding(0, 1)
	t.Spinner = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Thinking))
	t.ThinkingBlock = lipgloss.NewStyle().Background(lipgloss.Color(t.CodeBG)).Foreground(lipgloss.Color(t.Thinking)).Padding(0, 1).Italic(true)
	t.ToolCard = lipgloss.NewStyle().Background(lipgloss.Color(t.SurfaceElevated)).Foreground(lipgloss.Color(t.TextPrimary)).Padding(0, 1).Border(ThinBorder).BorderForeground(lipgloss.Color(t.Border))
	buildBadgeStyles(t)
	t.ProgressBar = lipgloss.NewStyle().Background(lipgloss.Color(t.SurfaceElevated)).Foreground(lipgloss.Color(t.Brand))
	t.Modal = lipgloss.NewStyle().Background(lipgloss.Color(t.SurfaceElevated)).Foreground(lipgloss.Color(t.TextPrimary)).Padding(1, 2).Border(NormalBorder).BorderForeground(lipgloss.Color(t.Brand))
	t.ModalTitle = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Brand)).Bold(true)

	// New card border styles
	t.CardBorder = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Brand))
	t.CardBorderActive = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Brand)).
		Bold(true)
	t.CardBorderError = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Error))
	t.CardBorderWarn = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Warning))

	// Workflow phase styles
	t.PhaseActive = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Brand)).Bold(true)
	t.PhasePast = lipgloss.NewStyle().Foreground(lipgloss.Color(t.TextMuted))
	t.PhaseFuture = lipgloss.NewStyle().Foreground(lipgloss.Color(t.TextMuted))

	// Timeline and metric styles
	t.TimelineDate = lipgloss.NewStyle().Foreground(lipgloss.Color(t.TextMuted)).Bold(true)
	t.MetricValue = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Brand)).Bold(true)
	t.MetricLabel = lipgloss.NewStyle().Foreground(lipgloss.Color(t.TextMuted))

	// Block character constants for progress bars, sparklines, density indicators
	t.BlockFull = "█"
	t.BlockHigh = "▓"
	t.BlockMed = "▒"
	t.BlockLow = "░"
}

func Auto() Theme {
	if lipgloss.HasDarkBackground() {
		return Dark()
	}
	return Light()
}

// WithAccent returns a new Theme with the accent color overridden
func (t *Theme) WithAccent(hex string) Theme {
	newTheme := *t
	newTheme.Brand = lipgloss.Color(hex)
	newTheme.Primary = lipgloss.Color(hex)
	newTheme.BorderActive = lipgloss.Color(hex)
	applyThemeStyles(&newTheme)
	return newTheme
}
