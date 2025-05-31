package tui

import (
	"context"
	"strings"
	"testing"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/charmbracelet/bubbles/list"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/tui/theme"
	"github.com/eshanized/M31A/internal/types"
)

// msMockProvider implements provider.LLMProvider for testing.
type msMockProvider struct {
	name   string
	models []types.ModelInfo
}

func (m *msMockProvider) Name() string                                         { return m.name }
func (m *msMockProvider) FetchModels(ctx context.Context) ([]types.ModelInfo, error) { return m.models, nil }
func (m *msMockProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
	return nil, nil
}
func (m *msMockProvider) EstimateCost(modelID string, usage types.Usage) float64 { return 0 }
func (m *msMockProvider) HealthCheck(ctx context.Context) types.HealthStatus     { return types.HealthStatus{Status: "live"} }
func (m *msMockProvider) GetModel(id string) (*types.ModelInfo, error) {
	for i := range m.models {
		if m.models[i].ID == id {
			return &m.models[i], nil
		}
	}
	return nil, nil
}

func newTestRegistry(t *testing.T) *provider.Registry {
	t.Helper()
	r := provider.NewRegistry()
	r.Register("openrouter", &msMockProvider{
		name: "openrouter",
		models: []types.ModelInfo{
			{ID: "openrouter/model-a", Name: "Model A", Provider: "openrouter", ContextLength: 8192,
				Pricing: types.Pricing{InputPerMToken: 1.0, OutputPerMToken: 3.0}},
			{ID: "openrouter/model-b", Name: "Model B", Provider: "openrouter", ContextLength: 32768,
				Pricing: types.Pricing{InputPerMToken: 2.0, OutputPerMToken: 6.0},
				Capabilities: types.CapFlags{Tools: true, Reasoning: true}},
		},
	})
	r.Register("zen", &msMockProvider{
		name: "zen",
		models: []types.ModelInfo{
			{ID: "zen/model-x", Name: "Model X", Provider: "zen", ContextLength: 16384,
				Pricing: types.Pricing{InputPerMToken: 0.5, OutputPerMToken: 1.5}},
		},
	})
	return r
}

func TestModelSelector_New(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())

	if ms.registry == nil {
		t.Fatal("expected non-nil registry")
	}
	if ms.ready {
		t.Error("expected ready to be false before models loaded")
	}
	if ms.err != "" {
		t.Errorf("expected no error, got %q", ms.err)
	}
}

func TestModelSelector_Init(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())

	cmd := ms.Init()
	if cmd == nil {
		t.Fatal("Init should return a non-nil command")
	}
}

func TestModelSelector_ModelsLoaded(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())

	allModels := []types.ModelInfo{
		{ID: "openrouter/model-a", Name: "Model A", Provider: "openrouter", ContextLength: 8192},
		{ID: "zen/model-x", Name: "Model X", Provider: "zen", ContextLength: 16384},
	}
	updated, _ := ms.Update(modelFetchCompleteMsg{models: allModels})
	ms = updated.(ModelSelector)

	if !ms.ready {
		t.Fatal("expected ready to be true after models loaded")
	}
	if len(ms.allModels) != 2 {
		t.Fatalf("expected 2 models, got %d", len(ms.allModels))
	}
}

func TestModelSelector_SelectModel(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())

	allModels := []types.ModelInfo{
		{ID: "openrouter/model-a", Name: "Model A", Provider: "openrouter", ContextLength: 8192},
		{ID: "openrouter/model-b", Name: "Model B", Provider: "openrouter", ContextLength: 32768},
	}
	updated, _ := ms.Update(modelFetchCompleteMsg{models: allModels})
	ms = updated.(ModelSelector)

	// Set up window size so list is properly sized
	updated, _ = ms.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	ms = updated.(ModelSelector)

	// Navigate down with 'j' (down arrow) to second model
	updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyDown})
	ms = updated.(ModelSelector)

	// Press Enter to select
	_, cmd := ms.Update(tea.KeyMsg{Type: tea.KeyEnter})
	if cmd == nil {
		t.Fatal("expected a command after selecting a model")
	}

	// Execute the command and verify it returns ModelSelectedMsg
	msg := cmd()
	appMsg, ok := msg.(AppMsg)
	if !ok {
		t.Fatalf("expected AppMsg, got %T", msg)
	}
	if appMsg.ModelSelected == nil {
		t.Fatal("expected ModelSelected to be set")
	}
	if appMsg.ModelSelected.Model.ID != "openrouter/model-b" {
		t.Errorf("expected model-b, got %s", appMsg.ModelSelected.Model.ID)
	}
}

func TestModelSelector_Cancel(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())

	allModels := []types.ModelInfo{
		{ID: "openrouter/model-a", Name: "Model A", Provider: "openrouter"},
	}
	updated, _ := ms.Update(modelFetchCompleteMsg{models: allModels})
	ms = updated.(ModelSelector)

	updated, _ = ms.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	ms = updated.(ModelSelector)

	// Press Esc to cancel
	updated, cmd := ms.Update(tea.KeyMsg{Type: tea.KeyEscape})
	ms = updated.(ModelSelector)
	if cmd != nil {
		t.Log("Esc returned a command")
	}

	// The ModelSelector itself doesn't change screen; AppState handles Esc
	// Verify the selector still has models
	if len(ms.allModels) != 1 {
		t.Error("models should still be present after cancel")
	}
}

func TestModelSelector_Search(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())

	allModels := []types.ModelInfo{
		{ID: "openrouter/model-a", Name: "Alpha", Provider: "openrouter"},
		{ID: "openrouter/model-b", Name: "Beta", Provider: "openrouter"},
		{ID: "zen/model-x", Name: "Xray", Provider: "zen"},
	}
	updated, _ := ms.Update(modelFetchCompleteMsg{models: allModels})
	ms = updated.(ModelSelector)

	// Focus search with '/'
	updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'/'}})
	ms = updated.(ModelSelector)
	if !ms.searchFocused {
		t.Fatal("expected search to be focused after '/'")
	}

	// Type "alpha"
	for _, ch := range "alpha" {
		updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{ch}})
		ms = updated.(ModelSelector)
	}

	// Check that the list is filtered
	items := ms.list.Items()
	if len(items) != 1 {
		t.Fatalf("expected 1 item after searching 'alpha', got %d", len(items))
	}
	mi := items[0].(ModelItem)
	if mi.Model.Name != "Alpha" {
		t.Errorf("expected Alpha, got %s", mi.Model.Name)
	}
}

func TestModelSelector_SearchCaseInsensitive(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())

	allModels := []types.ModelInfo{
		{ID: "openrouter/model-a", Name: "Alpha", Provider: "openrouter"},
		{ID: "openrouter/model-b", Name: "Beta", Provider: "openrouter"},
	}
	updated, _ := ms.Update(modelFetchCompleteMsg{models: allModels})
	ms = updated.(ModelSelector)

	// Focus search
	updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'/'}})
	ms = updated.(ModelSelector)

	// Type "ALPHA" in uppercase
	for _, ch := range "ALPHA" {
		updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{ch}})
		ms = updated.(ModelSelector)
	}

	items := ms.list.Items()
	if len(items) != 1 {
		t.Fatalf("expected 1 item for case-insensitive search 'ALPHA', got %d", len(items))
	}
}

func TestModelSelector_SearchClear(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())

	allModels := []types.ModelInfo{
		{ID: "openrouter/model-a", Name: "Alpha", Provider: "openrouter"},
		{ID: "openrouter/model-b", Name: "Beta", Provider: "openrouter"},
	}
	updated, _ := ms.Update(modelFetchCompleteMsg{models: allModels})
	ms = updated.(ModelSelector)

	// Focus and search
	updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'/'}})
	ms = updated.(ModelSelector)
	for _, ch := range "alpha" {
		updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{ch}})
		ms = updated.(ModelSelector)
	}

	// Clear search by setting value to empty and re-filtering
	ms.search.SetValue("")
	ms.refilterList()

	items := ms.list.Items()
	if len(items) != 2 {
		t.Fatalf("expected 2 items after clearing search, got %d", len(items))
	}
}

func TestModelSelector_ProviderFilter(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())

	allModels := []types.ModelInfo{
		{ID: "openrouter/model-a", Name: "Model A", Provider: "openrouter"},
		{ID: "zen/model-x", Name: "Model X", Provider: "zen"},
	}
	updated, _ := ms.Update(modelFetchCompleteMsg{models: allModels})
	ms = updated.(ModelSelector)

	// Press 'p' to cycle to first provider filter
	updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'p'}})
	ms = updated.(ModelSelector)

	// Should be filtered to one provider (openrouter since it comes first in Next() cycle)
	items := ms.list.Items()
	if len(items) < 1 {
		t.Fatalf("expected at least 1 item after provider filter, got %d", len(items))
	}

	// All items should be from the same provider
	providerName := items[0].(ModelItem).Provider
	for _, item := range items {
		mi := item.(ModelItem)
		if mi.Provider != providerName {
			t.Errorf("expected all items to be %s, got %s", providerName, mi.Provider)
		}
	}
}

func TestModelSelector_ProviderFilterCycle(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())

	allModels := []types.ModelInfo{
		{ID: "openrouter/model-a", Name: "Model A", Provider: "openrouter"},
		{ID: "zen/model-x", Name: "Model X", Provider: "zen"},
	}
	updated, _ := ms.Update(modelFetchCompleteMsg{models: allModels})
	ms = updated.(ModelSelector)

	// Start at filterAll
	if ms.filter != filterAll {
		t.Fatalf("expected filterAll, got %v", ms.filter)
	}

	// Press 'p' -> filterOpenRouter
	updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'p'}})
	ms = updated.(ModelSelector)
	if ms.filter != filterOpenRouter {
		t.Errorf("expected filterOpenRouter, got %v", ms.filter)
	}

	// Press 'p' -> filterZen
	updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'p'}})
	ms = updated.(ModelSelector)
	if ms.filter != filterZen {
		t.Errorf("expected filterZen, got %v", ms.filter)
	}

	// Press 'p' -> filterAll (wrap around)
	updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'p'}})
	ms = updated.(ModelSelector)
	if ms.filter != filterAll {
		t.Errorf("expected filterAll after cycling, got %v", ms.filter)
	}
}

func TestModelSelector_Navigation(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())

	allModels := []types.ModelInfo{
		{ID: "openrouter/model-a", Name: "A", Provider: "openrouter"},
		{ID: "openrouter/model-b", Name: "B", Provider: "openrouter"},
		{ID: "zen/model-x", Name: "X", Provider: "zen"},
	}
	updated, _ := ms.Update(modelFetchCompleteMsg{models: allModels})
	ms = updated.(ModelSelector)

	updated, _ = ms.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	ms = updated.(ModelSelector)

	// Navigate down twice
	updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyDown})
	ms = updated.(ModelSelector)
	updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyDown})
	ms = updated.(ModelSelector)

	// Should be on last item
	sel := ms.list.SelectedItem()
	if sel == nil {
		t.Fatal("expected a selected item after navigation")
	}
	mi := sel.(ModelItem)
	if mi.Model.Name != "X" {
		t.Errorf("expected X after navigating down twice, got %s", mi.Model.Name)
	}

	// Navigate up once
	updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyUp})
	ms = updated.(ModelSelector)
	sel = ms.list.SelectedItem()
	if sel == nil {
		t.Fatal("expected a selected item after navigating up")
	}
	mi = sel.(ModelItem)
	if mi.Model.Name != "B" {
		t.Errorf("expected B after navigating up, got %s", mi.Model.Name)
	}
}

func TestModelSelector_View_Loading(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())
	ms.width = 80
	ms.height = 24

	view := ms.View()
	if !strings.Contains(view, "Loading") {
		t.Errorf("expected loading message in view before models loaded, got: %s", view)
	}
}

func TestModelSelector_View_Loaded(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())
	ms.width = 80
	ms.height = 24

	// Size the list first
	updated, _ := ms.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	ms = updated.(ModelSelector)

	allModels := []types.ModelInfo{
		{ID: "openrouter/model-a", Name: "Model A", Provider: "openrouter"},
	}
	updated, _ = ms.Update(modelFetchCompleteMsg{models: allModels})
	ms = updated.(ModelSelector)

	view := ms.View()
	if strings.Contains(view, "Loading") {
		t.Error("expected no loading message after models loaded")
	}
	if !strings.Contains(view, "Model A") {
		t.Errorf("expected model name in view after models loaded, got: %q", view)
	}
}

func TestModelSelector_View_SearchActive(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())
	ms.width = 80
	ms.height = 24

	allModels := []types.ModelInfo{
		{ID: "openrouter/model-a", Name: "Model A", Provider: "openrouter"},
	}
	updated, _ := ms.Update(modelFetchCompleteMsg{models: allModels})
	ms = updated.(ModelSelector)

	// Focus search
	updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyRunes, Runes: []rune{'/'}})
	ms = updated.(ModelSelector)

	if !ms.searchFocused {
		t.Error("expected search to be focused")
	}
	// The search input should be visible
	view := ms.View()
	if !strings.Contains(view, "Search") {
		t.Error("expected search label in view when search is active")
	}
}

func TestModelSelector_EmptyModels(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())
	ms.width = 80
	ms.height = 24

	updated, _ := ms.Update(modelFetchCompleteMsg{models: []types.ModelInfo{}})
	ms = updated.(ModelSelector)

	view := ms.View()
	// Should handle empty models gracefully - list shows "No items found." or similar
	if strings.Contains(view, "Loading") {
		t.Error("should not show loading after models loaded (even if empty)")
	}
}

func TestModelSelector_ErrorState(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())
	ms.width = 80
	ms.height = 24

	updated, _ := ms.Update(modelFetchCompleteMsg{err: "network error"})
	ms = updated.(ModelSelector)

	if ms.err == "" {
		t.Fatal("expected error to be set")
	}

	view := ms.View()
	if !strings.Contains(view, "network error") {
		t.Error("expected error message in view")
	}
}

func TestModelSelector_DetailView(t *testing.T) {
	r := newTestRegistry(t)
	ms := NewModelSelector(r, nil, theme.Dark())
	ms.width = 80
	ms.height = 24

	// Size the list first
	updated, _ := ms.Update(tea.WindowSizeMsg{Width: 80, Height: 24})
	ms = updated.(ModelSelector)

	allModels := []types.ModelInfo{
		{ID: "openrouter/model-a", Name: "Model A", Provider: "openrouter",
			Description: "A test model", ContextLength: 8192,
			Pricing: types.Pricing{InputPerMToken: 1.0, OutputPerMToken: 3.0}},
	}
	updated, _ = ms.Update(modelFetchCompleteMsg{models: allModels})
	ms = updated.(ModelSelector)

	// Toggle detail view with Tab
	updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyTab})
	ms = updated.(ModelSelector)

	if !ms.showDetail {
		t.Error("expected detail view to be shown after Tab")
	}
	if ms.detailModel == nil {
		t.Error("expected detailModel to be set after Tab")
	}

	view := ms.View()
	if !strings.Contains(view, "Model A") {
		t.Error("expected model name in detail view")
	}

	// Toggle off
	updated, _ = ms.Update(tea.KeyMsg{Type: tea.KeyTab})
	ms = updated.(ModelSelector)
	if ms.showDetail {
		t.Error("expected detail view to be hidden after second Tab")
	}
}

// Ensure ModelSelector implements tea.Model interface (value receiver).
var _ tea.Model = ModelSelector{}

// Verify list items implement list.DefaultItem.
var _ list.DefaultItem = ModelItem{}
