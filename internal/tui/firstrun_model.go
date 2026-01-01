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
	Providers       []ProviderEntry
	ModelID         string
	SaveKeychain    bool
	DefaultProvider string
}

// firstRunModelsMsg carries fetched models for the wizard.
type firstRunModelsMsg struct {
	Models []suggestedModel
	Err    error
}

// firstRunKeyValidationMsg carries the result of an async API key health check.
type firstRunKeyValidationMsg struct {
	OK     bool
	ErrStr string
}

// FirstRunModel is a multi-step setup wizard shown on first launch.
type FirstRunModel struct {
	theme    theme.Theme
	registry *provider.Registry
	step     firstRunStep

	// Provider selection (multi-select)
	providers         []string
	providerCursor    int
	providerChecked   map[string]bool
	selectedProviders []string

	// API key entry (loops through selected providers)
	keyInput         textinput.Model
	keyErr           string
	keyProviderIndex int
	keyValidating    bool   // true while async health check runs
	keyValidationErr string // set when validation fails

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

	// UX-05: starfield render cache — avoid regenerating on every frame
	starfieldCache  string
	starfieldCacheW int
	starfieldCacheH int
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
		theme:           t,
		registry:        registry,
		step:            stepWelcome,
		providers:       providers,
		providerChecked: make(map[string]bool),
		keyInput:        keyTI,
		modelInput:      modelTI,
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

// effectiveWidth returns the usable terminal width for layout, accounting for
// the sidebar and the parent-provided content width. Returns the actual
// measured width so narrow terminals can collapse their layouts.
func (fr *FirstRunModel) effectiveWidth() int {
	w := fr.width
	if w < 1 {
		w = 80
	}
	if fr.contentWidth > 0 {
		return fr.contentWidth
	}
	if w >= WidthFull {
		return w - sidebarDefaultWidth
	}
	return w
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

	case firstRunKeyValidationMsg:
		fr.keyValidating = false
		if msg.OK {
			// Validation passed — collect key and advance
			provID := fr.selectedProviders[fr.keyProviderIndex]
			fr.opts.Providers = append(fr.opts.Providers, ProviderEntry{
				ID:     provID,
				APIKey: fr.keyInput.Value(),
			})
			fr.keyErr = ""
			fr.keyValidationErr = ""
			fr.keyInput.SetValue("")
			fr.keyProviderIndex++
			if fr.keyProviderIndex < len(fr.selectedProviders) {
				return fr, nil
			}
			fr.step = stepModelPick
			fr.keyInput.Blur()
			fr.modelInput.Focus()
			fr.modelsLoading = true
			fr.suggestedModels = nil
			return fr, fr.fetchModelsCmd()
		}
		fr.keyValidationErr = msg.ErrStr
		return fr, nil

	case tea.KeyMsg:
		if fr.keyValidating {
			return fr, nil // block input during validation
		}
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
	providerName := fr.opts.DefaultProvider
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

// validateKeyCmd returns a tea.Cmd that validates an API key format.
func (fr *FirstRunModel) validateKeyCmd(providerID, apiKey string) tea.Cmd {
	return func() tea.Msg {
		// Basic format validation per provider
		switch providerID {
		case "openrouter":
			if !strings.HasPrefix(apiKey, "sk-or-") && !strings.HasPrefix(apiKey, "sk-") {
				return firstRunKeyValidationMsg{
					OK:     false,
					ErrStr: "OpenRouter API keys typically start with 'sk-or-'",
				}
			}
		case "zen":
			if len(apiKey) < 8 {
				return firstRunKeyValidationMsg{
					OK:     false,
					ErrStr: "API key seems too short",
				}
			}
		}
		// Key looks valid
		return firstRunKeyValidationMsg{OK: true}
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
		case " ":
			cur := fr.providers[fr.providerCursor]
			fr.providerChecked[cur] = !fr.providerChecked[cur]
		case "enter":
			fr.selectedProviders = nil
			for _, p := range fr.providers {
				if fr.providerChecked[p] {
					fr.selectedProviders = append(fr.selectedProviders, p)
				}
			}
			if len(fr.selectedProviders) == 0 {
				return fr, nil
			}
			fr.opts.DefaultProvider = fr.selectedProviders[0]
			fr.keyProviderIndex = 0
			fr.step = stepAPIKey
			fr.keyErr = ""
			fr.keyInput.SetValue("")
			fr.keyInput.Focus()
		case "s":
			fr.selectedProviders = nil
			fr.opts.DefaultProvider = ""
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
			provID := fr.selectedProviders[fr.keyProviderIndex]
			fr.keyValidating = true
			fr.keyValidationErr = ""
			return fr, fr.validateKeyCmd(provID, key)
		case "tab":
			fr.opts.SaveKeychain = !fr.opts.SaveKeychain
		case "esc":
			if fr.keyProviderIndex > 0 {
				fr.opts.Providers = fr.opts.Providers[:fr.keyProviderIndex]
				fr.keyProviderIndex--
				fr.keyInput.SetValue("")
				fr.keyErr = ""
			} else {
				fr.opts.Providers = nil
				fr.step = stepProviderSelect
				fr.keyInput.Blur()
			}
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
			if fr.opts.DefaultProvider != "" {
				fr.keyProviderIndex = len(fr.selectedProviders) - 1
				if fr.keyProviderIndex < 0 {
					fr.keyProviderIndex = 0
				}
				if fr.keyProviderIndex < len(fr.opts.Providers) {
					fr.opts.Providers = fr.opts.Providers[:fr.keyProviderIndex]
				}
				fr.step = stepAPIKey
				fr.modelInput.Blur()
				fr.keyInput.SetValue("")
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

// completeSetup emits FirstRunCompleteMsg to register providers and transition to the REPL.
func (fr *FirstRunModel) completeSetup() tea.Cmd {
	opts := fr.opts
	return func() tea.Msg {
		return FirstRunCompleteMsg{
			Providers:       opts.Providers,
			ModelID:         opts.ModelID,
			SaveKeychain:    opts.SaveKeychain,
			DefaultProvider: opts.DefaultProvider,
		}
	}
}

// View implements tea.Model.
func (fr *FirstRunModel) View() string {
	return fr.renderFirstRun()
}
