package tui

import (
	"github.com/eshanized/M31A/internal/tui/layout"
)

// renderRuntimeContent renders the runtime verification screen content.
func (m *AppState) renderRuntimeContent(chrome layout.PageChrome) string {
	if m.runtimeModel == nil {
		return ""
	}

	m.runtimeModel.width = chrome.ContentWidth()
	m.runtimeModel.height = chrome.ContentHeight()

	return m.runtimeModel.renderContent()
}
