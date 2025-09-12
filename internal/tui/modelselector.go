package tui

import (
	"context"
	"sort"
	"strings"

	"github.com/charmbracelet/bubbles/list"
	"github.com/charmbracelet/bubbles/spinner"
	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
)

type providerFilter int

const (
	filterAll providerFilter = iota
	filterOpenRouter
	filterZen
)

func (f providerFilter) String() string {
	switch f {
	case filterAll:
		return "All"
	case filterOpenRouter:
		return "OpenRouter"
	case filterZen:
		return "Zen"
	default:
		return "Unknown"
	}
}

func (f providerFilter) Next() providerFilter {
	return (f + 1) % 3
}

type modelFetchCompleteMsg struct {
	models []types.ModelInfo
	err    string
}

type ModelSelector struct {
	registry      *provider.Registry
	list          list.Model
	search        textinput.Model
	filter        providerFilter
	showDetail    bool
	detailModel   *types.ModelInfo
	ready         bool
	width         int
	height        int
	err           string
	allModels     []types.ModelInfo
	searchFocused bool
	manager       *session.Manager
	spinner       spinner.Model
	theme         theme.Theme
	usageHistory  map[string][]float64 // model ID -> usage frequency data
}

func NewModelSelector(registry *provider.Registry, mgr *session.Manager, t theme.Theme) ModelSelector {
	ti := textinput.New()
	ti.Placeholder = "Search models..."
	ti.CharLimit = 100
	ti.Width = 40

	items := []list.Item{}
	delegate := newModelItemDelegate(t)
	l := list.New(items, delegate, 0, 0)
	l.Title = "Model Selector"
	l.SetShowStatusBar(false)
	l.SetFilteringEnabled(false)
	l.DisableQuitKeybindings()

	sp := spinner.New()
	sp.Spinner = spinner.Dot

	return ModelSelector{
		registry:     registry,
		list:         l,
		search:       ti,
		filter:       filterAll,
		manager:      mgr,
		spinner:      sp,
		theme:        t,
		usageHistory: make(map[string][]float64),
	}
}

func (m ModelSelector) Init() tea.Cmd {
	return tea.Batch(
		textinput.Blink,
		fetchModelsCmd(m.registry),
		m.spinner.Tick,
	)
}

func fetchModelsCmd(registry *provider.Registry) tea.Cmd {
	return func() tea.Msg {
		if registry == nil {
			return modelFetchCompleteMsg{err: "provider registry not available"}
		}
		providers := registry.List()
		var allModels []types.ModelInfo
		for _, name := range providers {
			p, err := registry.Get(name)
			if err != nil {
				continue
			}
			models, err := p.FetchModels(context.Background())
			if err != nil {
				continue
			}
			for i := range models {
				models[i].Provider = name
			}
			allModels = append(allModels, models...)
		}
		return modelFetchCompleteMsg{models: allModels}
	}
}

func (m ModelSelector) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		h, v := 6, 4
		m.list.SetSize(msg.Width-h, msg.Height-v)
		return m, nil

	case spinner.TickMsg:
		var cmd tea.Cmd
		m.spinner, cmd = m.spinner.Update(msg)
		return m, cmd

	case modelFetchCompleteMsg:
		m.ready = true
		m.allModels = msg.models
		if msg.err != "" {
			m.err = msg.err
		}
		m.refilterList()
		return m, nil

	case tea.KeyMsg:
		if m.searchFocused {
			switch msg.String() {
			case "esc":
				m.searchFocused = false
				m.search.Blur()
				return m, nil
			case "enter":
				m.searchFocused = false
				m.search.Blur()
				return m, nil
			default:
				var cmd tea.Cmd
				m.search, cmd = m.search.Update(msg)
				m.refilterList()
				return m, cmd
			}
		}

		switch msg.String() {
		case "p", "P":
			m.filter = m.filter.Next()
			m.refilterList()
			return m, nil

		case "tab":
			m.showDetail = !m.showDetail
			if m.showDetail {
				item := m.list.SelectedItem()
				if item != nil {
					mi, ok := item.(ModelItem)
					if ok {
						m.detailModel = &mi.Model
					}
				}
			} else {
				m.detailModel = nil
			}
			return m, nil

		case "enter":
			item := m.list.SelectedItem()
			if item == nil {
				return m, nil
			}
			mi, ok := item.(ModelItem)
			if !ok {
				return m, nil
			}
			return m, func() tea.Msg {
				return AppMsg{
					ModelSelected: &ModelSelectedMsg{
						Model:    mi.Model,
						Provider: mi.Provider,
					},
				}
			}

		case "f", "F":
			item := m.list.SelectedItem()
			if item != nil {
				mi, ok := item.(ModelItem)
				if ok && m.manager != nil {
					if err := m.manager.ToggleFavorite(mi.Model.ID); err == nil {
						m.refilterList()
					}
				}
			}
			return m, nil

		case "ctrl+c":
			return m, tea.Quit

		case "/", "?":
			m.searchFocused = true
			m.search.Focus()
			return m, nil

		default:
			var cmd tea.Cmd
			m.list, cmd = m.list.Update(msg)
			return m, cmd
		}
	}

	return m, nil
}

func (m *ModelSelector) SetRegistry(registry *provider.Registry) {
	m.registry = registry
}

func (m *ModelSelector) SetTheme(t theme.Theme) {
	m.theme = t
}

func (m *ModelSelector) refilterList() {
	if m.allModels == nil {
		m.list.SetItems(nil)
		return
	}

	var items []list.Item
	searchText := strings.ToLower(m.search.Value())

	var favorites map[string]bool
	if m.manager != nil {
		if data, err := m.manager.LoadRecentModels(); err == nil {
			favorites = data.Favorites
		}
	}

	for _, model := range m.allModels {
		if m.filter == filterOpenRouter && model.Provider != "openrouter" {
			continue
		}
		if m.filter == filterZen && model.Provider != "zen" {
			continue
		}

		if searchText != "" {
			searchTarget := strings.ToLower(model.Name + " " + model.ID + " " + model.Description + " " + model.Provider)
			if !strings.Contains(searchTarget, searchText) {
				continue
			}
		}

		// Get or generate usage data for sparkline
		usageData := m.usageHistory[model.ID]
		if usageData == nil {
			usageData = generateMockUsageData(model.ID)
			m.usageHistory[model.ID] = usageData
		}

		items = append(items, ModelItem{
			Model:      model,
			Provider:   model.Provider,
			IsFavorite: favorites[model.ID],
			UsageData:  usageData,
		})
	}

	sort.Slice(items, func(i, j int) bool {
		mi := items[i].(ModelItem)
		mj := items[j].(ModelItem)
		if mi.Provider != mj.Provider {
			return mi.Provider < mj.Provider
		}
		return mi.Model.Name < mj.Model.Name
	})

	m.list.SetItems(items)
}

// generateMockUsageData creates deterministic mock usage data based on model ID.
// In production, this would come from session history.
func generateMockUsageData(modelID string) []float64 {
	// Use a simple hash for deterministic but varied data
	hash := 0
	for _, ch := range modelID {
		hash = hash*31 + int(ch)
	}
	if hash < 0 {
		hash = -hash
	}

	// Generate 7 days of mock usage data
	data := make([]float64, 7)
	for i := range data {
		val := float64((hash>>(uint(i)*3))%8) / 7.0
		data[i] = val
	}
	return data
}
