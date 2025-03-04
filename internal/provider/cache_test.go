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
