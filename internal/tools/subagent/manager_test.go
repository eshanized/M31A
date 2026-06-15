package subagent

import (
	"context"
	"errors"
	"io"
	"sync/atomic"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/provider"
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

func (f *fakeDispatcher) Stop() { f.stop.Store(true) }

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
