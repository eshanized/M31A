package tui

import (
	"time"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// homePlaceholders are rotating placeholder prompts for the home input.
var homePlaceholders = []string{
	"Type a goal or question...",
	"Fix a bug in the codebase",
	"Start a workflow with /new",
	"What is the tech stack of this project?",
	"Add tests for the main package",
}

// homeTip represents a keyboard shortcut shown on the home screen.
type homeTip struct {
	key   string
	label string
}

var homeTips = []homeTip{
	{"ctrl+p", "commands"},
	{"ctrl+x", "leader"},
	{"ctrl+b", "sidebar"},
	{"ctrl+m", "models"},
	{"ctrl+h", "help"},
}

// HomeTickMsg advances the rotating placeholder text.
type HomeTickMsg struct{}

// HomeModel is the landing screen with logo, prompt input, and tips.
type HomeModel struct {
	theme   theme.Theme
	version string

	input         textinput.Model
	placeholderIx int
	tips          []homeTip

	width  int
	height int
}

// NewHomeModel creates a HomeModel.
func NewHomeModel(t theme.Theme, w, h int, version string) *HomeModel {
	ti := textinput.New()
	ti.Placeholder = homePlaceholders[0]
	ti.CharLimit = 1024
	ti.Focus()

	return &HomeModel{
		theme:   t,
		version: version,
		input:   ti,
		tips:    homeTips,
		width:   w,
		height:  h,
	}
}

// SetDimensions updates the model's available space.
func (hm *HomeModel) SetDimensions(w, h int) {
	hm.width = w
	hm.height = h
}

// SetTheme updates the theme.
func (hm *HomeModel) SetTheme(t theme.Theme) {
	hm.theme = t
}

// View implements tea.Model. Delegates to renderHome in home_view.go.
func (hm *HomeModel) View() string {
	return hm.renderHome()
}

// Init starts the placeholder rotation ticker.
func (hm *HomeModel) Init() tea.Cmd {
	return homeTickCmd()
}

func homeTickCmd() tea.Cmd {
	return tea.Tick(4*time.Second, func(time.Time) tea.Msg {
		return HomeTickMsg{}
	})
}

// Update handles key and tick messages.
func (hm *HomeModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		hm.SetDimensions(msg.Width, msg.Height)

	case HomeTickMsg:
		if hm.input.Value() == "" {
			hm.placeholderIx = (hm.placeholderIx + 1) % len(homePlaceholders)
			hm.input.Placeholder = homePlaceholders[hm.placeholderIx]
		}
		return hm, homeTickCmd()

	case tea.KeyMsg:
		switch msg.String() {
		case "enter":
			text := hm.input.Value()
			if text != "" {
				hm.input.SetValue("")
				return hm, func() tea.Msg { return HomeSubmitMsg{Text: text} }
			}
		}
	}

	var cmd tea.Cmd
	hm.input, cmd = hm.input.Update(msg)
	return hm, cmd
}
