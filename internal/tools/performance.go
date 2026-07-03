package tools

import (
	"sort"
	"sync"
)

// CachedResult provides a simple in-memory cache for expensive computations.
// The cache is safe for concurrent use. Results are stored by key and
// returned on subsequent calls without re-executing fn.
//
// Use this for operations where:
//   - The same computation is performed repeatedly with the same key
//   - The result is expensive to compute (file reads, network calls)
//   - The result does not change during the session
//
// Example:
//
//	fileContent := CachedResult[string]("config.toml", func() string {
//	    data, _ := os.ReadFile("config.toml")
//	    return string(data)
//	})
type CachedResult[T any] struct {
	mu      sync.RWMutex
	entries map[string]cachedEntry[T]
}

type cachedEntry[T any] struct {
	value T
	valid bool
}

// NewCachedResult creates a new CachedResult cache.
func NewCachedResult[T any]() *CachedResult[T] {
	return &CachedResult[T]{
		entries: make(map[string]cachedEntry[T]),
	}
}

// Get retrieves the cached value for the given key, or computes and caches
// it using fn. The cache is thread-safe.
func (c *CachedResult[T]) Get(key string, fn func() T) T {
	// Fast path: read lock
	c.mu.RLock()
	if entry, ok := c.entries[key]; ok && entry.valid {
		c.mu.RUnlock()
		return entry.value
	}
	c.mu.RUnlock()

	// Slow path: compute and store
	value := fn()
	c.mu.Lock()
	c.entries[key] = cachedEntry[T]{value: value, valid: true}
	c.mu.Unlock()
	return value
}

// Invalidate removes a cached entry, forcing recomputation on next Get.
func (c *CachedResult[T]) Invalidate(key string) {
	c.mu.Lock()
	delete(c.entries, key)
	c.mu.Unlock()
}

// InvalidateAll clears all cached entries.
func (c *CachedResult[T]) InvalidateAll() {
	c.mu.Lock()
	c.entries = make(map[string]cachedEntry[T])
	c.mu.Unlock()
}

// BatchOperation processes items in batches of the given size.
// Each batch is processed by fn. If fn returns an error, processing
// stops and the error is returned.
//
// Use this for operations where:
//   - Items can be processed in groups (API calls, file operations)
//   - Batching reduces overhead (fewer round trips, better cache utilization)
//   - Each batch is independent
func BatchOperation[T any](items []T, batchSize int, fn func([]T) error) error {
	if batchSize <= 0 {
		batchSize = 10
	}
	for i := 0; i < len(items); i += batchSize {
		end := i + batchSize
		if end > len(items) {
			end = len(items)
		}
		if err := fn(items[i:end]); err != nil {
			return err
		}
	}
	return nil
}

// PerformanceReview documents performance patterns found in the codebase.
//
// Optimized patterns:
//   - Tool definitions cached after first build (WorkflowCache.GetToolDefs)
//   - Project state cached per session (loadProjectCached)
//   - Plan parsing cached by hash (buildExecuteContext)
//   - Code intelligence indexer built lazily and invalidated on file changes
//   - Context preflight check with token count caching
//   - strings.Builder used for all string concatenation
//
// Benchmarks exist for:
//   - Code complexity analysis (BenchmarkAnalyze)
//   - Edit operation strategies (BenchmarkEditOperation)
//
// Remaining opportunities:
//   - File reads in readTaskFiles could use result caching
//   - Prompt assembly could benefit from cross-call caching
//   - Tool execution results could be cached for identical inputs

// SortedStrings returns a sorted copy of the input slice.
// Useful for deterministic output in logging and comparison.
func SortedStrings(items []string) []string {
	if len(items) <= 1 {
		return items
	}
	out := make([]string, len(items))
	copy(out, items)
	sort.Strings(out)
	return out
}

// Deduplicate removes duplicate strings from a slice while preserving order.
func Deduplicate(items []string) []string {
	if len(items) <= 1 {
		return items
	}
	seen := make(map[string]struct{}, len(items))
	out := make([]string, 0, len(items))
	for _, s := range items {
		if _, ok := seen[s]; !ok {
			seen[s] = struct{}{}
			out = append(out, s)
		}
	}
	return out
}
