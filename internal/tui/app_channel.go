package tui

import (
	"fmt"
	"log/slog"
	"sync"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/types"
)

// channelCloser wraps a chan struct{} with a sync.Once to guarantee
// exactly-once close semantics without a global sync.Map. Each
// channelCloser is allocated per phase in RunPhaseCmd, eliminating
// the unbounded global map that previously tracked close-once state.
type channelCloser struct {
	ch   chan struct{}
	once sync.Once
}

// newChannelCloser creates a new channelCloser wrapping ch.
func newChannelCloser(ch chan struct{}) *channelCloser {
	return &channelCloser{ch: ch}
}

// close closes the underlying channel exactly once. Returns true if
// this call performed the close, false otherwise.
func (cc *channelCloser) close() bool {
	closed := false
	cc.once.Do(func() {
		close(cc.ch)
		closed = true
	})
	return closed
}

// chan returns the underlying channel (read-only for callers that
// need to select on it).
func (cc *channelCloser) chan_() chan struct{} {
	return cc.ch
}

// channelEmitter implements workflow.MsgEmitter by sending messages into a channel.
type channelEmitter struct {
	ch chan tea.Msg
}

func (ce *channelEmitter) Emit(msg tea.Msg) {
	select {
	case ce.ch <- msg:
	case <-time.After(types.ChannelSendTimeout):
		// Channel full after timeout — drop to avoid blocking the engine.
		slog.Warn("workflow message dropped: channel full", "msg_type", fmt.Sprintf("%T", msg))
	}
}