package subagent

import (
	"context"
	"errors"
	"fmt"
	"io"
	"sync/atomic"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/types"
)

// fakeDispatcher satisfies ToolDispatcher without touching real tools.
type fakeDispatcher struct {
	calls atomic.Int32
	stop  atomic.Bool
}

func (f *fakeDispatcher) Execute(ctx context.Context, call ToolCallInput) (ToolCallOutput, error) {
	f.calls.Add(1)
	return ToolCallOutput{
		ToolCallID: call.ID,
		Output:     "fake output for " + call.Name,
	}, nil
}

func (f *fakeDispatcher) ListTools() []ToolDescriptor {
	return []ToolDescriptor{
		{Name: "Glob", Description: "glob files"},
		{Name: "FileRead", Description: "read a file"},
	}
}

func (f *fakeDispatcher) UnregisterTool(_ string) {}

func (f *fakeDispatcher) Stop() { f.stop.Store(true) }

// SetPermission implements ToolDispatcher interface.
func (f *fakeDispatcher) SetPermission(name string, allowed bool) {}

// stubProvider is a minimal LLMProvider for tests that need a registry.
type stubProvider struct{}

func (s *stubProvider) Name() string   { return "stub" }
func (s *stubProvider) APIKey() string { return "test-key" }
func (s *stubProvider) FetchModels(ctx context.Context) ([]types.ModelInfo, error) {
	return nil, nil
}
func (s *stubProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
	next := func() (*types.StreamChunk, error) { return nil, io.EOF }
	close := func() error { return nil }
	return &types.StreamIterator{Next: next, Close: close}, nil
}
func (s *stubProvider) EstimateCost(modelID string, usage types.Usage) float64 { return 0 }
func (s *stubProvider) HealthCheck(ctx context.Context) types.HealthStatus {
	return types.HealthStatus{Status: "live"}
}
func (s *stubProvider) GetModel(id string) (*types.ModelInfo, error) { return nil, nil }
func (s *stubProvider) CachedModels() []types.ModelInfo              { return nil }

func newTestRegistry() *provider.Registry {
	r := provider.NewRegistry()
	r.Register("stub", &stubProvider{})
	return r
}

// blockingProvider blocks in ChatCompletionStream until release is closed.
type blockingProvider struct {
	release <-chan struct{}
}

func (b *blockingProvider) Name() string   { return "blocking" }
func (b *blockingProvider) APIKey() string { return "test-key" }
func (b *blockingProvider) FetchModels(ctx context.Context) ([]types.ModelInfo, error) {
	return nil, nil
}
func (b *blockingProvider) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
	select {
	case <-b.release:
		next := func() (*types.StreamChunk, error) { return nil, io.EOF }
		close := func() error { return nil }
		return &types.StreamIterator{Next: next, Close: close}, nil
	case <-ctx.Done():
		return nil, ctx.Err()
	}
}
func (b *blockingProvider) EstimateCost(modelID string, usage types.Usage) float64 { return 0 }
func (b *blockingProvider) HealthCheck(ctx context.Context) types.HealthStatus {
	return types.HealthStatus{Status: "live"}
}
func (b *blockingProvider) GetModel(id string) (*types.ModelInfo, error) { return nil, nil }
func (b *blockingProvider) CachedModels() []types.ModelInfo              { return nil }

func newBlockingTestRegistry(release <-chan struct{}) *provider.Registry {
	r := provider.NewRegistry()
	r.Register("blocking", &blockingProvider{release: release})
	return r
}

// TestManager_Spawn_NoModel verifies that Spawn fails cleanly when no model
// is configured, without leaking the semaphore slot.
func TestManager_Spawn_NoModel(t *testing.T) {
	m := NewManager(Dependencies{
		WorkDir:       t.TempDir(),
		NewDispatcher: func(workDir string) (ToolDispatcher, error) { return &fakeDispatcher{}, nil },
		Registry:      newTestRegistry(),
	})
	_, _, err := m.Spawn(context.Background(), SpawnRequest{
		Description: "x",
		Prompt:      "y",
	})
	if err == nil {
		t.Fatal("expected error when no model configured")
	}
	// After the failure, spawning another should still work (slot released).
	m.deps.ActiveModel = &types.ModelInfo{ID: "fake-model"}
	// Use a background=true spawn with no real provider so the loop exits
	// via the provider error path — we just want to confirm Spawn()
	// returned a valid ID without blocking.
	id, _, err := m.Spawn(context.Background(), SpawnRequest{
		Description: "x",
		Prompt:      "y",
		Background:  true,
	})
	if err != nil {
		t.Fatalf("second spawn failed: %v", err)
	}
	if id == "" {
		t.Fatal("expected non-empty id")
	}
	// Give the loop goroutine a moment to fail out (provider is nil).
	time.Sleep(50 * time.Millisecond)
	// Cleanup
	m.Shutdown(context.Background())
}

// TestManager_MaxConcurrent verifies that the 9th concurrent spawn fails
// with ErrMaxConcurrent while 8 are running.
func TestManager_MaxConcurrent(t *testing.T) {
	release := make(chan struct{})

	m := NewManager(Dependencies{
		WorkDir:       t.TempDir(),
		ActiveModel:   &types.ModelInfo{ID: "fake"},
		Registry:      newBlockingTestRegistry(release),
		NewDispatcher: func(workDir string) (ToolDispatcher, error) { return &fakeDispatcher{}, nil },
	})
	defer m.Shutdown(context.Background())

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	for i := 0; i < MaxConcurrent; i++ {
		if _, _, err := m.Spawn(ctx, SpawnRequest{
			Description: "x", Prompt: "y", Background: true,
		}); err != nil {
			t.Fatalf("spawn %d: %v", i, err)
		}
	}

	// The 9th should fail immediately.
	_, _, err := m.Spawn(ctx, SpawnRequest{
		Description: "x", Prompt: "y", Background: true,
	})
	if !errors.Is(err, ErrMaxConcurrent) {
		t.Fatalf("expected ErrMaxConcurrent, got %v", err)
	}

	// Release all blocked providers so Shutdown can drain cleanly.
	close(release)
}

func TestSpawn_DefaultIsolation_IsWorktree(t *testing.T) {
	t.Parallel()

	release := make(chan struct{})
	m := NewManager(Dependencies{
		WorkDir:       t.TempDir(),
		ActiveModel:   &types.ModelInfo{ID: "fake"},
		Registry:      newBlockingTestRegistry(release),
		NewDispatcher: func(workDir string) (ToolDispatcher, error) { return &fakeDispatcher{}, nil },
	})
	defer m.Shutdown(context.Background())

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	// Spawn with empty Isolation — should default to IsolationWorktree
	id, _, err := m.Spawn(ctx, SpawnRequest{
		Description: "test default isolation",
		Prompt:      "echo test",
		Background:  true,
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	close(release)

	// Verify agent was created with worktree isolation
	a, ok := m.agents.Load(id)
	if !ok {
		t.Fatalf("agent %s not found", id)
	}
	sa := a.(*Subagent)
	if sa.Info.Isolation != IsolationWorktree {
		t.Errorf("expected default isolation 'worktree', got %q", sa.Info.Isolation)
	}
}

func TestSpawn_DegradedMode_EmitsSpawnFailedEvent(t *testing.T) {
	t.Parallel()

	// Create a worktree provider that always fails
	failWorktrees := &failWorktreeProvider{}

	release := make(chan struct{})
	m := NewManager(Dependencies{
		WorkDir:       t.TempDir(),
		ActiveModel:   &types.ModelInfo{ID: "fake"},
		Registry:      newBlockingTestRegistry(release),
		Worktrees:     failWorktrees,
		NewDispatcher: func(workDir string) (ToolDispatcher, error) { return &fakeDispatcher{}, nil },
	})
	defer m.Shutdown(context.Background())

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	// Collect events
	var events []SubagentEvent
	done := make(chan struct{})
	go func() {
		defer close(done)
		for ev := range m.Events() {
			events = append(events, ev)
			if ev.Type == EventSpawnFailed {
				return
			}
		}
	}()

	id, _, err := m.Spawn(ctx, SpawnRequest{
		Description: "test degraded mode",
		Prompt:      "echo test",
		Isolation:   IsolationWorktree,
		Background:  true,
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	close(release)

	// Wait for event
	select {
	case <-done:
	case <-time.After(3 * time.Second):
	}

	// Verify agent was created (degraded mode, not failure)
	a, ok := m.agents.Load(id)
	if !ok {
		t.Fatalf("agent %s not found in degraded mode", id)
	}
	sa := a.(*Subagent)
	// Worktree should be the parent dir (degraded)
	if sa.Info.Worktree == "" {
		t.Error("expected worktree to be set (parent dir in degraded mode)")
	}

	// Verify SpawnFailed event was emitted
	foundEvent := false
	for _, ev := range events {
		if ev.Type == EventSpawnFailed {
			foundEvent = true
			break
		}
	}
	if !foundEvent {
		t.Error("expected EventSpawnFailed event to be emitted")
	}
}

// failWorktreeProvider always returns an error on Create.
type failWorktreeProvider struct{}

func (f *failWorktreeProvider) IsRepo(_ string) bool { return true }
func (f *failWorktreeProvider) Create(_ context.Context, _, _, _ string) (string, error) {
	return "", fmt.Errorf("worktree creation failed: test injection")
}
func (f *failWorktreeProvider) Remove(_ context.Context, _ string) error { return nil }
