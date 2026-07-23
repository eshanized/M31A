package exec_test

import (
	"github.com/eshanized/M31A/internal/tools/exec"
	"sync"
	"sync/atomic"
	"testing"
)

func TestWithMutex(t *testing.T) {
	var mu sync.Mutex
	var counter int

	var wg sync.WaitGroup
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			exec.WithMutex(&mu, func() {
				counter++
			})
		}()
	}
	wg.Wait()

	if counter != 100 {
		t.Errorf("WithMutex: counter = %d, want 100", counter)
	}
}

func TestWithRWMutex_Read(t *testing.T) {
	var mu sync.RWMutex
	value := 42

	// Multiple readers should not block each other
	var wg sync.WaitGroup
	var readCount atomic.Int32
	for i := 0; i < 50; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			exec.WithRWMutex(&mu, func() {
				_ = value
				readCount.Add(1)
			}, false)
		}()
	}
	wg.Wait()

	if readCount.Load() != 50 {
		t.Errorf("WithRWMutex read: readCount = %d, want 50", readCount.Load())
	}
}

func TestWithRWMutex_Write(t *testing.T) {
	var mu sync.RWMutex
	var counter int

	var wg sync.WaitGroup
	for i := 0; i < 100; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			exec.WithRWMutex(&mu, func() {
				counter++
			}, true)
		}()
	}
	wg.Wait()

	if counter != 100 {
		t.Errorf("WithRWMutex write: counter = %d, want 100", counter)
	}
}

func TestReviewConcurrency(t *testing.T) {
	result := exec.ReviewConcurrency()
	if len(result.SafePatterns) == 0 {
		t.Error("ReviewConcurrency: expected at least one safe pattern")
	}
	if len(result.IssuesFound) != 0 {
		t.Errorf("ReviewConcurrency: expected no issues, got %d", len(result.IssuesFound))
	}
}
