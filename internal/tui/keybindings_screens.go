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

	// REPL bindings
	r.Register(CtxREPL, "ctrl+x b", "Toggle sidebar", emit("toggle_sidebar"))
	r.Register(CtxREPL, "ctrl+x n", "New session", emit("new_session"))
	r.Register(CtxREPL, "ctrl+x r", "Session list", emit("session_list"))
	r.Register(CtxREPL, "ctrl+x m", "Select model", emit("cycle_model"))
	r.Register(CtxREPL, "ctrl+x t", "Toggle theme", emit("toggle_theme"))
	r.Register(CtxREPL, "ctrl+x [", "Prev model", emit("cycle_model_backward"))
	r.Register(CtxREPL, "ctrl+x ]", "Next model", emit("cycle_model_forward"))
}
