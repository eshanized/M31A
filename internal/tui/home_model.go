package tui

import (
	"strings"
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
	{"ctrl+m", "model"},
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

	// Slash command autocomplete state
	cmdRegistry      *CommandRegistry
	slashVisible     bool
	slashSuggestions []CommandInfo
	slashSelected    int
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

// SetCommandRegistry sets the command registry for slash command suggestions.
func (hm *HomeModel) SetCommandRegistry(registry *CommandRegistry) {
	hm.cmdRegistry = registry
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

// updateSlashSuggestions updates slash command autocomplete based on current input.
func (hm *HomeModel) updateSlashSuggestions() {
	if hm.cmdRegistry == nil {
		return
	}

	current := hm.input.Value()
	if !strings.HasPrefix(current, "/") {
		hm.slashVisible = false
		hm.slashSuggestions = nil
		return
	}

	parts := strings.Fields(current)
	partial := ""
	if len(parts) > 0 {
		partial = strings.TrimPrefix(parts[0], "/")
	}

	allCmds := hm.cmdRegistry.AllCommands()
	hm.slashSuggestions = nil

	if partial == "" {
		hm.slashSuggestions = allCmds
	} else {
		q := strings.ToLower(partial)
		for _, cmd := range allCmds {
			name := strings.ToLower(cmd.Name)
			slash := strings.ToLower(strings.TrimPrefix(cmd.Slash, "/"))
			if strings.HasPrefix(slash, q) || strings.HasPrefix(name, q) || strings.Contains(name, q) {
				hm.slashSuggestions = append(hm.slashSuggestions, cmd)
			}
		}
	}

	if len(hm.slashSuggestions) > 0 {
		hm.slashVisible = true
		hm.slashSelected = 0
		if len(hm.slashSuggestions) > 8 {
			hm.slashSuggestions = hm.slashSuggestions[:8]
		}
	} else {
		hm.slashVisible = false
	}
}

// handleSlashComplete completes the selected slash suggestion.
func (hm *HomeModel) handleSlashComplete() tea.Cmd {
	if hm.slashSelected >= len(hm.slashSuggestions) {
		return nil
	}
	chosen := hm.slashSuggestions[hm.slashSelected]
	hm.input.SetValue(chosen.Slash + " ")
	hm.slashVisible = false
	hm.slashSuggestions = nil
	return nil
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
		// Handle slash suggestion navigation when visible
		if hm.slashVisible && len(hm.slashSuggestions) > 0 {
			switch msg.String() {
			case "tab":
				return hm, hm.handleSlashComplete()
			case "esc":
				hm.slashVisible = false
				hm.slashSuggestions = nil
			case "up":
				if hm.slashSelected > 0 {
					hm.slashSelected--
				}
				return hm, nil
			case "down":
				if hm.slashSelected < len(hm.slashSuggestions)-1 {
					hm.slashSelected++
				}
				return hm, nil
			}
		}

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

	// Update slash suggestions after input changes
	hm.updateSlashSuggestions()

	return hm, cmd
}
