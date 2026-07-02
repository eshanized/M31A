package tools

import (
	"context"
	"fmt"
	"net"
	"sync"
	"sync/atomic"
	"time"
)

// dnsCacheEntry caches DNS resolution results for a hostname to prevent
// DNS rebinding (TOCTOU) attacks. The TTL ensures stale entries are
// refreshed while still pinning IPs for the duration of a request.
type dnsCacheEntry struct {
	addrs   []net.IPAddr
	expires time.Time
}

// DNSCache is a shared DNS cache backed by a sync.Map with configurable TTL,
// periodic eviction, and maximum size limit. Both WebFetch and WebSearch share
// this implementation.
type DNSCache struct {
	cache     sync.Map
	ttl       time.Duration
	mu        sync.Mutex
	inserts   atomic.Int32
	threshold int32
	maxSize   int32 // maximum number of entries in the cache
}

// NewDNSCache creates a DNS cache with the given TTL, eviction threshold,
// and maximum size. Eviction runs when the number of inserts since last
// eviction reaches threshold. Cache size is capped at maxSize entries.
func NewDNSCache(ttl time.Duration, evictThreshold int32) *DNSCache {
	return &DNSCache{
		ttl:       ttl,
		threshold: evictThreshold,
		maxSize:   1024, // default max 1024 entries
	}
}

// Size returns the approximate number of entries in the cache.
func (dc *DNSCache) Size() int32 {
	var count int32
	dc.cache.Range(func(_, _ any) bool {
		count++
		return true
	})
	return count
}

// Resolve resolves DNS for a hostname, using the cache when available.
// If the host is a literal IP address, it is returned directly without caching.
// Enforces maximum cache size to prevent memory exhaustion attacks.
func (dc *DNSCache) Resolve(ctx context.Context, host string) ([]net.IPAddr, error) {
	if ip := net.ParseIP(host); ip != nil {
		return []net.IPAddr{{IP: ip}}, nil
	}

	now := time.Now()

	if cached, ok := dc.cache.Load(host); ok {
		entry := cached.(*dnsCacheEntry)
		if now.Before(entry.expires) {
			return entry.addrs, nil
		}
	}

	resolver := &net.Resolver{PreferGo: true}
	addrs, err := resolver.LookupIPAddr(ctx, host)
	if err != nil {
		return nil, fmt.Errorf("DNS resolution failed for %s: %w", host, err)
	}
	if len(addrs) == 0 {
		return nil, fmt.Errorf("no IP addresses found for %s", host)
	}

	entry := &dnsCacheEntry{
		addrs:   addrs,
		expires: now.Add(dc.ttl),
	}
	dc.cache.Store(host, entry)

	dc.mu.Lock()
	if dc.inserts.Add(1) >= dc.threshold {
		dc.inserts.Store(0)
		dc.evictExpired(now)
	}

	// Enforce maximum cache size by evicting oldest entries if needed
	if dc.Size() > dc.maxSize {
		dc.evictOldest(dc.maxSize / 2) // evict down to half capacity
	}
	dc.mu.Unlock()

	return addrs, nil
}

// evictExpired removes all expired entries from the cache.
func (dc *DNSCache) evictExpired(now time.Time) {
	dc.cache.Range(func(key, value any) bool {
		e := value.(*dnsCacheEntry)
		if now.After(e.expires) {
			dc.cache.Delete(key)
		}
		return true
	})
}

// evictOldest removes entries until the cache size is at or below the target.
// This is a best-effort eviction using Range which doesn't guarantee order.
func (dc *DNSCache) evictOldest(targetSize int32) {
	var keys []string
	dc.cache.Range(func(key, value any) bool {
		keys = append(keys, key.(string))
		return true
	})

	// If we're already at or below target, nothing to do
	if int32(len(keys)) <= targetSize {
		return
	}

	// Evict oldest entries (those with earliest expiry)
	type entryWithKey struct {
		key     string
		expires time.Time
	}
	entries := make([]entryWithKey, 0, len(keys))
	for _, k := range keys {
		if v, ok := dc.cache.Load(k); ok {
			e := v.(*dnsCacheEntry)
			entries = append(entries, entryWithKey{k, e.expires})
		}
	}

	// Sort by expiry time (oldest first)
	for i := 0; i < len(entries)-1; i++ {
		for j := i + 1; j < len(entries); j++ {
			if entries[i].expires.After(entries[j].expires) {
				entries[i], entries[j] = entries[j], entries[i]
			}
		}
	}

	// Evict oldest entries until we're at target size
	toEvict := int32(len(entries)) - targetSize
	for i := int32(0); i < toEvict && i < int32(len(entries)); i++ {
		dc.cache.Delete(entries[i].key)
	}
}
