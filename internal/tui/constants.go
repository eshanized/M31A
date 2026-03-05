package tui

import "time"

// Width thresholds for responsive layout
const (
	// WidthUltraCompact is the minimum width for which only the REPL viewport
	// and input area are shown — no header, no sidebar, no status bar chrome.
	WidthUltraCompact = 40

	// WidthCompact is the minimum width for a compact layout that includes
	// a compact header but no sidebar and no keyboard hints.
	WidthCompact = 60

	// WidthFull is the minimum width for the full layout including sidebar,
	// all keyboard hints, cost display, and all chrome elements.
	WidthFull = 80
)

// SidebarRefreshInterval is the interval at which the sidebar polls for git status updates.
const SidebarRefreshInterval = 5 * time.Second
