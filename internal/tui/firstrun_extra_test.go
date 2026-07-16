package tui

import (
	"testing"

	"github.com/eshanized/M31A/pkg/types"
)

func TestModelBrowserActiveCat(t *testing.T) {
	var b *modelBrowserState
	if b.activeCat() != nil {
		t.Error("nil receiver should return nil")
	}

	b = &modelBrowserState{categories: nil}
	if b.activeCat() != nil {
		t.Error("empty categories should return nil")
	}

	b = &modelBrowserState{
		categories: []modelCategory{
			{Title: "Free", Models: []types.ModelInfo{{ID: "m1"}}},
			{Title: "Paid", Models: []types.ModelInfo{{ID: "m2"}}},
		},
		catCursor: -1,
	}
	if b.activeCat() != nil {
		t.Error("negative cursor should return nil")
	}

	b.catCursor = 5
	if b.activeCat() != nil {
		t.Error("out of bounds cursor should return nil")
	}

	b.catCursor = 0
	cat := b.activeCat()
	if cat == nil || cat.Title != "Free" {
		t.Error("should return Free category")
	}

	b.catCursor = 1
	cat = b.activeCat()
	if cat == nil || cat.Title != "Paid" {
		t.Error("should return Paid category")
	}
}

func TestModelBrowserSelectedModel(t *testing.T) {
	b := &modelBrowserState{
		categories: []modelCategory{
			{Title: "A", Models: []types.ModelInfo{
				{ID: "m1", Name: "Model 1"},
				{ID: "m2", Name: "Model 2"},
			}},
		},
		catCursor:   0,
		modelCursor: -1,
	}
	if _, ok := b.selectedModel(); ok {
		t.Error("negative cursor should not select")
	}

	b.modelCursor = 5
	if _, ok := b.selectedModel(); ok {
		t.Error("out of bounds cursor should not select")
	}

	b.modelCursor = 0
	m, ok := b.selectedModel()
	if !ok || m.ID != "m1" {
		t.Error("should select m1")
	}

	b.modelCursor = 1
	m, ok = b.selectedModel()
	if !ok || m.ID != "m2" {
		t.Error("should select m2")
	}
}

func TestCategorizeModels(t *testing.T) {
	models := []types.ModelInfo{
		{ID: "free1", Pricing: types.Pricing{InputPerMToken: 0, OutputPerMToken: 0}},
		{ID: "reason1", Pricing: types.Pricing{InputPerMToken: 5}, Capabilities: types.CapFlags{Reasoning: true}},
		{ID: "tool1", Pricing: types.Pricing{InputPerMToken: 3}, Capabilities: types.CapFlags{Tools: true}},
		{ID: "vision1", Pricing: types.Pricing{InputPerMToken: 2}, Capabilities: types.CapFlags{Vision: true}},
		{ID: "other1", Pricing: types.Pricing{InputPerMToken: 5, OutputPerMToken: 10}},
	}

	cats := categorizeModels(models)
	if len(cats) != 5 {
		t.Fatalf("want 5 categories, got %d", len(cats))
	}

	catMap := make(map[string][]types.ModelInfo)
	for _, c := range cats {
		catMap[c.Title] = c.Models
	}

	if len(catMap["Free Models"]) != 1 || catMap["Free Models"][0].ID != "free1" {
		t.Error("free category wrong")
	}
	if len(catMap["Reasoning"]) != 1 || catMap["Reasoning"][0].ID != "reason1" {
		t.Error("reasoning category wrong")
	}
	if len(catMap["Tool-Use"]) != 1 || catMap["Tool-Use"][0].ID != "tool1" {
		t.Error("tool-use category wrong")
	}
	if len(catMap["Vision"]) != 1 || catMap["Vision"][0].ID != "vision1" {
		t.Error("vision category wrong")
	}
	if len(catMap["All Models"]) != 1 || catMap["All Models"][0].ID != "other1" {
		t.Error("other category wrong")
	}
}

func TestCategorizeModelsEmpty(t *testing.T) {
	cats := categorizeModels(nil)
	if len(cats) != 0 {
		t.Errorf("want 0 categories, got %d", len(cats))
	}
}

func TestCategorizeModelsToolsWithReasoning(t *testing.T) {
	// Models with both Tools and Reasoning should go to Reasoning, not Tool-Use
	models := []types.ModelInfo{
		{ID: "r+t", Pricing: types.Pricing{InputPerMToken: 5}, Capabilities: types.CapFlags{Reasoning: true, Tools: true}},
	}
	cats := categorizeModels(models)
	if len(cats) != 1 || cats[0].Title != "Reasoning" {
		t.Error("reasoning+tools should go to Reasoning")
	}
}

func TestFirstRunEffectiveWidth(t *testing.T) {
	fr := &FirstRunModel{width: 0}
	if got := fr.effectiveWidth(); got != WidthFull-sidebarDefaultWidth {
		t.Errorf("width=0: want %d, got %d", WidthFull-sidebarDefaultWidth, got)
	}

	fr.width = 200
	fr.contentWidth = 0
	if got := fr.effectiveWidth(); got != 200-sidebarDefaultWidth {
		t.Errorf("wide: want %d, got %d", 200-sidebarDefaultWidth, got)
	}

	fr.contentWidth = 100
	if got := fr.effectiveWidth(); got != 100 {
		t.Errorf("contentWidth set: want 100, got %d", got)
	}

	fr.width = 60
	fr.contentWidth = 0
	if got := fr.effectiveWidth(); got != 60 {
		t.Errorf("narrow: want 60, got %d", got)
	}
}

func TestFirstRunRebuildSelectedProviders(t *testing.T) {
	fr := &FirstRunModel{
		providers:       []string{"a", "b", "c", "d"},
		providerChecked: map[string]bool{"a": true, "c": true},
	}
	fr.rebuildSelectedProviders()
	if len(fr.selectedProviders) != 2 || fr.selectedProviders[0] != "a" || fr.selectedProviders[1] != "c" {
		t.Errorf("want [a,c], got %v", fr.selectedProviders)
	}
}

func TestFirstRunProviderVisibleCount(t *testing.T) {
	fr := &FirstRunModel{height: 40}
	got := fr.providerVisibleCount()
	if got < 2 {
		t.Errorf("want >= 2, got %d", got)
	}

	fr.height = 5
	got = fr.providerVisibleCount()
	if got < 2 {
		t.Errorf("min 2, got %d", got)
	}
}

func TestFirstRunClampProviderScrollWithVisible(t *testing.T) {
	fr := &FirstRunModel{
		providers:      []string{"a", "b", "c", "d", "e", "f"},
		providerCursor: 5,
		providerScroll: 0,
	}
	fr.clampProviderScrollWithVisible(3)
	if fr.providerScroll != 3 {
		t.Errorf("want scroll=3, got %d", fr.providerScroll)
	}

	fr.providerCursor = 0
	fr.providerScroll = 5
	fr.clampProviderScrollWithVisible(3)
	if fr.providerScroll != 0 {
		t.Errorf("want scroll=0, got %d", fr.providerScroll)
	}

	fr.providers = []string{"a"}
	fr.providerCursor = 0
	fr.providerScroll = 0
	fr.clampProviderScrollWithVisible(10)
	if fr.providerScroll != 0 {
		t.Errorf("want scroll=0 with 1 provider, got %d", fr.providerScroll)
	}

	fr.clampProviderScrollWithVisible(0)
	if fr.providerScroll != 0 {
		t.Errorf("want scroll=0 with visible=0, got %d", fr.providerScroll)
	}
}

func TestPlural(t *testing.T) {
	if plural(1) != "" {
		t.Error("singular should be empty")
	}
	if plural(0) != "s" {
		t.Error("zero should be 's'")
	}
	if plural(5) != "s" {
		t.Error("plural should be 's'")
	}
}

func TestTitleCase(t *testing.T) {
	if titleCase("") != "" {
		t.Error("empty should stay empty")
	}
	if titleCase("hello") != "Hello" {
		t.Error("should uppercase first")
	}
	if titleCase("Hello") != "Hello" {
		t.Error("already upper should stay")
	}
}

func TestLookupProviderInfo(t *testing.T) {
	info := lookupProviderInfo("openrouter")
	if info.Name != "OpenRouter" {
		t.Errorf("want OpenRouter, got %s", info.Name)
	}

	info = lookupProviderInfo("zen")
	if info.Name != "Zen" {
		t.Errorf("want Zen, got %s", info.Name)
	}

	info = lookupProviderInfo("unknown")
	if info.ID != "unknown" || info.Name != "unknown" {
		t.Errorf("unknown should return fallback with ID=Name='unknown', got ID=%q Name=%q", info.ID, info.Name)
	}
}
