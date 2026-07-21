package provider

import (
	"net/http"
	"testing"
	"time"
)

func TestIsRateLimited(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name string
		resp *http.Response
		want bool
	}{
		{"nil response", nil, false},
		{"200 OK", &http.Response{StatusCode: 200}, false},
		{"429 Too Many", &http.Response{StatusCode: 429}, true},
		{"500 error", &http.Response{StatusCode: 500}, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := IsRateLimited(tt.resp)
			if got != tt.want {
				t.Errorf("IsRateLimited() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestGetRetryAfter(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name string
		resp *http.Response
		want string
	}{
		{"nil response", nil, ""},
		{"no header", &http.Response{Header: http.Header{}}, ""},
		{"with header", &http.Response{Header: http.Header{"Retry-After": {"30"}}}, "30"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := GetRetryAfter(tt.resp)
			if got != tt.want {
				t.Errorf("GetRetryAfter() = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestInspectResponse_RateLimited(t *testing.T) {
	t.Parallel()

	resp := &http.Response{
		StatusCode: 429,
		Header:     http.Header{"Retry-After": {"60"}},
	}

	info := InspectResponse(resp)
	if !info.RateLimited {
		t.Error("expected RateLimited=true")
	}
	if info.RetryAfter != "60" {
		t.Errorf("RetryAfter = %q, want 60", info.RetryAfter)
	}
	if info.Wait != 60*time.Second {
		t.Errorf("Wait = %v, want 60s", info.Wait)
	}
}

func TestInspectResponse_NotRateLimited(t *testing.T) {
	t.Parallel()

	resp := &http.Response{
		StatusCode: 200,
		Header:     http.Header{},
	}

	info := InspectResponse(resp)
	if info.RateLimited {
		t.Error("expected RateLimited=false")
	}
	if info.Wait != 0 {
		t.Errorf("Wait = %v, want 0", info.Wait)
	}
}

func TestInspectResponse_NilResponse(t *testing.T) {
	t.Parallel()

	info := InspectResponse(nil)
	if info.RateLimited {
		t.Error("expected RateLimited=false for nil response")
	}
}

func TestInspectResponse_RetryAfterCap(t *testing.T) {
	t.Parallel()

	resp := &http.Response{
		StatusCode: 429,
		Header:     http.Header{"Retry-After": {"9999"}},
	}

	info := InspectResponse(resp)
	if info.Wait > maxRetryAfter {
		t.Errorf("Wait = %v exceeds maxRetryAfter %v", info.Wait, maxRetryAfter)
	}
}

func TestInspectResponse_InvalidRetryAfter(t *testing.T) {
	t.Parallel()

	resp := &http.Response{
		StatusCode: 429,
		Header:     http.Header{"Retry-After": {"not-a-number"}},
	}

	info := InspectResponse(resp)
	if info.Wait != 0 {
		t.Errorf("Wait = %v, want 0 for non-numeric Retry-After", info.Wait)
	}
}

func TestRollbackActive_MatchesFrom(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a"})
	r.Register("b", &mockProvider{name: "b"})
	r.SetActive("b")

	r.RollbackActive("b", "a")
	if r.Active() != "a" {
		t.Errorf("expected active 'a' after rollback, got %q", r.Active())
	}
}

func TestRollbackActive_NoMatch(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a"})
	r.Register("b", &mockProvider{name: "b"})
	r.SetActive("a")

	r.RollbackActive("b", "a") // fromName doesn't match current active
	if r.Active() != "a" {
		t.Errorf("expected active still 'a', got %q", r.Active())
	}
}

func TestRollbackActive_ToNameNotFound(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a"})
	r.SetActive("a")

	r.RollbackActive("a", "nonexistent") // toName doesn't exist
	if r.Active() != "a" {
		t.Errorf("expected active still 'a' when rollback target missing, got %q", r.Active())
	}
}

func TestRegistry_Get_Found(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a"})

	p, err := r.Get("a")
	if err != nil {
		t.Fatalf("Get failed: %v", err)
	}
	if p.Name() != "a" {
		t.Errorf("Name = %q, want 'a'", p.Name())
	}
}

func TestRegistry_Get_NotFound(t *testing.T) {
	t.Parallel()
	r := NewRegistry()

	_, err := r.Get("nonexistent")
	if err == nil {
		t.Fatal("expected error for nonexistent provider")
	}
}

func TestRegistry_TrySetActive(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a"})
	r.Register("b", &mockProvider{name: "b"})

	p, err := r.TrySetActive("b")
	if err != nil {
		t.Fatalf("TrySetActive failed: %v", err)
	}
	if p.Name() != "b" {
		t.Errorf("Name = %q, want 'b'", p.Name())
	}
	if r.Active() != "b" {
		t.Errorf("Active = %q, want 'b'", r.Active())
	}
}

func TestRegistry_TrySetActive_NotFound(t *testing.T) {
	t.Parallel()
	r := NewRegistry()

	_, err := r.TrySetActive("nonexistent")
	if err == nil {
		t.Fatal("expected error for nonexistent provider")
	}
}

func TestRegistry_ListAll(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	r.Register("b", &mockProvider{name: "b"})
	r.Register("a", &mockProvider{name: "a"})

	names := r.ListAll()
	if len(names) != 2 {
		t.Fatalf("expected 2 names, got %d", len(names))
	}
	if names[0] != "a" || names[1] != "b" {
		t.Errorf("expected sorted [a b], got %v", names)
	}
}

func TestRegistry_RegisterEmptyName(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	err := r.Register("", &mockProvider{name: ""})
	if err == nil {
		t.Fatal("expected error for empty name")
	}
}

func TestRegistry_SetActiveEmptyName(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	err := r.SetActive("")
	if err == nil {
		t.Fatal("expected error for empty name")
	}
}
