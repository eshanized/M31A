package tui

import (
	"fmt"
	"log/slog"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
)

// channelEmitter implements workflow.MsgEmitter by sending messages
// into a buffered channel. Used by RunPhaseCmd to relay workflow events
// into the Bubble Tea update loop.
type channelEmitter struct {
	ch chan tea.Msg
}

// Emit sends a message into the channel. If the channel is full after a
// timeout, the message is dropped with a warning log.
func (ce *channelEmitter) Emit(msg any) {
	select {
	case ce.ch <- msg:
	case <-time.After(types.ChannelSendTimeout * 2):
		slog.Warn("workflow message dropped: channel full",
			"msg_type", fmt.Sprintf("%T", msg))
	}
}
