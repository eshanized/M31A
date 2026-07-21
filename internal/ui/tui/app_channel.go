package tui

import (
	"fmt"
	"log/slog"
	"sync/atomic"
	"time"

	tea "github.com/charmbracelet/bubbletea"
)

const (
	// ChannelCap is the capacity of the workflow→TUI emitter channel.
	// Sized to absorb bursts from 4+ parallel tool executions without
	// dropping events. 512 covers ~64 seconds at 8 events/second.
	ChannelCap = 512

	// maxRetries is the maximum number of send attempts for a single
	// message before it is counted as dropped. Uses fixed backoff.
	maxRetries = 3

	// retryBackoff is the fixed delay between retry attempts.
	// Short enough to avoid stalling the workflow goroutine,
	// long enough to give the TUI a chance to drain.
	retryBackoff = 5 * time.Millisecond

	// maxDrainPerTick is the maximum number of messages drained in a
	// single high-load drain cycle. Keeps the TUI responsive by not
	// starving the render pipeline.
	maxDrainPerTick = 4
)

// DropCounter tracks dropped workflow messages with atomic operations.
// It is resettable for testing and never affects rendering performance.
type DropCounter struct {
	count atomic.Int64
}

// Add increments the drop counter and returns the new total.
func (dc *DropCounter) Add(n int64) int64 {
	return dc.count.Add(n)
}

// Load returns the current drop count.
func (dc *DropCounter) Load() int64 {
	return dc.count.Load()
}

// Reset sets the counter to zero. Intended for testing only.
func (dc *DropCounter) Reset() {
	dc.count.Store(0)
}

// channelEmitter implements workflow.MsgEmitter by sending messages
// into a buffered channel. Used by RunPhaseCmd to relay workflow events
// into the Bubble Tea update loop.
type channelEmitter struct {
	ch    chan tea.Msg
	drops *DropCounter
}

// Emit sends a message into the channel with bounded retry.
// If the channel is full, it retries up to maxRetries times with
// fixed backoff before counting the message as dropped.
func (ce *channelEmitter) Emit(msg any) {
	for attempt := 0; attempt <= maxRetries; attempt++ {
		select {
		case ce.ch <- msg:
			return
		default:
			if attempt < maxRetries {
				time.Sleep(retryBackoff)
			}
		}
	}
	// All retries exhausted — drop the message.
	dropped := ce.drops.Add(1)
	slog.Warn("workflow message dropped: channel full after retries",
		"msg_type", fmt.Sprintf("%T", msg),
		"retries", maxRetries,
		"total_dropped", dropped)
}

// ResetDropCounter resets the global drop counter to zero. For testing.
func ResetDropCounter() {
	globalDropCounter.Reset()
}

// DroppedMessages returns the current global drop count.
func DroppedMessages() int64 {
	return globalDropCounter.Load()
}

// globalDropCounter is the package-level drop counter shared by all
// channelEmitter instances in a session.
var globalDropCounter DropCounter

// newChannelEmitter creates a channelEmitter with the standard capacity
// and the global drop counter.
func newChannelEmitter() *channelEmitter {
	return &channelEmitter{
		ch:    make(chan tea.Msg, ChannelCap),
		drops: &globalDropCounter,
	}
}
