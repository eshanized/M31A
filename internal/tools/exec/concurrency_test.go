package exec

import (
	"sync"
	"testing"
	"time"
)

func TestWithMutex(t *testing.T) {
	t.Parallel()
	var mu sync.Mutex
	counter := 0
	done := make(chan struct{})

	// Spawn multiple goroutines that increment the counter
	for i := 0; i < 100; i++ {
		go func() {
			WithMutex(&mu, func() {
				counter++
			})
			done <- struct{}{}
		}()
	}

	// Wait for all goroutines
	for i := 0; i < 100; i++ {
		<-done
	}

	if counter != 100 {
		t.Errorf("expected counter=100, got %d", counter)
	}
}

func TestWithRWMutex_ReadLock(t *testing.T) {
	t.Parallel()
	var mu sync.RWMutex
	counter := 0
	var wg sync.WaitGroup

	// Writer
	wg.Add(1)
	go func() {
		defer wg.Done()
		for i := 0; i < 10; i++ {
			WithRWMutex(&mu, func() {
				counter++
			}, true)
			time.Sleep(time.Microsecond)
		}
	}()

	// Readers
	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < 10; j++ {
				WithRWMutex(&mu, func() {
					_ = counter // read
				}, false)
				time.Sleep(time.Microsecond)
			}
		}()
	}

	wg.Wait()
	if counter != 10 {
		t.Errorf("expected counter=10, got %d", counter)
	}
}

func TestWithRWMutex_WriteLock(t *testing.T) {
	t.Parallel()
	var mu sync.RWMutex
	counter := 0
	var wg sync.WaitGroup

	for i := 0; i < 50; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			WithRWMutex(&mu, func() {
				counter++
			}, true)
		}()
	}

	wg.Wait()
	if counter != 50 {
		t.Errorf("expected counter=50, got %d", counter)
	}
}

func TestReviewConcurrency(t *testing.T) {
	t.Parallel()
	result := ReviewConcurrency()
	if len(result.SafePatterns) == 0 {
		t.Error("expected at least one safe pattern")
	}
	// IssuesFound should be empty (no known issues)
	if len(result.IssuesFound) > 0 {
		t.Errorf("unexpected issues found: %v", result.IssuesFound)
	}
}
