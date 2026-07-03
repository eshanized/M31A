package tools

import (
	"fmt"
	"sync"
	"testing"
)

func TestCachedResult_Get(t *testing.T) {
	cache := NewCachedResult[int]()
	calls := 0

	result := cache.Get("key1", func() int {
		calls++
		return 42
	})
	if result != 42 {
		t.Fatalf("CachedResult: got %d, want 42", result)
	}
	if calls != 1 {
		t.Fatalf("CachedResult: expected 1 call, got %d", calls)
	}

	// Second call should use cache
	result2 := cache.Get("key1", func() int {
		calls++
		return 99
	})
	if result2 != 42 {
		t.Fatalf("CachedResult: second call got %d, want 42", result2)
	}
	if calls != 1 {
		t.Fatalf("CachedResult: expected 1 call (cached), got %d", calls)
	}
}

func TestCachedResult_Concurrent(t *testing.T) {
	cache := NewCachedResult[int]()
	var calls sync.WaitGroup
	var count int64

	for i := 0; i < 100; i++ {
		calls.Add(1)
		go func() {
			defer calls.Done()
			cache.Get("shared", func() int {
				count++
				return 1
			})
		}()
	}
	calls.Wait()

	// Should only compute once despite concurrent access
	if count != 1 {
		t.Fatalf("CachedResult concurrent: expected 1 computation, got %d", count)
	}
}

func TestCachedResult_Invalidate(t *testing.T) {
	cache := NewCachedResult[int]()
	calls := 0

	cache.Get("key", func() int {
		calls++
		return 1
	})

	cache.Invalidate("key")

	cache.Get("key", func() int {
		calls++
		return 2
	})

	if calls != 2 {
		t.Fatalf("CachedResult Invalidate: expected 2 calls, got %d", calls)
	}
}

func TestCachedResult_InvalidateAll(t *testing.T) {
	cache := NewCachedResult[string]()

	cache.Get("a", func() string { return "A" })
	cache.Get("b", func() string { return "B" })

	cache.InvalidateAll()

	calls := 0
	cache.Get("a", func() string {
		calls++
		return "A2"
	})
	cache.Get("b", func() string {
		calls++
		return "B2"
	})

	if calls != 2 {
		t.Fatalf("CachedResult InvalidateAll: expected 2 calls, got %d", calls)
	}
}

func TestBatchOperation(t *testing.T) {
	var batches [][]int
	items := []int{1, 2, 3, 4, 5, 6, 7}

	err := BatchOperation(items, 3, func(batch []int) error {
		batchCopy := make([]int, len(batch))
		copy(batchCopy, batch)
		batches = append(batches, batchCopy)
		return nil
	})
	if err != nil {
		t.Fatalf("BatchOperation: unexpected error: %v", err)
	}

	if len(batches) != 3 {
		t.Fatalf("BatchOperation: expected 3 batches, got %d", len(batches))
	}
	// First batch: [1,2,3], second: [4,5,6], third: [7]
	if len(batches[0]) != 3 || batches[0][0] != 1 {
		t.Fatalf("BatchOperation: first batch = %v, want [1,2,3]", batches[0])
	}
	if len(batches[2]) != 1 || batches[2][0] != 7 {
		t.Fatalf("BatchOperation: third batch = %v, want [7]", batches[2])
	}
}

func TestBatchOperation_Error(t *testing.T) {
	items := []int{1, 2, 3, 4, 5}

	err := BatchOperation(items, 2, func(batch []int) error {
		if batch[0] == 3 {
			return fmt.Errorf("error on batch starting with %d", batch[0])
		}
		return nil
	})
	if err == nil {
		t.Fatal("BatchOperation: expected error, got nil")
	}
}

func TestBatchOperation_ZeroBatchSize(t *testing.T) {
	var batches int
	BatchOperation([]int{1, 2, 3}, 0, func(batch []int) error {
		batches++
		return nil
	})
	// Default batch size is 10, so all 3 items in 1 batch
	if batches != 1 {
		t.Fatalf("BatchOperation zero size: expected 1 batch, got %d", batches)
	}
}

func TestSortedStrings(t *testing.T) {
	input := []string{"c", "a", "b"}
	result := SortedStrings(input)
	if len(result) != 3 || result[0] != "a" || result[1] != "b" || result[2] != "c" {
		t.Fatalf("SortedStrings: got %v, want [a b c]", result)
	}
	// Original should not be modified
	if input[0] != "c" {
		t.Fatalf("SortedStrings: modified original")
	}
}

func TestSortedStrings_Empty(t *testing.T) {
	result := SortedStrings(nil)
	if result != nil {
		t.Fatalf("SortedStrings(nil) = %v, want nil", result)
	}
}

func TestDeduplicate(t *testing.T) {
	input := []string{"a", "b", "a", "c", "b"}
	result := Deduplicate(input)
	if len(result) != 3 {
		t.Fatalf("Deduplicate: got %d items, want 3", len(result))
	}
	expected := []string{"a", "b", "c"}
	for i, s := range result {
		if s != expected[i] {
			t.Fatalf("Deduplicate: result[%d] = %q, want %q", i, s, expected[i])
		}
	}
}

func TestDeduplicate_NoDuplicates(t *testing.T) {
	input := []string{"a", "b", "c"}
	result := Deduplicate(input)
	if len(result) != 3 {
		t.Fatalf("Deduplicate no dupes: got %d, want 3", len(result))
	}
}

func TestDeduplicate_Empty(t *testing.T) {
	result := Deduplicate(nil)
	if result != nil {
		t.Fatalf("Deduplicate(nil) = %v, want nil", result)
	}
}
