package tui

import (
	"context"
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

// suggestedModel is kept for backward compat but the first-run wizard now
// uses a richer categorized browser. See modelBrowserEntry.
type suggestedModel struct {
	ID    string
	Label string
	Tag   string
}

// modelCategory is a named group of models in the browser.
type modelCategory struct {
	Title  string
	Icon   string
	Models []types.ModelInfo
}

// modelBrowserState holds the full categorized model list and cursor state
// for the first-run wizard's model-pick step.
type modelBrowserState struct {
	categories   []modelCategory
	catCursor    int // which category is active (tab-selectable)
	modelCursor  int // row cursor within the active category
	scrollOffset int // scroll offset within the active category list
}

// activeCat returns a pointer to the currently focused category, or nil.
func (b *modelBrowserState) activeCat() *modelCategory {
	if b == nil || len(b.categories) == 0 {
		return nil
	}
	if b.catCursor < 0 || b.catCursor >= len(b.categories) {
		return nil
	}
	return &b.categories[b.catCursor]
}

// selectedModel returns the ModelInfo for the current cursor position, or the zero value.
func (b *modelBrowserState) selectedModel() (types.ModelInfo, bool) {
	cat := b.activeCat()
	if cat == nil || len(cat.Models) == 0 {
		return types.ModelInfo{}, false
	}
	if b.modelCursor < 0 || b.modelCursor >= len(cat.Models) {
		return types.ModelInfo{}, false
	}
	return cat.Models[b.modelCursor], true
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
	Models    []suggestedModel // legacy field, kept for compat
	AllModels []types.ModelInfo // full model list for the categorized browser
	Err       error
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
	ctx      context.Context
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

	// Model browser (replaces 3-chip quick-pick)
	modelInput      textinput.Model
	suggestedModels []suggestedModel // kept for compat — not used in new browser
	modelsLoading   bool
	browser         *modelBrowserState // nil until models load

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
// Accepts a context that is cancelled on app shutdown to prevent resource leaks.
func NewFirstRunModel(t theme.Theme, registry *provider.Registry, ctx context.Context) *FirstRunModel {
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
		ctx:             ctx,
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
		if msg.Err == nil && len(msg.AllModels) > 0 {
			// Build categorized browser from the full model list.
			cats := categorizeModels(msg.AllModels)
			fr.browser = &modelBrowserState{categories: cats}
			// Pre-fill the text input with the first free model if available.
			if len(cats) > 0 && len(cats[0].Models) > 0 {
				if fr.modelInput.Value() == "" {
					fr.modelInput.SetValue(cats[0].Models[0].ID)
				}
			}
		} else if msg.Err == nil && len(msg.Models) > 0 {
			// Legacy path (shouldn't be hit in new code).
			fr.suggestedModels = msg.Models
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

// fetchModelsCmd returns a tea.Cmd that fetches all models from the active provider.
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

		ctx, cancel := context.WithTimeout(fr.ctx, 20*time.Second)
		defer cancel()

		models, err := p.FetchModels(ctx)
		if err != nil {
			return firstRunModelsMsg{Err: err}
		}

		// Return all models; the TUI categorizes them for display.
		return firstRunModelsMsg{Models: nil, AllModels: models}
	}
}

// categorizeModels groups all models into display categories.
func categorizeModels(models []types.ModelInfo) []modelCategory {
	var free, reasoning, toolUse, vision, other []types.ModelInfo
	for _, m := range models {
		switch {
		case m.Pricing.InputPerMToken == 0 && m.Pricing.OutputPerMToken == 0:
			free = append(free, m)
		case m.Capabilities.Reasoning:
			reasoning = append(reasoning, m)
		case m.Capabilities.Tools && !m.Capabilities.Reasoning:
			toolUse = append(toolUse, m)
		case m.Capabilities.Vision:
			vision = append(vision, m)
		default:
			other = append(other, m)
		}
	}

	var cats []modelCategory
	if len(free) > 0 {
		cats = append(cats, modelCategory{Title: "Free Models", Icon: "🆓", Models: free})
	}
	if len(reasoning) > 0 {
		cats = append(cats, modelCategory{Title: "Reasoning", Icon: "💡", Models: reasoning})
	}
	if len(toolUse) > 0 {
		cats = append(cats, modelCategory{Title: "Tool-Use", Icon: "🔧", Models: toolUse})
	}
	if len(vision) > 0 {
		cats = append(cats, modelCategory{Title: "Vision", Icon: "👁", Models: vision})
	}
	if len(other) > 0 {
		cats = append(cats, modelCategory{Title: "All Models", Icon: "⚡", Models: other})
	}
	return cats
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
		b := fr.browser
		switch msg.String() {
		case "up", "k":
			if b != nil {
				if b.modelCursor > 0 {
					b.modelCursor--
					// Scroll up
					if b.modelCursor < b.scrollOffset {
						b.scrollOffset = b.modelCursor
					}
				}
				// Sync text input to highlighted model
				if m, ok := b.selectedModel(); ok {
					fr.modelInput.SetValue(m.ID)
				}
			}
		case "down", "j":
			if b != nil {
				cat := b.activeCat()
				if cat != nil && b.modelCursor < len(cat.Models)-1 {
					b.modelCursor++
					// Scroll down (visible rows handled in view)
				}
				if m, ok := b.selectedModel(); ok {
					fr.modelInput.SetValue(m.ID)
				}
			}
		case "tab":
			// Switch category
			if b != nil && len(b.categories) > 1 {
				b.catCursor = (b.catCursor + 1) % len(b.categories)
				b.modelCursor = 0
				b.scrollOffset = 0
				if m, ok := b.selectedModel(); ok {
					fr.modelInput.SetValue(m.ID)
				}
			}
		case "shift+tab":
			if b != nil && len(b.categories) > 1 {
				b.catCursor = (b.catCursor - 1 + len(b.categories)) % len(b.categories)
				b.modelCursor = 0
				b.scrollOffset = 0
				if m, ok := b.selectedModel(); ok {
					fr.modelInput.SetValue(m.ID)
				}
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
			// Typing in the model ID input — allows manual override
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
