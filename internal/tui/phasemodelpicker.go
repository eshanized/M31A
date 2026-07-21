package tui

import (
	"context"
	"strings"

	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/tui/components"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// pickerPanel is one of the two model-selection panels (Planning or Coding).
type pickerPanel struct {
	label       string // e.g. "Planning Model"
	description string // one-liner about which phases this covers
	searchInput textinput.Model
	models      []types.ModelInfo
	filtered    []types.ModelInfo
	cursor      int
	offset      int
	selected    *types.ModelInfo // nil = use default
	loading     bool
	spinner     components.Spinner
}

func newPickerPanel(label, description string) pickerPanel {
	ti := textinput.New()
	ti.Placeholder = "Search models..."
	ti.CharLimit = 80
	return pickerPanel{
		label:       label,
		description: description,
		loading:     true,
		spinner:     components.NewSpinner(),
	}
}

// applyFilter rebuilds filtered list from search input.
func (p *pickerPanel) applyFilter() {
	query := strings.ToLower(strings.TrimSpace(p.searchInput.Value()))
	if query == "" {
		p.filtered = filterChatModels(p.models)
		return
	}
	p.filtered = nil
	for _, m := range p.models {
		if !m.Capabilities.Chat {
			continue
		}
		if strings.Contains(strings.ToLower(m.ID), query) ||
			strings.Contains(strings.ToLower(m.Name), query) ||
			strings.Contains(strings.ToLower(m.Provider), query) {
			p.filtered = append(p.filtered, m)
		}
	}
}

// clampScroll keeps the cursor in view.
func (p *pickerPanel) clampScroll(visibleRows int) {
	if p.cursor < p.offset {
		p.offset = p.cursor
	}
	if p.cursor >= p.offset+visibleRows {
		p.offset = p.cursor - visibleRows + 1
	}
}

// PhaseModelPickerModel is the Bubble Tea model for the dual-model picker.
// It shows two panels side-by-side: Planning Model (left) and Coding Model (right).
type PhaseModelPickerModel struct {
	ctx      context.Context
	registry *provider.Registry
	theme    theme.Theme

	panels [2]pickerPanel // 0 = planning, 1 = coding
	focus  int            // 0 or 1 — which panel has keyboard focus

	width  int
	height int

	// loadedProviders tracks which providers have finished loading.
	loadedProviders map[string]bool
	providerNames   []string
}

// planningPhases labels
const planningPanelLabel = "Planning Model"
const planningPanelDesc = "Used for: Discuss · Plan · Verify"
const codingPanelLabel = "Coding Model"
const codingPanelDesc = "Used for: Execute · Ship"

// NewPhaseModelPickerModel creates a PhaseModelPickerModel.
func NewPhaseModelPickerModel(ctx context.Context, registry *provider.Registry, t theme.Theme, w, h int) *PhaseModelPickerModel {
	m := &PhaseModelPickerModel{
		ctx:             ctx,
		registry:        registry,
		theme:           t,
		width:           w,
		height:          h,
		loadedProviders: make(map[string]bool),
	}
	m.panels[0] = newPickerPanel(planningPanelLabel, planningPanelDesc)
	m.panels[1] = newPickerPanel(codingPanelLabel, codingPanelDesc)
	// Focus the search input of the active (left) panel.
	m.panels[0].searchInput.Focus()
	return m
}

// SetTheme updates the theme.
func (m *PhaseModelPickerModel) SetTheme(t theme.Theme) { m.theme = t }

// SetDimensions updates dimensions.
func (m *PhaseModelPickerModel) SetDimensions(w, h int) {
	m.width = w
	m.height = h
}

// Init fires async model fetches for all registered providers.
func (m *PhaseModelPickerModel) Init() tea.Cmd {
	names := m.registry.ListAll()
	m.providerNames = names
	if len(names) == 0 {
		m.panels[0].loading = false
		m.panels[1].loading = false
		return nil
	}
	cmds := []tea.Cmd{StreamTickCmd()}
	for _, name := range names {
		cmds = append(cmds, m.fetchCmd(name))
	}
	return tea.Batch(cmds...)
}

// fetchCmd fetches models for one provider.
// A hard deadline of FetchModelsTimeout is applied so the goroutine can never
// block indefinitely when the remote server is slow or unresponsive.
func (m *PhaseModelPickerModel) fetchCmd(provName string) tea.Cmd {
	return func() tea.Msg {
		p, err := m.registry.Get(provName)
		if err != nil {
			return modelSelectorLoadedMsg{providerName: provName, err: err}
		}
		fetchCtx, cancel := context.WithTimeout(m.ctx, types.FetchModelsTimeout)
		defer cancel()
		models, err := p.FetchModels(fetchCtx)
		return modelSelectorLoadedMsg{providerName: provName, models: models, err: err}
	}
}

// Update handles messages.
func (m *PhaseModelPickerModel) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return m, nil

	case TickMsg:
		if m.panels[0].loading || m.panels[1].loading {
			m.panels[0].spinner.Next()
			m.panels[1].spinner.Next()
			return m, StreamTickCmd()
		}
		return m, nil

	case modelSelectorLoadedMsg:
		m.loadedProviders[msg.providerName] = true
		if msg.err == nil && len(msg.models) > 0 {
			// Ensure each model has its Provider field set from the source provider.
			models := make([]types.ModelInfo, len(msg.models))
			copy(models, msg.models)
			for i := range models {
				if models[i].Provider == "" {
					models[i].Provider = msg.providerName
				}
			}
			// Append loaded models to both panels.
			m.panels[0].models = append(m.panels[0].models, models...)
			m.panels[1].models = append(m.panels[1].models, models...)
		}
		// Check if all providers are done.
		allDone := true
		for _, name := range m.providerNames {
			if !m.loadedProviders[name] {
				allDone = false
				break
			}
		}
		if allDone {
			m.panels[0].loading = false
			m.panels[1].loading = false
			m.panels[0].applyFilter()
			m.panels[1].applyFilter()
		}
		return m, nil

	case tea.KeyMsg:
		return m.handleKey(msg)
	}
	return m, nil
}

func (m *PhaseModelPickerModel) handleKey(msg tea.KeyMsg) (tea.Model, tea.Cmd) {
	switch msg.String() {
	case "esc":
		// Skip picker — use default models for both phases.
		return m, func() tea.Msg {
			return PhaseModelPickedMsg{}
		}

	case "ctrl+enter":
		// Confirm selections.
		return m, m.confirmCmd()

	case "tab":
		// Switch focus between panels.
		m.panels[m.focus].searchInput.Blur()
		m.focus = 1 - m.focus
		m.panels[m.focus].searchInput.Focus()
		return m, nil

	case "up", "k":
		p := &m.panels[m.focus]
		if p.cursor > 0 {
			p.cursor--
			p.clampScroll(m.visibleRows())
		}
		return m, nil

	case "down", "j":
		p := &m.panels[m.focus]
		if p.cursor < len(p.filtered)-1 {
			p.cursor++
			p.clampScroll(m.visibleRows())
		}
		return m, nil

	case "enter":
		// Select highlighted model for this panel.
		p := &m.panels[m.focus]
		if len(p.filtered) > 0 {
			selected := p.filtered[p.cursor]
			p.selected = &selected
		}
		return m, nil

	default:
		// Route to focused panel's search input.
		p := &m.panels[m.focus]
		var cmd tea.Cmd
		p.searchInput, cmd = p.searchInput.Update(msg)
		p.cursor = 0
		p.offset = 0
		p.applyFilter()
		return m, cmd
	}
}

// confirmCmd emits PhaseModelPickedMsg with both selected model IDs.
func (m *PhaseModelPickerModel) confirmCmd() tea.Cmd {
	planningID := ""
	planningProv := ""
	if m.panels[0].selected != nil {
		planningID = m.panels[0].selected.ID
		planningProv = m.panels[0].selected.Provider
	}
	codingID := ""
	codingProv := ""
	if m.panels[1].selected != nil {
		codingID = m.panels[1].selected.ID
		codingProv = m.panels[1].selected.Provider
	}
	return func() tea.Msg {
		return PhaseModelPickedMsg{
			PlanningModelID:  planningID,
			PlanningProvider: planningProv,
			CodingModelID:    codingID,
			CodingProvider:   codingProv,
		}
	}
}

// visibleRows returns available list rows per panel.
func (m *PhaseModelPickerModel) visibleRows() int {
	h := m.height - 14 // header + search + footer + padding
	if h < 4 {
		return 4
	}
	return h
}

// View delegates to the view renderer.
func (m *PhaseModelPickerModel) View() string {
	return m.renderView()
}
