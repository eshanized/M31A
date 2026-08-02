package main

import (
	"fmt"
	"io/ioutil"
	"strings"
)

func main() {
	path := "internal/ui/tui/app_nav.go"
	content, err := ioutil.ReadFile(path)
	if err != nil {
		fmt.Println("Error reading file:", err)
		return
	}
	str := string(content)

	search := `func (m *AppState) contentDimensions() (w, h int) {
	w = m.width
	h = m.height - layout.ChromeHeight
	if h < 1 {
		h = 1
	}
	if m.sidebarModel != nil && m.sidebarModel.IsVisible() && layout.ShowSidebar(m.width) {
		sidebarW := m.sidebarModel.GetWidth()
		if sidebarW > 0 && sidebarW < w {
			w -= sidebarW
		}
	}
	if w < 1 {
		w = 1
	}
	slog.Debug("contentDimensions", "terminalW", m.width, "terminalH", m.height, "contentW", w, "contentH", h, "sidebarVisible", m.sidebarModel != nil && m.sidebarModel.IsVisible(), "showSidebar", layout.ShowSidebar(m.width))
	return w, h
}`
	replace := `func (m *AppState) contentDimensions() (w, h int) {
	w = m.width
	if m.zenMode {
		h = m.height
	} else {
		h = m.height - layout.ChromeHeight
	}
	if h < 1 {
		h = 1
	}
	if !m.zenMode && m.sidebarModel != nil && m.sidebarModel.IsVisible() && layout.ShowSidebar(m.width) {
		sidebarW := m.sidebarModel.GetWidth()
		if sidebarW > 0 && sidebarW < w {
			w -= sidebarW
		}
	}
	if w < 1 {
		w = 1
	}
	slog.Debug("contentDimensions", "terminalW", m.width, "terminalH", m.height, "contentW", w, "contentH", h, "sidebarVisible", m.sidebarModel != nil && m.sidebarModel.IsVisible(), "showSidebar", layout.ShowSidebar(m.width), "zenMode", m.zenMode)
	return w, h
}`

	out := strings.Replace(str, search, replace, 1)

	err = ioutil.WriteFile(path, []byte(out), 0644)
	if err != nil {
		fmt.Println("Error writing file:", err)
		return
	}
	fmt.Println("Done")
}
