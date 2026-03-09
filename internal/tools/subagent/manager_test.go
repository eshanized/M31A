package subagent

import (
	"context"
	"errors"
	"sync/atomic"
	"testing"
	"time"

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

// TestManager_Spawn_NoModel verifies that Spawn fails cleanly when no model
// is configured, without leaking the semaphore slot.
func TestManager_Spawn_NoModel(t *testing.T) {
	m := NewManager(Dependencies{
		WorkDir:       t.TempDir(),
		NewDispatcher: func(workDir string) (ToolDispatcher, error) { return &fakeDispatcher{}, nil },
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
	m := NewManager(Dependencies{
		WorkDir:       t.TempDir(),
		ActiveModel:   &types.ModelInfo{ID: "fake"},
		NewDispatcher: func(workDir string) (ToolDispatcher, error) { return &fakeDispatcher{}, nil },
	})
	defer m.Shutdown(context.Background())

	// Block spawns by using a dispatcher factory that hangs until released.
	release := make(chan struct{})
	blocking := func(workDir string) (ToolDispatcher, error) {
		return &blockingDispatcher{release: release}, nil
	}
	m.deps.NewDispatcher = blocking

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

	// Release all blocked dispatchers so Shutdown can drain cleanly.
	close(release)
}

type blockingDispatcher struct {
	release <-chan struct{}
	fakeDispatcher
}

func (b *blockingDispatcher) Execute(ctx context.Context, call ToolCallInput) (ToolCallOutput, error) {
	select {
	case <-b.release:
		return ToolCallOutput{Output: "released"}, nil
	case <-ctx.Done():
		return ToolCallOutput{}, ctx.Err()
	}
}
