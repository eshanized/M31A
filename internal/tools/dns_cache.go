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

// DNSCache is a shared DNS cache backed by a sync.Map with configurable TTL
// and periodic eviction. Both WebFetch and WebSearch share this implementation.
type DNSCache struct {
	cache     sync.Map
	ttl       time.Duration
	inserts   atomic.Int32
	threshold int32
}

// NewDNSCache creates a DNS cache with the given TTL and eviction threshold.
// Eviction runs when the number of inserts since last eviction reaches threshold.
func NewDNSCache(ttl time.Duration, evictThreshold int32) *DNSCache {
	return &DNSCache{
		ttl:       ttl,
		threshold: evictThreshold,
	}
}

// Resolve resolves DNS for a hostname, using the cache when available.
// If the host is a literal IP address, it is returned directly without caching.
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

	if dc.inserts.Add(1) >= dc.threshold {
		dc.inserts.Store(0)
		dc.cache.Range(func(key, value any) bool {
			e := value.(*dnsCacheEntry)
			if now.After(e.expires) {
				dc.cache.Delete(key)
			}
			return true
		})
	}

	return addrs, nil
}
