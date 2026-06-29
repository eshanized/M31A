package decision

import (
	"sync"
	"time"
)

const (
	defaultCapacity = 256
	ringCapacity    = 512
	flushThreshold  = 10
)

// Logger provides non-blocking decision logging with a goroutine-backed channel
// and ring buffer overflow.
type Logger struct {
	ch        chan DecisionReceipt
	ring      []DecisionReceipt
	ringMu    sync.Mutex
	ringHead  int
	ringLen   int
	capacity  int
	flushed   []DecisionReceipt
	flushMu   sync.Mutex
	flushOpMu sync.Mutex    // serializes Flush() callers to prevent deadlock
	closeCh   chan struct{} // closed by Close() to signal shutdown
	done      chan struct{} // closed by drain() when it has fully exited
	flushCh   chan chan []DecisionReceipt
}

// NewLogger creates a new decision logger with the given capacity.
// If capacity <= 0, defaults to 256.
func NewLogger(capacity int) *Logger {
	if capacity <= 0 {
		capacity = defaultCapacity
	}
	l := &Logger{
		ch:       make(chan DecisionReceipt, capacity),
		ring:     make([]DecisionReceipt, ringCapacity),
		capacity: capacity,
		closeCh:  make(chan struct{}),
		done:     make(chan struct{}),
		flushCh:  make(chan chan []DecisionReceipt, 1),
	}
	go l.drain()
	return l
}

// Close signals the logger to shut down, flushes remaining decisions,
// and waits for the drain goroutine to exit. Safe to call multiple times.
func (l *Logger) Close() {
	select {
	case <-l.closeCh:
		<-l.done
		return
	default:
		close(l.closeCh)
	}
	close(l.ch)
	<-l.done
}

// Log records a decision non-blocking. If the channel is full, writes to ring buffer.
func (l *Logger) Log(r DecisionReceipt) {
	if r.Timestamp.IsZero() {
		r.Timestamp = time.Now()
	}
	select {
	case <-l.closeCh:
		return
	default:
	}
	select {
	case l.ch <- r:
	case <-l.closeCh:
		return
	default:
		// Channel full — overflow to ring buffer
		l.ringMu.Lock()
		l.ring[l.ringHead] = r
		l.ringHead = (l.ringHead + 1) % ringCapacity
		if l.ringLen < ringCapacity {
			l.ringLen++
		}
		l.ringMu.Unlock()
	}
}

// drain reads from channel and appends to flushed list.
func (l *Logger) drain() {
	defer close(l.done)
	for {
		select {
		case r, ok := <-l.ch:
			if !ok {
				// Channel closed by Close() — flush ring buffer and exit
				l.flushRing()
				l.drainPendingFlushes()
				return
			}
			l.flushMu.Lock()
			l.flushed = append(l.flushed, r)
			l.flushMu.Unlock()
			// Check if flush requested
			select {
			case reply := <-l.flushCh:
				l.processFlush(reply)
			default:
			}
		case <-l.closeCh:
			// Close requested — drain remaining channel items, flush ring, exit
			l.drainRemainingCh()
			l.flushRing()
			l.drainPendingFlushes()
			return
		}
	}
}

// drainRemainingCh drains any items still buffered in the channel.
func (l *Logger) drainRemainingCh() {
	for {
		select {
		case r, ok := <-l.ch:
			if !ok {
				return
			}
			l.flushMu.Lock()
			l.flushed = append(l.flushed, r)
			l.flushMu.Unlock()
		default:
			return
		}
	}
}

// flushRing moves all ring buffer entries into the flushed slice.
func (l *Logger) flushRing() {
	l.flushMu.Lock()
	l.ringMu.Lock()
	if l.ringLen > 0 {
		start := (l.ringHead - l.ringLen + ringCapacity) % ringCapacity
		for i := 0; i < l.ringLen; i++ {
			l.flushed = append(l.flushed, l.ring[(start+i)%ringCapacity])
		}
		l.ringLen = 0
		l.ringHead = 0
	}
	l.ringMu.Unlock()
	l.flushMu.Unlock()
}

// processFlush collects all buffered data and sends it on the reply channel.
func (l *Logger) processFlush(reply chan []DecisionReceipt) {
	l.flushMu.Lock()
	l.ringMu.Lock()
	all := make([]DecisionReceipt, 0, len(l.flushed)+l.ringLen)
	all = append(all, l.flushed...)
	if l.ringLen > 0 {
		start := (l.ringHead - l.ringLen + ringCapacity) % ringCapacity
		for i := 0; i < l.ringLen; i++ {
			all = append(all, l.ring[(start+i)%ringCapacity])
		}
	}
	l.ringLen = 0
	l.ringHead = 0
	l.flushed = make([]DecisionReceipt, 0, cap(l.flushed))
	l.ringMu.Unlock()
	l.flushMu.Unlock()
	reply <- all
}

// drainPendingFlushes processes any flush requests that arrived during shutdown.
func (l *Logger) drainPendingFlushes() {
	for {
		select {
		case reply := <-l.flushCh:
			l.processFlush(reply)
		default:
			return
		}
	}
}

// Flush synchronously returns all logged decisions and resets the buffer.
// Safe for concurrent callers.
func (l *Logger) Flush() []DecisionReceipt {
	l.flushOpMu.Lock()
	defer l.flushOpMu.Unlock()

	reply := make(chan []DecisionReceipt, 1)
	select {
	case l.flushCh <- reply:
		return <-reply
	case <-l.closeCh:
		// Logger closing, collect directly
		l.flushMu.Lock()
		defer l.flushMu.Unlock()
		l.ringMu.Lock()
		defer l.ringMu.Unlock()
		all := make([]DecisionReceipt, 0, len(l.flushed)+l.ringLen)
		all = append(all, l.flushed...)
		if l.ringLen > 0 {
			start := (l.ringHead - l.ringLen + ringCapacity) % ringCapacity
			for i := 0; i < l.ringLen; i++ {
				all = append(all, l.ring[(start+i)%ringCapacity])
			}
		}
		l.ringLen = 0
		l.ringHead = 0
		l.flushed = make([]DecisionReceipt, 0, cap(l.flushed))
		return all
	}
}

// Snapshot returns a copy of all buffered decisions without flushing.
func (l *Logger) Snapshot() []DecisionReceipt {
	l.flushMu.Lock()
	l.ringMu.Lock()
	defer l.flushMu.Unlock()
	defer l.ringMu.Unlock()

	result := make([]DecisionReceipt, 0, len(l.flushed)+l.ringLen)
	result = append(result, l.flushed...)
	if l.ringLen > 0 {
		start := (l.ringHead - l.ringLen + ringCapacity) % ringCapacity
		for i := 0; i < l.ringLen; i++ {
			result = append(result, l.ring[(start+i)%ringCapacity])
		}
	}
	return result
}

// Len returns the approximate number of buffered decisions.
func (l *Logger) Len() int {
	l.flushMu.Lock()
	l.ringMu.Lock()
	defer l.flushMu.Unlock()
	defer l.ringMu.Unlock()
	return len(l.flushed) + l.ringLen
}
