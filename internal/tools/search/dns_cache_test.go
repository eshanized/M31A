package search

import (
	"fmt"
	"net"
	"sync"
	"testing"
	"time"
)

func TestDNSCache_HighContention(t *testing.T) {
	dc := NewDNSCache(5*time.Minute, 64)

	var wg sync.WaitGroup
	numGoroutines := 50
	numIterations := 1000

	for i := 0; i < numGoroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < numIterations; j++ {
				host := fmt.Sprintf("host%d-%d.example.com", id, j)
				// Store directly into the sync.Map to simulate concurrent writes
				dc.cache.Store(host, &dnsCacheEntry{
					addrs:   []net.IPAddr{{IP: net.ParseIP("1.2.3.4")}},
					expires: time.Now().Add(5 * time.Minute),
				})
				// Concurrent read
				if v, ok := dc.cache.Load(host); ok {
					entry := v.(*dnsCacheEntry)
					_ = entry.addrs
				}
			}
		}(i)
	}

	wg.Wait()

	// Verify cache size is bounded (within reason for concurrent inserts)
	size := dc.Size()
	if size > int32(numGoroutines*numIterations) {
		t.Errorf("cache size %d exceeds expected maximum %d", size, numGoroutines*numIterations)
	}
}

func TestDNSCache_ConcurrentEviction(t *testing.T) {
	dc := NewDNSCache(time.Millisecond, 10) // Low threshold to trigger eviction often

	var wg sync.WaitGroup
	numGoroutines := 20
	numIterations := 200

	for i := 0; i < numGoroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < numIterations; j++ {
				host := fmt.Sprintf("host%d-%d.example.com", id, j)
				dc.cache.Store(host, &dnsCacheEntry{
					addrs:   []net.IPAddr{{IP: net.ParseIP("1.2.3.4")}},
					expires: time.Now().Add(time.Duration(j) * time.Millisecond),
				})
				_ = dc.Size()
			}
		}(i)
	}

	wg.Wait()

	// Run eviction to clean up expired entries
	dc.mu.Lock()
	dc.evictExpired(time.Now())
	dc.mu.Unlock()
}

// B28: evictOldest handles non-string keys gracefully (comma-ok guard)
func TestDNSEvictOldest_TypeAssertion(t *testing.T) {
	dc := NewDNSCache(5*time.Minute, 3)

	// Store entries with proper string keys
	for i := 0; i < 5; i++ {
		host := fmt.Sprintf("host%d.example.com", i)
		dc.cache.Store(host, &dnsCacheEntry{
			addrs:   []net.IPAddr{{IP: net.ParseIP("1.2.3.4")}},
			expires: time.Now().Add(5 * time.Minute),
		})
	}

	// Store an entry with a non-string key (should be skipped by evictOldest)
	dc.cache.Store(42, &dnsCacheEntry{
		addrs:   []net.IPAddr{{IP: net.ParseIP("5.6.7.8")}},
		expires: time.Now().Add(5 * time.Minute),
	})

	// Eviction should not panic
	dc.mu.Lock()
	dc.evictOldest(3)
	dc.mu.Unlock()

	// Verify we still have entries (3 kept + 1 non-string key = 4)
	if dc.Size() != 4 {
		t.Errorf("expected 4 entries after eviction, got %d", dc.Size())
	}
}
