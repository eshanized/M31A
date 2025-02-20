package zen

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

func TestNew_ValidKey(t *testing.T) {
	c, err := New("sk-zen-testkey")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if c == nil {
		t.Fatal("expected non-nil client")
	}
	if c.Name() != "zen" {
		t.Fatalf("expected name %q, got %q", "zen", c.Name())
	}
}

func TestNew_EmptyKey(t *testing.T) {
	_, err := New("")
	if err != m31errors.ErrInvalidKey {
		t.Fatalf("expected ErrInvalidKey, got %v", err)
	}
}

func TestHealthCheck_Live(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/models" {
			w.WriteHeader(http.StatusOK)
			w.Write([]byte(`[{"id":"test/model","name":"Test"}]`))
		}
	}))
	defer ts.Close()

	c, _ := New("test-key")
	c.baseURL = ts.URL

	status := c.HealthCheck(context.Background())
	if status.Status != "live" {
		t.Fatalf("expected status %q, got %q", "live", status.Status)
	}
}

func TestFetchModels_PopulatesCache(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/models" {
			resp := []map[string]any{
				{
					"id":              "deepseek/deepseek-r1",
					"name":            "DeepSeek R1",
					"context_length":  float64(65536),
					"pricing_prompt":  0.00000055,
					"pricing_completion": 0.00000219,
				},
			}
			json.NewEncoder(w).Encode(resp)
		}
	}))
	defer ts.Close()

	c, _ := New("test-key")
	c.baseURL = ts.URL

	models, err := c.FetchModels(context.Background())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(models) == 0 {
		t.Fatal("expected at least 1 model")
	}

	m := models[0]
	if m.TopProvider != "zen" {
		t.Fatalf("expected TopProvider %q, got %q", "zen", m.TopProvider)
	}

	cached, err := c.GetModel("deepseek/deepseek-r1")
	if err != nil {
		t.Fatalf("expected to find model in cache: %v", err)
	}
	if cached.ContextLength != 65536 {
		t.Fatalf("expected context length 65536, got %d", cached.ContextLength)
	}
}

func TestChatCompletionStream_ContentOnly(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/chat/completions" {
			w.Header().Set("Content-Type", "text/event-stream")
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\ndata: [DONE]\n\n"))
		}
	}))
	defer ts.Close()

	c, _ := New("test-key")
	c.baseURL = ts.URL

	it, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test/model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	defer it.Close()

	chunk, err := it.Next()
	if err != nil {
		t.Fatalf("expected chunk, got error: %v", err)
	}
	if chunk.Type != "content" {
		t.Fatalf("expected type %q, got %q", "content", chunk.Type)
	}
	if chunk.Delta != "Hello" {
		t.Fatalf("expected delta %q, got %q", "Hello", chunk.Delta)
	}

	_, err = it.Next()
	if err != io.EOF {
		t.Fatalf("expected EOF, got %v", err)
	}
}

func TestEstimateCost(t *testing.T) {
	c, _ := New("test-key")
	cost := c.EstimateCost(types.Usage{PromptTokens: 100, CompletionTokens: 50})
	if cost != 0 {
		t.Fatalf("expected 0, got %f", cost)
	}
}

func TestChatCompletionStream_Headers(t *testing.T) {
	var authHeader, refererHeader, titleHeader string
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/chat/completions" {
			authHeader = r.Header.Get("Authorization")
			refererHeader = r.Header.Get("HTTP-Referer")
			titleHeader = r.Header.Get("X-Title")
			w.Header().Set("Content-Type", "text/event-stream")
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("data: [DONE]\n\n"))
		}
	}))
	defer ts.Close()

	c, _ := New("test-key")
	c.baseURL = ts.URL

	it, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test/model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	defer it.Close()

	if !strings.Contains(authHeader, "Bearer ") {
		t.Fatal("expected Authorization header")
	}
	if refererHeader != "" {
		t.Fatalf("expected no HTTP-Referer header for Zen, got %q", refererHeader)
	}
	if titleHeader != "" {
		t.Fatalf("expected no X-Title header for Zen, got %q", titleHeader)
	}
}
