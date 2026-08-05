package zen

import (
	"context"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/provider"
)

func TestChatCompletionStream_Retry(t *testing.T) {
	attempts := 0
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		attempts++
		if attempts <= 1 {
			w.WriteHeader(http.StatusBadGateway)
			w.Write([]byte(`{"error": "bad gateway"}`)) //nolint:errcheck
			return
		}
		// Second attempt succeeds with minimal SSE response
		w.Header().Set("Content-Type", "text/event-stream")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n")) //nolint:errcheck
	}))
	defer server.Close()

	c, err := New("test-key", Options{BaseURL: server.URL})
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	iter, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test-model",
		Messages: []types.Message{{Role: "user", Content: "hello"}},
	})
	if err != nil {
		t.Fatalf("ChatCompletionStream() error = %v", err)
	}
	defer func() {
		if err := iter.Close(); err != nil {
			t.Logf("close stream iterator: %v", err)
		}
	}()

	// Consume the stream
	for {
		chunk, err := iter.Next()
		if err != nil {
			break
		}
		_ = chunk
	}

	if attempts != 2 {
		t.Errorf("expected 2 attempts (1 retry), got %d", attempts)
	}
}
