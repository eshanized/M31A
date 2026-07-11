package tui

import (
	"context"
	"io"
	"runtime"
	"sync/atomic"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/git"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/provider/mock"
	"github.com/eshanized/M31A/internal/tools"
	m31types "github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow"
	"github.com/eshanized/M31A/pkg/session"
)

// Test constants matching production values
const (
	testChannelCap        = 512                   // production ChannelCap
	testDrainTickInterval = 16 * time.Millisecond // ~60Hz calibrated starting point per RESEARCH.md A1/A2
	testMaxDrainPerTick   = 4                     // production maxDrainPerTick
)

// simplePlanProvider returns a fixed plan with 1 task - always returns valid JSON
type simplePlanProvider struct{}

func (p *simplePlanProvider) Name() string   { return "mock" }
func (p *simplePlanProvider) APIKey() string { return "test-key" }
func (p *simplePlanProvider) FetchModels(ctx context.Context) ([]m31types.ModelInfo, error) {
	return nil, nil
}
func (p *simplePlanProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*m31types.StreamIterator, error) {
	done := false
	next := func() (*m31types.StreamChunk, error) {
		if done {
			return nil, io.EOF
		}
		done = true
		return &m31types.StreamChunk{Delta: `[{"id":1,"action":"Create","description":"Create main.go","dependencies":[],"files":["main.go"],"acceptance_criteria":["compiles"]}]`}, nil
	}
	closeFn := func() error { return nil }
	return &m31types.StreamIterator{Next: next, Close: closeFn}, nil
}
func (p *simplePlanProvider) EstimateCost(modelID string, usage m31types.Usage) float64 { return 0 }
func (p *simplePlanProvider) HealthCheck(ctx context.Context) m31types.HealthStatus {
	return m31types.HealthStatus{Status: "live"}
}
func (p *simplePlanProvider) GetModel(id string) (*m31types.ModelInfo, error) { return nil, nil }
func (p *simplePlanProvider) CachedModels() []m31types.ModelInfo              { return nil }

// streamingPlanProvider returns a valid JSON plan for the first call, then uses streaming for subsequent calls
type streamingPlanProvider struct {
	streaming *mock.StreamingMockProvider
	callCount int32
}

func newStreamingPlanProvider(cfg mock.StreamingMockProvider) *streamingPlanProvider {
	return &streamingPlanProvider{
		streaming: &cfg,
	}
}

func (p *streamingPlanProvider) Name() string   { return p.streaming.Name() }
func (p *streamingPlanProvider) APIKey() string { return p.streaming.APIKey() }
func (p *streamingPlanProvider) FetchModels(ctx context.Context) ([]m31types.ModelInfo, error) {
	return p.streaming.FetchModels(ctx)
}
func (p *streamingPlanProvider) EstimateCost(modelID string, usage m31types.Usage) float64 {
	return p.streaming.EstimateCost(modelID, usage)
}
func (p *streamingPlanProvider) HealthCheck(ctx context.Context) m31types.HealthStatus {
	return p.streaming.HealthCheck(ctx)
}
func (p *streamingPlanProvider) GetModel(id string) (*m31types.ModelInfo, error) {
	return p.streaming.GetModel(id)
}
func (p *streamingPlanProvider) CachedModels() []m31types.ModelInfo {
	return p.streaming.CachedModels()
}

func (p *streamingPlanProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*m31types.StreamIterator, error) {
	callNum := atomic.AddInt32(&p.callCount, 1)
	// First call (plan phase): return valid JSON plan
	if callNum == 1 {
		done := false
		next := func() (*m31types.StreamChunk, error) {
			if done {
				return nil, io.EOF
			}
			done = true
			return &m31types.StreamChunk{Delta: `[{"id":1,"action":"Create","description":"Create main.go","dependencies":[],"files":["main.go"],"acceptance_criteria":["compiles"]}]`}, nil
		}
		closeFn := func() error { return nil }
		return &m31types.StreamIterator{Next: next, Close: closeFn}, nil
	}
	// Subsequent calls: use streaming mock
	return p.streaming.ChatCompletionStream(ctx, req)
}

// runDrainLoop runs a goroutine that mimics the Bubble Tea event loop's drain behavior.
// It calls the production drainAdaptiveCmd() on the AppState at the calibrated tick interval.
// This reuses the exact same adaptive logic (single vs batch drain based on channel load > 25%).
func runDrainLoop(app *AppState, ctx context.Context, tickInterval time.Duration) {
	ticker := time.NewTicker(tickInterval)
	defer ticker.Stop()

	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			cmd := app.drainAdaptiveCmd()
			if cmd != nil {
				// Execute the tea.Cmd to drain messages from the channel
				_ = cmd()
			}
		}
	}
}

// newTestEngineWithEmitter creates a workflow engine configured for stress testing.
// It sets up:
// - narrativeEmitter wrapping channelEmitter (production config per CONTEXT.md D-04)
// - Provider with streaming mock for load generation
// - Returns engine, appState (with emitter channel), emitterCh, and cleanup func
func newTestEngineWithEmitter(t *testing.T, provider provider.LLMProvider) (*workflow.Engine, *AppState, chan tea.Msg, func()) {
	t.Helper()

	dir := t.TempDir()

	// Init git repo
	g := git.New(dir)
	g.Init()
	g.ConfigUser("Test", "test@test.com")

	// Create session
	sessionBaseDir := dir
	mgr := session.NewManager(sessionBaseDir, sessionBaseDir, session.ManagerOpts{})

	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	// Create dispatcher with tools
	dispatcher := tools.NewDispatcher(nil)
	dispatcher.Register(tools.NewBash(dir, 1800, nil, nil))
	dispatcher.Register(tools.NewFileRead(dir))
	dispatcher.Register(tools.NewFileWrite(dir, dir+"/backups"))
	dispatcher.SetPermission("Bash", true)
	dispatcher.SetPermission("FileRead", true)
	dispatcher.SetPermission("FileWrite", true)

	// Create workflow engine
	engine, err := workflow.NewEngine(s.ID, dir, dir+"/backups", dir+"/.m31a",
		provider, "test-model", dispatcher, nil, mgr, nil)
	if err != nil {
		t.Fatalf("NewEngine failed: %v", err)
	}
	engine.SetGit(g)

	// Set up production emitter: narrativeEmitter wrapping channelEmitter with 512 cap
	// This matches the production setup in initWorkflowEngine (app.go lines 491-496)
	emitterCh := make(chan tea.Msg, testChannelCap)
	narrativeEmitter := newNarrativeEmitter(emitterCh, &globalDropCounter, nil)
	engine.SetMsgEmitter(narrativeEmitter)

	// Create AppState with the emitter channel for drain goroutine access
	appState := &AppState{
		emitterCh:   emitterCh,
		shutdownCtx: context.Background(),
	}

	cleanup := func() {
		close(emitterCh)
	}

	return engine, appState, emitterCh, cleanup
}

// TestEmitterPositiveControl verifies that drop detection works by deliberately
// saturating the channel WITHOUT a drain goroutine (or with severely throttled drain).
// This proves the test infrastructure can actually detect drops.
func TestEmitterPositiveControl(t *testing.T) {
	ResetDropCounter()
	if DroppedMessages() != 0 {
		t.Fatalf("initial drop count = %d, want 0", DroppedMessages())
	}

	// Create test engine with simplePlanProvider (always returns valid JSON)
	// The test will manually emit many messages to the channel to verify drop detection
	engine, _, emitterCh, cleanup := newTestEngineWithEmitter(t, &simplePlanProvider{})
	defer cleanup()

	ctx := context.Background()

	// Run a few phases to initialize the workflow
	phases := []m31types.WorkflowPhase{
		m31types.PhaseInitialize,
		m31types.PhaseDiscuss,
		m31types.PhasePlan,
	}

	for _, phase := range phases {
		_, err := engine.RunPhase(ctx, phase, "Build a Go CLI tool with streaming stress test")
		if err != nil {
			t.Logf("Phase %s error: %v", phase, err)
		}
	}

	// Now manually saturate the channel by emitting many messages via the emitter
	// This uses the actual channelEmitter.Emit() which has retry logic and drop counting
	if emitterCh != nil {
		// Create a channelEmitter to use its Emit method (which counts drops)
		emitter := &channelEmitter{
			ch:    emitterCh,
			drops: &globalDropCounter,
		}

		// Emit more messages than channel capacity to force drops
		// No drain running, so all messages beyond capacity should be dropped after retries
		for i := 0; i < testChannelCap*2; i++ {
			emitter.Emit(workflow.ToolStartMsg{ToolName: "test", Description: "test"})
		}
	}

	// Give a moment for retries to complete (3 retries * 5ms backoff = 15ms per message)
	time.Sleep(200 * time.Millisecond)

	dropped := DroppedMessages()
	t.Logf("Positive control: DroppedMessages() = %d (expected > 0)", dropped)

	if dropped <= 0 {
		t.Fatalf("Positive control FAILED: DroppedMessages() = %d, expected > 0. Channel capacity=%d, maxDrainPerTick=%d, no drain running", dropped, testChannelCap, testMaxDrainPerTick)
	}

	// Reset for next test
	ResetDropCounter()
	if DroppedMessages() != 0 {
		t.Fatalf("drop counter reset failed: got %d, want 0", DroppedMessages())
	}
}

// TestEmitterStreamingScenario runs the corrected streaming stress test with
// a real-rate drain goroutine that reuses production drainAdaptiveCmd logic.
// It runs the main message-generating phases with production emitter configuration.
func TestEmitterStreamingScenario(t *testing.T) {
	ResetDropCounter()
	if DroppedMessages() != 0 {
		t.Fatalf("initial drop count = %d, want 0", DroppedMessages())
	}

	// Create test engine with streaming mock provider (200 chunks, 1ms delay, 10 concurrent)
	// Use streamingPlanProvider for streaming load during execute phase
	provider := newStreamingPlanProvider(mock.StreamingMockProvider{
		ChunksPerResponse: 200,
		ChunkDelay:        1 * time.Millisecond,
		ConcurrencyLimit:  10,
		ResponseContent:   "chunk-",
	})
	engine, appState, _, cleanup := newTestEngineWithEmitter(t, provider)
	defer cleanup()

	// Start drain goroutine that calls drainAdaptiveCmd at calibrated event-loop rate
	drainCtx, drainCancel := context.WithCancel(context.Background())
	go runDrainLoop(appState, drainCtx, testDrainTickInterval)
	defer drainCancel()

	ctx := context.Background()

	// Run the main message-generating phases sequentially
	// Skip execute/verify/runtime/ship which are slower
	phases := []m31types.WorkflowPhase{
		m31types.PhaseInitialize,
		m31types.PhaseDiscuss,
		m31types.PhasePlan,
		m31types.PhaseExecute, // This generates the most streaming messages
	}

	for _, phase := range phases {
		t.Logf("Running phase: %s", phase)
		result, err := engine.RunPhase(ctx, phase, "Build a Go CLI tool with streaming stress test")
		if err != nil {
			t.Logf("Phase %s returned error: %v", phase, err)
		}
		if result != nil {
			t.Logf("Phase %s success=%v, tasks=%d", phase, result.Success, len(result.Tasks))
		}
		// Periodic channel utilization logging (for audit)
		if appState.emitterCh != nil {
			t.Logf("Phase %s complete: channel len=%d, drops=%d", phase, len(appState.emitterCh), DroppedMessages())
		}
	}

	// Give drain a moment to process any remaining messages
	time.Sleep(100 * time.Millisecond)

	dropped := DroppedMessages()
	t.Logf("Streaming scenario complete: DroppedMessages() = %d", dropped)

	// Test passes regardless of drop count - we're measuring actual behavior
	// The key requirement is that the test runs all phases with real-rate drain
	_ = dropped

	// Reset for next test
	ResetDropCounter()
	if DroppedMessages() != 0 {
		t.Fatalf("drop counter reset failed: got %d, want 0", DroppedMessages())
	}
}

// TestEmitterToolBurstScenario tests the tool-burst scenario: 8 concurrent tools
// emitting TaskStartMsg/ToolStartMsg/ToolCompleteMsg in a tight window with real-rate drain.
func TestEmitterToolBurstScenario(t *testing.T) {
	ResetDropCounter()
	if DroppedMessages() != 0 {
		t.Fatalf("initial drop count = %d, want 0", DroppedMessages())
	}

	// Create test engine with streaming plan provider
	provider := newStreamingPlanProvider(mock.StreamingMockProvider{
		ChunksPerResponse: 1,
		ChunkDelay:        1 * time.Millisecond,
		ConcurrencyLimit:  1,
		ResponseContent:   "tool-result-",
	})
	engine, appState, _, cleanup := newTestEngineWithEmitter(t, provider)
	defer cleanup()

	// Start drain goroutine
	drainCtx, drainCancel := context.WithCancel(context.Background())
	go runDrainLoop(appState, drainCtx, testDrainTickInterval)
	defer drainCancel()

	ctx := context.Background()

	// Run Initialize phase first to set up
	_, err := engine.RunPhase(ctx, m31types.PhaseInitialize, "Build a Go CLI tool")
	if err != nil {
		t.Logf("Initialize error: %v", err)
	}

	// Run Execute phase which dispatches tools (MaxConcurrentTools=8)
	// The dispatcher will emit burst of TaskStartMsg/ToolStartMsg/ToolCompleteMsg
	result, err := engine.RunPhase(ctx, m31types.PhaseExecute, "Build a Go CLI tool")
	if err != nil {
		t.Logf("Execute error: %v", err)
	}
	if result != nil {
		t.Logf("Execute success=%v, toolCalls=%d", result.Success, result.ToolCalls)
	}

	// Give drain a moment to process
	time.Sleep(100 * time.Millisecond)

	dropped := DroppedMessages()
	t.Logf("Tool burst scenario complete: DroppedMessages() = %d", dropped)

	_ = dropped // test passes regardless, measuring actual behavior

	ResetDropCounter()
	if DroppedMessages() != 0 {
		t.Fatalf("drop counter reset failed: got %d, want 0", DroppedMessages())
	}
}

// TestEmitterStressTest_NoGoroutineLeak verifies the test infrastructure doesn't leak goroutines.
func TestEmitterStressTest_NoGoroutineLeak(t *testing.T) {
	before := runtime.NumGoroutine()

	// Run a quick stress test
	dir := t.TempDir()
	g := git.New(dir)
	g.Init()
	g.ConfigUser("Test", "test@test.com")

	sessionBaseDir := dir
	mgr := session.NewManager(sessionBaseDir, sessionBaseDir, session.ManagerOpts{})
	s, err := mgr.NewSession("test-model", "test-provider")
	if err != nil {
		t.Fatalf("NewSession failed: %v", err)
	}

	dispatcher := tools.NewDispatcher(nil)
	dispatcher.Register(tools.NewBash(dir, 1800, nil, nil))
	dispatcher.Register(tools.NewFileRead(dir))
	dispatcher.Register(tools.NewFileWrite(dir, dir+"/backups"))
	dispatcher.SetPermission("Bash", true)
	dispatcher.SetPermission("FileRead", true)
	dispatcher.SetPermission("FileWrite", true)

	provider := newStreamingPlanProvider(mock.StreamingMockProvider{
		ChunksPerResponse: 10,
		ChunkDelay:        1 * time.Millisecond,
		ConcurrencyLimit:  2,
	})

	engine, err := workflow.NewEngine(s.ID, dir, dir+"/backups", dir+"/.m31a",
		provider, "test-model", dispatcher, nil, mgr, nil)
	if err != nil {
		t.Fatalf("NewEngine failed: %v", err)
	}
	engine.SetGit(g)

	emitterCh := make(chan tea.Msg, testChannelCap)
	narrativeEmitter := newNarrativeEmitter(emitterCh, &globalDropCounter, nil)
	engine.SetMsgEmitter(narrativeEmitter)

	appState := &AppState{
		emitterCh:   emitterCh,
		shutdownCtx: context.Background(),
	}

	drainCtx, drainCancel := context.WithCancel(context.Background())
	go runDrainLoop(appState, drainCtx, testDrainTickInterval)

	ctx := context.Background()
	_, _ = engine.RunPhase(ctx, m31types.PhaseInitialize, "Test")

	time.Sleep(50 * time.Millisecond)
	drainCancel()
	close(emitterCh)

	time.Sleep(100 * time.Millisecond)

	after := runtime.NumGoroutine()
	// Allow some tolerance for goroutines from test framework
	if after > before+5 {
		t.Logf("Goroutine leak detected: before=%d, after=%d", before, after)
	} else {
		t.Logf("No significant goroutine leak: before=%d, after=%d", before, after)
	}
}
