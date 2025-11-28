package tui

import (
	"context"
	"sort"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
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

// suggestedModel is a quick-pick model option shown on the model step.
type suggestedModel struct {
	ID    string
	Label string
	Tag   string
}

// providerInfo holds display metadata for a provider.
type providerInfo struct {
	ID          string
	Name        string
	Icon        string
	Description string
	Recommended bool
}

var providerCatalog = []providerInfo{
	{
		ID:          "openrouter",
		Name:        "OpenRouter",
		Icon:        "◈",
		Description: "Unified access to 200+ models from all major labs.",
		Recommended: true,
	},
	{
		ID:          "zen",
		Name:        "Zen",
		Icon:        "◆",
		Description: "Zen gateway with built-in cost controls.",
		Recommended: false,
	},
}

// FirstRunOpts carries the data collected from the first-run wizard.
type FirstRunOpts struct {
	Provider     string
	APIKey       string
	ModelID      string
	SaveKeychain bool
}

// firstRunModelsMsg carries fetched models for the wizard.
type firstRunModelsMsg struct {
	Models []suggestedModel
	Err    error
}

// FirstRunModel is a multi-step setup wizard shown on first launch.
type FirstRunModel struct {
	theme    theme.Theme
	registry *provider.Registry
	step     firstRunStep

	// Provider selection
	providers      []string
	providerCursor int

	// API key entry
	keyInput textinput.Model
	keyErr   string

	// Model selection
	modelInput      textinput.Model
	suggestedModels []suggestedModel
	suggestedCursor int
	modelsLoading   bool

	// Collected values
	opts FirstRunOpts

	width        int
	height       int
	contentWidth int // available width for content (accounts for sidebar)
}

// NewFirstRunModel creates a FirstRunModel.
func NewFirstRunModel(t theme.Theme, registry *provider.Registry) *FirstRunModel {
	keyTI := textinput.New()
	keyTI.Placeholder = "sk-or-..."
	keyTI.EchoMode = textinput.EchoPassword
	keyTI.CharLimit = 200

	modelTI := textinput.New()
	modelTI.Placeholder = "e.g. anthropic/claude-sonnet-4"
	modelTI.CharLimit = 120

	providers := make([]string, len(providerCatalog))
	for i, p := range providerCatalog {
		providers[i] = p.ID
	}

	return &FirstRunModel{
		theme:      t,
		registry:   registry,
		step:       stepWelcome,
		providers:  providers,
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

// SetContentWidth sets the available content width (accounting for sidebar).
// When set, the wizard centers within this width instead of the full terminal.
func (fr *FirstRunModel) SetContentWidth(w int) {
	fr.contentWidth = w
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

	case firstRunModelsMsg:
		fr.modelsLoading = false
		if msg.Err == nil && len(msg.Models) > 0 {
			fr.suggestedModels = msg.Models
			if fr.suggestedCursor >= len(fr.suggestedModels) {
				fr.suggestedCursor = 0
			}
		}
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

// fetchModelsCmd returns a tea.Cmd that fetches models from the active provider.
func (fr *FirstRunModel) fetchModelsCmd() tea.Cmd {
	reg := fr.registry
	providerName := fr.opts.Provider
	if reg == nil || providerName == "" {
		return func() tea.Msg {
			return firstRunModelsMsg{}
		}
	}

	return func() tea.Msg {
		p, err := reg.Get(providerName)
		if err != nil || p == nil {
			return firstRunModelsMsg{Err: err}
		}

		ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
		defer cancel()

		models, err := p.FetchModels(ctx)
		if err != nil {
			return firstRunModelsMsg{Err: err}
		}

		suggested := pickTopModels(models, 3)
		return firstRunModelsMsg{Models: suggested}
	}
}

// pickTopModels selects up to n models from the list, preferring popular ones.
func pickTopModels(models []types.ModelInfo, n int) []suggestedModel {
	// Filter to models with names and sort by context length (proxy for quality)
	var candidates []types.ModelInfo
	for _, m := range models {
		if m.Name != "" && m.ContextLength > 0 {
			candidates = append(candidates, m)
		}
	}
	sort.Slice(candidates, func(i, j int) bool {
		return candidates[i].ContextLength > candidates[j].ContextLength
	})

	var result []suggestedModel
	seen := make(map[string]bool)
	for _, m := range candidates {
		if len(result) >= n {
			break
		}
		if seen[m.ID] {
			continue
		}
		seen[m.ID] = true
		tag := ""
		switch {
		case m.ContextLength >= 1000000:
			tag = "Large ctx"
		case m.Capabilities.Tools:
			tag = "Tools"
		case m.Capabilities.Vision:
			tag = "Vision"
		case m.Capabilities.Reasoning:
			tag = "Reasoning"
		default:
			tag = "Popular"
		}
		result = append(result, suggestedModel{
			ID:    m.ID,
			Label: m.Name,
			Tag:   tag,
		})
	}
	return result
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
		case "s":
			// Skip provider/API key — no models to fetch
			fr.opts.Provider = ""
			fr.suggestedModels = nil
			fr.step = stepModelPick
			fr.modelInput.Focus()
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
			// Start fetching models in background
			fr.modelsLoading = true
			fr.suggestedModels = nil
			return fr, fr.fetchModelsCmd()
		case "tab":
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
		case "1", "2", "3":
			idx := int(msg.String()[0] - '1')
			if idx >= 0 && idx < len(fr.suggestedModels) {
				fr.suggestedCursor = idx
				fr.modelInput.SetValue(fr.suggestedModels[idx].ID)
			}
		case "left", "h":
			if fr.suggestedCursor > 0 {
				fr.suggestedCursor--
				fr.modelInput.SetValue(fr.suggestedModels[fr.suggestedCursor].ID)
			}
		case "right", "l":
			if len(fr.suggestedModels) > 0 && fr.suggestedCursor < len(fr.suggestedModels)-1 {
				fr.suggestedCursor++
				fr.modelInput.SetValue(fr.suggestedModels[fr.suggestedCursor].ID)
			}
		case "enter":
			modelID := strings.TrimSpace(fr.modelInput.Value())
			fr.opts.ModelID = modelID
			fr.step = stepDone
			fr.modelInput.Blur()
			return fr, fr.completeSetup()
		case "esc":
			if fr.opts.Provider != "" {
				fr.step = stepAPIKey
				fr.modelInput.Blur()
				fr.keyInput.Focus()
			} else {
				fr.step = stepProviderSelect
				fr.modelInput.Blur()
			}
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
