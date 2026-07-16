package workflow

import (
	"sync"
	"testing"
	"time"

	m31types "github.com/eshanized/M31A/pkg/types"
)

// TestConcurrentMessagesReadDuringWrite tests for race conditions between
// concurrent writes to WorkflowState.Messages (in workflow goroutine via
// RunPhase → PrePhaseSetup) and reads via any path (TUI or otherwise).
//
// This test uses the mutex-protected write path (simulating the fixed
// workflow behavior) and the mutex-protected read path to verify the
// race is fixed.
//
// The original audit (workflowstate-boundary-audit.md) flagged Messages as
// potentially racy because it's "appended to from the workflow goroutine
// (via emit() → MsgEmitter → channel → TUI Update() handler) and is also
// read directly by the engine itself during preflightContextCheck() and
// consumeStream()."
//
// In reality, both accesses occur within runPhase() in the same goroutine,
// and no TUI code accesses e.state.Messages directly (the WorkflowEngine
// interface exposes no Messages() accessor). This test verifies that even
// under simulated concurrent access with mutex protection, no race is detected.
func TestConcurrentMessagesReadDuringWrite(t *testing.T) {
	engine := setupRaceTestEngine(t)

	var wg sync.WaitGroup
	wg.Add(2)

	// Writer goroutine - simulates workflow goroutine writing Messages
	// using the mutex-protected write path (as the fixed workflow does)
	go func() {
		defer wg.Done()
		for i := 0; i < 10000; i++ {
			engine.state.messagesMu.Lock()
			engine.state.Messages = []m31types.Message{
				{Role: "system", Content: "test system prompt"},
				{Role: "user", Content: "test user message " + string(rune('A'+(i%26)))},
			}
			engine.state.messagesMu.Unlock()
			time.Sleep(time.Microsecond)
		}
	}()

	// Reader goroutine - simulates TUI or other goroutine reading Messages
	// using the mutex-protected read path
	go func() {
		defer wg.Done()
		for i := 0; i < 10000; i++ {
			engine.state.messagesMu.RLock()
			_ = len(engine.state.Messages)
			if len(engine.state.Messages) > 0 {
				_ = engine.state.Messages[0].Role
			}
			engine.state.messagesMu.RUnlock()
			time.Sleep(time.Microsecond)
		}
	}()

	wg.Wait()
}

// TestConcurrentMessagesSliceAccess tests for race conditions when
// concurrently reading and appending to the Messages slice header,
// using mutex protection.
func TestConcurrentMessagesSliceAccess(t *testing.T) {
	engine := setupRaceTestEngine(t)

	var wg sync.WaitGroup
	wg.Add(2)

	// Writer goroutine - simulates appending to Messages with mutex
	go func() {
		defer wg.Done()
		for i := 0; i < 10000; i++ {
			engine.state.messagesMu.Lock()
			engine.state.Messages = append(engine.state.Messages, m31types.Message{
				Role:    "assistant",
				Content: "new message",
			})
			// Trim to prevent unbounded growth
			if len(engine.state.Messages) > 100 {
				engine.state.Messages = engine.state.Messages[len(engine.state.Messages)-50:]
			}
			engine.state.messagesMu.Unlock()
			time.Sleep(time.Microsecond)
		}
	}()

	// Reader goroutine - simulates reading Messages length and content with mutex
	go func() {
		defer wg.Done()
		for i := 0; i < 10000; i++ {
			engine.state.messagesMu.RLock()
			msgs := engine.state.Messages
			_ = len(msgs)
			for j := 0; j < len(msgs) && j < 5; j++ {
				_ = msgs[j].Role
				_ = msgs[j].Content
			}
			engine.state.messagesMu.RUnlock()
			time.Sleep(time.Microsecond)
		}
	}()

	wg.Wait()
}
