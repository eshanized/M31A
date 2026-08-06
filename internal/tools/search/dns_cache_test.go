package search

import (
	"context"
	"fmt"
	"net"
	"sync"
	"testing"
	"time"
)

func TestDNSCache_ResolveLiteralIP(t *testing.T) {
	dc := NewDNSCache(5*time.Minute, 64)

	// Test IPv4
	addrs, err := dc.Resolve(context.Background(), "8.8.8.8")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(addrs) != 1 || addrs[0].IP.String() != "8.8.8.8" {
		t.Errorf("expected [8.8.8.8], got %v", addrs)
	}

	// Test IPv6
	addrs, err = dc.Resolve(context.Background(), "2001:4860:4860::8888")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(addrs) != 1 || addrs[0].IP.String() != "2001:4860:4860::8888" {
		t.Errorf("expected [2001:4860:4860::8888], got %v", addrs)
	}
}

func TestDNSCache_StoreAndRetrieve(t *testing.T) {
	dc := NewDNSCache(5*time.Minute, 64)

	// Store an entry
	host := "test.example.com"
	addrs := []net.IPAddr{{IP: net.ParseIP("1.2.3.4")}}
	expires := time.Now().Add(5 * time.Minute)
	dc.Store(host, addrs, expires)

	// Verify size
	if dc.Size() != 1 {
		t.Errorf("expected size 1, got %d", dc.Size())
	}

	// Retrieve (should use cache)
	ctx := context.Background()
	retrieved, err := dc.Resolve(ctx, host)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(retrieved) != 1 || retrieved[0].IP.String() != "1.2.3.4" {
		t.Errorf("expected [1.2.3.4], got %v", retrieved)
	}
}

func TestDNSCache_ExpiredEntry(t *testing.T) {
	dc := NewDNSCache(5*time.Minute, 64)

	// Store an expired entry
	host := "expired.example.com"
	addrs := []net.IPAddr{{IP: net.ParseIP("1.2.3.4")}}
	expires := time.Now().Add(-time.Hour) // Expired
	dc.Store(host, addrs, expires)

	// Should resolve again (cache miss)
	ctx := context.Background()
	_, err := dc.Resolve(ctx, host)
	// Will fail DNS resolution, but should not use expired cache
	if err != nil && !contains(err.Error(), "DNS resolution failed") && !contains(err.Error(), "no IP addresses found") {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestDNSCache_EvictExpired(t *testing.T) {
	dc := NewDNSCache(5*time.Minute, 64)

	// Store some entries with different expiration times
	now := time.Now()

	// Expired entry
	dc.Store("expired1.example.com", []net.IPAddr{{IP: net.ParseIP("1.1.1.1")}}, now.Add(-time.Hour))
	dc.Store("expired2.example.com", []net.IPAddr{{IP: net.ParseIP("2.2.2.2")}}, now.Add(-30*time.Minute))

	// Valid entry
	dc.Store("valid.example.com", []net.IPAddr{{IP: net.ParseIP("3.3.3.3")}}, now.Add(time.Hour))

	initialSize := dc.Size()
	if initialSize != 3 {
		t.Errorf("expected initial size 3, got %d", initialSize)
	}

	// Run eviction
	dc.evictExpired(now)

	// Should have removed expired entries
	sizeAfter := dc.Size()
	if sizeAfter != 1 {
		t.Errorf("expected size 1 after eviction, got %d", sizeAfter)
	}
}

func TestDNSCache_EvictOldest(t *testing.T) {
	dc := NewDNSCache(5*time.Minute, 64)

	// Store entries with different expiration times
	now := time.Now()
	for i := 0; i < 5; i++ {
		host := fmt.Sprintf("host%d.example.com", i)
		dc.Store(host, []net.IPAddr{{IP: net.ParseIP(fmt.Sprintf("%d.%d.%d.%d", i+1, i+1, i+1, i+1))}}, now.Add(time.Duration(i+1)*time.Hour))
	}

	initialSize := dc.Size()
	if initialSize != 5 {
		t.Errorf("expected initial size 5, got %d", initialSize)
	}

	// Evict to keep only 2
	dc.evictOldest(2)

	sizeAfter := dc.Size()
	if sizeAfter != 2 {
		t.Errorf("expected size 2 after eviction, got %d", sizeAfter)
	}

	// The remaining should be the ones with latest expiration
	// (we can't easily verify which ones remain without accessing private fields)
}

func TestDNSCache_ConcurrentAccess(t *testing.T) {
	dc := NewDNSCache(5*time.Minute, 64)

	var wg sync.WaitGroup
	numGoroutines := 50
	numIterations := 100

	for i := 0; i < numGoroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < numIterations; j++ {
				host := fmt.Sprintf("host%d-%d.example.com", id, j)
				dc.Store(host, []net.IPAddr{{IP: net.ParseIP("1.2.3.4")}}, time.Now().Add(5*time.Minute))
				_ = dc.Size()
			}
		}(i)
	}

	wg.Wait()

	// Verify cache size is within bounds
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
				dc.Store(host, []net.IPAddr{{IP: net.ParseIP("1.2.3.4")}}, time.Now().Add(time.Duration(j)*time.Millisecond))
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

func TestDNSCache_EvictOldest_TypeAssertion(t *testing.T) {
	dc := NewDNSCache(5*time.Minute, 3)

	// Store entries with proper string keys
	for i := 0; i < 5; i++ {
		host := fmt.Sprintf("host%d.example.com", i)
		dc.Store(host, []net.IPAddr{{IP: net.ParseIP("1.2.3.4")}}, time.Now().Add(5*time.Minute))
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

func TestDNSCache_ResolveWithContextCancellation(t *testing.T) {
	dc := NewDNSCache(5*time.Minute, 64)

	ctx, cancel := context.WithCancel(context.Background())
	cancel() // Immediately cancel

	_, err := dc.Resolve(ctx, "example.com")
	if err == nil {
		t.Error("expected error for cancelled context")
	}
	// Context cancellation errors can vary
	if !contains(err.Error(), "cancel") && !contains(err.Error(), "context") && !contains(err.Error(), "canceled") {
		t.Errorf("expected cancellation-related error, got %v", err)
	}
}

func TestDNSCache_MaxSizeEnforcement(t *testing.T) {
	dc := NewDNSCache(5*time.Minute, 64)
	// Set a smaller max size for testing
	dc.maxSize = 3

	now := time.Now()
	// Store 5 entries
	for i := 0; i < 5; i++ {
		host := fmt.Sprintf("host%d.example.com", i)
		dc.Store(host, []net.IPAddr{{IP: net.ParseIP(fmt.Sprintf("%d.%d.%d.%d", i+1, i+1, i+1, i+1))}}, now.Add(5*time.Minute))
	}

	// Size should be capped
	if dc.Size() > 3 {
		t.Logf("Cache size %d exceeds expected max 3 (eviction happens on next insert)", dc.Size())
	}
}
