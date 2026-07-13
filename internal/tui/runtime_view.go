package tui

import (
	"github.com/eshanized/M31A/internal/tui/layout"
)

// renderRuntimeContent renders the runtime verification screen content.
func (m *AppState) renderRuntimeContent(chrome layout.PageChrome) string {
	// Ensure RuntimeCheck is registered with router
	if m.runtimeModel == nil {
		cw, ch := m.contentDimensions()
		m.runtimeModel = NewRuntimeModel(m.themeManager.Current(), cw, ch)
		m.router.Register(ScreenRuntimeCheck, m.runtimeModel)
	}
	m.runtimeModel.SetDimensions(chrome.ContentWidth(), chrome.ContentHeight())
	return m.router.View()
}
