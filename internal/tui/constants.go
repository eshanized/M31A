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

// Memory management limits to prevent unbounded growth
const (
	// MaxMessages is the maximum number of messages to keep in REPL history.
	// Older messages are pruned when this limit is exceeded.
	MaxMessages = 500

	// MaxTodoItems is the maximum number of TODO items to display in the sidebar.
	MaxTodoItems = 50

	// MaxToolCallCache is the maximum number of tool calls to cache in the message renderer.
	MaxToolCallCache = 200

	// MaxScreenStack is the maximum size of the screen navigation stack.
	MaxScreenStack = 20
)

// Layout dimension constants
const (
	// MinReplWidth is the minimum width for the REPL content area.
	MinReplWidth = 20

	// MinModalWidth is the minimum width for modal dialogs.
	MinModalWidth = 40

	// MinModalHeight is the minimum height for modal dialogs.
	MinModalHeight = 10

	// DefaultSidebarWidth is the default width for the sidebar panel.
	DefaultSidebarWidth = 30
)
