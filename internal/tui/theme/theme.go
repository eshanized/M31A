package theme

import "github.com/charmbracelet/lipgloss"

type Mode int

const (
	ModeDark Mode = iota
	ModeLight
	ModeAuto
)

type Theme struct {
	Mode             Mode
	Background       lipgloss.Color
	Surface          lipgloss.Color
	SurfaceElevated  lipgloss.Color
	Border           lipgloss.Color
	Brand            lipgloss.Color
	TextPrimary      lipgloss.Color
	TextSecondary    lipgloss.Color
	Thinking         lipgloss.Color
	Success          lipgloss.Color
	Error            lipgloss.Color
	Warning          lipgloss.Color
	CodeBG           lipgloss.Color
	Header           lipgloss.Style
	ModelBadge       lipgloss.Style
	ContextBar       lipgloss.Style
	StatusLive       lipgloss.Style
	StatusSlow       lipgloss.Style
	StatusOffline    lipgloss.Style
	UserBubble       lipgloss.Style
	AssistantBubble  lipgloss.Style
	InputArea        lipgloss.Style
	Spinner          lipgloss.Style
	ThinkingBlock    lipgloss.Style
	ToolCard         lipgloss.Style
	ToolLabel        map[string]lipgloss.Style
	SuccessBadge     lipgloss.Style
	ErrorBadge       lipgloss.Style
	WarningBadge     lipgloss.Style
	ProgressBar      lipgloss.Style
	Modal            lipgloss.Style
	ModalTitle       lipgloss.Style
}

func Dark() Theme {
	t := Theme{
		Mode:            ModeDark,
		Background:      lipgloss.Color("#0D0D0D"),
		Surface:         lipgloss.Color("#1A1A1A"),
		SurfaceElevated: lipgloss.Color("#242424"),
		Border:          lipgloss.Color("#2E2E2E"),
		Brand:           lipgloss.Color("#D77757"),
		TextPrimary:     lipgloss.Color("#E8EAED"),
		TextSecondary:   lipgloss.Color("#9AA0A6"),
		Thinking:        lipgloss.Color("#8AB4F8"),
		Success:         lipgloss.Color("#81C995"),
		Error:           lipgloss.Color("#F28B82"),
		Warning:         lipgloss.Color("#FDD663"),
		CodeBG:          lipgloss.Color("#2D2D2D"),
		ToolLabel:       make(map[string]lipgloss.Style),
	}
	t.Header = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Brand)).
		Foreground(lipgloss.Color("#FFFFFF")).
		Padding(0, 2).
		Bold(true)
	t.ModelBadge = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Brand)).
		Foreground(lipgloss.Color("#FFFFFF")).
		Padding(0, 1).
		Bold(true)
	t.ContextBar = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Surface)).
		Foreground(lipgloss.Color(t.TextSecondary)).
		Padding(0, 1)
	t.StatusLive = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Success)).
		Bold(true)
	t.StatusSlow = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Warning)).
		Bold(true)
	t.StatusOffline = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Error)).
		Bold(true)
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
	t.InputArea = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Surface)).
		Foreground(lipgloss.Color(t.TextPrimary)).
		Padding(0, 1).
		Border(lipgloss.NormalBorder()).
		BorderForeground(lipgloss.Color(t.Border))
	t.Spinner = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Thinking))
	t.ThinkingBlock = lipgloss.NewStyle().
		Background(lipgloss.Color(t.CodeBG)).
		Foreground(lipgloss.Color(t.Thinking)).
		Padding(0, 1).
		Italic(true)
	t.ToolCard = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SurfaceElevated)).
		Foreground(lipgloss.Color(t.TextPrimary)).
		Padding(0, 1).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Border))
	t.ToolLabel["Bash"] = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Warning)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.ToolLabel["FileRead"] = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Thinking)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.ToolLabel["FileWrite"] = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Brand)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.ToolLabel["Glob"] = lipgloss.NewStyle().
		Background(lipgloss.Color(t.TextSecondary)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.ToolLabel["Grep"] = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Thinking)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.SuccessBadge = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Success)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.ErrorBadge = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Error)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.WarningBadge = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Warning)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.ProgressBar = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SurfaceElevated)).
		Foreground(lipgloss.Color(t.Brand))
	t.Modal = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SurfaceElevated)).
		Foreground(lipgloss.Color(t.TextPrimary)).
		Padding(1, 2).
		Border(lipgloss.DoubleBorder()).
		BorderForeground(lipgloss.Color(t.Brand))
	t.ModalTitle = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Brand)).
		Bold(true)
	return t
}

func Light() Theme {
	t := Theme{
		Mode:            ModeLight,
		Background:      lipgloss.Color("#FFFFFF"),
		Surface:         lipgloss.Color("#F8F9FA"),
		SurfaceElevated: lipgloss.Color("#FFFFFF"),
		Border:          lipgloss.Color("#E0E0E0"),
		Brand:           lipgloss.Color("#C45C3A"),
		TextPrimary:     lipgloss.Color("#1F1F1F"),
		TextSecondary:   lipgloss.Color("#5F6368"),
		Thinking:        lipgloss.Color("#1967D2"),
		Success:         lipgloss.Color("#1E8E3E"),
		Error:           lipgloss.Color("#D93025"),
		Warning:         lipgloss.Color("#F9AB00"),
		CodeBG:          lipgloss.Color("#F1F3F4"),
		ToolLabel:       make(map[string]lipgloss.Style),
	}
	t.Header = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Brand)).
		Foreground(lipgloss.Color("#FFFFFF")).
		Padding(0, 2).
		Bold(true)
	t.ModelBadge = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Brand)).
		Foreground(lipgloss.Color("#FFFFFF")).
		Padding(0, 1).
		Bold(true)
	t.ContextBar = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Surface)).
		Foreground(lipgloss.Color(t.TextSecondary)).
		Padding(0, 1)
	t.StatusLive = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Success)).
		Bold(true)
	t.StatusSlow = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Warning)).
		Bold(true)
	t.StatusOffline = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Error)).
		Bold(true)
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
	t.InputArea = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Surface)).
		Foreground(lipgloss.Color(t.TextPrimary)).
		Padding(0, 1).
		Border(lipgloss.NormalBorder()).
		BorderForeground(lipgloss.Color(t.Border))
	t.Spinner = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Thinking))
	t.ThinkingBlock = lipgloss.NewStyle().
		Background(lipgloss.Color(t.CodeBG)).
		Foreground(lipgloss.Color(t.Thinking)).
		Padding(0, 1).
		Italic(true)
	t.ToolCard = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SurfaceElevated)).
		Foreground(lipgloss.Color(t.TextPrimary)).
		Padding(0, 1).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color(t.Border))
	t.ToolLabel["Bash"] = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Warning)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.ToolLabel["FileRead"] = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Thinking)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.ToolLabel["FileWrite"] = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Brand)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.ToolLabel["Glob"] = lipgloss.NewStyle().
		Background(lipgloss.Color(t.TextSecondary)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.ToolLabel["Grep"] = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Thinking)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.SuccessBadge = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Success)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.ErrorBadge = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Error)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.WarningBadge = lipgloss.NewStyle().
		Background(lipgloss.Color(t.Warning)).
		Foreground(lipgloss.Color("#000000")).
		Padding(0, 1).
		Bold(true)
	t.ProgressBar = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SurfaceElevated)).
		Foreground(lipgloss.Color(t.Brand))
	t.Modal = lipgloss.NewStyle().
		Background(lipgloss.Color(t.SurfaceElevated)).
		Foreground(lipgloss.Color(t.TextPrimary)).
		Padding(1, 2).
		Border(lipgloss.DoubleBorder()).
		BorderForeground(lipgloss.Color(t.Brand))
	t.ModalTitle = lipgloss.NewStyle().
		Foreground(lipgloss.Color(t.Brand)).
		Bold(true)
	return t
}

func Auto() Theme {
	if lipgloss.HasDarkBackground() {
		return Dark()
	}
	return Light()
}

func Default() Theme {
	return Dark()
}

type Manager struct {
	current Theme
	mode    Mode
}

func NewManager(mode Mode) *Manager {
	m := &Manager{mode: mode}
	m.resolve()
	return m
}

func (m *Manager) resolve() {
	switch m.mode {
	case ModeDark:
		m.current = Dark()
	case ModeLight:
		m.current = Light()
	case ModeAuto:
		m.current = Auto()
	default:
		m.current = Dark()
	}
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
