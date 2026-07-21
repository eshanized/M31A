package theme

import "github.com/charmbracelet/lipgloss"

// Mode represents the theme mode.
// M31A ships with a single dark theme.
type Mode int

const (
	ModeDark Mode = iota // the only supported mode
)

// Standard borders used across the TUI
var (
	// NormalBorder is the default rounded border for cards and modals.
	NormalBorder = lipgloss.RoundedBorder()

	// ThinBorder is a lighter border for compact cards and tool cards.
	ThinBorder = lipgloss.Border{
		Top:         "─",
		Bottom:      "─",
		Left:        "│",
		Right:       "│",
		TopLeft:     "┌",
		TopRight:    "┐",
		BottomLeft:  "└",
		BottomRight: "┘",
	}

	// DoubleBorder is reserved for important modals only.
	DoubleBorder = lipgloss.DoubleBorder()
)

// SplitBorder is a vertical separator border.
var SplitBorder = lipgloss.Border{
	Top:         "",
	Bottom:      "",
	Left:        "\u2503",
	Right:       "",
	TopLeft:     "",
	TopRight:    "",
	BottomLeft:  "",
	BottomRight: "",
}

// Theme holds all visual tokens for the M31A interface.
type Theme struct {
	Mode              Mode
	Background        lipgloss.Color
	Surface           lipgloss.Color
	SurfaceElevated   lipgloss.Color
	Border            lipgloss.Color
	Brand             lipgloss.Color
	TextPrimary       lipgloss.Color
	TextSecondary     lipgloss.Color
	Thinking          lipgloss.Color
	Success           lipgloss.Color
	Error             lipgloss.Color
	Warning           lipgloss.Color
	CodeBG            lipgloss.Color
	Header            lipgloss.Style
	ModelBadge        lipgloss.Style
	ContextBar        lipgloss.Style
	StatusLive        lipgloss.Style
	StatusSlow        lipgloss.Style
	StatusOffline     lipgloss.Style
	UserBubble        lipgloss.Style
	AssistantBubble   lipgloss.Style
	InputArea         lipgloss.Style
	Spinner           lipgloss.Style
	ThinkingBlock     lipgloss.Style
	ToolCard          lipgloss.Style
	ToolLabel         map[string]lipgloss.Style
	SuccessBadge      lipgloss.Style
	ErrorBadge        lipgloss.Style
	WarningBadge      lipgloss.Style
	ProgressBar       lipgloss.Style
	Modal             lipgloss.Style
	ModalTitle        lipgloss.Style
	BackgroundPanel   lipgloss.Color
	BackgroundElement lipgloss.Color
	Text              lipgloss.Color
	TextMuted         lipgloss.Color
	BorderActive      lipgloss.Color
	BorderSubtle      lipgloss.Color
	Primary           lipgloss.Color
	Secondary         lipgloss.Color
	Accent            lipgloss.Color
	Info              lipgloss.Color
	DiffAdded         lipgloss.Color
	DiffRemoved       lipgloss.Color
	DiffAddedBg       lipgloss.Color
	DiffRemovedBg     lipgloss.Color
	DiffContextBg     lipgloss.Color
	BadgeForeground   lipgloss.Color
	BadgeTextLight    lipgloss.Color
	BadgeTextDark     lipgloss.Color

	// DividerChar is the unicode character used for horizontal dividers.
	DividerChar string

	// HeaderHeight is the number of rows the header occupies.
	HeaderHeight int

	// ShadowColor is used for modal drop shadows.
	ShadowColor lipgloss.Color

	// SelectionBg is the background color for selected/highlighted items.
	SelectionBg lipgloss.Color

	// Card border styles for panel/card rendering.
	CardBorder       lipgloss.Style
	CardBorderActive lipgloss.Style
	CardBorderError  lipgloss.Style
	CardBorderWarn   lipgloss.Style

	// Workflow phase styles
	PhaseActive lipgloss.Style
	PhasePast   lipgloss.Style
	PhaseFuture lipgloss.Style

	// Timeline and metric styles
	TimelineDate lipgloss.Style
	MetricValue  lipgloss.Style
	MetricLabel  lipgloss.Style

	// Block character constants for progress bars, sparklines, and density indicators
	BlockFull string
	BlockHigh string
	BlockMed  string
	BlockLow  string
}

// Manager manages the active theme.
type Manager struct {
	current     Theme
	mode        Mode
	profile     ColorProfile
	borderStyle string
	accentColor string
	cache       *StyleCache // lazily built, invalidated on theme change
}

// NewManager creates a new theme manager.
// M31A ships with a single dark theme. Mode is accepted for backward
// compatibility but always produces the M31A dark theme.
func NewManager(mode Mode) *Manager {
	m := &Manager{
		mode:        ModeDark,
		profile:     DetectColorProfile(),
		borderStyle: "rounded",
		accentColor: "",
	}
	m.resolve()
	return m
}

// SetBorderStyle sets the border style for the theme manager.
func (m *Manager) SetBorderStyle(style string) {
	m.borderStyle = style
	m.resolve()
}

// SetAccentColor sets the accent color override for the theme manager.
func (m *Manager) SetAccentColor(hex string) {
	m.accentColor = hex
	m.resolve()
}

// CurrentBorder returns the current border style based on config.
func (m *Manager) CurrentBorder() lipgloss.Border {
	return BorderByName(m.borderStyle)
}

func (m *Manager) resolve() {
	// M31A ships with a single dark theme
	base := M31A()

	// Apply 16-color ANSI fallback if terminal doesn't support 256/truecolor
	if m.profile == Profile16 {
		base = ansiPalette()
		applyThemeStyles(&base)
	}

	// Apply accent color override if set
	if m.accentColor != "" {
		base = base.WithAccent(m.accentColor)
	}

	m.current = base
	m.invalidateCache()
}

// Current returns the active theme.
func (m *Manager) Current() Theme {
	return m.current
}

// Default returns the default M31A theme.
func Default() Theme {
	return M31A()
}
