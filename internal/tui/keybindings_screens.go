package tui

import tea "github.com/charmbracelet/bubbletea"

// RegisterDefaultBindings registers all application-level key bindings.
// These bindings use KeyActionMsg so that AppState.handleKeyAction
// dispatches them without creating circular references.
func (r *KeyRegistry) RegisterDefaultBindings() {
	emit := func(action string) KeyAction {
		return func() tea.Cmd {
			return func() tea.Msg {
				return KeyActionMsg{Action: action}
			}
		}
	}

	// Global bindings (available from any screen)
	r.Register(CtxGlobal, "ctrl+x s", "Open settings", emit("open_settings"))
	r.Register(CtxGlobal, "ctrl+x h", "Help", emit("open_help"))
	r.Register(CtxGlobal, "ctrl+x l", "Ledger", emit("open_ledger"))
	r.Register(CtxGlobal, "ctrl+x k", "Rollback", emit("open_rollback"))
	r.Register(CtxGlobal, "ctrl+x d", "Dashboard", emit("open_dashboard"))
	r.Register(CtxGlobal, "ctrl+x p", "Theme picker", emit("open_themes"))
	r.Register(CtxGlobal, "ctrl+x !", "Notifications", emit("open_notifications"))
	r.Register(CtxGlobal, "ctrl+x f", "File explorer", emit("open_files"))
	r.Register(CtxGlobal, "ctrl+x a", "Subagents panel", emit("toggle_subagents"))
	r.Register(CtxGlobal, "ctrl+x c", "Config viewer", emit("open_config"))
	r.Register(CtxGlobal, "ctrl+x i", "Session detail", emit("open_session_detail"))
	r.Register(CtxGlobal, "ctrl+x o", "Tool output", emit("open_tool_detail"))

	// REPL bindings
	r.Register(CtxREPL, "ctrl+x b", "Toggle sidebar", emit("toggle_sidebar"))
	r.Register(CtxREPL, "ctrl+x n", "New session", emit("new_session"))
	r.Register(CtxREPL, "ctrl+x r", "Session list", emit("session_list"))
	r.Register(CtxREPL, "ctrl+x m", "Select model", emit("cycle_model"))
	r.Register(CtxREPL, "ctrl+x t", "Toggle theme", emit("toggle_theme"))
	r.Register(CtxREPL, "ctrl+x [", "Widen sidebar", emit("sidebar_wider"))
	r.Register(CtxREPL, "ctrl+x ]", "Narrow sidebar", emit("sidebar_narrower"))
}
