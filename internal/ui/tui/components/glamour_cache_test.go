package components

import (
	"fmt"
	"testing"

	"github.com/eshanized/M31A/internal/ui/tui/theme"
)

func TestGlamourCache_BasicHitMiss(t *testing.T) {
	c := NewGlamourCache()

	// Miss on empty cache
	if _, ok := c.Get("hello", 80); ok {
		t.Error("expected miss on empty cache")
	}

	// Set and hit
	c.Set("hello", 80, "rendered hello")
	if got, ok := c.Get("hello", 80); !ok || got != "rendered hello" {
		t.Errorf("expected hit, got %q, ok=%v", got, ok)
	}

	// Miss on different content
	if _, ok := c.Get("world", 80); ok {
		t.Error("expected miss on different content")
	}

	// Miss on different width
	if _, ok := c.Get("hello", 60); ok {
		t.Error("expected miss on different width")
	}
}

func TestGlamourCache_UpdateExisting(t *testing.T) {
	c := NewGlamourCache()

	c.Set("hello", 80, "v1")
	c.Set("hello", 80, "v2")

	if got, ok := c.Get("hello", 80); !ok || got != "v2" {
		t.Errorf("expected updated value, got %q", got)
	}
	if c.Len() != 1 {
		t.Errorf("expected 1 entry after update, got %d", c.Len())
	}
}

func TestGlamourCache_Eviction(t *testing.T) {
	c := NewGlamourCache()

	// Fill to capacity
	for i := 0; i < maxGlamourCacheEntries; i++ {
		c.Set(fmt.Sprintf("content-%d", i), 80, fmt.Sprintf("rendered-%d", i))
	}
	if c.Len() != maxGlamourCacheEntries {
		t.Errorf("expected %d entries, got %d", maxGlamourCacheEntries, c.Len())
	}

	// Adding one more should evict the oldest
	c.Set("content-new", 80, "rendered-new")
	if c.Len() != maxGlamourCacheEntries {
		t.Errorf("expected %d entries after eviction, got %d", maxGlamourCacheEntries, c.Len())
	}

	// New entry should be present
	if _, ok := c.Get("content-new", 80); !ok {
		t.Error("expected new entry to be present")
	}

	// Oldest entry (content-0) should have been evicted
	if _, ok := c.Get("content-0", 80); ok {
		t.Error("expected oldest entry to be evicted")
	}
}

func TestGlamourCache_LRUPromotion(t *testing.T) {
	c := NewGlamourCache()

	// Fill to capacity
	for i := 0; i < maxGlamourCacheEntries; i++ {
		c.Set(fmt.Sprintf("content-%d", i), 80, fmt.Sprintf("rendered-%d", i))
	}

	// Access content-0 to promote it to front
	c.Get("content-0", 80)

	// Add one more — should evict content-1 (now the LRU), not content-0
	c.Set("content-new", 80, "rendered-new")

	if _, ok := c.Get("content-0", 80); !ok {
		t.Error("expected content-0 to survive (was recently accessed)")
	}
	if _, ok := c.Get("content-1", 80); ok {
		t.Error("expected content-1 to be evicted (was LRU)")
	}
}

func TestGlamourCache_Clear(t *testing.T) {
	c := NewGlamourCache()

	c.Set("hello", 80, "rendered")
	c.Set("world", 80, "rendered")

	c.Clear()

	if c.Len() != 0 {
		t.Errorf("expected 0 entries after clear, got %d", c.Len())
	}
	if _, ok := c.Get("hello", 80); ok {
		t.Error("expected miss after clear")
	}
}

func TestGlamourCache_ContentSensitivity(t *testing.T) {
	c := NewGlamourCache()

	c.Set("**bold**", 80, "bold rendered")
	c.Set("*italic*", 80, "italic rendered")

	got1, _ := c.Get("**bold**", 80)
	got2, _ := c.Get("*italic*", 80)

	if got1 != "bold rendered" {
		t.Errorf("expected bold rendered, got %q", got1)
	}
	if got2 != "italic rendered" {
		t.Errorf("expected italic rendered, got %q", got2)
	}
}

func TestGlamourCache_WidthSensitivity(t *testing.T) {
	c := NewGlamourCache()

	c.Set("hello", 60, "narrow")
	c.Set("hello", 80, "wide")

	got1, _ := c.Get("hello", 60)
	got2, _ := c.Get("hello", 80)

	if got1 != "narrow" {
		t.Errorf("expected narrow, got %q", got1)
	}
	if got2 != "wide" {
		t.Errorf("expected wide, got %q", got2)
	}
}

func TestGlamourCache_Integration(t *testing.T) {
	r, err := NewMessageRenderer(theme.Dark(), 80)
	if err != nil {
		t.Fatalf("failed to create renderer: %v", err)
	}

	content := "**bold text** and *italic*"

	// First render — cache miss
	result1 := r.renderContentSegment(content, 80)
	if result1 == "" {
		t.Fatal("expected non-empty result")
	}
	if r.renderCache.Len() != 1 {
		t.Errorf("expected 1 cache entry, got %d", r.renderCache.Len())
	}

	// Second render — cache hit
	result2 := r.renderContentSegment(content, 80)
	if result1 != result2 {
		t.Error("expected identical output from cache")
	}
	if r.renderCache.Len() != 1 {
		t.Errorf("expected still 1 cache entry, got %d", r.renderCache.Len())
	}

	// Different content — cache miss
	result3 := r.renderContentSegment("*different*", 80)
	if result3 == "" {
		t.Fatal("expected non-empty result")
	}
	if r.renderCache.Len() != 2 {
		t.Errorf("expected 2 cache entries, got %d", r.renderCache.Len())
	}

	// Width change — cache cleared
	if err := r.SetWidth(100); err != nil {
		t.Fatalf("failed to set width: %v", err)
	}
	if r.renderCache.Len() != 0 {
		t.Errorf("expected 0 cache entries after width change, got %d", r.renderCache.Len())
	}
}

func TestGlamourCache_VisualIdentity(t *testing.T) {
	r, err := NewMessageRenderer(theme.Dark(), 80)
	if err != nil {
		t.Fatalf("failed to create renderer: %v", err)
	}

	contents := []string{
		"Hello **world**",
		"- item 1\n- item 2\n- item 3",
		"```go\nfmt.Println(\"hello\")\n```",
		"A paragraph with **bold** and *italic* and `code`.",
	}

	for _, content := range contents {
		// Render without cache (first time)
		result1 := r.renderContentSegment(content, 80)
		// Render with cache (second time)
		result2 := r.renderContentSegment(content, 80)

		if result1 != result2 {
			t.Errorf("cache output differs from fresh render for %q", content)
		}
	}
}
