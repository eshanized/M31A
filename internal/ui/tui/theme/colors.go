package theme

import (
	"os"
	"strings"

	"github.com/charmbracelet/lipgloss"
)

// ColorProfile represents terminal color capability.
type ColorProfile int

const (
	ProfileNone ColorProfile = -1 // NO_COLOR mode — no color output

	ProfileTrueColor ColorProfile = iota
	Profile256
	Profile16
)

// DetectColorProfile determines terminal color capability from environment variables.
func DetectColorProfile() ColorProfile {
	// Respect NO_COLOR standard (https://no-color.org/)
	if os.Getenv("NO_COLOR") != "" {
		return ProfileNone
	}

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

// ansiPalette returns a Theme that uses ANSI 16-color names
// for reliable rendering on terminals without 256-color or truecolor support.
func ansiPalette() Theme {
	t := M31A()
	// Backgrounds: map dark hex backgrounds to ANSI dark colors
	t.Background = lipgloss.Color("0")      // black
	t.Surface = lipgloss.Color("0")         // black (dark base)
	t.SurfaceElevated = lipgloss.Color("8") // bright black (slightly lighter)
	t.BackgroundPanel = lipgloss.Color("0")
	t.BackgroundElement = lipgloss.Color("8")
	// Text: map to ANSI white/bright white hierarchy
	t.TextPrimary = lipgloss.Color("15")  // bright white
	t.TextSecondary = lipgloss.Color("7") // white (gray)
	t.TextMuted = lipgloss.Color("8")     // bright black (dark gray)
	t.Text = lipgloss.Color("15")
	// Borders
	t.Border = lipgloss.Color("8")        // bright black
	t.BorderActive = lipgloss.Color("12") // bright blue
	t.BorderSubtle = lipgloss.Color("0")  // black (invisible)
	// Accent colors
	t.Brand = lipgloss.Color("12") // bright blue (professional)
	t.Primary = lipgloss.Color("12")
	t.Accent = lipgloss.Color("13") // bright magenta
	t.Info = lipgloss.Color("12")
	// Semantic colors
	t.Success = lipgloss.Color("10")  // bright green
	t.Error = lipgloss.Color("9")     // bright red
	t.Warning = lipgloss.Color("11")  // bright yellow
	t.Thinking = lipgloss.Color("12") // bright blue
	t.Secondary = lipgloss.Color("13")
	// Badge foreground
	t.BadgeForeground = lipgloss.Color("15") // bright white
	t.BadgeTextLight = lipgloss.Color("15")
	t.BadgeTextDark = lipgloss.Color("0")
	// Selection and shadow
	t.SelectionBg = lipgloss.Color("12") // bright blue
	t.ShadowColor = lipgloss.Color("0")
	// Code
	t.CodeBG = lipgloss.Color("0")
	// Diff colors
	t.DiffAdded = lipgloss.Color("10")
	t.DiffRemoved = lipgloss.Color("9")
	t.DiffAddedBg = lipgloss.Color("10")
	t.DiffRemovedBg = lipgloss.Color("9")
	t.DiffContextBg = lipgloss.Color("0")
	return t
}

// M31A returns the single official M31A theme.
// Everblush theme.
func M31A() Theme {
	t := Theme{
		Mode:              ModeDark,
		Background:        lipgloss.Color(BgBase),
		Surface:           lipgloss.Color(BgSurface),
		SurfaceElevated:   lipgloss.Color(BgSurfaceHigh),
		Border:            lipgloss.Color(BorderDefault),
		Brand:             lipgloss.Color(AccentPrimary),
		TextPrimary:       lipgloss.Color(TextPrimary),
		TextSecondary:     lipgloss.Color(TextSecondary),
		Thinking:          lipgloss.Color(Thinking),
		Success:           lipgloss.Color(Success),
		Error:             lipgloss.Color(Error),
		Warning:           lipgloss.Color(Warning),
		CodeBG:            lipgloss.Color(CodeBg),
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color(BgSurface),
		BackgroundElement: lipgloss.Color(BgSurfaceHigh),
		Text:              lipgloss.Color(TextPrimary),
		TextMuted:         lipgloss.Color(TextMuted),
		BorderActive:      lipgloss.Color(AccentPrimary),
		BorderSubtle:      lipgloss.Color(BorderSubtle),
		Primary:           lipgloss.Color(AccentPrimary),
		Secondary:         lipgloss.Color(Thinking),
		Accent:            lipgloss.Color(Thinking),
		Info:              lipgloss.Color(Thinking),
		DiffAdded:         lipgloss.Color(DiffAdded),
		DiffRemoved:       lipgloss.Color(DiffRemoved),
		DiffAddedBg:       lipgloss.Color(DiffAddedBg),
		DiffRemovedBg:     lipgloss.Color(DiffRemovedBg),
		DiffContextBg:     lipgloss.Color(DiffContextBg),
		BadgeForeground:   lipgloss.Color("#FFFFFF"),
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),
		BadgeTextDark:     lipgloss.Color("#000000"),

		DividerChar:  "─",
		HeaderHeight: 1,
		ShadowColor:  lipgloss.Color(Shadow),
		SelectionBg:  lipgloss.Color(Selection),
	}
	applyThemeStyles(&t)
	return t
}

// Dark returns the M31A theme (backward compatibility alias).
func Dark() Theme {
	return M31A()
}

// Light is deprecated. M31A ships with a single dark theme.
// Returns M31A() for backward compatibility.
func Light() Theme {
	return M31A()
}

// buildBadgeStyles sets all badge/label styles using the theme's BadgeForeground color.
func buildBadgeStyles(t *Theme) {
	fg := lipgloss.Color(t.BadgeForeground)
	// Execution tools — calm, muted backgrounds
	t.ToolLabel["Bash"] = lipgloss.NewStyle().Background(lipgloss.Color(Warning)).Foreground(fg).Padding(0, 1)
	t.ToolLabel["Agent"] = lipgloss.NewStyle().Background(lipgloss.Color(Autonomous)).Foreground(fg).Padding(0, 1)
	// File tools — distinct but not loud
	t.ToolLabel["FileRead"] = lipgloss.NewStyle().Background(lipgloss.Color(Thinking)).Foreground(fg).Padding(0, 1)
	t.ToolLabel["FileWrite"] = lipgloss.NewStyle().Background(lipgloss.Color(AccentPrimary)).Foreground(fg).Padding(0, 1)
	t.ToolLabel["FileDelete"] = lipgloss.NewStyle().Background(lipgloss.Color(Error)).Foreground(fg).Padding(0, 1)
	t.ToolLabel["FileMove"] = lipgloss.NewStyle().Background(lipgloss.Color(Warning)).Foreground(fg).Padding(0, 1)
	t.ToolLabel["FileList"] = lipgloss.NewStyle().Background(lipgloss.Color(TextSecondary)).Foreground(fg).Padding(0, 1)
	// Code analysis tools
	t.ToolLabel["CodeMap"] = lipgloss.NewStyle().Background(lipgloss.Color(Success)).Foreground(fg).Padding(0, 1)
	t.ToolLabel["Edit"] = lipgloss.NewStyle().Background(lipgloss.Color(AccentPrimary)).Foreground(fg).Padding(0, 1)
	t.ToolLabel["Glob"] = lipgloss.NewStyle().Background(lipgloss.Color(TextSecondary)).Foreground(fg).Padding(0, 1)
	t.ToolLabel["Grep"] = lipgloss.NewStyle().Background(lipgloss.Color(Thinking)).Foreground(fg).Padding(0, 1)
	// Web tools
	t.ToolLabel["WebFetch"] = lipgloss.NewStyle().Background(lipgloss.Color(Thinking)).Foreground(fg).Padding(0, 1)
	t.ToolLabel["WebSearch"] = lipgloss.NewStyle().Background(lipgloss.Color(TextSecondary)).Foreground(fg).Padding(0, 1)
	// User interaction tools
	t.ToolLabel["AskUserQuestion"] = lipgloss.NewStyle().Background(lipgloss.Color(AccentPrimary)).Foreground(fg).Padding(0, 1)
	t.ToolLabel["TodoWrite"] = lipgloss.NewStyle().Background(lipgloss.Color(Success)).Foreground(fg).Padding(0, 1)
	// Badge styles
	t.SuccessBadge = lipgloss.NewStyle().Background(lipgloss.Color(Success)).Foreground(fg).Padding(0, 1)
	t.ErrorBadge = lipgloss.NewStyle().Background(lipgloss.Color(Error)).Foreground(fg).Padding(0, 1)
	t.WarningBadge = lipgloss.NewStyle().Background(lipgloss.Color(Warning)).Foreground(fg).Padding(0, 1)
}

// applyThemeStyles sets all computed styles that depend on base colors.
func applyThemeStyles(t *Theme) {
	// Header — bold brand
	t.Header = lipgloss.NewStyle().Bold(true)

	// Model badge — filled accent background
	t.ModelBadge = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Brand)).
		Foreground(lipgloss.Color(t.BadgeTextLight)).
		Padding(0, 1).
		Bold(true)

	// Context bar — subtle surface background
	t.ContextBar = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Surface)).
		Foreground(lipgloss.Color(t.TextSecondary)).
		Padding(0, 1)

	// Status indicators — calm, semantic
	t.StatusLive = lipgloss.NewStyle().Foreground(lipgloss.Color(Success)).Bold(true)
	t.StatusSlow = lipgloss.NewStyle().Foreground(lipgloss.Color(Warning)).Bold(true)
	t.StatusOffline = lipgloss.NewStyle().Foreground(lipgloss.Color(Error)).Bold(true)

	// Message bubbles — subtle elevation
	t.UserBubble = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SurfaceElevated)).
		Foreground(lipgloss.Color(t.TextPrimary)).
		Padding(0, 1).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Border))
	t.AssistantBubble = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Surface)).
		Foreground(lipgloss.Color(t.TextPrimary)).
		Padding(0, 1).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Border))

	// Input area — surface background
	t.InputArea = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Surface)).
		Foreground(lipgloss.Color(t.TextPrimary)).
		Padding(0, 1)

	// Spinner — thinking color
	t.Spinner = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Thinking))

	// Thinking block — code background, thinking color
	t.ThinkingBlock = lipgloss.NewStyle().
		Background(lipgloss.Color(t.CodeBG)).
		Foreground(lipgloss.Color(t.Thinking)).
		Padding(0, 1).
		Italic(true)

	// Tool card — elevated surface, thin border
	t.ToolCard = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SurfaceElevated)).
		Foreground(lipgloss.Color(t.TextPrimary)).
		Padding(0, 1).
		Border(ThinBorder).
		BorderForeground(lipgloss.Color(t.Border))

	// Badge styles
	buildBadgeStyles(t)

	// Progress bar — accent foreground on surface
	t.ProgressBar = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SurfaceElevated)).
		Foreground(lipgloss.Color(t.Brand))

	// Modal — elevated surface, accent border
	t.Modal = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SurfaceElevated)).
		Foreground(lipgloss.Color(t.TextPrimary)).
		Padding(1, 2).
		Border(NormalBorder).
		BorderForeground(lipgloss.Color(t.Brand))
	t.ModalTitle = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Brand)).
		Bold(true)

	// Card border styles — rounded, semantic colors
	t.CardBorder = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Border))
	t.CardBorderActive = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Brand))
	t.CardBorderError = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Error))
	t.CardBorderWarn = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Warning))

	// Workflow phase styles — subtle hierarchy
	t.PhaseActive = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Brand)).Bold(true)
	t.PhasePast = lipgloss.NewStyle().Foreground(lipgloss.Color(t.TextMuted))
	t.PhaseFuture = lipgloss.NewStyle().Foreground(lipgloss.Color(t.TextMuted))

	// Timeline and metric styles
	t.TimelineDate = lipgloss.NewStyle().Foreground(lipgloss.Color(t.TextMuted)).Bold(true)
	t.MetricValue = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Brand)).Bold(true)
	t.MetricLabel = lipgloss.NewStyle().Foreground(lipgloss.Color(t.TextMuted))

	// Block character constants for progress bars
	t.BlockFull = "█"
	t.BlockHigh = "▓"
	t.BlockMed = "▒"
	t.BlockLow = "░"
}

// WithAccent returns a new Theme with the accent color overridden.
func (t *Theme) WithAccent(hex string) Theme {
	newTheme := *t
	newTheme.Brand = lipgloss.Color(hex)
	newTheme.Primary = lipgloss.Color(hex)
	newTheme.BorderActive = lipgloss.Color(hex)
	applyThemeStyles(&newTheme)
	return newTheme
}
