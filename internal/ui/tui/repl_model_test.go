package tui

import (
	"testing"

	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

func TestReplView_Empty_RendersWelcome(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := NewReplModel(tm.Current(), "test")
	m.SetDimensions(80, 24)
	view := m.View()
	if view == "" {
		t.Error("Empty REPL should render welcome view")
	}
}

func TestReplView_WithMessages_RendersMessages(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := NewReplModel(tm.Current(), "test")
	m.SetDimensions(80, 24)
	m.AddMessage(MakeUserMsg("hello"))
	m.AddMessage(MakeAssistantMsg("hi there"))
	view := m.View()
	if view == "" {
		t.Error("REPL with messages should render non-empty view")
	}
}

func TestReplView_ContentDimensions(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := NewReplModel(tm.Current(), "test")
	m.SetDimensions(80, 24)
	content := m.ViewContent(20, 80)
	if content == "" {
		t.Error("ViewContent should return non-empty content")
	}
}

func TestReplModel_SetDimensions_UpdatesViewport(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := NewReplModel(tm.Current(), "test")
	m.SetDimensions(100, 30)
	if m.width != 100 {
		t.Errorf("width=%d, want 100", m.width)
	}
	if m.height != 30 {
		t.Errorf("height=%d, want 30", m.height)
	}
}

func TestReplModel_SetTheme_UpdatesTheme(t *testing.T) {
	tm := theme.NewManager(theme.ModeDark)
	m := NewReplModel(tm.Current(), "test")
	newTheme := theme.M31A()
	m.SetTheme(newTheme)
	if m.theme.Brand != newTheme.Brand {
		t.Error("Theme brand should be updated")
	}
}
