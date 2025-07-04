package openrouter

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

func TestNew_ValidKey(t *testing.T) {
	c, err := New("sk-or-v1-testkey", Options{})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if c == nil {
		t.Fatal("expected non-nil client")
	}
	if c.Name() != "openrouter" {
		t.Fatalf("expected name %q, got %q", "openrouter", c.Name())
	}
}

func TestNew_EmptyKey(t *testing.T) {
	_, err := New("", Options{})
	if err != m31errors.ErrInvalidKey {
		t.Fatalf("expected ErrInvalidKey, got %v", err)
	}
}

func TestHealthCheck_Live(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/auth/key" {
			w.WriteHeader(http.StatusOK)
			w.Write([]byte(`{"status":"ok"}`))
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.baseURL = ts.URL

	status := c.HealthCheck(context.Background())
	if status.Status != "live" {
		t.Fatalf("expected status %q, got %q", "live", status.Status)
	}
	if status.LatencyMs >= 2000 {
		t.Fatalf("expected latency < 2000ms, got %dms", status.LatencyMs)
	}
}

func TestHealthCheck_Slow(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/auth/key" {
			time.Sleep(2500 * time.Millisecond)
			w.WriteHeader(http.StatusOK)
			w.Write([]byte(`{"status":"ok"}`))
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.baseURL = ts.URL

	status := c.HealthCheck(context.Background())
	if status.Status != "slow" {
		t.Fatalf("expected status %q, got %q", "slow", status.Status)
	}
}

func TestHealthCheck_Offline(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/auth/key" {
			w.WriteHeader(http.StatusUnauthorized)
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.baseURL = ts.URL

	status := c.HealthCheck(context.Background())
	if status.Status != "offline" {
		t.Fatalf("expected status %q, got %q", "offline", status.Status)
	}
}

func TestFetchModels_PopulatesCache(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/models" {
			resp := map[string][]map[string]any{
				"data": {
					{
						"id":             "deepseek/deepseek-r1",
						"name":           "DeepSeek R1",
						"context_length": float64(65536),
						"top_provider":   "DeepSeek",
						"pricing":        map[string]any{"prompt_token": 0.00000055, "completion_token": 0.00000219},
						"architecture":   map[string]any{"modality": "text", "tokenizer": "gpt"},
					},
				},
			}
			json.NewEncoder(w).Encode(resp)
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.baseURL = ts.URL

	models, err := c.FetchModels(context.Background())
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(models) == 0 {
		t.Fatal("expected at least 1 model")
	}

	m, err := c.GetModel("deepseek/deepseek-r1")
	if err != nil {
		t.Fatalf("expected to find model in cache: %v", err)
	}
	if m.ContextLength != 65536 {
		t.Fatalf("expected context length 65536, got %d", m.ContextLength)
	}
}

func TestFetchModels_StaleFallback(t *testing.T) {
	callCount := 0
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		callCount++
		if r.URL.Path == "/models" && callCount == 1 {
			resp := map[string][]map[string]any{
				"data": {
					{
						"id":             "test/model",
						"name":           "Test Model",
						"context_length": float64(4096),
						"top_provider":   "Test",
						"pricing":        map[string]any{"prompt_token": 0.000001, "completion_token": 0.000002},
						"architecture":   map[string]any{"modality": "text", "tokenizer": "gpt"},
					},
				},
			}
			json.NewEncoder(w).Encode(resp)
		} else {
			w.WriteHeader(http.StatusInternalServerError)
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.baseURL = ts.URL

	_, err := c.FetchModels(context.Background())
	if err != nil {
		t.Fatalf("first fetch should succeed: %v", err)
	}

	// Second fetch should return cached data (not error)
	models, err := c.FetchModels(context.Background())
	if err != nil {
		t.Fatalf("stale fallback should not error: %v", err)
	}
	if len(models) != 0 {
		t.Log("stale fallback returned cached models")
	}
}

func TestEstimateCost_UnknownModel(t *testing.T) {
	c, _ := New("test-key", Options{})
	cost := c.EstimateCost("nonexistent/model", types.Usage{PromptTokens: 100, CompletionTokens: 50})
	if cost != 0 {
		t.Fatalf("expected 0 for unknown model, got %f", cost)
	}
}

func TestEstimateCost_KnownModel(t *testing.T) {
	c, _ := New("test-key", Options{})

	// Manually populate cache with a model that has pricing data
	c.cache.Set([]types.ModelInfo{
		{
			ID:   "test/model",
			Name: "Test Model",
			Pricing: types.Pricing{
				InputPerMToken:  1.0, // $1 per million input tokens
				OutputPerMToken: 2.0, // $2 per million output tokens
			},
		},
	})

	// 1000 prompt tokens + 500 completion tokens
	cost := c.EstimateCost("test/model", types.Usage{PromptTokens: 1000, CompletionTokens: 500})
	expected := (1000.0/1_000_000)*1.0 + (500.0/1_000_000)*2.0
	if cost != expected {
		t.Fatalf("expected %f, got %f", expected, cost)
	}
}

func TestFetchModels_CacheHit(t *testing.T) {
	callCount := 0
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		callCount++
		if r.URL.Path == "/models" {
			resp := map[string][]map[string]any{
				"data": {
					{
						"id":             "test/model",
						"name":           "Test Model",
						"context_length": float64(4096),
						"top_provider":   "Test",
						"pricing":        map[string]any{"prompt_token": 0.000001, "completion_token": 0.000002},
						"architecture":   map[string]any{"modality": "text", "tokenizer": "gpt"},
					},
				},
			}
			json.NewEncoder(w).Encode(resp)
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.baseURL = ts.URL

	// First call populates cache
	_, err := c.FetchModels(context.Background())
	if err != nil {
		t.Fatalf("first fetch should succeed: %v", err)
	}
	if callCount != 1 {
		t.Fatalf("expected 1 API call, got %d", callCount)
	}

	// Second call should use cache
	models, err := c.FetchModels(context.Background())
	if err != nil {
		t.Fatalf("cached fetch should not error: %v", err)
	}
	if callCount != 1 {
		t.Fatalf("expected 0 additional API calls on cache hit, got %d", callCount)
	}
	if len(models) != 1 {
		t.Fatalf("expected 1 model from cache, got %d", len(models))
	}
}

func TestFetchModels_CacheExpired(t *testing.T) {
	callCount := 0
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		callCount++
		if r.URL.Path == "/models" {
			resp := map[string][]map[string]any{
				"data": {
					{
						"id":             "test/model",
						"name":           "Test Model",
						"context_length": float64(4096),
						"top_provider":   "Test",
						"pricing":        map[string]any{"prompt_token": 0.000001, "completion_token": 0.000002},
						"architecture":   map[string]any{"modality": "text", "tokenizer": "gpt"},
					},
				},
			}
			json.NewEncoder(w).Encode(resp)
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.baseURL = ts.URL

	// First call populates cache
	_, err := c.FetchModels(context.Background())
	if err != nil {
		t.Fatalf("first fetch should succeed: %v", err)
	}
	if callCount != 1 {
		t.Fatalf("expected 1 API call, got %d", callCount)
	}

	// Second call should use cache (not expired yet)
	_, err = c.FetchModels(context.Background())
	if err != nil {
		t.Fatalf("cached fetch should succeed: %v", err)
	}
	if callCount != 1 {
		t.Fatalf("expected no API call on cache hit, got %d", callCount)
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

	c, _ := New("test-key", Options{})
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

func TestChatCompletionStream_WithThinking(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/chat/completions" {
			w.Header().Set("Content-Type", "text/event-stream")
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"thinking...\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\" response\"}}]}\n\ndata: [DONE]\n\n"))
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.baseURL = ts.URL

	it, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "deepseek/deepseek-r1",
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
	if chunk.Type != "thinking" {
		t.Fatalf("expected first chunk type %q, got %q", "thinking", chunk.Type)
	}
}

func TestChatCompletionStream_Done(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/chat/completions" {
			w.Header().Set("Content-Type", "text/event-stream")
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("data: [DONE]\n\n"))
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.baseURL = ts.URL

	it, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test/model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	defer it.Close()

	_, err = it.Next()
	if err != io.EOF {
		t.Fatalf("expected EOF, got %v", err)
	}
}

func TestChatCompletionStream_ContextExceeded(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/chat/completions" {
			w.WriteHeader(http.StatusBadRequest)
			w.Write([]byte(`{"error":{"message":"context_length exceeded"}}`))
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.baseURL = ts.URL

	_, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test/model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
	})
	if err != m31errors.ErrContextExceeded {
		t.Fatalf("expected ErrContextExceeded, got %v", err)
	}
}

func TestChatCompletionStream_ReasoningEnabled(t *testing.T) {
	var requestBody map[string]any
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/chat/completions" {
			bodyBytes, _ := io.ReadAll(r.Body)
			json.Unmarshal(bodyBytes, &requestBody)
			w.Header().Set("Content-Type", "text/event-stream")
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("data: {\"choices\":[{\"delta\":{\"reasoning\":\"thinking...\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"response\"}}]}\n\ndata: [DONE]\n\n"))
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.baseURL = ts.URL

	it, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:            "openai/o3-mini",
		Messages:         []types.Message{{Role: "user", Content: "hi"}},
		ReasoningEnabled: true,
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	defer it.Close()

	// Verify the request body contains reasoning params
	if requestBody == nil {
		t.Fatal("expected request body to be captured")
	}

	// openai/o- models should have reasoning_effort param added
	reasoningEffort, ok := requestBody["reasoning_effort"]
	if !ok {
		t.Fatalf("expected reasoning_effort param in request body when ReasoningEnabled is true, got keys: %v", requestBody)
	}
	if reasoningEffort != "medium" {
		t.Fatalf("expected reasoning_effort to be %q, got %q", "medium", reasoningEffort)
	}

	// Verify the stream returns reasoning content first
	chunk, err := it.Next()
	if err != nil {
		t.Fatalf("expected chunk, got error: %v", err)
	}
	if chunk.Type != "thinking" {
		t.Fatalf("expected first chunk type %q, got %q", "thinking", chunk.Type)
	}
}

func TestChatCompletionStream_Headers(t *testing.T) {
	var authHeader, refererHeader, titleHeader, uaHeader string
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/chat/completions" {
			authHeader = r.Header.Get("Authorization")
			refererHeader = r.Header.Get("HTTP-Referer")
			titleHeader = r.Header.Get("X-Title")
			uaHeader = r.Header.Get("User-Agent")
			w.Header().Set("Content-Type", "text/event-stream")
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("data: [DONE]\n\n"))
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
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
	if refererHeader != "https://github.com/eshanized/M31A" {
		t.Fatalf("expected HTTP-Referer header, got %q", refererHeader)
	}
	if titleHeader != "M31A" {
		t.Fatalf("expected X-Title header, got %q", titleHeader)
	}
	if uaHeader == "" {
		t.Fatal("expected User-Agent header")
	}
}
