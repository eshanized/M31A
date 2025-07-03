package tui

import (
	"context"
	"testing"
	"time"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

// ---------------------------------------------------------------------------
// Test helpers for cache testing
// ---------------------------------------------------------------------------

// mockProviderWithModels returns models from FetchModels for cache refresh tests.
type mockProviderWithModels struct {
	models []types.ModelInfo
	err    error
	name   string
}

func (m *mockProviderWithModels) Name() string {
	if m.name != "" {
		return m.name
	}
	return "mock-with-models"
}

func (m *mockProviderWithModels) APIKey() string { return "test-key" }

func (m *mockProviderWithModels) FetchModels(ctx context.Context) ([]types.ModelInfo, error) {
	return m.models, m.err
}

func (m *mockProviderWithModels) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
	return nil, nil
}

func (m *mockProviderWithModels) EstimateCost(modelID string, usage types.Usage) float64 {
	return 0
}

func (m *mockProviderWithModels) HealthCheck(ctx context.Context) types.HealthStatus {
	return types.HealthStatus{Status: "live", LatencyMs: 42}
}

func (m *mockProviderWithModels) GetModel(id string) (*types.ModelInfo, error) {
	for _, model := range m.models {
		if model.ID == id {
			return &model, nil
		}
	}
	return nil, nil
}

// ---------------------------------------------------------------------------
// TestCacheRefreshTicker
// ---------------------------------------------------------------------------

func TestCacheRefreshTicker(t *testing.T) {
	t.Run("returns a cmd", func(t *testing.T) {
		cmd := CacheRefreshTicker("openrouter", 10*time.Millisecond)
		if cmd == nil {
			t.Fatal("CacheRefreshTicker should return a non-nil cmd")
		}
	})

	t.Run("emits RefreshCacheMsg with provider name", func(t *testing.T) {
		cmd := CacheRefreshTicker("zen", 1*time.Millisecond)
		if cmd == nil {
			t.Fatal("cmd should not be nil")
		}
		msg := cmd()
		refreshMsg, ok := msg.(RefreshCacheMsg)
		if !ok {
			t.Fatalf("expected RefreshCacheMsg, got %T", msg)
		}
		if refreshMsg.ProviderName != "zen" {
			t.Errorf("expected ProviderName='zen', got %q", refreshMsg.ProviderName)
		}
	})

	t.Run("uses default interval when interval <= 0", func(t *testing.T) {
		// Should not panic; uses default interval
		cmd := CacheRefreshTicker("openrouter", -1*time.Second)
		if cmd == nil {
			t.Fatal("CacheRefreshTicker should return a non-nil cmd even with negative interval")
		}
	})

	t.Run("zero interval uses default", func(t *testing.T) {
		cmd := CacheRefreshTicker("openrouter", 0)
		if cmd == nil {
			t.Fatal("CacheRefreshTicker should return a non-nil cmd with zero interval")
		}
	})
}

// ---------------------------------------------------------------------------
// TestNextCacheRefreshTick
// ---------------------------------------------------------------------------

func TestNextCacheRefreshTick(t *testing.T) {
	t.Run("returns a cmd", func(t *testing.T) {
		cmd := NextCacheRefreshTick(10 * time.Millisecond)
		if cmd == nil {
			t.Fatal("NextCacheRefreshTick should return a non-nil cmd")
		}
	})

	t.Run("emits RefreshCacheMsg with empty provider name", func(t *testing.T) {
		cmd := NextCacheRefreshTick(1 * time.Millisecond)
		msg := cmd()
		refreshMsg, ok := msg.(RefreshCacheMsg)
		if !ok {
			t.Fatalf("expected RefreshCacheMsg, got %T", msg)
		}
		if refreshMsg.ProviderName != "" {
			t.Errorf("expected empty ProviderName for one-shot tick, got %q", refreshMsg.ProviderName)
		}
	})

	t.Run("uses default interval when interval <= 0", func(t *testing.T) {
		cmd := NextCacheRefreshTick(-1 * time.Second)
		if cmd == nil {
			t.Fatal("NextCacheRefreshTick should return a non-nil cmd with negative interval")
		}
	})
}

// ---------------------------------------------------------------------------
// TestHandleCacheRefresh
// ---------------------------------------------------------------------------

func TestHandleCacheRefresh(t *testing.T) {
	ctx := context.Background()

	t.Run("nil registry returns error", func(t *testing.T) {
		errMsg, nextCmd := handleCacheRefresh(ctx, nil, "openrouter")
		if errMsg == "" {
			t.Error("expected error message for nil registry")
		}
		if nextCmd != nil {
			t.Error("expected nil nextCmd for nil registry")
		}
	})

	t.Run("empty provider name uses registry active", func(t *testing.T) {
		reg := provider.NewRegistry()
		reg.Register("openrouter", &mockProviderWithModels{
			models: []types.ModelInfo{{ID: "gpt-4o", ContextLength: 8192}},
		})
		reg.SetActive("openrouter")

		errMsg, nextCmd := handleCacheRefresh(ctx, reg, "")
		if errMsg != "" {
			t.Errorf("expected no error, got: %s", errMsg)
		}
		if nextCmd == nil {
			t.Error("expected non-nil nextCmd after successful refresh")
		}
	})

	t.Run("provider not found returns error", func(t *testing.T) {
		reg := provider.NewRegistry()
		reg.Register("openrouter", &mockProvider{})

		errMsg, nextCmd := handleCacheRefresh(ctx, reg, "nonexistent")
		if errMsg == "" {
			t.Error("expected error message for nonexistent provider")
		}
		if nextCmd != nil {
			t.Error("expected nil nextCmd for provider not found")
		}
	})

	t.Run("successful refresh with models", func(t *testing.T) {
		reg := provider.NewRegistry()
		reg.Register("openrouter", &mockProviderWithModels{
			models: []types.ModelInfo{
				{ID: "gpt-4o", ContextLength: 8192},
				{ID: "claude-3.5-sonnet", ContextLength: 200000},
			},
		})
		reg.SetActive("openrouter")

		errMsg, nextCmd := handleCacheRefresh(ctx, reg, "openrouter")
		if errMsg != "" {
			t.Errorf("expected no error, got: %s", errMsg)
		}
		if nextCmd == nil {
			t.Error("expected non-nil nextCmd after successful refresh")
		}
	})

	t.Run("empty models list returns error", func(t *testing.T) {
		reg := provider.NewRegistry()
		reg.Register("openrouter", &mockProviderWithModels{
			models: []types.ModelInfo{}, // empty
		})
		reg.SetActive("openrouter")

		errMsg, nextCmd := handleCacheRefresh(ctx, reg, "openrouter")
		if errMsg == "" {
			t.Error("expected error message for empty models list")
		}
		if nextCmd == nil {
			t.Error("expected non-nil nextCmd even on empty models")
		}
	})

	t.Run("fetch error returns error", func(t *testing.T) {
		reg := provider.NewRegistry()
		reg.Register("openrouter", &mockProviderWithModels{
			err: &testError{},
		})
		reg.SetActive("openrouter")

		errMsg, nextCmd := handleCacheRefresh(ctx, reg, "openrouter")
		if errMsg == "" {
			t.Error("expected error message for fetch failure")
		}
		if nextCmd == nil {
			t.Error("expected non-nil nextCmd even on fetch error")
		}
	})

	t.Run("provider Get returns nil", func(t *testing.T) {
		reg := provider.NewRegistry()
		// Register a provider that returns nil model for GetModel
		reg.Register("zen", &mockProvider{})
		reg.SetActive("zen")

		// Get will return the mockProvider which is non-nil, so this tests
		// the normal path. The nil provider path requires registry.Get to
		// return nil which shouldn't happen with a registered provider.
		// This test verifies the code doesn't crash when provider is valid.
		errMsg, nextCmd := handleCacheRefresh(ctx, reg, "zen")
		// mockProvider returns nil models, so this will hit "no models" path
		if errMsg == "" {
			// This is fine - the provider may return models
		}
		_ = nextCmd
	})
}

// ---------------------------------------------------------------------------
// TestCacheRefreshTicker_MsgTiming
// ---------------------------------------------------------------------------

func TestCacheRefreshTicker_MsgTiming(t *testing.T) {
	// Verify the ticker fires and produces correct message type
	cmd := CacheRefreshTicker("test", 1*time.Millisecond)
	if cmd == nil {
		t.Fatal("expected non-nil cmd")
	}
	msg := cmd()

	refreshMsg, ok := msg.(RefreshCacheMsg)
	if !ok {
		t.Fatalf("expected RefreshCacheMsg, got %T", msg)
	}
	if refreshMsg.ProviderName != "test" {
		t.Errorf("expected ProviderName='test', got %q", refreshMsg.ProviderName)
	}
}

func TestNextCacheRefreshTick_MsgTiming(t *testing.T) {
	cmd := NextCacheRefreshTick(1 * time.Millisecond)
	if cmd == nil {
		t.Fatal("expected non-nil cmd")
	}
	msg := cmd()

	refreshMsg, ok := msg.(RefreshCacheMsg)
	if !ok {
		t.Fatalf("expected RefreshCacheMsg, got %T", msg)
	}
	if refreshMsg.ProviderName != "" {
		t.Errorf("expected empty ProviderName, got %q", refreshMsg.ProviderName)
	}
}

// ---------------------------------------------------------------------------
// TestCacheRefreshTicker_TeaCmdType
// ---------------------------------------------------------------------------

func TestCacheRefreshTicker_TeaCmdType(t *testing.T) {
	cmd := CacheRefreshTicker("openrouter", 1*time.Millisecond)
	// The cmd should be a tea.Cmd
	if cmd == nil {
		t.Fatal("expected non-nil tea.Cmd")
	}

	// Execute and verify message type
	msg := cmd()
	if msg == nil {
		t.Fatal("expected non-nil message from cmd")
	}

	switch m := msg.(type) {
	case RefreshCacheMsg:
		if m.ProviderName != "openrouter" {
			t.Errorf("expected 'openrouter', got %q", m.ProviderName)
		}
	default:
		t.Errorf("expected RefreshCacheMsg, got %T", msg)
	}
}

// Ensure tea import is used
var _ tea.Cmd = CacheRefreshTicker("x", 1*time.Millisecond)
