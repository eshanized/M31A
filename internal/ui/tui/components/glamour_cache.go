package components

import (
	"container/list"
	"hash/fnv"
)

// maxGlamourCacheEntries is the maximum number of rendered markdown outputs
// to keep in the cache. At ~2-4KB per entry (typical rendered message), this
// caps memory at ~0.5-1MB for the cache. 256 entries covers a long conversation
// (128 user+assistant pairs) with margin.
const maxGlamourCacheEntries = 256

// cacheKey uniquely identifies a glamour render input: the content string
// and the terminal width that determines word wrapping. The glamour renderer
// is already scoped to a specific theme/style, so theme changes don't need
// a separate key — they recreate the renderer (and we clear the cache).
type cacheKey struct {
	contentHash uint64
	width       int
}

// cacheEntry holds a single cached render result.
type cacheEntry struct {
	key      cacheKey
	rendered string
}

// GlamourCache is an LRU cache for glamour-rendered markdown output.
// It avoids re-rendering identical content at the same width.
//
// Invalidation:
//   - On width change: Clear() is called (the glamour renderer is recreated).
//   - On content change: naturally misses (different contentHash).
//   - On theme change: the MessageRenderer is recreated, which creates a new cache.
//
// Thread safety: not needed — Bubble Tea is single-threaded, and all render
// calls happen on the main goroutine.
type GlamourCache struct {
	items map[cacheKey]*list.Element
	order *list.List // front = most recent, back = least recent
	size  int
}

// NewGlamourCache creates an empty cache.
func NewGlamourCache() *GlamourCache {
	return &GlamourCache{
		items: make(map[cacheKey]*list.Element, 64),
		order: list.New(),
	}
}

// Get looks up a cached render result. Returns the rendered string and true
// on cache hit, empty string and false on miss.
func (c *GlamourCache) Get(content string, width int) (string, bool) {
	key := makeCacheKey(content, width)
	if elem, ok := c.items[key]; ok {
		c.order.MoveToFront(elem)
		return elem.Value.(*cacheEntry).rendered, true
	}
	return "", false
}

// Set stores a rendered result in the cache. If the cache is at capacity,
// the least-recently-used entry is evicted.
func (c *GlamourCache) Set(content string, width int, rendered string) {
	key := makeCacheKey(content, width)

	// Update existing entry if present
	if elem, ok := c.items[key]; ok {
		c.order.MoveToFront(elem)
		elem.Value.(*cacheEntry).rendered = rendered
		return
	}

	// Evict LRU if at capacity
	if c.size >= maxGlamourCacheEntries {
		c.evict()
	}

	entry := &cacheEntry{key: key, rendered: rendered}
	elem := c.order.PushFront(entry)
	c.items[key] = elem
	c.size++
}

// Clear removes all entries. Called on width change (glamour renderer recreated).
func (c *GlamourCache) Clear() {
	c.items = make(map[cacheKey]*list.Element, 64)
	c.order.Init()
	c.size = 0
}

// Len returns the number of entries in the cache.
func (c *GlamourCache) Len() int {
	return c.size
}

// evict removes the least-recently-used entry.
func (c *GlamourCache) evict() {
	back := c.order.Back()
	if back == nil {
		return
	}
	entry := back.Value.(*cacheEntry)
	delete(c.items, entry.key)
	c.order.Remove(back)
	c.size--
}

// makeCacheKey creates a cache key by hashing content+width.
// FNV-1a is fast (~2ns for typical content) and produces good distribution.
func makeCacheKey(content string, width int) cacheKey {
	h := fnv.New64a()
	h.Write([]byte(content))
	// Write width as bytes to include it in the hash
	var buf [8]byte
	buf[0] = byte(width)
	buf[1] = byte(width >> 8)
	buf[2] = byte(width >> 16)
	buf[3] = byte(width >> 24)
	h.Write(buf[:4])
	return cacheKey{contentHash: h.Sum64(), width: width}
}
