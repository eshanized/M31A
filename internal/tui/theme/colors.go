package theme

import "github.com/charmbracelet/lipgloss"

func Dark() Theme {
	t := Theme{
		Mode:              ModeDark,
		Background:        lipgloss.Color("#0D0D0D"),  // was #0F0F1A
		Surface:           lipgloss.Color("#1A1A1A"),  // was #1E1E2E
		SurfaceElevated:   lipgloss.Color("#252525"),  // was #2A2A3E
		Border:            lipgloss.Color("#3C4043"),  // was #2E2E2E
		Brand:             lipgloss.Color("#D77757"),  // was #7C3AED
		TextPrimary:       lipgloss.Color("#E8EAED"),  // was #E2E8F0
		TextSecondary:     lipgloss.Color("#9AA0A6"),  // was #94A3B8
		Thinking:          lipgloss.Color("#8AB4F8"),  // was #8B5CF6
		Success:           lipgloss.Color("#81C995"),  // was #10B981
		Error:             lipgloss.Color("#F28B82"),  // was #EF4444
		Warning:           lipgloss.Color("#FDD663"),  // was #F59E0B
		CodeBG:            lipgloss.Color("#2D2D2D"),  // keep
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color("#1A1A1A"),  // was #1E1E2E
		BackgroundElement: lipgloss.Color("#252525"),  // was #2A2A3E
		Text:              lipgloss.Color("#E8EAED"),  // was #E2E8F0
		TextMuted:         lipgloss.Color("#9AA0A6"),  // was #475569
		BorderActive:      lipgloss.Color("#D77757"),  // was #484848 (brand color)
		BorderSubtle:      lipgloss.Color("#3C4043"),  // was #3C3C3C
		Primary:           lipgloss.Color("#D77757"),  // was #7C3AED
		Secondary:         lipgloss.Color("#8AB4F8"),  // was #06B6D4
		Accent:            lipgloss.Color("#8AB4F8"),  // was #06B6D4
		Info:              lipgloss.Color("#8AB4F8"),  // was #06B6D4
		ThinkingOpacity:   0.6,  // keep
		DiffAdded:         lipgloss.Color("#81C995"),  // keep
		DiffRemoved:       lipgloss.Color("#F28B82"),  // keep
		DiffAddedBg:       lipgloss.Color("#81C99520"),  // keep
		DiffRemovedBg:     lipgloss.Color("#F28B8220"),  // keep
		DiffContextBg:     lipgloss.Color("#2A2A3E"),  // keep
		BadgeForeground:   lipgloss.Color("#000000"),  // keep
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),  // keep
		BadgeTextDark:     lipgloss.Color("#000000"),  // keep

		DividerChar:  "─",
		HeaderHeight: 1,
		ShadowColor:  lipgloss.Color("#00000040"),
		CompactMode:  false,
		SelectionBg:  lipgloss.Color("#3C4043"),
		CardPadding:  1,
	}
	applyThemeStyles(&t)

	return t
}

func Light() Theme {
	t := Theme{
		Mode:              ModeLight,
		Background:        lipgloss.Color("#FFFFFF"),  // keep
		Surface:           lipgloss.Color("#F8F9FA"),  // keep
		SurfaceElevated:   lipgloss.Color("#E8EAED"),  // was #FFFFFF
		Border:            lipgloss.Color("#DADCE0"),  // was #E0E0E0
		Brand:             lipgloss.Color("#D77757"),  // was #7C3AED (same as dark — brand is brand)
		TextPrimary:       lipgloss.Color("#202124"),  // was #1E293B
		TextSecondary:     lipgloss.Color("#5F6368"),  // was #64748B
		Thinking:          lipgloss.Color("#1A73E8"),  // was #7C3AED
		Success:           lipgloss.Color("#137333"),  // was #059669
		Error:             lipgloss.Color("#C5221F"),  // was #DC2626
		Warning:           lipgloss.Color("#EA8600"),  // was #D97706
		CodeBG:            lipgloss.Color("#F1F3F4"),  // keep
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color("#F8F9FA"),  // keep
		BackgroundElement: lipgloss.Color("#FFFFFF"),  // keep
		Text:              lipgloss.Color("#202124"),  // was #1E293B
		TextMuted:         lipgloss.Color("#9AA0A6"),  // was #94A3B8
		BorderActive:      lipgloss.Color("#D77757"),  // was #C0C0C0
		BorderSubtle:      lipgloss.Color("#E8E8E8"),  // keep
		Primary:           lipgloss.Color("#D77757"),  // was #7C3AED
		Secondary:         lipgloss.Color("#1A73E8"),  // was #0891B2
		Accent:            lipgloss.Color("#1A73E8"),  // was #0891B2
		Info:              lipgloss.Color("#1A73E8"),  // was #0891B2
		ThinkingOpacity:   0.6,  // keep
		DiffAdded:         lipgloss.Color("#137333"),  // was #059669
		DiffRemoved:       lipgloss.Color("#C5221F"),  // was #DC2626
		DiffAddedBg:       lipgloss.Color("#13733320"),  // was #05966920
		DiffRemovedBg:     lipgloss.Color("#C5221F20"),  // was #DC262620
		DiffContextBg:     lipgloss.Color("#F8F9FA"),  // keep
		BadgeForeground:   lipgloss.Color("#000000"),  // keep
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),  // keep
		BadgeTextDark:     lipgloss.Color("#000000"),  // keep

		DividerChar:  "─",
		HeaderHeight: 1,
		ShadowColor:  lipgloss.Color("#00000040"),
		CompactMode:  false,
		SelectionBg:  lipgloss.Color("#DADCE0"),
		CardPadding:  1,
	}
	applyThemeStyles(&t)

	return t
}

// buildBadgeStyles sets all badge/label styles using the theme's BadgeForeground color.
func buildBadgeStyles(t *Theme) {
	fg := lipgloss.Color(t.BadgeForeground)
	t.ToolLabel["Bash"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Warning)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ToolLabel["FileRead"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Thinking)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ToolLabel["FileWrite"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Brand)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ToolLabel["Glob"] = lipgloss.NewStyle().Background(lipgloss.Color(t.TextSecondary)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ToolLabel["Grep"] = lipgloss.NewStyle().Background(lipgloss.Color(t.Thinking)).Foreground(fg).Padding(0, 1).Bold(true)
	t.SuccessBadge = lipgloss.NewStyle().Background(lipgloss.Color(t.Success)).Foreground(fg).Padding(0, 1).Bold(true)
	t.ErrorBadge = lipgloss.NewStyle().Background(lipgloss.Color(t.Error)).Foreground(fg).Padding(0, 1).Bold(true)
	t.WarningBadge = lipgloss.NewStyle().Background(lipgloss.Color(t.Warning)).Foreground(fg).Padding(0, 1).Bold(true)
}

// applyThemeStyles sets all computed styles that depend on base colors.
func applyThemeStyles(t *Theme) {
	t.Header = lipgloss.NewStyle().Background(lipgloss.Color(t.Brand)).Foreground(lipgloss.Color(t.BadgeTextLight)).Padding(0, 2).Bold(true)
	t.ModelBadge = lipgloss.NewStyle().Background(lipgloss.Color(t.Brand)).Foreground(lipgloss.Color(t.BadgeTextLight)).Padding(0, 1).Bold(true)
	t.ContextBar = lipgloss.NewStyle().Background(lipgloss.Color(t.Surface)).Foreground(lipgloss.Color(t.TextSecondary)).Padding(0, 1)
	t.StatusLive = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Success)).Bold(true)
	t.StatusSlow = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Warning)).Bold(true)
	t.StatusOffline = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Error)).Bold(true)
	t.UserBubble = lipgloss.NewStyle().Background(lipgloss.Color(t.SurfaceElevated)).Foreground(lipgloss.Color(t.TextPrimary)).Padding(0, 1).Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color(t.Border))
	t.AssistantBubble = lipgloss.NewStyle().Background(lipgloss.Color(t.Surface)).Foreground(lipgloss.Color(t.TextPrimary)).Padding(0, 1).Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color(t.Border))
	t.InputArea = lipgloss.NewStyle().Background(lipgloss.Color(t.Surface)).Foreground(lipgloss.Color(t.TextPrimary)).Padding(0, 1).Border(lipgloss.NormalBorder()).BorderForeground(lipgloss.Color(t.Border))
	t.Spinner = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Thinking))
	t.ThinkingBlock = lipgloss.NewStyle().Background(lipgloss.Color(t.CodeBG)).Foreground(lipgloss.Color(t.Thinking)).Padding(0, 1).Italic(true)
	t.ToolCard = lipgloss.NewStyle().Background(lipgloss.Color(t.SurfaceElevated)).Foreground(lipgloss.Color(t.TextPrimary)).Padding(0, 1).Border(lipgloss.RoundedBorder()).BorderForeground(lipgloss.Color(t.Border))
	buildBadgeStyles(t)
	t.ProgressBar = lipgloss.NewStyle().Background(lipgloss.Color(t.SurfaceElevated)).Foreground(lipgloss.Color(t.Brand))
	t.Modal = lipgloss.NewStyle().Background(lipgloss.Color(t.SurfaceElevated)).Foreground(lipgloss.Color(t.TextPrimary)).Padding(1, 2).Border(DoubleBorder).BorderForeground(lipgloss.Color(t.Brand))
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

// DoubleBorder is a reusable double-line border style for modals and panels.
var DoubleBorder = lipgloss.DoubleBorder()
