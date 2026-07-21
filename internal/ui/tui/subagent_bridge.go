package tui

import (
	"context"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/tools/subagent"
)

// SubagentEventMsg is delivered to AppState.Update whenever the subagent
// manager emits an event. It carries a single subagent.SubagentEvent.
type SubagentEventMsg struct {
	Event subagent.SubagentEvent
}

// subagentListenerCmd returns a tea.Cmd that blocks until the manager's
// event channel yields one event (or the context is cancelled) and forwards
// it as a SubagentEventMsg. Callers re-register the command after handling
// each event so the stream continues — the same pattern used for the
// permission/question listeners.
func subagentListenerCmd(ctx context.Context, ch <-chan subagent.SubagentEvent) tea.Cmd {
	if ch == nil {
		return nil
	}
	return func() tea.Msg {
		select {
		case <-ctx.Done():
			return nil
		case ev, ok := <-ch:
			if !ok {
				return nil
			}
			return SubagentEventMsg{Event: ev}
		}
	}
}
