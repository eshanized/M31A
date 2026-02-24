package tui

import (
	"strings"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// ThemePickerModel lets users browse and preview theme presets.
type ThemePickerModel struct {
	theme  theme.Theme
	themes []themePreset
	cursor int
	width  int
	height int
}

type themePreset struct {
	Name       string
	Background string
	Brand      string
	Text       string
}

// NewThemePickerModel creates a ThemePickerModel with built-in presets.
func NewThemePickerModel(t theme.Theme, w, h int) *ThemePickerModel {
	presets := []themePreset{
		{Name: "Dark (Default)", Background: "#1e1e2e", Brand: "#cba6f7", Text: "#cdd6f4"},
		{Name: "Light", Background: "#eff1f5", Brand: "#8839ef", Text: "#4c4f69"},
		{Name: "Nord", Background: "#2e3440", Brand: "#88c0d0", Text: "#d8dee9"},
		{Name: "Tokyo Night", Background: "#1a1b26", Brand: "#7aa2f7", Text: "#c0caf5"},
		{Name: "Gruvbox", Background: "#282828", Brand: "#d3869b", Text: "#ebdbb2"},
		{Name: "Rosé Pine", Background: "#191724", Brand: "#c4a7e7", Text: "#e0def4"},
		{Name: "Dracula", Background: "#282a36", Brand: "#ff79c6", Text: "#f8f8f2"},
		{Name: "Solarized", Background: "#002b36", Brand: "#268bd2", Text: "#839496"},
		{Name: "Monochrome", Background: "#0a0a0a", Brand: "#ffffff", Text: "#ffffff"},
		{Name: "Catppuccin", Background: "#1e1e2e", Brand: "#cba6f7", Text: "#cdd6f4"},
	}
	return &ThemePickerModel{
		theme:  t,
		themes: presets,
		width:  w,
		height: h,
	}
}

// SetTheme updates the theme.
func (tp *ThemePickerModel) SetTheme(t theme.Theme) { tp.theme = t }

// SetDimensions updates dimensions.
func (tp *ThemePickerModel) SetDimensions(w, h int) {
	tp.width = w
	tp.height = h
}

// Init implements tea.Model.
func (tp *ThemePickerModel) Init() tea.Cmd { return nil }

// Update implements tea.Model.
func (tp *ThemePickerModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		tp.SetDimensions(msg.Width, msg.Height)
	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return tp, func() tea.Msg { return PopScreenMsg{} }
		case "up", "k":
			if tp.cursor > 0 {
				tp.cursor--
			}
		case "down", "j":
			if tp.cursor < len(tp.themes)-1 {
				tp.cursor++
			}
		case "enter":
			if tp.cursor < len(tp.themes) {
				name := strings.ToLower(tp.themes[tp.cursor].Name)
				if strings.Contains(name, "light") {
					return tp, func() tea.Msg { return ThemeChangedMsg{Theme: "light"} }
				}
				return tp, func() tea.Msg { return ThemeChangedMsg{Theme: "dark"} }
			}
		}
	}
	return tp, nil
}

// View implements tea.Model.
func (tp *ThemePickerModel) View() string {
	t := tp.theme
	w := tp.width
	if w < 40 {
		w = 80
	}

	var lines []string
	title := lipgloss.NewStyle().Foreground(t.Brand).Bold(true).PaddingLeft(2).
		Render("Theme Presets")
	lines = append(lines, "", title, "")

	for i, preset := range tp.themes {
		selected := i == tp.cursor
		swatches := lipgloss.NewStyle().Foreground(t.TextMuted).Render("  ") +
			lipgloss.NewStyle().Background(lipgloss.Color(preset.Background)).Render(" ■ ") +
			lipgloss.NewStyle().Background(lipgloss.Color(preset.Brand)).Render(" ■ ") +
			lipgloss.NewStyle().Background(lipgloss.Color(preset.Text)).Render(" ■ ")

		nameStyle := lipgloss.NewStyle().Foreground(t.Text).Width(20)
		if selected {
			nameStyle = nameStyle.Foreground(t.Brand).Bold(true)
		}

		prefix := "    "
		if selected {
			prefix = lipgloss.NewStyle().Foreground(t.Brand).Render("  ▶ ")
		}

		line := prefix + nameStyle.Render(preset.Name) + swatches
		lines = append(lines, line)
	}

	// Preview card
	if tp.cursor < len(tp.themes) {
		p := tp.themes[tp.cursor]
		previewContent := lipgloss.JoinVertical(lipgloss.Left,
			lipgloss.NewStyle().Foreground(lipgloss.Color(p.Brand)).Bold(true).Render("Preview Card"),
			lipgloss.NewStyle().Foreground(lipgloss.Color(p.Text)).Render("Sample text in selected theme"),
			lipgloss.NewStyle().Foreground(lipgloss.Color(p.Brand)).Render("✓ Success  ✗ Error  ⚠ Warning"),
		)
		preview := lipgloss.NewStyle().
			Border(lipgloss.RoundedBorder()).
			BorderForeground(lipgloss.Color(p.Brand)).
			Background(lipgloss.Color(p.Background)).
			Padding(1, 2).
			Width(w - 8).
			Render(previewContent)
		lines = append(lines, "", "  "+preview)
	}

	footer := lipgloss.NewStyle().Foreground(t.TextMuted).PaddingLeft(2).
		Render("[enter] Apply   [esc] Cancel")
	lines = append(lines, "", footer)

	return strings.Join(lines, "\n")
}
