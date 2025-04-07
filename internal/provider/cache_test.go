package provider

import (
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

func TestModelCache_Get_StaleFallback(t *testing.T) {
	c := NewModelCache(5 * time.Minute)

	// Populate cache with a model
	c.Set([]types.ModelInfo{
		{ID: "test/model", Name: "Test Model"},
	})

	// Should serve fresh data
	m, ok := c.Get("test/model")
	if !ok {
		t.Fatal("expected to find model in cache")
	}
	if m.ID != "test/model" {
		t.Fatalf("expected model ID %q, got %q", "test/model", m.ID)
	}

	// Manually set fetched time to just past TTL but within stale window
	c.fetched = time.Now().Add(-10 * time.Minute)

	// Should still serve data (stale but not fully stale)
	m, ok = c.Get("test/model")
	if !ok {
		t.Fatal("expected stale data to be served within stale window")
	}
	if m.ID != "test/model" {
		t.Fatalf("expected model ID %q, got %q", "test/model", m.ID)
	}

	// Set fetched time beyond stale TTL (> 24h)
	c.fetched = time.Now().Add(-25 * time.Hour)

	// Should NOT serve data (beyond stale window)
	_, ok = c.Get("test/model")
	if ok {
		t.Fatal("expected no data beyond stale TTL")
	}
}

func TestModelCache_Get_UnknownModel(t *testing.T) {
	c := NewModelCache(5 * time.Minute)
	c.Set([]types.ModelInfo{{ID: "known/model"}})

	_, ok := c.Get("unknown/model")
	if ok {
		t.Fatal("expected false for unknown model")
	}
}

func TestModelCache_Get_EmptyCache(t *testing.T) {
	c := NewModelCache(5 * time.Minute)

	_, ok := c.Get("any/model")
	if ok {
		t.Fatal("expected false for empty cache")
	}
}

func TestModelCache_Get_Set(t *testing.T) {
	c := NewModelCache(5 * time.Minute)

	models := []types.ModelInfo{
		{ID: "test/model-1", Name: "Model 1", ContextLength: 8192},
		{ID: "test/model-2", Name: "Model 2", ContextLength: 16384},
	}
	c.Set(models)

	m, ok := c.Get("test/model-1")
	if !ok {
		t.Fatal("expected to find model-1")
	}
	if m.Name != "Model 1" {
		t.Errorf("expected Model 1, got %s", m.Name)
	}
	if m.ContextLength != 8192 {
		t.Errorf("expected 8192, got %d", m.ContextLength)
	}

	m2, ok := c.Get("test/model-2")
	if !ok {
		t.Fatal("expected to find model-2")
	}
	if m2.Name != "Model 2" {
		t.Errorf("expected Model 2, got %s", m2.Name)
	}
}

func TestModelCache_Get_Missing(t *testing.T) {
	c := NewModelCache(5 * time.Minute)
	c.Set([]types.ModelInfo{{ID: "exists/model"}})

	_, ok := c.Get("missing/model")
	if ok {
		t.Fatal("expected false for missing model")
	}
}

func TestModelCache_IsExpired(t *testing.T) {
	c := NewModelCache(10 * time.Millisecond)
	c.Set([]types.ModelInfo{{ID: "test/model"}})

	if c.IsExpired() {
		t.Error("should not be expired immediately after set")
	}

	// Wait for TTL to expire
	time.Sleep(20 * time.Millisecond)
	if !c.IsExpired() {
		t.Error("should be expired after TTL")
	}
}

func TestModelCache_IsStale(t *testing.T) {
	c := NewModelCache(5 * time.Minute)
	c.Set([]types.ModelInfo{{ID: "test/model"}})

	if c.IsStale() {
		t.Error("should not be stale immediately after set")
	}

	// Manually backdate to just before stale TTL
	c.fetched = time.Now().Add(-23 * time.Hour)
	if c.IsStale() {
		t.Error("should not be stale before 24h")
	}

	// Backdate beyond stale TTL
	c.fetched = time.Now().Add(-25 * time.Hour)
	if !c.IsStale() {
		t.Error("should be stale after 24h")
	}
}

func TestModelCache_Len(t *testing.T) {
	c := NewModelCache(5 * time.Minute)
	if c.Len() != 0 {
		t.Errorf("expected 0, got %d", c.Len())
	}

	c.Set([]types.ModelInfo{
		{ID: "a"}, {ID: "b"}, {ID: "c"},
	})
	if c.Len() != 3 {
		t.Errorf("expected 3, got %d", c.Len())
	}
}

func TestModelCache_RefreshTicker_Interval(t *testing.T) {
	ticker := NewModelCacheRefreshTicker(50 * time.Millisecond)
	if ticker.Interval() != 50*time.Millisecond {
		t.Errorf("expected 50ms interval, got %v", ticker.Interval())
	}
	ticker.Stop()
}

func TestModelCache_RefreshTicker_DefaultInterval(t *testing.T) {
	ticker := NewModelCacheRefreshTicker(0)
	if ticker.Interval() != DefaultCacheRefreshInterval {
		t.Errorf("expected default interval %v, got %v", DefaultCacheRefreshInterval, ticker.Interval())
	}
	ticker.Stop()
}

func TestModelCache_RefreshTicker_Tick(t *testing.T) {
	ticker := NewModelCacheRefreshTicker(20 * time.Millisecond)
	ch := ticker.Tick()

	select {
	case <-ch:
		// Got a tick — good
	case <-time.After(100 * time.Millisecond):
		t.Fatal("expected a tick within 100ms")
	}

	ticker.Stop()
}

func TestModelCache_RefreshTicker_Stop(t *testing.T) {
	ticker := NewModelCacheRefreshTicker(10 * time.Millisecond)
	ch := ticker.Tick()

	// Consume first tick
	<-ch

	ticker.Stop()

	// Channel should be closed after stop
	_, ok := <-ch
	if ok {
		t.Fatal("expected channel to be closed after Stop")
	}
}

func TestModelCache_RefreshTicker_StopBeforeStart(t *testing.T) {
	ticker := NewModelCacheRefreshTicker(5 * time.Minute)
	// Should not panic
	ticker.Stop()
}
