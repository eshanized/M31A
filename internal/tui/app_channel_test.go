package tui

import (
	"context"
	"runtime"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
)

// newTestEmitter creates an emitter with its own isolated drop counter.
func newTestEmitter() *channelEmitter {
	return &channelEmitter{
		ch:    make(chan tea.Msg, ChannelCap),
		drops: &DropCounter{},
	}
}

// ─── Channel Capacity ────────────────────────────────────────────────────────

func TestChannelCap(t *testing.T) {
	t.Parallel()
	if ChannelCap != 512 {
		t.Errorf("ChannelCap = %d, want 512", ChannelCap)
	}
}

// ─── DropCounter ─────────────────────────────────────────────────────────────

func TestDropCounter_AddAndLoad(t *testing.T) {
	t.Parallel()
	var dc DropCounter
	if dc.Load() != 0 {
		t.Errorf("initial load = %d, want 0", dc.Load())
	}
	dc.Add(1)
	dc.Add(2)
	if dc.Load() != 3 {
		t.Errorf("after Add(1)+Add(2): load = %d, want 3", dc.Load())
	}
}

func TestDropCounter_Reset(t *testing.T) {
	t.Parallel()
	var dc DropCounter
	dc.Add(5)
	dc.Reset()
	if dc.Load() != 0 {
		t.Errorf("after Reset: load = %d, want 0", dc.Load())
	}
}

func TestDropCounter_Concurrent(t *testing.T) {
	t.Parallel()
	var dc DropCounter
	const goroutines = 100
	const perGoroutine = 100

	var wg sync.WaitGroup
	wg.Add(goroutines)
	for i := 0; i < goroutines; i++ {
		go func() {
			defer wg.Done()
			for j := 0; j < perGoroutine; j++ {
				dc.Add(1)
			}
		}()
	}
	wg.Wait()

	expected := int64(goroutines * perGoroutine)
	if dc.Load() != expected {
		t.Errorf("concurrent load = %d, want %d", dc.Load(), expected)
	}
}

// ─── Emit Success ────────────────────────────────────────────────────────────

func TestChannelEmitter_EmitSuccess(t *testing.T) {
	t.Parallel()
	emitter := newTestEmitter()
	msg := tea.Msg("test-message")
	emitter.Emit(msg)

	select {
	case got := <-emitter.ch:
		if got != msg {
			t.Errorf("received %v, want %v", got, msg)
		}
	case <-time.After(100 * time.Millisecond):
		t.Fatal("timed out waiting for message")
	}
}

// ─── Emit Ordering ───────────────────────────────────────────────────────────

func TestChannelEmitter_Ordering(t *testing.T) {
	t.Parallel()
	emitter := newTestEmitter()

	const n = 100
	for i := 0; i < n; i++ {
		emitter.Emit(i)
	}

	for i := 0; i < n; i++ {
		select {
		case got := <-emitter.ch:
			if got != i {
				t.Errorf("message %d: got %v, want %d", i, got, i)
			}
		case <-time.After(100 * time.Millisecond):
			t.Fatalf("timed out at message %d", i)
		}
	}
}

// ─── Retry Exhaustion and Drop Counting ──────────────────────────────────────

func TestChannelEmitter_RetryExhaustion(t *testing.T) {
	t.Parallel()
	emitter := newTestEmitter()

	// Fill the channel completely.
	for i := 0; i < ChannelCap; i++ {
		emitter.ch <- i
	}

	// This Emit should retry and then drop.
	emitter.Emit("should-drop")

	if got := emitter.drops.Load(); got != 1 {
		t.Errorf("dropped = %d, want 1", got)
	}

	// Drain one message to make room and verify the dropped message is gone.
	<-emitter.ch
	select {
	case msg := <-emitter.ch:
		if msg == "should-drop" {
			t.Error("dropped message found in channel")
		}
	case <-time.After(100 * time.Millisecond):
		// OK — channel may be empty.
	}
}

func TestChannelEmitter_MultipleDrops(t *testing.T) {
	t.Parallel()
	emitter := newTestEmitter()

	// Fill the channel.
	for i := 0; i < ChannelCap; i++ {
		emitter.ch <- i
	}

	// Try to emit 10 messages — all should be dropped.
	for i := 0; i < 10; i++ {
		emitter.Emit(i)
	}

	if got := emitter.drops.Load(); got != 10 {
		t.Errorf("dropped = %d, want 10", got)
	}
}

// ─── Queue Saturation ────────────────────────────────────────────────────────

func TestChannelEmitter_QueueSaturation(t *testing.T) {
	t.Parallel()
	emitter := newTestEmitter()

	for i := 0; i < ChannelCap+100; i++ {
		emitter.Emit(i)
	}

	received := 0
drainLoop:
	for {
		select {
		case <-emitter.ch:
			received++
		default:
			break drainLoop
		}
	}

	if received != ChannelCap {
		t.Errorf("received = %d, want %d (channel capacity)", received, ChannelCap)
	}
	if got := emitter.drops.Load(); got != 100 {
		t.Errorf("dropped = %d, want 100", got)
	}
}

// ─── Cancellation ────────────────────────────────────────────────────────────

func TestDrainEmitterCmd_Cancellation(t *testing.T) {
	t.Parallel()
	ctx, cancel := context.WithCancel(context.Background())
	ch := make(chan tea.Msg, 16)

	m := &AppState{
		emitterCh:   ch,
		shutdownCtx: ctx,
	}

	cmd := m.drainEmitterCmd()
	if cmd == nil {
		t.Fatal("drainEmitterCmd returned nil")
	}

	cancel()

	msg := cmd()
	if msg != nil {
		t.Errorf("expected nil after cancellation, got %v", msg)
	}
}

func TestDrainEmitterCmd_ReadsMessage(t *testing.T) {
	t.Parallel()
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	ch := make(chan tea.Msg, 16)
	ch <- "hello"

	m := &AppState{
		emitterCh:   ch,
		shutdownCtx: ctx,
	}

	cmd := m.drainEmitterCmd()
	msg := cmd()
	if msg != "hello" {
		t.Errorf("got %v, want 'hello'", msg)
	}
}

// ─── Bounded Draining ────────────────────────────────────────────────────────

func TestDrainMultipleCmd_SingleMessage(t *testing.T) {
	t.Parallel()
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	ch := make(chan tea.Msg, 16)
	ch <- "only-one"

	m := &AppState{
		emitterCh:   ch,
		shutdownCtx: ctx,
	}

	cmd := m.drainMultipleCmd()
	msg := cmd()
	if msg == nil {
		t.Fatal("got nil")
	}
	if msg != "only-one" {
		t.Errorf("got %v, want 'only-one'", msg)
	}
}

func TestDrainMultipleCmd_BatchMessages(t *testing.T) {
	t.Parallel()
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	ch := make(chan tea.Msg, 16)
	for i := 0; i < 4; i++ {
		ch <- i
	}

	m := &AppState{
		emitterCh:   ch,
		shutdownCtx: ctx,
	}

	cmd := m.drainMultipleCmd()
	msg := cmd()
	if msg == nil {
		t.Fatal("got nil")
	}
	batch, ok := msg.(DrainBatchMsg)
	if !ok {
		t.Fatalf("expected DrainBatchMsg, got %T", msg)
	}
	if len(batch.Messages) != 4 {
		t.Errorf("batch size = %d, want 4", len(batch.Messages))
	}
	for i, m := range batch.Messages {
		if m != i {
			t.Errorf("batch[%d] = %v, want %d", i, m, i)
		}
	}
}

func TestDrainMultipleCmd_CapsAtMax(t *testing.T) {
	t.Parallel()
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	ch := make(chan tea.Msg, 16)
	for i := 0; i < maxDrainPerTick+5; i++ {
		ch <- i
	}

	m := &AppState{
		emitterCh:   ch,
		shutdownCtx: ctx,
	}

	cmd := m.drainMultipleCmd()
	msg := cmd()
	batch, ok := msg.(DrainBatchMsg)
	if !ok {
		t.Fatalf("expected DrainBatchMsg, got %T", msg)
	}
	if len(batch.Messages) != maxDrainPerTick {
		t.Errorf("batch size = %d, want %d", len(batch.Messages), maxDrainPerTick)
	}
	remaining := len(ch)
	if remaining != 5 {
		t.Errorf("remaining in channel = %d, want 5", remaining)
	}
}

func TestDrainMultipleCmd_Cancellation(t *testing.T) {
	t.Parallel()
	ctx, cancel := context.WithCancel(context.Background())
	ch := make(chan tea.Msg, 16)

	m := &AppState{
		emitterCh:   ch,
		shutdownCtx: ctx,
	}

	cmd := m.drainMultipleCmd()
	cancel()
	msg := cmd()
	if msg != nil {
		t.Errorf("expected nil after cancellation, got %v", msg)
	}
}

func TestDrainMultipleCmd_NilChannel(t *testing.T) {
	t.Parallel()
	m := &AppState{
		emitterCh:   nil,
		shutdownCtx: context.Background(),
	}
	cmd := m.drainMultipleCmd()
	if cmd != nil {
		t.Error("expected nil for nil channel")
	}
}

// ─── Adaptive Draining ───────────────────────────────────────────────────────

func TestDrainAdaptiveCmd_LowLoad(t *testing.T) {
	t.Parallel()
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	ch := make(chan tea.Msg, ChannelCap)
	ch <- "low-load-msg"

	m := &AppState{
		emitterCh:   ch,
		shutdownCtx: ctx,
	}

	cmd := m.drainAdaptiveCmd()
	msg := cmd()
	if msg != "low-load-msg" {
		t.Errorf("got %v, want 'low-load-msg'", msg)
	}
}

func TestDrainAdaptiveCmd_HighLoad(t *testing.T) {
	t.Parallel()
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	ch := make(chan tea.Msg, ChannelCap)
	for i := 0; i < ChannelCap/4+1; i++ {
		ch <- i
	}

	m := &AppState{
		emitterCh:   ch,
		shutdownCtx: ctx,
	}

	cmd := m.drainAdaptiveCmd()
	msg := cmd()
	if msg == nil {
		t.Fatal("got nil")
	}
	_, ok := msg.(DrainBatchMsg)
	if !ok {
		t.Fatalf("expected DrainBatchMsg under high load, got %T", msg)
	}
}

func TestDrainAdaptiveCmd_NilChannel(t *testing.T) {
	t.Parallel()
	m := &AppState{
		emitterCh:   nil,
		shutdownCtx: context.Background(),
	}
	cmd := m.drainAdaptiveCmd()
	if cmd != nil {
		t.Error("expected nil for nil channel")
	}
}

// ─── Emitter Load ────────────────────────────────────────────────────────────

func TestEmitterLoad(t *testing.T) {
	t.Parallel()
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	ch := make(chan tea.Msg, ChannelCap)
	m := &AppState{
		emitterCh:   ch,
		shutdownCtx: ctx,
	}

	if m.emitterLoad() != 0 {
		t.Errorf("empty channel load = %d, want 0", m.emitterLoad())
	}

	ch <- "a"
	ch <- "b"
	if m.emitterLoad() != 2 {
		t.Errorf("2-item channel load = %d, want 2", m.emitterLoad())
	}
}

func TestEmitterLoad_NilChannel(t *testing.T) {
	t.Parallel()
	m := &AppState{
		emitterCh: nil,
	}
	if m.emitterLoad() != 0 {
		t.Errorf("nil channel load = %d, want 0", m.emitterLoad())
	}
}

// ─── Goroutine Leak Check ───────────────────────────────────────────────────

func TestChannelEmitter_NoGoroutineLeak(t *testing.T) {
	t.Parallel()

	before := runtime.NumGoroutine()

	emitter := newTestEmitter()
	// Fill and overflow — bounded retry is synchronous, no goroutines.
	for i := 0; i < ChannelCap+50; i++ {
		emitter.Emit(i)
	}

	time.Sleep(50 * time.Millisecond)

	after := runtime.NumGoroutine()
	if after > before+2 {
		t.Errorf("goroutine leak: before=%d, after=%d", before, after)
	}
}

// ─── TodoWrite Bounded Retry ────────────────────────────────────────────────

func TestTodoWriteBoundedRetry(t *testing.T) {
	t.Parallel()

	ch := make(chan tea.Msg, 16)
	for i := 0; i < 16; i++ {
		ch <- i
	}

	msg := "todo-update"
	sent := false
	for attempt := 0; attempt <= maxRetries; attempt++ {
		select {
		case ch <- msg:
			sent = true
		default:
			if attempt < maxRetries {
				time.Sleep(retryBackoff)
			}
		}
	}

	if sent {
		t.Error("message should not have been sent (channel full)")
	}
}

// ─── Global Drop Counter ────────────────────────────────────────────────────

func TestGlobalDropCounter(t *testing.T) {
	t.Parallel()
	ResetDropCounter()

	if DroppedMessages() != 0 {
		t.Errorf("initial = %d, want 0", DroppedMessages())
	}

	globalDropCounter.Add(3)
	if DroppedMessages() != 3 {
		t.Errorf("after Add(3): = %d, want 3", DroppedMessages())
	}

	ResetDropCounter()
	if DroppedMessages() != 0 {
		t.Errorf("after reset: = %d, want 0", DroppedMessages())
	}
}

// ─── Concurrent Emit and Drain ───────────────────────────────────────────────

func TestChannelEmitter_ConcurrentEmitAndDrain(t *testing.T) {
	t.Parallel()

	emitter := newTestEmitter()
	var received atomic.Int64

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Drainer goroutine.
	go func() {
		for {
			select {
			case <-emitter.ch:
				received.Add(1)
			case <-ctx.Done():
				return
			}
		}
	}()

	// Emitter goroutines.
	const emitCount = 500
	var wg sync.WaitGroup
	wg.Add(10)
	for g := 0; g < 10; g++ {
		go func() {
			defer wg.Done()
			for i := 0; i < emitCount; i++ {
				emitter.Emit(i)
			}
		}()
	}

	wg.Wait()
	time.Sleep(50 * time.Millisecond)
	cancel()

	total := int64(10 * emitCount)
	dropped := emitter.drops.Load()
	got := received.Load()

	if got+dropped != total {
		t.Errorf("received(%d) + dropped(%d) = %d, want %d", got, dropped, got+dropped, total)
	}
	if dropped > 0 {
		t.Logf("dropped %d messages under concurrent load (expected under saturation)", dropped)
	}
}

// ─── Constants ───────────────────────────────────────────────────────────────

func TestRetryConstants(t *testing.T) {
	t.Parallel()
	if maxRetries != 3 {
		t.Errorf("maxRetries = %d, want 3", maxRetries)
	}
	if retryBackoff <= 0 || retryBackoff > 100*time.Millisecond {
		t.Errorf("retryBackoff = %v, want (0, 100ms]", retryBackoff)
	}
	if maxDrainPerTick != 4 {
		t.Errorf("maxDrainPerTick = %d, want 4", maxDrainPerTick)
	}
}

// ─── DrainBatchMsg Type ──────────────────────────────────────────────────────

func TestDrainBatchMsg_Fields(t *testing.T) {
	t.Parallel()
	msgs := []tea.Msg{"a", "b", "c"}
	batch := DrainBatchMsg{Messages: msgs}
	if len(batch.Messages) != 3 {
		t.Errorf("len = %d, want 3", len(batch.Messages))
	}
	if batch.Messages[0] != "a" || batch.Messages[1] != "b" || batch.Messages[2] != "c" {
		t.Errorf("messages = %v, want [a b c]", batch.Messages)
	}
}

// ─── ResetDropCounter isolation ──────────────────────────────────────────────

func TestResetDropCounter_Isolation(t *testing.T) {
	t.Parallel()
	ResetDropCounter()
	if DroppedMessages() != 0 {
		t.Errorf("after reset: = %d, want 0", DroppedMessages())
	}
}
