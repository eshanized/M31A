package provider

import (
	"testing"
	"time"
)

func TestCacheRefresh(t *testing.T) {
	t.Run("IsRefreshing returns false initially", func(t *testing.T) {
		cache := NewModelCache(5 * time.Minute)
		if cache.IsRefreshing() {
			t.Error("expected IsRefreshing to be false initially")
		}
	})

	t.Run("IsRefreshing returns true during refresh", func(t *testing.T) {
		cache := NewModelCache(5 * time.Minute)
		// We can't easily test the actual refresh without a mock,
		// but we can test the flag behavior using atomic operations
		cache.refreshing.Store(true)

		if !cache.IsRefreshing() {
			t.Error("expected IsRefreshing to be true during refresh")
		}
	})

	t.Run("IsRefreshing returns false after refresh", func(t *testing.T) {
		cache := NewModelCache(5 * time.Minute)
		cache.refreshing.Store(false)

		if cache.IsRefreshing() {
			t.Error("expected IsRefreshing to be false after refresh")
		}
	})

	t.Run("IsRefreshing is thread-safe", func(t *testing.T) {
		cache := NewModelCache(5 * time.Minute)
		done := make(chan bool)
		go func() {
			for i := 0; i < 100; i++ {
				cache.IsRefreshing()
			}
			done <- true
		}()
		go func() {
			for i := 0; i < 100; i++ {
				cache.refreshing.Store(!cache.refreshing.Load())
			}
			done <- true
		}()
		<-done
		<-done
		// If we get here without race condition, test passes
	})
}
