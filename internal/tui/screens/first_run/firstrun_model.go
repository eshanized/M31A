package first_run

import (
	"github.com/eshanized/M31A/internal/tui/tuitypes"
	"context"
	"fmt"
	"strings"
	"time"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/provider/nvidia"
	"github.com/eshanized/M31A/internal/provider/openrouter"
	"github.com/eshanized/M31A/internal/provider/zen"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// Default sidebar width used for layout calculations.
const sidebarDefaultWidth = 30

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
		ID:          types.ProviderOpenRouter,
		Name:        "OpenRouter",
		Icon:        "⬡",
		Description: "Unified access to 200+ models from all major labs.",
		Recommended: true,
	},
	{
		ID:          types.ProviderZen,
		Name:        "Zen",
		Icon:        "◉",
		Description: "Zen gateway with built-in cost controls.",
		Recommended: false,
	},
	{
		ID:          types.ProviderNvidia,
		Name:        "NVIDIA NIM",
		Icon:        "◆",
		Description: "NVIDIA NIM with coding-optimized models (Nemotron, Llama, DeepSeek).",
		Recommended: false,
	},
}

// FirstRunOpts carries the data collected from the first-run wizard.
type FirstRunOpts struct {
	Providers       []tuitypes.ProviderEntry
	ModelID         string
	SaveKeychain    bool
	DefaultProvider string
}

// firstRunModelsMsg carries fetched models for the wizard.
type firstRunModelsMsg struct {
	Models    []suggestedModel  // legacy field, kept for compat
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
	config   *config.Config
	version  string
	ctx      context.Context
	step     firstRunStep

	// Provider selection (multi-select)
	providers         []string
	providerCursor    int
	providerChecked   map[string]bool
	selectedProviders []string
	providerScroll    int // vertical scroll offset for provider card list

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

	// Model list layout tracking (set by renderer, used by mouse handler)
	modelListY int // Y offset of model list top in rendered view
	modelListH int // visible height of model list in rows
}

// NewFirstRunModel creates a FirstRunModel.
// Accepts a context that is cancelled on app shutdown to prevent resource leaks.
func NewFirstRunModel(t theme.Theme, registry *provider.Registry, cfg *config.Config, version string, ctx context.Context) *FirstRunModel {
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
		config:          cfg,
		version:         version,
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
	if w >= tuitypes.WidthFull {
		return w - sidebarDefaultWidth
	}
	return w
}

// Init implements tea.Model.
func (fr *FirstRunModel) Init() tea.Cmd {
	return nil
}

// Update implements tuitypes.Screenable.
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
			fr.opts.Providers = append(fr.opts.Providers, tuitypes.ProviderEntry{
				ID:     provID,
				APIKey: fr.keyInput.Value(),
			})
			fr.keyErr = ""
			fr.keyValidationErr = ""
			fr.keyInput.SetValue("")
			fr.keyProviderIndex++
			if fr.keyProviderIndex < len(fr.selectedProviders) {
				fr.keyInput.Placeholder = keyPlaceholderForProvider(fr.selectedProviders[fr.keyProviderIndex])
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

	case tea.MouseMsg:
		return fr.handleMouse(msg)
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

	// Find the API key collected in the wizard for this provider.
	var apiKey string
	for _, entry := range fr.opts.Providers {
		if entry.ID == providerName {
			apiKey = entry.APIKey
			break
		}
	}

	return func() tea.Msg {
		// Register (or re-register) the provider with the wizard-collected API key
		// so that FetchModels can authenticate. On first run the registry is empty.
		if apiKey != "" {
			var p provider.LLMProvider
			switch providerName {
			case types.ProviderOpenRouter:
				p, _ = openrouter.New(apiKey, openrouter.Options{
					BaseURL:           fr.config.Provider.OpenRouterBaseURL,
					Referer:           fr.config.Provider.OpenRouterReferer,
					Title:             fr.config.Provider.OpenRouterTitle,
					Version:           fr.version,
				})
			case types.ProviderZen:
				p, _ = zen.New(apiKey, zen.Options{
					BaseURL:       fr.config.Provider.ZenBaseURL,
					Version:       fr.version,
				})
			case types.ProviderNvidia:
				p, _ = nvidia.New(apiKey, nvidia.Options{
					BaseURL:       fr.config.Provider.NvidiaBaseURL,
					Version:       fr.version,
				})
			default:
				return firstRunModelsMsg{Err: fmt.Errorf("unknown provider: %s", providerName)}
			}
			if err := reg.Register(providerName, p); err != nil {
				return firstRunModelsMsg{Err: err}
			}
		}

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

// keyPlaceholderForProvider returns the API key placeholder text for a given provider.
func keyPlaceholderForProvider(providerID string) string {
	switch providerID {
	case types.ProviderOpenRouter:
		return "sk-or-..."
	case types.ProviderNvidia:
		return "nvapi-..."
	case types.ProviderZen:
		return "API key"
	default:
		return "API key"
	}
}

// validateKeyCmd returns a tea.Cmd that validates an API key format.
func (fr *FirstRunModel) validateKeyCmd(providerID, apiKey string) tea.Cmd {
	return func() tea.Msg {
		// Basic format validation per provider
		switch providerID {
		case types.ProviderOpenRouter:
			if !strings.HasPrefix(apiKey, "sk-or-") && !strings.HasPrefix(apiKey, "sk-") {
				return firstRunKeyValidationMsg{
					OK:     false,
					ErrStr: "OpenRouter API keys typically start with 'sk-or-'",
				}
			}
		case types.ProviderZen:
			if len(apiKey) < 8 {
				return firstRunKeyValidationMsg{
					OK:     false,
					ErrStr: "API key seems too short",
				}
			}
		case types.ProviderNvidia:
			if !strings.HasPrefix(apiKey, "nvapi-") {
				return firstRunKeyValidationMsg{
					OK:     false,
					ErrStr: "NVIDIA NIM API keys typically start with 'nvapi-'",
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
		case "esc":
			// Allow escaping back to REPL if user has already configured a provider
			return fr, func() tea.Msg { return tuitypes.PopScreenMsg{} }
		case "q", "ctrl+c":
			return fr, tea.Quit
		}

	case stepProviderSelect:
		switch msg.String() {
		case "up", "k":
			if fr.providerCursor > 0 {
				fr.providerCursor--
				fr.clampProviderScroll()
			}
		case "down", "j":
			if fr.providerCursor < len(fr.providers)-1 {
				fr.providerCursor++
				fr.clampProviderScroll()
			}
		case " ":
			cur := fr.providers[fr.providerCursor]
			fr.providerChecked[cur] = !fr.providerChecked[cur]
			fr.rebuildSelectedProviders()
		case "enter":
			fr.rebuildSelectedProviders()
			if len(fr.selectedProviders) == 0 {
				fr.keyErr = "Select at least one provider (space to toggle)"
				return fr, nil
			}
			fr.keyErr = ""
			fr.opts.DefaultProvider = fr.selectedProviders[0]
			fr.keyProviderIndex = 0
			fr.providerScroll = 0
			fr.step = stepAPIKey
			fr.keyInput.SetValue("")
			fr.keyInput.Placeholder = keyPlaceholderForProvider(fr.selectedProviders[0])
			fr.keyInput.Focus()
		case "s":
			fr.rebuildSelectedProviders()
			fr.opts.DefaultProvider = ""
			fr.providerScroll = 0
			fr.suggestedModels = nil
			fr.step = stepModelPick
			fr.modelInput.Focus()
		case "esc":
			fr.keyErr = ""
			fr.providerScroll = 0
			fr.step = stepWelcome
		case "q", "ctrl+c":
			return fr, tea.Quit
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
			fr.keyErr = ""
			fr.keyValidationErr = ""
			if fr.keyProviderIndex > 0 {
				fr.opts.Providers = fr.opts.Providers[:fr.keyProviderIndex]
				fr.keyProviderIndex--
				fr.keyInput.SetValue("")
			} else {
				fr.opts.Providers = nil
				fr.rebuildSelectedProviders()
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
				fr.keyInput.Placeholder = keyPlaceholderForProvider(fr.selectedProviders[fr.keyProviderIndex])
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

// handleMouse processes mouse events for the first-run wizard.
func (fr *FirstRunModel) handleMouse(msg tea.MouseMsg) (tea.Model, tea.Cmd) {
	if msg.Action != tea.MouseActionPress {
		return fr, nil
	}

	switch msg.Button {
	case tea.MouseButtonWheelUp:
		if fr.step == stepModelPick && fr.browser != nil {
			b := fr.browser
			if b.modelCursor > 0 {
				b.modelCursor--
				if b.modelCursor < b.scrollOffset {
					b.scrollOffset = b.modelCursor
				}
				if m, ok := b.selectedModel(); ok {
					fr.modelInput.SetValue(m.ID)
				}
			}
		}

	case tea.MouseButtonWheelDown:
		if fr.step == stepModelPick && fr.browser != nil {
			b := fr.browser
			cat := b.activeCat()
			if cat != nil && b.modelCursor < len(cat.Models)-1 {
				b.modelCursor++
				// Scroll down if cursor exceeds visible window
				if b.modelCursor >= b.scrollOffset+fr.modelListH {
					b.scrollOffset = b.modelCursor - fr.modelListH + 1
				}
			}
			if m, ok := b.selectedModel(); ok {
				fr.modelInput.SetValue(m.ID)
			}
		}

	case tea.MouseButtonLeft:
		if fr.step == stepModelPick && fr.browser != nil {
			b := fr.browser
			cat := b.activeCat()
			if cat == nil || len(cat.Models) == 0 {
				return fr, nil
			}
			// Map absolute Y to model list row
			row := msg.Y - fr.modelListY
			if row < 0 || row >= fr.modelListH {
				return fr, nil
			}
			idx := b.scrollOffset + row
			if idx < 0 || idx >= len(cat.Models) {
				return fr, nil
			}
			// If clicking the already-selected model, confirm it
			if idx == b.modelCursor {
				modelID := strings.TrimSpace(fr.modelInput.Value())
				if modelID != "" {
					fr.opts.ModelID = modelID
					fr.step = stepDone
					fr.modelInput.Blur()
					return fr, fr.completeSetup()
				}
			}
			// Otherwise, select the clicked model
			b.modelCursor = idx
			if m, ok := b.selectedModel(); ok {
				fr.modelInput.SetValue(m.ID)
			}
		}
	}

	return fr, nil
}

// rebuildSelectedProviders syncs selectedProviders from providerChecked,
// preserving the catalog order. Call after any toggle or when returning to the
// provider-select step so summary UI (count badge, default marker) is live.
func (fr *FirstRunModel) rebuildSelectedProviders() {
	fr.selectedProviders = fr.selectedProviders[:0]
	for _, p := range fr.providers {
		if fr.providerChecked[p] {
			fr.selectedProviders = append(fr.selectedProviders, p)
		}
	}
}

// providerVisibleCount returns the maximum number of provider cards that fit
// in the current terminal height, given the fixed chrome (header, dots,
// subtitle, summary, hints, and outer box padding/border). Always returns at
// least 2 so the user can compare adjacent cards on reasonably-sized terminals.
func (fr *FirstRunModel) providerVisibleCount() int {
	// Chrome budget: outer rounded box (border+padding) ≈ 4 lines + inner
	// header row (1) + blank (1) + step line (1) + blank (1) + subtitle (1) +
	// blank (1) + summary (1) + blank (1) + blank (1) + hints (1) + outer
	// bottom border (1) ≈ 18 lines total.
	chrome := 18
	avail := fr.height - chrome
	if avail < 10 {
		avail = 10
	}
	// Each rendered card is ~6 lines tall (1 blank prefix + 4 card body incl.
	// border + 1 blank gap). Use 6 as a conservative estimate.
	n := avail / 6
	if n < 2 {
		n = 2
	}
	return n
}

// clampProviderScroll adjusts providerScroll so that the cursor falls within
// the visible window [providerScroll, providerScroll + visible). Uses the
// heuristic visible count from providerVisibleCount.
func (fr *FirstRunModel) clampProviderScroll() {
	fr.clampProviderScrollWithVisible(fr.providerVisibleCount())
}

// clampProviderScrollWithVisible is like clampProviderScroll but accepts an
// externally-measured visible count (used by the renderer, which knows the
// exact available height).
func (fr *FirstRunModel) clampProviderScrollWithVisible(visible int) {
	if visible > len(fr.providers) {
		visible = len(fr.providers)
	}
	if visible < 1 {
		visible = 1
	}
	if fr.providerCursor < fr.providerScroll {
		fr.providerScroll = fr.providerCursor
	}
	if fr.providerCursor >= fr.providerScroll+visible {
		fr.providerScroll = fr.providerCursor - visible + 1
	}
	if fr.providerScroll < 0 {
		fr.providerScroll = 0
	}
	maxScroll := len(fr.providers) - visible
	if maxScroll < 0 {
		maxScroll = 0
	}
	if fr.providerScroll > maxScroll {
		fr.providerScroll = maxScroll
	}
}

// completeSetup emits tuitypes.FirstRunCompleteMsg to register providers and transition to the REPL.
func (fr *FirstRunModel) completeSetup() tea.Cmd {
	opts := fr.opts
	return func() tea.Msg {
		return tuitypes.FirstRunCompleteMsg{
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

// centerScreen centers a content block within the given dimensions.
func centerScreen(content string, contentW, contentH int) string {
	lines := strings.Split(content, "\n")
	if len(lines) == 0 {
		return ""
	}
	// For simplicity, just return the content as-is.
	// A full implementation would add padding.
	return content
}

// resolveLogoText returns the custom logo text from config.
func resolveLogoText(ui config.UIConfig) string {
	if ui.LogoText != "" {
		return ui.LogoText
	}
	return ""
}
