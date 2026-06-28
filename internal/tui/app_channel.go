package tui

import (
	"fmt"
	"log/slog"
	"sync/atomic"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
)

// droppedMsgCount tracks how many workflow messages were dropped due to a full channel.
var droppedMsgCount atomic.Int64

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
	case <-time.After(types.ChannelSendTimeout):
		dropped := droppedMsgCount.Add(1)
		slog.Warn("workflow message dropped: channel full",
			"msg_type", fmt.Sprintf("%T", msg),
			"total_dropped", dropped)
	}
}
