package efie

import (
	"crypto/sha256"
	"fmt"
	"strings"
	"sync"
)

// QueryCache is a simple thread-safe LRU cache for query results.
type QueryCache struct {
	mu      sync.RWMutex
	maxSize int
	keys    []string
	items   map[string][]ScoredFile
}

// NewQueryCache creates a new cache with the given max size.
func NewQueryCache(maxSize int) *QueryCache {
	return &QueryCache{
		maxSize: maxSize,
		keys:    make([]string, 0, maxSize),
		items:   make(map[string][]ScoredFile, maxSize),
	}
}

// Get retrieves a cached result. Returns (result, true) on hit, (nil, false) on miss.
func (c *QueryCache) Get(key string) ([]ScoredFile, bool) {
	c.mu.RLock()
	defer c.mu.RUnlock()

	if result, ok := c.items[key]; ok {
		return result, true
	}
	return nil, false
}

// Put stores a result in the cache. Evicts oldest entry if at capacity.
func (c *QueryCache) Put(key string, result []ScoredFile) {
	c.mu.Lock()
	defer c.mu.Unlock()

	// Update existing key
	if _, ok := c.items[key]; ok {
		c.items[key] = result
		return
	}

	// Evict oldest if at capacity
	if len(c.keys) >= c.maxSize {
		oldest := c.keys[0]
		c.keys = c.keys[1:]
		delete(c.items, oldest)
	}

	c.keys = append(c.keys, key)
	c.items[key] = result
}

// ComputeCacheKey generates a cache key from query parameters.
func ComputeCacheKey(targetFiles []string, description string, topN int) string {
	h := sha256.New()

	// Sort targets for stable keys
	sorted := make([]string, len(targetFiles))
	copy(sorted, targetFiles)
	for i := range sorted {
		for j := i + 1; j < len(sorted); j++ {
			if sorted[i] > sorted[j] {
				sorted[i], sorted[j] = sorted[j], sorted[i]
			}
		}
	}

	for _, t := range sorted {
		h.Write([]byte(t))
		h.Write([]byte{0})
	}
	h.Write([]byte(description))
	h.Write([]byte{0})
	fmt.Fprintf(h, "%d", topN)

	return fmt.Sprintf("%x", h.Sum(nil)[:16])
}

// FormatCacheKeyForDebug returns a human-readable cache key for debugging.
func FormatCacheKeyForDebug(targetFiles []string, description string, topN int) string {
	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("targets=%v,topN=%d", targetFiles, topN))
	if description != "" {
		if len(description) > 50 {
			description = description[:50] + "..."
		}
		sb.WriteString(fmt.Sprintf(",desc=%q", description))
	}
	return sb.String()
}
