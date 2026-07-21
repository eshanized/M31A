package tui

import (
	"testing"
)

func TestPickerPanelClampScroll(t *testing.T) {
	p := &pickerPanel{cursor: 5, offset: 0}
	p.clampScroll(3)
	if p.offset != 3 {
		t.Errorf("want offset=3, got %d", p.offset)
	}

	p = &pickerPanel{cursor: 0, offset: 5}
	p.clampScroll(3)
	if p.offset != 0 {
		t.Errorf("want offset=0, got %d", p.offset)
	}
}

func TestPhaseModelPickerSetters(t *testing.T) {
	m := &PhaseModelPickerModel{}
	m.SetTheme(testTheme())
	m.SetDimensions(100, 40)
	if m.width != 100 || m.height != 40 {
		t.Error("dimensions not set")
	}
}

func TestPhaseModelPickerVisibleRows(t *testing.T) {
	m := &PhaseModelPickerModel{height: 30}
	got := m.visibleRows()
	if got < 4 {
		t.Errorf("want >= 4, got %d", got)
	}

	m.height = 2
	got = m.visibleRows()
	if got != 4 {
		t.Errorf("min 4, got %d", got)
	}
}

func TestModelSelectorSetters(t *testing.T) {
	ms := &ModelSelector{}
	ms.SetTheme(testTheme())
	ms.SetDimensions(100, 40)
	if ms.width != 100 || ms.height != 40 {
		t.Error("dimensions not set")
	}
}

func TestModelSelectorVisibleRows(t *testing.T) {
	ms := &ModelSelector{height: 30}
	got := ms.visibleRows()
	if got < 4 {
		t.Errorf("want >= 4, got %d", got)
	}

	ms.height = 2
	got = ms.visibleRows()
	if got != 4 {
		t.Errorf("min 4, got %d", got)
	}
}

func TestModelSelectorClampScroll(t *testing.T) {
	ms := &ModelSelector{height: 30, cursor: 5, offset: 0}
	ms.clampScroll()
	if ms.cursor < ms.offset || ms.cursor >= ms.offset+ms.visibleRows() {
		t.Error("cursor should be visible")
	}

	ms = &ModelSelector{height: 30, cursor: 0, offset: 10}
	ms.clampScroll()
	if ms.offset != 0 {
		t.Error("offset should be 0")
	}
}

func TestModelSelectorCycleProvider(t *testing.T) {
	ms := &ModelSelector{
		providers:      []string{"openai", "anthropic"},
		activeProvider: "",
	}
	ms.cycleProvider()
	if ms.activeProvider != "openai" {
		t.Errorf("want openai, got %s", ms.activeProvider)
	}
	ms.cycleProvider()
	if ms.activeProvider != "anthropic" {
		t.Errorf("want anthropic, got %s", ms.activeProvider)
	}
	ms.cycleProvider()
	if ms.activeProvider != "" {
		t.Errorf("want '' (all), got %s", ms.activeProvider)
	}
}
