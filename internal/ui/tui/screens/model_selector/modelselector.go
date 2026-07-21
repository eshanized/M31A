package model_selector

import (
	"context"
	"github.com/eshanized/M31A/internal/ui/tui/tuitypes"
	"strings"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/engine/session"
	"github.com/eshanized/M31A/internal/ui/tui/components"
	"github.com/eshanized/M31A/internal/ui/tui/streaming"
	"github.com/eshanized/M31A/internal/ui/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// modelSelectorLoadedMsg carries models fetched asynchronously.
type modelSelectorLoadedMsg struct {
	providerName string
	models       []types.ModelInfo
	err          error
}

// ModelSelector is a full-screen model/provider picker with search, scroll, and pricing.
type ModelSelector struct {
	ctx            context.Context
	registry       *provider.Registry
	sessionManager *session.Manager
	theme          theme.Theme

	// Loaded models grouped by provider name.
	providers    []string
	modelsByProv map[string][]types.ModelInfo

	// State.
	searchInput    textinput.Model
	activeProvider string
	filtered       []types.ModelInfo // filtered results
	cursor         int
	offset         int // scroll offset
	loading        bool
	errored        map[string]bool // providers that returned errors
	errMsg         string

	width  int
	height int

	// Loading spinner
	spinner components.Spinner
}

// NewModelSelector creates a ModelSelector backed by the given registry.
func NewModelSelector(ctx context.Context, registry *provider.Registry, sessionManager *session.Manager, t theme.Theme) *ModelSelector {
	ti := textinput.New()
	ti.Placeholder = "Search models..."
	ti.Focus()
	ti.CharLimit = 80

	return &ModelSelector{
		ctx:            ctx,
		registry:       registry,
		sessionManager: sessionManager,
		theme:          t,
		modelsByProv:   make(map[string][]types.ModelInfo),
		errored:        make(map[string]bool),
		searchInput:    ti,
		loading:        true,
		spinner:        components.NewSpinner(),
	}
}

// SetTheme updates the theme.
func (ms *ModelSelector) SetTheme(t theme.Theme) {
	ms.theme = t
}

// SetDimensions updates the model selector dimensions.
func (ms *ModelSelector) SetDimensions(w, h int) {
	ms.width = w
	ms.height = h
}

// Init starts model fetch commands for all registered providers.
func (ms *ModelSelector) Init() tea.Cmd {
	provNames := ms.registry.ListAll()
	if len(provNames) == 0 {
		ms.loading = false
		return nil
	}
	ms.providers = provNames
	ms.activeProvider = ms.registry.Active()

	cmds := make([]tea.Cmd, 0, len(provNames)+1)
	cmds = append(cmds, streaming.StreamTickCmd())
	for _, name := range provNames {
		cmds = append(cmds, ms.fetchModelsCmd(name))
	}
	return tea.Batch(cmds...)
}

// fetchModelsCmd fetches models for a single provider asynchronously.
// A hard deadline of FetchModelsTimeout is applied so the goroutine can never
// block indefinitely when the remote server is slow or unresponsive.
func (ms *ModelSelector) fetchModelsCmd(provName string) tea.Cmd {
	return func() tea.Msg {
		p, err := ms.registry.Get(provName)
		if err != nil {
			return modelSelectorLoadedMsg{providerName: provName, err: err}
		}
		fetchCtx, cancel := context.WithTimeout(ms.ctx, types.FetchModelsTimeout)
		defer cancel()
		models, err := p.FetchModels(fetchCtx)
		return modelSelectorLoadedMsg{providerName: provName, models: models, err: err}
	}
}

// Update handles key events and async load messages.
func (ms *ModelSelector) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		ms.width = msg.Width
		ms.height = msg.Height
		return ms, nil

	case streaming.TickMsg:
		if ms.loading {
			ms.spinner.Next()
			return ms, streaming.StreamTickCmd()
		}
		return ms, nil

	case modelSelectorLoadedMsg:
		if msg.err == nil && len(msg.models) > 0 {
			// Ensure each model has its Provider field set from the source provider.
			models := make([]types.ModelInfo, len(msg.models))
			copy(models, msg.models)
			for i := range models {
				if models[i].Provider == "" {
					models[i].Provider = msg.providerName
				}
			}
			ms.modelsByProv[msg.providerName] = models
		} else if msg.err != nil {
			ms.errMsg = msg.err.Error()
			ms.errored[msg.providerName] = true
		}
		// Check if all providers loaded (either successfully or with errors)
		allDone := true
		for _, name := range ms.providers {
			if _, ok := ms.modelsByProv[name]; !ok {
				if !ms.errored[name] {
					allDone = false
					break
				}
			}
		}
		if allDone {
			ms.loading = false
			ms.applyFilter()
		}
		return ms, nil

	case tea.KeyMsg:
		switch msg.String() {
		case "esc", "q":
			return ms, func() tea.Msg {
				return tuitypes.AppMsg{Screen: tuitypes.ScreenREPL}
			}
		case "up", "k":
			if ms.cursor > 0 {
				ms.cursor--
				ms.clampScroll()
			}
			return ms, nil
		case "down", "j":
			if ms.cursor < len(ms.filtered)-1 {
				ms.cursor++
				ms.clampScroll()
			}
			return ms, nil
		case "tab":
			// Cycle provider filter
			ms.cycleProvider()
			return ms, nil
		case "enter":
			return ms, ms.selectCurrent()
		default:
			// Feed into search input
			var cmd tea.Cmd
			ms.searchInput, cmd = ms.searchInput.Update(msg)
			ms.cursor = 0
			ms.offset = 0
			ms.applyFilter()
			return ms, cmd
		}
	}
	return ms, nil
}

// View is delegated to modelselector_view.go.
func (ms *ModelSelector) View() string {
	return ms.renderView()
}

// applyFilter rebuilds filtered list based on current search and provider.
func (ms *ModelSelector) applyFilter() {
	query := strings.ToLower(strings.TrimSpace(ms.searchInput.Value()))

	var all []types.ModelInfo
	if ms.activeProvider == "" {
		for _, name := range ms.providers {
			all = append(all, ms.modelsByProv[name]...)
		}
	} else {
		all = ms.modelsByProv[ms.activeProvider]
	}

	if query == "" {
		ms.filtered = filterChatModels(all)
		return
	}
	ms.filtered = nil
	for _, m := range all {
		if !m.Capabilities.Chat {
			continue
		}
		if strings.Contains(strings.ToLower(m.ID), query) ||
			strings.Contains(strings.ToLower(m.Name), query) ||
			strings.Contains(strings.ToLower(m.Provider), query) {
			ms.filtered = append(ms.filtered, m)
		}
	}
}

// filterChatModels returns only models that support chat completions.
func filterChatModels(models []types.ModelInfo) []types.ModelInfo {
	out := make([]types.ModelInfo, 0, len(models))
	for _, m := range models {
		if m.Capabilities.Chat {
			out = append(out, m)
		}
	}
	return out
}

// cycleProvider rotates through "" (all) and each provider name.
func (ms *ModelSelector) cycleProvider() {
	all := append([]string{""}, ms.providers...)
	for i, p := range all {
		if p == ms.activeProvider {
			ms.activeProvider = all[(i+1)%len(all)]
			break
		}
	}
	ms.cursor = 0
	ms.offset = 0
	ms.applyFilter()
}

// selectCurrent emits tuitypes.AppMsg for the highlighted model.
func (ms *ModelSelector) selectCurrent() tea.Cmd {
	if len(ms.filtered) == 0 {
		return nil
	}
	m := ms.filtered[ms.cursor]
	return func() tea.Msg {
		return tuitypes.AppMsg{
			ModelSelected: &tuitypes.ModelSelectedMsg{
				Model:    m,
				Provider: m.Provider,
			},
		}
	}
}

// clampScroll keeps the cursor visible in the viewport.
func (ms *ModelSelector) clampScroll() {
	listHeight := ms.visibleRows()
	if ms.cursor < ms.offset {
		ms.offset = ms.cursor
	}
	if ms.cursor >= ms.offset+listHeight {
		ms.offset = ms.cursor - listHeight + 1
	}
}

// visibleRows returns the number of list rows that fit in the terminal.
func (ms *ModelSelector) visibleRows() int {
	h := ms.height - 8 // header + search + footer
	if h < 4 {
		return 4
	}
	return h
}
