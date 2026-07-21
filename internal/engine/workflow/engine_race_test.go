package workflow

import (
	"sync"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
)

// TestConcurrentSetModelAndProviderAndModel verifies that SetModel and
// providerAndModel do not race when called concurrently. Run with -race.
func TestConcurrentSetModelAndProviderAndModel(t *testing.T) {
	engine, _ := setupTestEngine(t)

	var wg sync.WaitGroup
	const goroutines = 10
	const iterations = 100

	// Concurrent writers: SetModel
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				p := &mockProviderWithModel{
					model: &types.ModelInfo{ID: "model-writer"},
				}
				engine.SetModel("model-writer", p)
			}
		}(i)
	}

	// Concurrent readers: providerAndModel
	for i := 0; i < goroutines; i++ {
		wg.Add(1)
		go func(id int) {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				p, m := engine.providerAndModel()
				// If provider is non-nil, model must be consistent
				if p != nil && m == "" {
					t.Errorf("goroutine %d iter %d: provider set but modelID empty", id, j)
				}
			}
		}(i)
	}

	wg.Wait()
}

// TestProviderAndModelConsistency verifies that providerAndModel returns
// a consistent (provider, modelID) pair.
func TestProviderAndModelConsistency(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Set a known model and provider
	p := &mockProviderWithModel{
		model: &types.ModelInfo{ID: "test-model"},
	}
	engine.SetModel("test-model", p)

	gotP, gotM := engine.providerAndModel()
	if gotP != p {
		t.Errorf("provider mismatch: got %v, want %v", gotP, p)
	}
	if gotM != "test-model" {
		t.Errorf("modelID mismatch: got %q, want %q", gotM, "test-model")
	}
}

// TestSetModelNilProviderDoesNotOverwrite verifies that SetModel with a nil
// provider preserves the existing provider.
func TestSetModelNilProviderDoesNotOverwrite(t *testing.T) {
	engine, _ := setupTestEngine(t)

	// Set initial provider
	initial := &mockProviderWithModel{
		model: &types.ModelInfo{ID: "initial"},
	}
	engine.SetModel("initial", initial)

	// Set with nil provider should not overwrite
	engine.SetModel("updated", nil)

	gotP, gotM := engine.providerAndModel()
	if gotP != initial {
		t.Errorf("nil provider should not overwrite existing; got %v, want %v", gotP, initial)
	}
	if gotM != "updated" {
		t.Errorf("modelID should update even with nil provider; got %q, want %q", gotM, "updated")
	}
}
