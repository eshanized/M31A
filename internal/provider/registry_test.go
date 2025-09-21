package provider

import (
	"context"
	"errors"
	"testing"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

type mockProvider struct {
	name         string
	healthStatus string
}

func (m *mockProvider) Name() string {
	return m.name
}

func (m *mockProvider) APIKey() string {
	return "mock-key"
}

func (m *mockProvider) FetchModels(ctx context.Context) ([]types.ModelInfo, error) {
	return nil, nil
}

func (m *mockProvider) ChatCompletionStream(ctx context.Context, req ChatRequest) (*types.StreamIterator, error) {
	return nil, nil
}

func (m *mockProvider) EstimateCost(modelID string, usage types.Usage) float64 {
	return 0
}

func (m *mockProvider) HealthCheck(ctx context.Context) types.HealthStatus {
	return types.HealthStatus{Status: m.healthStatus}
}

func (m *mockProvider) GetModel(id string) (*types.ModelInfo, error) {
	return nil, nil
}

func TestRegistry_RegisterAndActive(t *testing.T) {
	r := NewRegistry()
	r.Register("test", &mockProvider{name: "test"})

	if active := r.Active(); active != "test" {
		t.Fatalf("expected active %q, got %q", "test", active)
	}

	ap := r.ActiveProvider()
	if ap == nil {
		t.Fatal("expected non-nil ActiveProvider")
	}
	if ap.Name() != "test" {
		t.Fatalf("expected ActiveProvider.Name() %q, got %q", "test", ap.Name())
	}
}

func TestRegistry_SetActive_Unknown(t *testing.T) {
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a"})

	err := r.SetActive("nonexistent")
	if err == nil || !errors.Is(err, m31errors.ErrProviderNotFound) {
		t.Fatalf("expected ErrProviderNotFound, got %v", err)
	}

	if active := r.Active(); active != "a" {
		t.Fatalf("expected active still %q, got %q", "a", active)
	}
}

func TestRegistry_List(t *testing.T) {
	r := NewRegistry()
	r.Register("b", &mockProvider{name: "b"})
	r.Register("a", &mockProvider{name: "a"})

	names := r.List()
	if len(names) != 2 {
		t.Fatalf("expected 2 names, got %d", len(names))
	}
	if names[0] != "a" {
		t.Fatalf("expected first name %q, got %q", "a", names[0])
	}
	// "b" was registered first, so it's the active provider — List() appends " (active)"
	if names[1] != "b (active)" {
		t.Fatalf("expected second name %q, got %q", "b (active)", names[1])
	}
}

func TestFindFallbackProvider_Switches(t *testing.T) {
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a", healthStatus: "offline"})
	r.Register("b", &mockProvider{name: "b", healthStatus: "live"})
	r.SetActive("a")

	newName, event, err := FindFallbackProvider(r, "a")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if newName != "b" {
		t.Fatalf("expected fallback to %q, got %q", "b", newName)
	}
	if event.From != "a" {
		t.Fatalf("expected From %q, got %q", "a", event.From)
	}
	if event.To != "b" {
		t.Fatalf("expected To %q, got %q", "b", event.To)
	}
	if r.Active() != "b" {
		t.Fatalf("expected active to be %q, got %q", "b", r.Active())
	}
}

func TestFindFallbackProvider_NoAlternative(t *testing.T) {
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a", healthStatus: "offline"})
	r.SetActive("a")

	_, _, err := FindFallbackProvider(r, "a")
	if err != m31errors.ErrProviderUnreachable {
		t.Fatalf("expected ErrProviderUnreachable, got %v", err)
	}
}
