package tui

import (
	"strings"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tui/theme"
)

// firstRunStep represents a step in the first-run wizard.
type firstRunStep int

const (
	stepWelcome firstRunStep = iota
	stepProviderSelect
	stepAPIKey
	stepModelPick
	stepDone
)

// FirstRunOpts carries the data collected from the first-run wizard.
type FirstRunOpts struct {
	Provider    string
	APIKey      string
	ModelID     string
	SaveKeychain bool
}

// FirstRunModel is a multi-step setup wizard shown on first launch.
type FirstRunModel struct {
	theme theme.Theme
	step  firstRunStep

	// Provider selection
	providers      []string
	providerCursor int

	// API key entry
	keyInput textinput.Model
	keyErr   string

	// Model selection (simplified: user types a model ID)
	modelInput textinput.Model

	// Collected values
	opts FirstRunOpts

	width  int
	height int
}

// NewFirstRunModel creates a FirstRunModel.
func NewFirstRunModel(t theme.Theme) *FirstRunModel {
	keyTI := textinput.New()
	keyTI.Placeholder = "sk-or-..."
	keyTI.EchoMode = textinput.EchoPassword
	keyTI.CharLimit = 200

	modelTI := textinput.New()
	modelTI.Placeholder = "e.g. anthropic/claude-3-5-sonnet"
	modelTI.CharLimit = 120

	return &FirstRunModel{
		theme:      t,
		step:       stepWelcome,
		providers:  []string{"openrouter", "zen"},
		keyInput:   keyTI,
		modelInput: modelTI,
	}
}

// SetTheme updates the theme.
func (fr *FirstRunModel) SetTheme(t theme.Theme) {
	fr.theme = t
}

// SetDimensions updates the wizard dimensions.
func (fr *FirstRunModel) SetDimensions(w, h int) {
	fr.width = w
	fr.height = h
}

// Init implements tea.Model.
func (fr *FirstRunModel) Init() tea.Cmd {
	return nil
}

// Update implements tea.Model.
func (fr *FirstRunModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		fr.width = msg.Width
		fr.height = msg.Height
		return fr, nil
	case tea.KeyMsg:
		return fr.handleKey(msg)
	}
	// Delegate to focused input if applicable
	var cmd tea.Cmd
	switch fr.step {
	case stepAPIKey:
		fr.keyInput, cmd = fr.keyInput.Update(msg)
	case stepModelPick:
		fr.modelInput, cmd = fr.modelInput.Update(msg)
	}
	return fr, cmd
}

// handleKey routes key events per step.
func (fr *FirstRunModel) handleKey(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch fr.step {
	case stepWelcome:
		switch msg.String() {
		case "enter", " ":
			fr.step = stepProviderSelect
		case "q", "ctrl+c":
			return fr, tea.Quit
		}

	case stepProviderSelect:
		switch msg.String() {
		case "up", "k":
			if fr.providerCursor > 0 {
				fr.providerCursor--
			}
		case "down", "j":
			if fr.providerCursor < len(fr.providers)-1 {
				fr.providerCursor++
			}
		case "enter", " ":
			fr.opts.Provider = fr.providers[fr.providerCursor]
			fr.step = stepAPIKey
			fr.keyErr = ""
			fr.keyInput.Focus()
		case "esc":
			fr.step = stepWelcome
		}

	case stepAPIKey:
		switch msg.String() {
		case "enter":
			key := strings.TrimSpace(fr.keyInput.Value())
			if key == "" {
				fr.keyErr = "API key cannot be empty"
				return fr, nil
			}
			fr.opts.APIKey = key
			fr.keyErr = ""
			fr.step = stepModelPick
			fr.keyInput.Blur()
			fr.modelInput.Focus()
		case "tab":
			// Toggle save-to-keychain
			fr.opts.SaveKeychain = !fr.opts.SaveKeychain
		case "esc":
			fr.step = stepProviderSelect
			fr.keyInput.Blur()
		default:
			var cmd tea.Cmd
			fr.keyInput, cmd = fr.keyInput.Update(msg)
			return fr, cmd
		}

	case stepModelPick:
		switch msg.String() {
		case "enter":
			modelID := strings.TrimSpace(fr.modelInput.Value())
			fr.opts.ModelID = modelID
			fr.step = stepDone
			fr.modelInput.Blur()
			return fr, fr.completeSetup()
		case "esc":
			fr.step = stepAPIKey
			fr.modelInput.Blur()
			fr.keyInput.Focus()
		default:
			var cmd tea.Cmd
			fr.modelInput, cmd = fr.modelInput.Update(msg)
			return fr, cmd
		}
	}
	return fr, nil
}

// completeSetup emits AppMsg to transition to the REPL.
func (fr *FirstRunModel) completeSetup() tea.Cmd {
	opts := fr.opts
	return func() tea.Msg {
		return AppMsg{
			Screen:       ScreenREPL,
			SaveKeychain: opts.SaveKeychain,
		}
	}
}

// View implements tea.Model.
func (fr *FirstRunModel) View() string {
	return fr.renderFirstRun()
}
