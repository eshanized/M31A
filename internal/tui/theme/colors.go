package theme

import "github.com/charmbracelet/lipgloss"

func Dark() Theme {
	t := Theme{
		Mode:              ModeDark,
		Background:        lipgloss.Color("#0F0F1A"),
		Surface:           lipgloss.Color("#1E1E2E"),
		SurfaceElevated:   lipgloss.Color("#2A2A3E"),
		Border:            lipgloss.Color("#2E2E2E"),
		Brand:             lipgloss.Color("#7C3AED"),
		TextPrimary:       lipgloss.Color("#E2E8F0"),
		TextSecondary:     lipgloss.Color("#94A3B8"),
		Thinking:          lipgloss.Color("#8B5CF6"),
		Success:           lipgloss.Color("#10B981"),
		Error:             lipgloss.Color("#EF4444"),
		Warning:           lipgloss.Color("#F59E0B"),
		CodeBG:            lipgloss.Color("#2D2D2D"),
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color("#1E1E2E"),
		BackgroundElement: lipgloss.Color("#2A2A3E"),
		Text:              lipgloss.Color("#E2E8F0"),
		TextMuted:         lipgloss.Color("#475569"),
		BorderActive:      lipgloss.Color("#484848"),
		BorderSubtle:      lipgloss.Color("#3C3C3C"),
		Primary:           lipgloss.Color("#7C3AED"),
		Secondary:         lipgloss.Color("#06B6D4"),
		Accent:            lipgloss.Color("#06B6D4"),
		Info:              lipgloss.Color("#06B6D4"),
		ThinkingOpacity:   0.6,
		DiffAdded:         lipgloss.Color("#81C995"),
		DiffRemoved:       lipgloss.Color("#F28B82"),
		DiffAddedBg:       lipgloss.Color("#81C99520"),
		DiffRemovedBg:     lipgloss.Color("#F28B8220"),
		DiffContextBg:     lipgloss.Color("#2A2A3E"),
		BadgeForeground:   lipgloss.Color("#000000"),
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),
		BadgeTextDark:     lipgloss.Color("#000000"),
	}
	applyThemeStyles(&t)

	return t
}

func Light() Theme {
	t := Theme{
		Mode:              ModeLight,
		Background:        lipgloss.Color("#FFFFFF"),
		Surface:           lipgloss.Color("#F8F9FA"),
		SurfaceElevated:   lipgloss.Color("#FFFFFF"),
		Border:            lipgloss.Color("#E0E0E0"),
		Brand:             lipgloss.Color("#7C3AED"),
		TextPrimary:       lipgloss.Color("#1E293B"),
		TextSecondary:     lipgloss.Color("#64748B"),
		Thinking:          lipgloss.Color("#7C3AED"),
		Success:           lipgloss.Color("#059669"),
		Error:             lipgloss.Color("#DC2626"),
		Warning:           lipgloss.Color("#D97706"),
		CodeBG:            lipgloss.Color("#F1F3F4"),
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color("#F8F9FA"),
		BackgroundElement: lipgloss.Color("#FFFFFF"),
		Text:              lipgloss.Color("#1E293B"),
		TextMuted:         lipgloss.Color("#94A3B8"),
		BorderActive:      lipgloss.Color("#C0C0C0"),
		BorderSubtle:      lipgloss.Color("#E8E8E8"),
		Primary:           lipgloss.Color("#7C3AED"),
		Secondary:         lipgloss.Color("#0891B2"),
		Accent:            lipgloss.Color("#0891B2"),
		Info:              lipgloss.Color("#0891B2"),
		ThinkingOpacity:   0.6,
		DiffAdded:         lipgloss.Color("#059669"),
		DiffRemoved:       lipgloss.Color("#DC2626"),
		DiffAddedBg:       lipgloss.Color("#05966920"),
		DiffRemovedBg:     lipgloss.Color("#DC262620"),
		DiffContextBg:     lipgloss.Color("#F8F9FA"),
		BadgeForeground:   lipgloss.Color("#000000"),
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),
		BadgeTextDark:     lipgloss.Color("#000000"),
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
