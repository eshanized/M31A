package theme

import "github.com/charmbracelet/lipgloss"

// StyleCache holds pre-computed lipgloss.Style values for a Theme.
// Build once per theme change; reuse on every render to avoid per-frame
// lipgloss.NewStyle() allocations.
type StyleCache struct {
	// Text styles
	Brand         lipgloss.Style
	BrandBold     lipgloss.Style
	TextPrimary   lipgloss.Style
	TextSecondary lipgloss.Style
	TextMuted     lipgloss.Style
	TextMutedFade lipgloss.Style

	// Semantic styles
	Success      lipgloss.Style
	SuccessBold  lipgloss.Style
	Error        lipgloss.Style
	ErrorBold    lipgloss.Style
	Warning      lipgloss.Style
	WarningBold  lipgloss.Style
	Info         lipgloss.Style
	Thinking     lipgloss.Style
	ThinkingFade lipgloss.Style

	// UI element styles
	Border       lipgloss.Style
	BorderSubtle lipgloss.Style
	BorderActive lipgloss.Style
	Surface      lipgloss.Style
	SurfaceElev  lipgloss.Style
	Selection    lipgloss.Style

	// Header/Footer styles
	HeaderBrand  lipgloss.Style
	HeaderCrumb  lipgloss.Style
	HeaderDots   lipgloss.Style
	FooterCwd    lipgloss.Style
	FooterBranch lipgloss.Style
	FooterOp     lipgloss.Style
	FooterHint   lipgloss.Style
	FooterLeader lipgloss.Style

	// Badge styles
	BadgeSuccess lipgloss.Style
	BadgeError   lipgloss.Style
	BadgeWarning lipgloss.Style
	BadgeBrand   lipgloss.Style
	BadgeInfo    lipgloss.Style

	// Code block
	CodeBG   lipgloss.Style
	LineNum  lipgloss.Style
	LineHigh lipgloss.Style

	// Message styles
	UserGutter  lipgloss.Style
	UserContent lipgloss.Style
	AsstGutter  lipgloss.Style
	AsstContent lipgloss.Style

	// Card styles
	CardBorder  lipgloss.Style
	CardBrand   lipgloss.Style
	CardSuccess lipgloss.Style
	CardError   lipgloss.Style
	CardWarning lipgloss.Style

	// Misc
	FocusRing    lipgloss.Style
	FocusRingOff lipgloss.Style

	// Typography (pre-computed from tokens)
	Heading       lipgloss.Style
	Subheading    lipgloss.Style
	Body          lipgloss.Style
	Caption       lipgloss.Style
	Faint         lipgloss.Style
	BrandText     lipgloss.Style
	MutedText     lipgloss.Style
	SecondaryText lipgloss.Style

	// Button-like styles
	ButtonPrimary   lipgloss.Style
	ButtonSecondary lipgloss.Style
	ButtonGhost     lipgloss.Style

	// Separator
	SeparatorH lipgloss.Style
	SeparatorV lipgloss.Style

	// Status
	StatusLive    lipgloss.Style
	StatusSlow    lipgloss.Style
	StatusOffline lipgloss.Style

	// Code
	CodeInline lipgloss.Style

	// Overlay
	DimOverlay lipgloss.Style

	// S is the complete semantic component style library.
	// Components should use S.* instead of creating styles inline.
	S SemanticStyles
}

// NewStyleCache builds a StyleCache from a Theme. Call this once per theme
// change and pass the cache to components.
func NewStyleCache(t Theme) *StyleCache {
	c := &StyleCache{}

	// Text
	c.Brand = lipgloss.NewStyle().Foreground(t.Brand)
	c.BrandBold = lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
	c.TextPrimary = lipgloss.NewStyle().Foreground(t.TextPrimary)
	c.TextSecondary = lipgloss.NewStyle().Foreground(t.TextSecondary)
	c.TextMuted = lipgloss.NewStyle().Foreground(t.TextMuted)
	c.TextMutedFade = lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true)

	// Semantic
	c.Success = lipgloss.NewStyle().Foreground(t.Success)
	c.SuccessBold = lipgloss.NewStyle().Foreground(t.Success).Bold(true)
	c.Error = lipgloss.NewStyle().Foreground(t.Error)
	c.ErrorBold = lipgloss.NewStyle().Foreground(t.Error).Bold(true)
	c.Warning = lipgloss.NewStyle().Foreground(t.Warning)
	c.WarningBold = lipgloss.NewStyle().Foreground(t.Warning).Bold(true)
	c.Info = lipgloss.NewStyle().Foreground(t.Info)
	c.Thinking = lipgloss.NewStyle().Foreground(t.Thinking).Italic(true)
	c.ThinkingFade = lipgloss.NewStyle().Foreground(t.Thinking).Italic(true).Faint(true)

	// UI elements
	c.Border = lipgloss.NewStyle().Foreground(t.Border)
	c.BorderSubtle = lipgloss.NewStyle().Foreground(t.BorderSubtle)
	c.BorderActive = lipgloss.NewStyle().Foreground(t.BorderActive)
	c.Surface = lipgloss.NewStyle().Background(t.Surface)
	c.SurfaceElev = lipgloss.NewStyle().Background(t.SurfaceElevated)
	c.Selection = lipgloss.NewStyle().Background(t.SelectionBg)

	// Header/Footer
	c.HeaderBrand = lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
	c.HeaderCrumb = lipgloss.NewStyle().Foreground(t.TextSecondary)
	c.HeaderDots = lipgloss.NewStyle().Foreground(t.BorderSubtle)
	c.FooterCwd = lipgloss.NewStyle().Foreground(t.TextMuted)
	c.FooterBranch = lipgloss.NewStyle().Foreground(t.TextSecondary)
	c.FooterOp = lipgloss.NewStyle().Foreground(t.TextMuted)
	c.FooterHint = lipgloss.NewStyle().Foreground(t.TextMuted)
	c.FooterLeader = lipgloss.NewStyle().Foreground(t.Brand).Bold(true)

	// Badges — subtle border, semantic color
	c.BadgeSuccess = lipgloss.NewStyle().Foreground(t.Success).Border(lipgloss.RoundedBorder()).BorderForeground(t.Success).Padding(0, 1)
	c.BadgeError = lipgloss.NewStyle().Foreground(t.Error).Border(lipgloss.RoundedBorder()).BorderForeground(t.Error).Padding(0, 1)
	c.BadgeWarning = lipgloss.NewStyle().Foreground(t.Warning).Border(lipgloss.RoundedBorder()).BorderForeground(t.Warning).Padding(0, 1)
	c.BadgeBrand = lipgloss.NewStyle().Foreground(t.Brand).Border(lipgloss.RoundedBorder()).BorderForeground(t.Brand).Padding(0, 1)
	c.BadgeInfo = lipgloss.NewStyle().Foreground(t.Info).Border(lipgloss.RoundedBorder()).BorderForeground(t.Info).Padding(0, 1)

	// Code
	c.CodeBG = lipgloss.NewStyle().Background(t.CodeBG)
	c.LineNum = lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true)
	c.LineHigh = lipgloss.NewStyle().BorderLeft(true).BorderForeground(t.Brand)

	// Messages
	c.UserGutter = lipgloss.NewStyle().Foreground(t.TextSecondary).Bold(true)
	c.UserContent = lipgloss.NewStyle().Foreground(t.TextPrimary)
	c.AsstGutter = lipgloss.NewStyle().Foreground(t.Brand).Bold(true)
	c.AsstContent = lipgloss.NewStyle().Foreground(t.TextPrimary)

	// Cards
	c.CardBorder = lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(t.Border)
	c.CardBrand = lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(t.Brand)
	c.CardSuccess = lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(t.Success)
	c.CardError = lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(t.Error)
	c.CardWarning = lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(t.Warning)

	// Misc
	c.FocusRing = lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(t.Brand)
	c.FocusRingOff = lipgloss.NewStyle().Border(lipgloss.RoundedBorder()).BorderForeground(t.BorderSubtle)

	// Typography — pre-computed from token palette
	c.Heading = lipgloss.NewStyle().Bold(true).Foreground(t.TextPrimary)
	c.Subheading = lipgloss.NewStyle().Bold(true).Foreground(t.TextSecondary)
	c.Body = lipgloss.NewStyle().Foreground(t.TextPrimary)
	c.Caption = lipgloss.NewStyle().Foreground(t.TextMuted)
	c.Faint = lipgloss.NewStyle().Foreground(t.TextMuted).Faint(true)
	c.BrandText = lipgloss.NewStyle().Foreground(t.Brand)
	c.MutedText = lipgloss.NewStyle().Foreground(t.TextMuted)
	c.SecondaryText = lipgloss.NewStyle().Foreground(t.TextSecondary)

	// Button-like styles
	c.ButtonPrimary = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Brand)).
		Foreground(lipgloss.Color(t.BadgeTextLight)).
		Padding(0, 2).
		Bold(true)
	c.ButtonSecondary = lipgloss.NewStyle().
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Brand)).
		Foreground(lipgloss.Color(t.Brand)).
		Padding(0, 1)
	c.ButtonGhost = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.TextMuted)).
		Padding(0, 1)

	// Separators
	c.SeparatorH = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Border))
	c.SeparatorV = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Border))

	// Status
	c.StatusLive = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Success)).Bold(true)
	c.StatusSlow = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Warning)).Bold(true)
	c.StatusOffline = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Error)).Bold(true)

	// Code
	c.CodeInline = lipgloss.NewStyle().Foreground(lipgloss.Color(t.Brand))

	// Overlay
	c.DimOverlay = lipgloss.NewStyle().Faint(true)

	// Semantic component library
	c.S = BuildSemanticStyles(t)

	return c
}
