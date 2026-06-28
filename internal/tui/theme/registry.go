package theme

import "github.com/charmbracelet/lipgloss"

// ThemeDefinition represents a theme preset
type ThemeDefinition struct {
	ID          string
	Name        string
	Description string
	Mode        Mode
	Constructor func() Theme
}

var registry = []ThemeDefinition{
	{"dark", "Midnight", "Default dark theme", ModeDark, Dark},
	{"light", "Daylight", "Default light theme", ModeLight, Light},
	{"catppuccin", "Catppuccin Mocha", "Warm purple-blue", ModeDark, Catppuccin},
	{"nord", "Nord Frost", "Cool blue-gray", ModeDark, Nord},
	{"tokyo", "Tokyo Night", "Deep indigo", ModeDark, TokyoNight},
	{"gruvbox", "Gruvbox Dark", "Warm retro brown", ModeDark, GruvboxDark},
	{"rose", "Rosé Pine", "Soft dusty pink", ModeDark, RosePine},
	{"dracula", "Dracula", "High contrast purple", ModeDark, Dracula},
	{"solarized", "Solarized Dark", "Teal/olive", ModeDark, SolarizedDark},
	{"monochrome", "Pure Mono", "Zero saturation", ModeDark, Monochrome},
	{"high-contrast", "High Contrast", "Maximum accessibility", ModeLight, HighContrast},
}

// Available returns all available theme definitions
func Available() []ThemeDefinition {
	return registry
}

// ByID returns a theme by its ID
func ByID(id string) (Theme, bool) {
	for _, def := range registry {
		if def.ID == id {
			return def.Constructor(), true
		}
	}
	return Theme{}, false
}

// Catppuccin returns the Catppuccin Mocha theme
func Catppuccin() Theme {
	t := Theme{
		Mode:              ModeDark,
		Background:        lipgloss.Color("#1e1e2e"),
		Surface:           lipgloss.Color("#313244"),
		SurfaceElevated:   lipgloss.Color("#45475a"),
		Border:            lipgloss.Color("#585b70"),
		Brand:             lipgloss.Color("#cba6f7"),
		TextPrimary:       lipgloss.Color("#cdd6f4"),
		TextSecondary:     lipgloss.Color("#a6adc8"),
		Thinking:          lipgloss.Color("#89b4fa"),
		Success:           lipgloss.Color("#a6e3a1"),
		Error:             lipgloss.Color("#f38ba8"),
		Warning:           lipgloss.Color("#f9e2af"),
		CodeBG:            lipgloss.Color("#313244"),
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color("#313244"),
		BackgroundElement: lipgloss.Color("#45475a"),
		Text:              lipgloss.Color("#cdd6f4"),
		TextMuted:         lipgloss.Color("#a6adc8"),
		BorderActive:      lipgloss.Color("#cba6f7"),
		BorderSubtle:      lipgloss.Color("#585b70"),
		Primary:           lipgloss.Color("#cba6f7"),
		Secondary:         lipgloss.Color("#89b4fa"),
		Accent:            lipgloss.Color("#89b4fa"),
		Info:              lipgloss.Color("#89b4fa"),
		ThinkingOpacity:   0.6,
		DiffAdded:         lipgloss.Color("#a6e3a1"),
		DiffRemoved:       lipgloss.Color("#f38ba8"),
		DiffAddedBg:       lipgloss.Color("#a6e3a120"),
		DiffRemovedBg:     lipgloss.Color("#f38ba820"),
		DiffContextBg:     lipgloss.Color("#313244"),
		BadgeForeground:   lipgloss.Color("#000000"),
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),
		BadgeTextDark:     lipgloss.Color("#000000"),
		DividerChar:       "─",
		HeaderHeight:      1,
		ShadowColor:       lipgloss.Color("#00000040"),
		CompactMode:       false,
		SelectionBg:       lipgloss.Color("#585b70"),
		CardPadding:       1,
	}
	applyThemeStyles(&t)
	return t
}

// Nord returns the Nord Frost theme
func Nord() Theme {
	t := Theme{
		Mode:              ModeDark,
		Background:        lipgloss.Color("#2e3440"),
		Surface:           lipgloss.Color("#3b4252"),
		SurfaceElevated:   lipgloss.Color("#434c5e"),
		Border:            lipgloss.Color("#4c566a"),
		Brand:             lipgloss.Color("#88c0d0"),
		TextPrimary:       lipgloss.Color("#d8dee9"),
		TextSecondary:     lipgloss.Color("#e5e9f0"),
		Thinking:          lipgloss.Color("#81a1c1"),
		Success:           lipgloss.Color("#b4d4a0"),
		Error:             lipgloss.Color("#d08770"),
		Warning:           lipgloss.Color("#ebcb8b"),
		CodeBG:            lipgloss.Color("#3b4252"),
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color("#3b4252"),
		BackgroundElement: lipgloss.Color("#434c5e"),
		Text:              lipgloss.Color("#d8dee9"),
		TextMuted:         lipgloss.Color("#e5e9f0"),
		BorderActive:      lipgloss.Color("#88c0d0"),
		BorderSubtle:      lipgloss.Color("#4c566a"),
		Primary:           lipgloss.Color("#88c0d0"),
		Secondary:         lipgloss.Color("#81a1c1"),
		Accent:            lipgloss.Color("#81a1c1"),
		Info:              lipgloss.Color("#81a1c1"),
		ThinkingOpacity:   0.6,
		DiffAdded:         lipgloss.Color("#a3be8c"),
		DiffRemoved:       lipgloss.Color("#bf616a"),
		DiffAddedBg:       lipgloss.Color("#a3be8c20"),
		DiffRemovedBg:     lipgloss.Color("#bf616a20"),
		DiffContextBg:     lipgloss.Color("#3b4252"),
		BadgeForeground:   lipgloss.Color("#000000"),
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),
		BadgeTextDark:     lipgloss.Color("#000000"),
		DividerChar:       "─",
		HeaderHeight:      1,
		ShadowColor:       lipgloss.Color("#00000040"),
		CompactMode:       false,
		SelectionBg:       lipgloss.Color("#4c566a"),
		CardPadding:       1,
	}
	applyThemeStyles(&t)
	return t
}

// TokyoNight returns the Tokyo Night theme
func TokyoNight() Theme {
	t := Theme{
		Mode:              ModeDark,
		Background:        lipgloss.Color("#1a1b26"),
		Surface:           lipgloss.Color("#24283b"),
		SurfaceElevated:   lipgloss.Color("#2f3349"),
		Border:            lipgloss.Color("#3b4261"),
		Brand:             lipgloss.Color("#7aa2f7"),
		TextPrimary:       lipgloss.Color("#c0caf5"),
		TextSecondary:     lipgloss.Color("#9aa5ce"),
		Thinking:          lipgloss.Color("#bb9af7"),
		Success:           lipgloss.Color("#9ece6a"),
		Error:             lipgloss.Color("#f7768e"),
		Warning:           lipgloss.Color("#e0af68"),
		CodeBG:            lipgloss.Color("#24283b"),
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color("#24283b"),
		BackgroundElement: lipgloss.Color("#2f3349"),
		Text:              lipgloss.Color("#c0caf5"),
		TextMuted:         lipgloss.Color("#9aa5ce"),
		BorderActive:      lipgloss.Color("#7aa2f7"),
		BorderSubtle:      lipgloss.Color("#3b4261"),
		Primary:           lipgloss.Color("#7aa2f7"),
		Secondary:         lipgloss.Color("#bb9af7"),
		Accent:            lipgloss.Color("#7dcfff"),
		Info:              lipgloss.Color("#7dcfff"),
		ThinkingOpacity:   0.6,
		DiffAdded:         lipgloss.Color("#9ece6a"),
		DiffRemoved:       lipgloss.Color("#f7768e"),
		DiffAddedBg:       lipgloss.Color("#9ece6a20"),
		DiffRemovedBg:     lipgloss.Color("#f7768e20"),
		DiffContextBg:     lipgloss.Color("#24283b"),
		BadgeForeground:   lipgloss.Color("#000000"),
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),
		BadgeTextDark:     lipgloss.Color("#000000"),
		DividerChar:       "─",
		HeaderHeight:      1,
		ShadowColor:       lipgloss.Color("#00000040"),
		CompactMode:       false,
		SelectionBg:       lipgloss.Color("#3b4261"),
		CardPadding:       1,
	}
	applyThemeStyles(&t)
	return t
}

// GruvboxDark returns the Gruvbox Dark theme
func GruvboxDark() Theme {
	t := Theme{
		Mode:              ModeDark,
		Background:        lipgloss.Color("#282828"),
		Surface:           lipgloss.Color("#3c3836"),
		SurfaceElevated:   lipgloss.Color("#504945"),
		Border:            lipgloss.Color("#665c54"),
		Brand:             lipgloss.Color("#d3869b"),
		TextPrimary:       lipgloss.Color("#ebdbb2"),
		TextSecondary:     lipgloss.Color("#bdae93"),
		Thinking:          lipgloss.Color("#83a598"),
		Success:           lipgloss.Color("#b8bb26"),
		Error:             lipgloss.Color("#fb4934"),
		Warning:           lipgloss.Color("#fabd2f"),
		CodeBG:            lipgloss.Color("#3c3836"),
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color("#3c3836"),
		BackgroundElement: lipgloss.Color("#504945"),
		Text:              lipgloss.Color("#ebdbb2"),
		TextMuted:         lipgloss.Color("#bdae93"),
		BorderActive:      lipgloss.Color("#d3869b"),
		BorderSubtle:      lipgloss.Color("#665c54"),
		Primary:           lipgloss.Color("#d3869b"),
		Secondary:         lipgloss.Color("#83a598"),
		Accent:            lipgloss.Color("#8ec07c"),
		Info:              lipgloss.Color("#8ec07c"),
		ThinkingOpacity:   0.6,
		DiffAdded:         lipgloss.Color("#b8bb26"),
		DiffRemoved:       lipgloss.Color("#fb4934"),
		DiffAddedBg:       lipgloss.Color("#b8bb2620"),
		DiffRemovedBg:     lipgloss.Color("#fb493420"),
		DiffContextBg:     lipgloss.Color("#3c3836"),
		BadgeForeground:   lipgloss.Color("#000000"),
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),
		BadgeTextDark:     lipgloss.Color("#000000"),
		DividerChar:       "─",
		HeaderHeight:      1,
		ShadowColor:       lipgloss.Color("#00000040"),
		CompactMode:       false,
		SelectionBg:       lipgloss.Color("#665c54"),
		CardPadding:       1,
	}
	applyThemeStyles(&t)
	return t
}

// RosePine returns the Rosé Pine theme
func RosePine() Theme {
	t := Theme{
		Mode:              ModeDark,
		Background:        lipgloss.Color("#191724"),
		Surface:           lipgloss.Color("#1f1d2e"),
		SurfaceElevated:   lipgloss.Color("#26233a"),
		Border:            lipgloss.Color("#403d52"),
		Brand:             lipgloss.Color("#c4a7e7"),
		TextPrimary:       lipgloss.Color("#e0def4"),
		TextSecondary:     lipgloss.Color("#908caa"),
		Thinking:          lipgloss.Color("#9ccfd8"),
		Success:           lipgloss.Color("#31748f"),
		Error:             lipgloss.Color("#eb6f92"),
		Warning:           lipgloss.Color("#f6c177"),
		CodeBG:            lipgloss.Color("#1f1d2e"),
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color("#1f1d2e"),
		BackgroundElement: lipgloss.Color("#26233a"),
		Text:              lipgloss.Color("#e0def4"),
		TextMuted:         lipgloss.Color("#908caa"),
		BorderActive:      lipgloss.Color("#c4a7e7"),
		BorderSubtle:      lipgloss.Color("#403d52"),
		Primary:           lipgloss.Color("#c4a7e7"),
		Secondary:         lipgloss.Color("#9ccfd8"),
		Accent:            lipgloss.Color("#ebbcba"),
		Info:              lipgloss.Color("#9ccfd8"),
		ThinkingOpacity:   0.6,
		DiffAdded:         lipgloss.Color("#31748f"),
		DiffRemoved:       lipgloss.Color("#eb6f92"),
		DiffAddedBg:       lipgloss.Color("#31748f20"),
		DiffRemovedBg:     lipgloss.Color("#eb6f9220"),
		DiffContextBg:     lipgloss.Color("#1f1d2e"),
		BadgeForeground:   lipgloss.Color("#000000"),
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),
		BadgeTextDark:     lipgloss.Color("#000000"),
		DividerChar:       "─",
		HeaderHeight:      1,
		ShadowColor:       lipgloss.Color("#00000040"),
		CompactMode:       false,
		SelectionBg:       lipgloss.Color("#403d52"),
		CardPadding:       1,
	}
	applyThemeStyles(&t)
	return t
}

// Dracula returns the Dracula theme
func Dracula() Theme {
	t := Theme{
		Mode:              ModeDark,
		Background:        lipgloss.Color("#282a36"),
		Surface:           lipgloss.Color("#44475a"),
		SurfaceElevated:   lipgloss.Color("#565f89"),
		Border:            lipgloss.Color("#6272a4"),
		Brand:             lipgloss.Color("#ff79c6"),
		TextPrimary:       lipgloss.Color("#f8f8f2"),
		TextSecondary:     lipgloss.Color("#6c7186"),
		Thinking:          lipgloss.Color("#bd93f9"),
		Success:           lipgloss.Color("#50fa7b"),
		Error:             lipgloss.Color("#ff5555"),
		Warning:           lipgloss.Color("#f1fa8c"),
		CodeBG:            lipgloss.Color("#44475a"),
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color("#44475a"),
		BackgroundElement: lipgloss.Color("#4d5578"),
		Text:              lipgloss.Color("#f8f8f2"),
		TextMuted:         lipgloss.Color("#565f89"),
		BorderActive:      lipgloss.Color("#ff79c6"),
		BorderSubtle:      lipgloss.Color("#44475a"),
		Primary:           lipgloss.Color("#ff79c6"),
		Secondary:         lipgloss.Color("#bd93f9"),
		Accent:            lipgloss.Color("#8be9fd"),
		Info:              lipgloss.Color("#8be9fd"),
		ThinkingOpacity:   0.6,
		DiffAdded:         lipgloss.Color("#50fa7b"),
		DiffRemoved:       lipgloss.Color("#ff5555"),
		DiffAddedBg:       lipgloss.Color("#50fa7b20"),
		DiffRemovedBg:     lipgloss.Color("#ff555520"),
		DiffContextBg:     lipgloss.Color("#44475a"),
		BadgeForeground:   lipgloss.Color("#000000"),
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),
		BadgeTextDark:     lipgloss.Color("#000000"),
		DividerChar:       "─",
		HeaderHeight:      1,
		ShadowColor:       lipgloss.Color("#00000040"),
		CompactMode:       false,
		SelectionBg:       lipgloss.Color("#44475a"),
		CardPadding:       1,
	}
	applyThemeStyles(&t)
	return t
}

// SolarizedDark returns the Solarized Dark theme
func SolarizedDark() Theme {
	t := Theme{
		Mode:              ModeDark,
		Background:        lipgloss.Color("#002b36"),
		Surface:           lipgloss.Color("#073642"),
		SurfaceElevated:   lipgloss.Color("#586e75"),
		Border:            lipgloss.Color("#586e75"),
		Brand:             lipgloss.Color("#268bd2"),
		TextPrimary:       lipgloss.Color("#839496"),
		TextSecondary:     lipgloss.Color("#93a1a1"),
		Thinking:          lipgloss.Color("#6c71c4"),
		Success:           lipgloss.Color("#859900"),
		Error:             lipgloss.Color("#dc322f"),
		Warning:           lipgloss.Color("#b58900"),
		CodeBG:            lipgloss.Color("#073642"),
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color("#073642"),
		BackgroundElement: lipgloss.Color("#586e75"),
		Text:              lipgloss.Color("#839496"),
		TextMuted:         lipgloss.Color("#93a1a1"),
		BorderActive:      lipgloss.Color("#268bd2"),
		BorderSubtle:      lipgloss.Color("#586e75"),
		Primary:           lipgloss.Color("#268bd2"),
		Secondary:         lipgloss.Color("#6c71c4"),
		Accent:            lipgloss.Color("#2aa198"),
		Info:              lipgloss.Color("#2aa198"),
		ThinkingOpacity:   0.6,
		DiffAdded:         lipgloss.Color("#859900"),
		DiffRemoved:       lipgloss.Color("#dc322f"),
		DiffAddedBg:       lipgloss.Color("#85990020"),
		DiffRemovedBg:     lipgloss.Color("#dc322f20"),
		DiffContextBg:     lipgloss.Color("#073642"),
		BadgeForeground:   lipgloss.Color("#000000"),
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),
		BadgeTextDark:     lipgloss.Color("#000000"),
		DividerChar:       "─",
		HeaderHeight:      1,
		ShadowColor:       lipgloss.Color("#00000040"),
		CompactMode:       false,
		SelectionBg:       lipgloss.Color("#586e75"),
		CardPadding:       1,
	}
	applyThemeStyles(&t)
	return t
}

// Monochrome returns the Pure Mono theme
func Monochrome() Theme {
	t := Theme{
		Mode:              ModeDark,
		Background:        lipgloss.Color("#0a0a0a"),
		Surface:           lipgloss.Color("#1a1a1a"),
		SurfaceElevated:   lipgloss.Color("#2a2a2a"),
		Border:            lipgloss.Color("#3a3a3a"),
		Brand:             lipgloss.Color("#ffffff"),
		TextPrimary:       lipgloss.Color("#ffffff"),
		TextSecondary:     lipgloss.Color("#808080"),
		Thinking:          lipgloss.Color("#c0c0c0"),
		Success:           lipgloss.Color("#e0e0e0"),
		Error:             lipgloss.Color("#808080"),
		Warning:           lipgloss.Color("#b0b0b0"),
		CodeBG:            lipgloss.Color("#1a1a1a"),
		ToolLabel:         make(map[string]lipgloss.Style),
		BackgroundPanel:   lipgloss.Color("#1a1a1a"),
		BackgroundElement: lipgloss.Color("#2a2a2a"),
		Text:              lipgloss.Color("#ffffff"),
		TextMuted:         lipgloss.Color("#808080"),
		BorderActive:      lipgloss.Color("#ffffff"),
		BorderSubtle:      lipgloss.Color("#3a3a3a"),
		Primary:           lipgloss.Color("#ffffff"),
		Secondary:         lipgloss.Color("#c0c0c0"),
		Accent:            lipgloss.Color("#e0e0e0"),
		Info:              lipgloss.Color("#c0c0c0"),
		ThinkingOpacity:   0.6,
		DiffAdded:         lipgloss.Color("#a0a0a0"),
		DiffRemoved:       lipgloss.Color("#d0d0d0"),
		DiffAddedBg:       lipgloss.Color("#a0a0a020"),
		DiffRemovedBg:     lipgloss.Color("#d0d0d020"),
		DiffContextBg:     lipgloss.Color("#1a1a1a"),
		BadgeForeground:   lipgloss.Color("#000000"),
		BadgeTextLight:    lipgloss.Color("#FFFFFF"),
		BadgeTextDark:     lipgloss.Color("#000000"),
		DividerChar:       "─",
		HeaderHeight:      1,
		ShadowColor:       lipgloss.Color("#00000040"),
		CompactMode:       false,
		SelectionBg:       lipgloss.Color("#3a3a3a"),
		CardPadding:       1,
	}
	applyThemeStyles(&t)
	return t
}
