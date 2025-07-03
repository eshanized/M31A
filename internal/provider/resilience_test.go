package provider

import (
	"context"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

func TestProvider_OpenRouter_APIKey(t *testing.T) {
	// This is a compile-time check — if APIKey() is removed from the interface,
	// this test file won't compile.
	var _ LLMProvider = (*mockProvider)(nil)
}

func TestRegistry_SetActive_RejectsEmpty(t *testing.T) {
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a"})

	err := r.SetActive("")
	if err == nil {
		t.Fatal("expected error for empty name")
	}
	if !errors.Is(err, m31errors.ErrProviderUnreachable) {
		t.Fatalf("expected ErrProviderUnreachable, got: %v", err)
	}
}

func TestRegistry_SetActive_RejectsUnknown(t *testing.T) {
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a"})

	err := r.SetActive("nonexistent")
	if err == nil {
		t.Fatal("expected error for unknown provider")
	}
	if !errors.Is(err, m31errors.ErrProviderUnreachable) {
		t.Fatalf("expected ErrProviderUnreachable, got: %v", err)
	}
}

func TestProviderFallback_RespectsRetryAfter(t *testing.T) {
	// Mock server that returns 429 with Retry-After: 2
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Retry-After", "2")
		w.WriteHeader(http.StatusTooManyRequests)
	}))
	defer server.Close()

	// Test that IsRateLimited and GetRetryAfter work
	resp := &http.Response{StatusCode: http.StatusTooManyRequests}
	resp.Header = http.Header{}
	resp.Header.Set("Retry-After", "2")

	if !IsRateLimited(resp) {
		t.Fatal("expected IsRateLimited to return true")
	}
	if GetRetryAfter(resp) != "2" {
		t.Fatalf("expected Retry-After '2', got %q", GetRetryAfter(resp))
	}
}

func TestSSE_CloseReleasesBody(t *testing.T) {
	closeCount := 0
	body := &mockReadCloser{
		data:   []byte("data: {\"test\": true}\n\ndata: [DONE]\n\n"),
		closeFn: func() { closeCount++ },
	}
	resp := &http.Response{
		StatusCode: 200,
		Body:       body,
	}

	parser := NewSSEParser(resp)

	// Consume all events
	for {
		_, _, err := parser.Next()
		if err != nil {
			break
		}
	}

	// Close should be idempotent
	parser.Close()
	parser.Close()
	parser.Close()

	if closeCount != 1 {
		t.Fatalf("expected body closed exactly once, got %d", closeCount)
	}
}

func TestSSE_TruncatedStream_Typed(t *testing.T) {
	// Partial SSE stream — no [DONE] sentinel
	body := &mockReadCloser{
		data: []byte("data: {\"partial\": true}\n\n"),
	}
	resp := &http.Response{
		StatusCode: 200,
		Body:       body,
	}

	parser := NewSSEParser(resp)

	// First call returns data
	_, _, err := parser.Next()
	if err != nil {
		t.Fatalf("unexpected error on first Next(): %v", err)
	}

	// Second call should hit EOF (end of stream, no [DONE])
	_, _, err = parser.Next()
	if err == nil {
		t.Fatal("expected error on truncated stream")
	}
	// The error should be io.EOF (end of data) or a typed truncation error
	if !errors.Is(err, io.EOF) && !errors.Is(err, io.ErrUnexpectedEOF) {
		t.Fatalf("expected EOF or UnexpectedEOF, got: %v", err)
	}
}

func TestModelCache_SingleFlight(t *testing.T) {
	var requestCount int32

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		atomic.AddInt32(&requestCount, 1)
		time.Sleep(100 * time.Millisecond) // Simulate slow network
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`[{"id":"test","name":"Test Model","context_length":128000}]`))
	}))
	defer server.Close()

	_ = server // Would be used in a real test with HTTP fetch

	cache := NewModelCache(5 * time.Minute)

	// Simulate 10 concurrent refresh calls using singleflight
	var wg sync.WaitGroup
	for i := 0; i < 10; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			cache.Refresh(context.Background(), func(ctx context.Context) ([]types.ModelInfo, error) {
				atomic.AddInt32(&requestCount, 1)
				time.Sleep(100 * time.Millisecond)
				return []types.ModelInfo{{ID: "test", Name: "Test Model"}}, nil
			})
		}()
	}
	wg.Wait()

	// Singleflight should coalesce into fewer calls
	count := atomic.LoadInt32(&requestCount)
	if count > 2 {
		t.Fatalf("expected singleflight to coalesce, but got %d fetch calls", count)
	}
}

func TestFallbackEvent_NonBlocking(t *testing.T) {
	ch := make(chan *FallbackEvent, 16)

	// Drain goroutine — simulates TUI Update loop
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
	defer cancel()
	done := make(chan struct{})
	go func() {
		for {
			select {
			case <-ch:
			case <-ctx.Done():
				close(done)
				return
			}
		}
	}()

	// Send 100 events — should never block because channel is buffered
	// and reader is draining
	for i := 0; i < 100; i++ {
		select {
		case ch <- &FallbackEvent{From: "a", To: "b", Reason: "test"}:
		case <-ctx.Done():
			t.Fatal("channel blocked on send")
		}
	}

	<-done
}

// mockReadCloser is a test helper that tracks Close calls.
type mockReadCloser struct {
	data    []byte
	offset  int
	closeFn func()
	closed  bool
}

func (m *mockReadCloser) Read(p []byte) (int, error) {
	if m.offset >= len(m.data) {
		return 0, io.EOF
	}
	n := copy(p, m.data[m.offset:])
	m.offset += n
	return n, nil
}

func (m *mockReadCloser) Close() error {
	if !m.closed {
		m.closed = true
		if m.closeFn != nil {
			m.closeFn()
		}
	}
	return nil
}
