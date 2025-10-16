package tui

import (
	"fmt"
	"log/slog"
	"sync"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
)

// channelCloser wraps a chan struct{} with sync.Once to guarantee
// exactly-once close semantics without a global sync.Map.
type channelCloser struct {
	ch   chan struct{}
	once sync.Once
}

// newChannelCloser creates a new channelCloser wrapping ch.
func newChannelCloser(ch chan struct{}) *channelCloser {
	return &channelCloser{ch: ch}
}

// close closes the underlying channel exactly once.
// Returns true if this call performed the close.
func (cc *channelCloser) close() bool {
	closed := false
	cc.once.Do(func() {
		close(cc.ch)
		closed = true
	})
	return closed
}

// chan_ returns the underlying channel (read-only reference).
func (cc *channelCloser) chan_() chan struct{} {
	return cc.ch
}

// channelEmitter implements workflow.MsgEmitter by sending messages
// into a buffered channel. Used by RunPhaseCmd to relay workflow events
// into the Bubble Tea update loop.
type channelEmitter struct {
	ch chan tea.Msg
}

// Emit sends a message into the channel. If the channel is full after a
// short timeout, the message is dropped with a warning log.
func (ce *channelEmitter) Emit(msg tea.Msg) {
	select {
	case ce.ch <- msg:
	case <-time.After(types.ChannelSendTimeout):
		slog.Warn("workflow message dropped: channel full",
			"msg_type", fmt.Sprintf("%T", msg))
	}
}
