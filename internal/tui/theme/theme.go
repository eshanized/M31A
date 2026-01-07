package theme

import "github.com/charmbracelet/lipgloss"

type Mode int

const (
	ModeDark Mode = iota
	ModeLight
	ModeAuto
)

// Standard borders used across the TUI
var (
	NormalBorder = lipgloss.RoundedBorder() // ╭─╮ for cards, modals

	// ThinBorder is a lighter border for compact cards and tool cards
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

	// DoubleBorder is reserved for important modals only
	DoubleBorder = lipgloss.DoubleBorder()
)

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
	ThinkingOpacity   float64
	DiffAdded         lipgloss.Color
	DiffRemoved       lipgloss.Color
	DiffAddedBg       lipgloss.Color
	DiffRemovedBg     lipgloss.Color
	DiffContextBg     lipgloss.Color
	BadgeForeground   lipgloss.Color // foreground color for colored badges (black on both dark/light)
	BadgeTextLight    lipgloss.Color // white text for dark-themed badges
	BadgeTextDark     lipgloss.Color // black text for light-themed badges

	// DividerChar is the unicode character used for horizontal dividers
	DividerChar string

	// HeaderHeight is the number of rows the header occupies
	HeaderHeight int

	// ShadowColor is used for modal drop shadows
	ShadowColor lipgloss.Color

	// CompactMode enables reduced spacing for dense terminals
	CompactMode bool

	// SelectionBg is the background color for selected/highlighted items
	SelectionBg lipgloss.Color

	// CardPadding is the default padding for all card-style components
	CardPadding int

	// Card border styles for panel/card rendering
	CardBorder       lipgloss.Style // rounded card border, default brand color
	CardBorderActive lipgloss.Style // focused card border, brand + bold
	CardBorderError  lipgloss.Style // error card border, error color
	CardBorderWarn   lipgloss.Style // warning card border, warning color

	// Workflow phase styles
	PhaseActive lipgloss.Style // current workflow phase, brand + bold
	PhasePast   lipgloss.Style // completed workflow phases, muted
	PhaseFuture lipgloss.Style // upcoming workflow phases, muted

	// Timeline and metric styles
	TimelineDate lipgloss.Style // session timeline date headers
	MetricValue  lipgloss.Style // large metric numbers, brand foreground
	MetricLabel  lipgloss.Style // metric label below value, muted

	// Block character constants for progress bars, sparklines, and density indicators
	BlockFull string // "█" full block
	BlockHigh string // "▓" high density
	BlockMed  string // "▒" medium density
	BlockLow  string // "░" low density
}

type Manager struct {
	current    Theme
	mode       Mode
	profile    ColorProfile
	borderStyle string
	accentColor string
}

func NewManager(mode Mode) *Manager {
	m := &Manager{
		mode:        mode,
		profile:     DetectColorProfile(),
		borderStyle: "rounded",
		accentColor: "",
	}
	m.resolve()
	return m
}

// SetBorderStyle sets the border style for the theme manager
func (m *Manager) SetBorderStyle(style string) {
	m.borderStyle = style
	m.resolve()
}

// SetAccentColor sets the accent color override for the theme manager
func (m *Manager) SetAccentColor(hex string) {
	m.accentColor = hex
	m.resolve()
}

// CurrentBorder returns the current border style based on config
func (m *Manager) CurrentBorder() lipgloss.Border {
	return BorderByName(m.borderStyle)
}

func (m *Manager) resolve() {
	var base Theme
	switch m.mode {
	case ModeDark:
		base = Dark()
	case ModeLight:
		base = Light()
	case ModeAuto:
		base = Auto()
	default:
		base = Dark()
	}

	// Apply 16-color ANSI fallback if terminal doesn't support 256/truecolor
	if m.profile == Profile16 {
		// Copy base and override with ANSI colors
		base.Brand = lipgloss.Color("208")
		base.Success = lipgloss.Color("2")
		base.Error = lipgloss.Color("1")
		base.Warning = lipgloss.Color("3")
		base.Thinking = lipgloss.Color("4")
		applyThemeStyles(&base)
	}

	// Apply accent color override if set
	if m.accentColor != "" {
		base = base.WithAccent(m.accentColor)
	}

	m.current = base
}

func (m *Manager) Cycle() Mode {
	switch m.mode {
	case ModeDark:
		m.mode = ModeLight
	case ModeLight:
		m.mode = ModeAuto
	case ModeAuto:
		m.mode = ModeDark
	default:
		m.mode = ModeDark
	}
	m.resolve()
	return m.mode
}

func (m *Manager) Current() Theme {
	return m.current
}

func Default() Theme {
	return Dark()
}
