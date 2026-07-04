package coordinator

import (
	"context"
	"sync"
	"time"
)

// DemandType distinguishes explicit runs requests from advisory wake signals.
type DemandType int

const (
	DemandRun  DemandType = iota // explicit drain — caller wants results
	DemandWake                   // advisory — new durable work may be available
)

// Coordinator manages concurrent execution of sessions identified by Key.
// At most one drain is active per key; additional demands are coalesced
// into a single pending rerun.
type Coordinator[Key comparable] struct {
	mu      sync.Mutex
	entries map[Key]*entry
}

type entry struct {
	running  bool
	pending  DemandType
	done     chan struct{}
	stopping bool
	cancel   context.CancelFunc
}

// New creates a Coordinator.
func New[Key comparable]() *Coordinator[Key] {
	return &Coordinator[Key]{
		entries: make(map[Key]*entry),
	}
}

// Run starts an explicit drain for the given key. If a drain is already
// active, the demand is coalesced. Returns a context that is cancelled
// when the drain completes or is interrupted.
func (c *Coordinator[Key]) Run(key Key) context.Context {
	c.mu.Lock()
	e := c.getOrCreate(key)

	if e.running {
		e.pending = DemandRun
		c.mu.Unlock()
		return c.awaitDone(e)
	}

	e.running = true
	e.pending = 0
	e.stopping = false
	ctx, cancel := context.WithCancel(context.Background())
	e.cancel = cancel
	e.done = make(chan struct{})
	c.mu.Unlock()

	return ctx
}

// Wake sends an advisory signal that new work may be available.
// If no drain is active, this is a no-op (the caller should use Run).
func (c *Coordinator[Key]) Wake(key Key) {
	c.mu.Lock()
	defer c.mu.Unlock()

	e, ok := c.entries[key]
	if !ok || !e.running {
		return
	}

	if e.pending < DemandWake {
		e.pending = DemandWake
	}
}

// Interrupt stops the current drain for the given key.
func (c *Coordinator[Key]) Interrupt(key Key) {
	c.mu.Lock()
	defer c.mu.Unlock()

	e, ok := c.entries[key]
	if !ok || !e.running {
		return
	}

	e.stopping = true
	if e.cancel != nil {
		e.cancel()
	}
}

// AwaitIdle blocks until no drain is active for the given key.
func (c *Coordinator[Key]) AwaitIdle(key Key) {
	c.mu.Lock()
	e, ok := c.entries[key]
	if !ok || !e.running {
		c.mu.Unlock()
		return
	}
	done := e.done
	c.mu.Unlock()

	if done != nil {
		<-done
	}
}

// Complete signals that a drain has finished. Call this when the session's
// work is done so the coordinator can start a pending rerun.
func (c *Coordinator[Key]) Complete(key Key) DemandType {
	c.mu.Lock()
	defer c.mu.Unlock()

	e, ok := c.entries[key]
	if !ok || !e.running {
		return 0
	}

	e.running = false
	if e.done != nil {
		close(e.done)
	}

	pending := e.pending
	e.pending = 0

	// Clean up the entry if there's no pending demand to avoid unbounded map growth.
	if pending == 0 {
		delete(c.entries, key)
	}

	return pending
}

// HasPending returns true if there is a pending demand for the given key.
func (c *Coordinator[Key]) HasPending(key Key) bool {
	c.mu.Lock()
	defer c.mu.Unlock()

	e, ok := c.entries[key]
	return ok && e.pending > 0
}

func (c *Coordinator[Key]) getOrCreate(key Key) *entry {
	e, ok := c.entries[key]
	if !ok {
		e = &entry{}
		c.entries[key] = e
	}
	return e
}

func (c *Coordinator[Key]) awaitDone(e *entry) context.Context {
	ctx, cancel := context.WithCancel(context.Background())
	if e.done != nil {
		go func() {
			select {
			case <-e.done:
			case <-ctx.Done():
			case <-time.After(5 * time.Minute):
				// Safety net: if Complete() is never called, prevent goroutine leak
			}
			cancel()
		}()
	} else {
		cancel()
	}
	return ctx
}
