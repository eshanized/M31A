package tui

import (
	"context"
	"fmt"
	"io"
	"sort"
	"strings"

	"github.com/charmbracelet/bubbles/list"
	"github.com/charmbracelet/bubbles/textinput"
	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/lipgloss"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/pkg/session"
)

// providerFilter represents which provider's models to show.
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

// Styling constants for variant badge and favorite indicator.
var (
	modelVariantStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("#9AA0A6")).Italic(true)
	favoriteStarStyle = lipgloss.NewStyle().Foreground(lipgloss.Color("#D77757")).Bold(true)
)

// ModelItem implements list.DefaultItem for model display.
type ModelItem struct {
	Model      types.ModelInfo
	Provider   string
	IsFavorite bool
}

func (i ModelItem) Title() string {
	name := i.Model.Name
	if name == "" {
		name = i.Model.ID
	}
	title := fmt.Sprintf("%s [%s]", name, providerBadge(i.Provider))

	// Append variant badge if Variant is set (non-nil)
	if i.Model.Variant != nil {
		title += " " + modelVariantStyle.Render(*i.Model.Variant)
	}

	return title
}

func (i ModelItem) Description() string {
	// Favorite indicator
	var desc string
	if i.IsFavorite {
		desc = favoriteStarStyle.Render("★") + " favorite | "
	}

	// Standard usage estimate: 100K input tokens, 50K output tokens
	estCost := (i.Model.Pricing.InputPerMToken * 0.1) + (i.Model.Pricing.OutputPerMToken * 0.05)
	costDesc := fmt.Sprintf("$%.4f (100K in + 50K out)", estCost)
	if i.Model.Pricing.InputPerMToken == 0 && i.Model.Pricing.OutputPerMToken == 0 {
		costDesc = "pricing N/A"
	}
	ctxDesc := fmt.Sprintf("context: %dK", i.Model.ContextLength/1024)
	caps := capabilityString(i.Model.Capabilities)
	desc += fmt.Sprintf("%s \u00b7 %s \u00b7 %s", costDesc, ctxDesc, caps)
	return desc
}

func (i ModelItem) FilterValue() string {
	return strings.ToLower(i.Model.Name + " " + i.Model.ID + " " + i.Model.Description + " " + i.Model.Provider)
}

// modelItemDelegate provides custom coloring for model list items.
type modelItemDelegate struct {
	defaultDelegate list.DefaultDelegate
}

func newModelItemDelegate() modelItemDelegate {
	d := list.NewDefaultDelegate()
	d.Styles.SelectedTitle = d.Styles.SelectedTitle.Foreground(lipgloss.Color("#D77757"))
	d.Styles.SelectedDesc = d.Styles.SelectedDesc.Foreground(lipgloss.Color("#D77757"))
	return modelItemDelegate{defaultDelegate: d}
}

func (d modelItemDelegate) Height() int                               { return d.defaultDelegate.Height() }
func (d modelItemDelegate) Spacing() int                              { return d.defaultDelegate.Spacing() }
func (d modelItemDelegate) Update(msg tea.Msg, m *list.Model) tea.Cmd { return d.defaultDelegate.Update(msg, m) }
func (d modelItemDelegate) Render(w io.Writer, m list.Model, index int, item list.Item) {
	d.defaultDelegate.Render(w, m, index, item)
}

// modelFetchCompleteMsg is an internal message carrying fetched model data.
type modelFetchCompleteMsg struct {
	models []types.ModelInfo
	err    string
}

// ModelSelector provides a full-screen overlay for browsing and selecting
// LLM models from all registered providers.
type ModelSelector struct {
	registry    *provider.Registry
	list        list.Model
	search      textinput.Model
	filter      providerFilter
	showDetail  bool
	detailModel *types.ModelInfo
	ready       bool
	width       int
	height      int
	err         string
	allModels   []types.ModelInfo // unfiltered model list
	searchFocused bool
	manager     *session.Manager // for favorites & recent models
}

// NewModelSelector creates a ModelSelector with search input and model list.
func NewModelSelector(registry *provider.Registry, mgr *session.Manager) ModelSelector {
	ti := textinput.New()
	ti.Placeholder = "Search models..."
	ti.CharLimit = 100
	ti.Width = 40

	items := []list.Item{}
	delegate := newModelItemDelegate()
	l := list.New(items, delegate, 0, 0)
	l.Title = "Model Selector"
	l.SetShowStatusBar(false)
	l.SetFilteringEnabled(false)
	l.DisableQuitKeybindings()

	return ModelSelector{
		registry: registry,
		list:     l,
		search:   ti,
		filter:   filterAll,
		manager:  mgr,
	}
}

// Init returns the initial commands for the ModelSelector.
func (m ModelSelector) Init() tea.Cmd {
	return tea.Batch(
		textinput.Blink,
		fetchModelsCmd(m.registry),
	)
}

// fetchModelsCmd fetches models from all registered providers.
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
				models[i].Provider = name // tag with provider name
			}
			allModels = append(allModels, models...)
		}
		return modelFetchCompleteMsg{models: allModels}
	}
}

// Update handles messages for the model selector.
func (m ModelSelector) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {
	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		h, v := 6, 4
		m.list.SetSize(msg.Width-h, msg.Height-v)
		return m, nil

	case modelFetchCompleteMsg:
		m.ready = true
		m.allModels = msg.models
		if msg.err != "" {
			m.err = msg.err
		}
		m.refilterList()
		return m, nil

	case tea.KeyMsg:
		// If search is focused, handle search-specific keys first
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
					// Toggle favorite status via session manager
					if err := m.manager.ToggleFavorite(mi.Model.ID); err == nil {
						// Update the item's IsFavorite flag in the list
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

// View renders the model selector screen.
func (m ModelSelector) View() string {
	if !m.ready {
		return lipgloss.Place(m.width, m.height,
			lipgloss.Center, lipgloss.Center,
			"Loading models...",
		)
	}

	var parts []string

	// Top bar: filter indicator + key bindings
	topBar := m.renderTopBar()
	parts = append(parts, topBar)

	// Search input
	searchView := m.renderSearchInput()
	parts = append(parts, searchView)

	// List
	listView := m.list.View()
	parts = append(parts, listView)

	// Detail pane (if toggled)
	if m.showDetail && m.detailModel != nil {
		detailView := m.detailView()
		parts = append(parts, detailView)
	}

	// Error message
	if m.err != "" {
		errStyle := lipgloss.NewStyle().Foreground(lipgloss.Color("#F28B82"))
		parts = append(parts, errStyle.Render(m.err))
	}

	// Help bar
	helpBar := m.renderHelpBar()
	parts = append(parts, helpBar)

	return lipgloss.JoinVertical(lipgloss.Top, parts...)
}

// renderTopBar renders the filter indicator and key binding hints.
func (m ModelSelector) renderTopBar() string {
	filterText := fmt.Sprintf("Showing: %s", m.filter.String())
	filterStyle := lipgloss.NewStyle().
		Foreground(lipgloss.Color("#D77757")).
		Bold(true)

	hints := lipgloss.NewStyle().
		Foreground(lipgloss.Color("#9AA0A6")).
		Render("[P] Filter  [Tab] Details  [F] Favorite  [/] Search  [Enter] Select  [Esc] Back")

	return lipgloss.JoinHorizontal(lipgloss.Top,
		filterStyle.Render(filterText),
		"  ",
		hints,
	)
}

// renderSearchInput renders the search text input.
func (m ModelSelector) renderSearchInput() string {
	searchStyle := lipgloss.NewStyle().
		Border(lipgloss.NormalBorder()).
		BorderForeground(lipgloss.Color("#2E2E2E")).
		Padding(0, 1)

	searchLabel := lipgloss.NewStyle().
		Foreground(lipgloss.Color("#9AA0A6")).
		Render("Search:")

	return searchStyle.Render(searchLabel + " " + m.search.View())
}

// renderHelpBar renders the bottom help bar.
func (m ModelSelector) renderHelpBar() string {
	return lipgloss.NewStyle().
		Foreground(lipgloss.Color("#9AA0A6")).
		Render("P: cycle provider filter  |  Tab: toggle details  |  F: toggle favorite  |  /: search  |  Enter: select  |  Esc: back")
}

// detailView renders the detail pane for the currently selected model.
func (m ModelSelector) detailView() string {
	if m.detailModel == nil {
		return ""
	}
	d := m.detailModel

	// Cost estimation: standard usage (100K in + 50K out) per CONTEXT.md
	estCost := (d.Pricing.InputPerMToken * 0.1) + (d.Pricing.OutputPerMToken * 0.05)

	var b strings.Builder
	b.WriteString(fmt.Sprintf("Model: %s\n", d.ID))
	b.WriteString(fmt.Sprintf("Provider: %s\n", d.Provider))
	if d.Variant != nil {
		b.WriteString(fmt.Sprintf("Variant: %s\n", *d.Variant))
	}
	b.WriteString(fmt.Sprintf("Description: %s\n", d.Description))
	b.WriteString(fmt.Sprintf("Context: %d tokens\n", d.ContextLength))
	b.WriteString(fmt.Sprintf("Tokenizer: %s\n", d.Architecture.TokenizerFamily))
	b.WriteString(fmt.Sprintf("Pricing: $%.4f/M in, $%.4f/M out (est. $%.4f/100K+50K)\n",
		d.Pricing.InputPerMToken, d.Pricing.OutputPerMToken, estCost))
	b.WriteString(fmt.Sprintf("Capabilities: %s\n", capabilityString(d.Capabilities)))

	// Favorite status line
	if m.manager != nil {
		isFav := m.manager.IsFavorite(d.ID)
		if isFav {
			b.WriteString(favoriteStarStyle.Render("Favorite: Yes\n"))
		} else {
			b.WriteString("Favorite: No\n")
		}
	}

	return lipgloss.NewStyle().
		Padding(1, 2).
		Border(lipgloss.RoundedBorder()).
		BorderForeground(lipgloss.Color("#D77757")).
		Width(m.width - 6).
		Render(b.String())
}

// refilterList rebuilds the list items based on the current provider filter and search text.
func (m *ModelSelector) refilterList() {
	if m.allModels == nil {
		m.list.SetItems(nil)
		return
	}

	var items []list.Item
	searchText := strings.ToLower(m.search.Value())

	// Load favorites map from manager (best effort — nil manager or error means no favorites)
	var favorites map[string]bool
	if m.manager != nil {
		if data, err := m.manager.LoadRecentModels(); err == nil {
			favorites = data.Favorites
		}
	}

	for _, model := range m.allModels {
		// Provider filter
		if m.filter == filterOpenRouter && model.Provider != "openrouter" {
			continue
		}
		if m.filter == filterZen && model.Provider != "zen" {
			continue
		}

		// Manual search filter
		if searchText != "" {
			searchTarget := strings.ToLower(model.Name + " " + model.ID + " " + model.Description + " " + model.Provider)
			if !strings.Contains(searchTarget, searchText) {
				continue
			}
		}

		items = append(items, ModelItem{
			Model:      model,
			Provider:   model.Provider,
			IsFavorite: favorites[model.ID],
		})
	}

	// Sort by provider then by name
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

// providerBadge returns a short badge string for the provider.
func providerBadge(provider string) string {
	switch provider {
	case "openrouter":
		return "OR"
	case "zen":
		return "ZEN"
	default:
		return strings.ToUpper(provider)
	}
}

// capabilityString returns a human-readable capabilities string.
func capabilityString(c types.CapFlags) string {
	var parts []string
	if c.Tools {
		parts = append(parts, "tools")
	}
	if c.Reasoning {
		parts = append(parts, "reasoning")
	}
	if c.Vision {
		parts = append(parts, "vision")
	}
	if len(parts) == 0 {
		return "basic"
	}
	return strings.Join(parts, ", ")
}
