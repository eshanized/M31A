package zen

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/tests/testutil"
	"github.com/eshanized/M31A/internal/core/types"
)

func TestNew_ValidKey(t *testing.T) {
	testutil.LoadTestDotEnv(t)

	apiKey := os.Getenv("ZEN_API_KEY")
	if apiKey == "" {
		apiKey = "sk-zen-testkey"
	}
	c, err := New(apiKey, Options{})
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
	_, err := New("", Options{})
	if err != m31errors.ErrInvalidKey {
		t.Fatalf("expected ErrInvalidKey, got %v", err)
	}
}

func TestHealthCheck_Live(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/models" {
			w.WriteHeader(http.StatusOK)
			w.Write([]byte(`{"object":"list","data":[{"id":"test/model"}]}`))
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	status := c.HealthCheck(context.Background())
	if status.Status != "live" {
		t.Fatalf("expected status %q, got %q", "live", status.Status)
	}
}

func TestFetchModels_PopulatesCache(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/models" {
			resp := map[string]any{
				"object": "list",
				"data": []map[string]any{
					{
						"id": "deepseek/deepseek-r1",
					},
				},
			}
			json.NewEncoder(w).Encode(resp)
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

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
	// Context length is enriched from OpenRouter or local metadata.
	// Before enrichment it was 128000 (default); after enrichment it may be
	// the real value from OpenRouter. Accept any value > 0.
	if cached.ContextLength <= 0 {
		t.Fatalf("expected positive context length, got %d", cached.ContextLength)
	}
}

func TestFetchModels_CacheHit(t *testing.T) {
	callCount := 0
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		callCount++
		if r.URL.Path == "/models" {
			resp := map[string]any{
				"object": "list",
				"data": []map[string]any{
					{
						"id": "test/model",
					},
				},
			}
			json.NewEncoder(w).Encode(resp)
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

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
	c.BaseURLField = ts.URL

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
	c.Cache.Set([]types.ModelInfo{
		{
			ID:   "test/model",
			Name: "Test Model",
			Pricing: types.Pricing{
				InputPerMToken:  0.5, // $0.5 per million input tokens
				OutputPerMToken: 1.5, // $1.5 per million output tokens
			},
		},
	})

	// 2000 prompt tokens + 1000 completion tokens
	cost := c.EstimateCost("test/model", types.Usage{PromptTokens: 2000, CompletionTokens: 1000})
	expected := (2000.0/1_000_000)*0.5 + (1000.0/1_000_000)*1.5
	if cost != expected {
		t.Fatalf("expected %f, got %f", expected, cost)
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

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

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

func TestFetchModels_StaleFallback(t *testing.T) {
	callCount := 0
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		callCount++
		if r.URL.Path == "/models" && callCount == 1 {
			resp := map[string]any{
				"object": "list",
				"data": []map[string]any{
					{"id": "test/model"},
				},
			}
			json.NewEncoder(w).Encode(resp)
		} else {
			w.WriteHeader(http.StatusInternalServerError)
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	// First call populates cache
	_, err := c.FetchModels(context.Background())
	if err != nil {
		t.Fatalf("first fetch should succeed: %v", err)
	}

	// Second fetch should return cached data via stale fallback
	models, err := c.FetchModels(context.Background())
	if err != nil {
		t.Fatalf("stale fallback should not error: %v", err)
	}
	if len(models) != 1 {
		t.Fatalf("expected 1 model from stale cache, got %d", len(models))
	}
	if models[0].ID != "test/model" {
		t.Fatalf("expected model ID %q, got %q", "test/model", models[0].ID)
	}
}

func TestFetchModels_StaleFallbackNoCache(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusInternalServerError)
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	// No cache, API fails - should return error
	_, err := c.FetchModels(context.Background())
	if err != m31errors.ErrProviderUnreachable {
		t.Fatalf("expected ErrProviderUnreachable, got %v", err)
	}
}

func TestFetchModels_NetworkError(t *testing.T) {
	c, _ := New("test-key", Options{})
	c.BaseURLField = "http://127.0.0.1:1" // unreachable port

	_, err := c.FetchModels(context.Background())
	if err != m31errors.ErrProviderUnreachable {
		t.Fatalf("expected ErrProviderUnreachable on network error, got %v", err)
	}
}

func TestChatCompletionStream_RateLimited(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusTooManyRequests)
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	_, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test/model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
	})
	if err != m31errors.ErrRateLimited {
		t.Fatalf("expected ErrRateLimited, got %v", err)
	}
}

func TestChatCompletionStream_Unauthorized(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusUnauthorized)
		w.Write([]byte(`{"error":"invalid api key"}`))
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	_, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test/model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
	})
	if err != m31errors.ErrInvalidKey {
		t.Fatalf("expected ErrInvalidKey, got %v", err)
	}
}

func TestChatCompletionStream_NoCredits(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusUnauthorized)
		w.Write([]byte(`{"error":"CreditsError: no credits remaining"}`))
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	_, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test/model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
	})
	if err == nil {
		t.Fatal("expected error for no credits")
	}
	if !strings.Contains(err.Error(), "no credits") {
		t.Fatalf("expected 'no credits' error, got %v", err)
	}
}

func TestChatCompletionStream_PaymentError(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusUnauthorized)
		w.Write([]byte(`{"error":"billing issue detected"}`))
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	_, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test/model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
	})
	if err == nil {
		t.Fatal("expected error for payment issue")
	}
	if !strings.Contains(err.Error(), "payment") && !strings.Contains(err.Error(), "billing") {
		t.Fatalf("expected payment/billing error, got %v", err)
	}
}

func TestChatCompletionStream_ServiceUnavailable(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusServiceUnavailable)
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	_, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test/model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
	})
	if err != m31errors.ErrProviderUnreachable {
		t.Fatalf("expected ErrProviderUnreachable, got %v", err)
	}
}

func TestChatCompletionStream_ContextExceeded(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusBadRequest)
		w.Write([]byte(`{"error":{"message":"context_length exceeded"}}`))
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	_, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test/model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
	})
	if err != m31errors.ErrContextExceeded {
		t.Fatalf("expected ErrContextExceeded, got %v", err)
	}
}

func TestChatCompletionStream_ContextError(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusBadRequest)
		w.Write([]byte(`{"error":"context window exceeded"}`))
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	_, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test/model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
	})
	if err != m31errors.ErrContextExceeded {
		t.Fatalf("expected ErrContextExceeded for context error, got %v", err)
	}
}

func TestChatCompletionStream_UnexpectedStatus(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusBadGateway)
		w.Write([]byte(`{"error":"bad gateway"}`))
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	_, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test/model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
	})
	if err == nil {
		t.Fatal("expected error for unexpected status")
	}
	if !strings.Contains(err.Error(), "502") {
		t.Fatalf("expected error to contain status 502, got %v", err)
	}
}

func TestChatCompletionStream_NetworkError(t *testing.T) {
	c, _ := New("test-key", Options{})
	c.BaseURLField = "http://127.0.0.1:1" // unreachable port

	_, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test/model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
	})
	if err == nil {
		t.Fatal("expected error for network failure")
	}
	if !strings.Contains(err.Error(), "send request") {
		t.Fatalf("expected 'send request' error, got %v", err)
	}
}

func TestChatCompletionStream_WithMaxTokens(t *testing.T) {
	var requestBody map[string]any
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		bodyBytes, _ := io.ReadAll(r.Body)
		json.Unmarshal(bodyBytes, &requestBody)
		w.Header().Set("Content-Type", "text/event-stream")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("data: [DONE]\n\n"))
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	it, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:     "test/model",
		Messages:  []types.Message{{Role: "user", Content: "hi"}},
		MaxTokens: 1000,
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	defer it.Close()

	if requestBody == nil {
		t.Fatal("expected request body to be captured")
	}
	maxTokens, ok := requestBody["max_tokens"]
	if !ok {
		t.Fatal("expected max_tokens in request body")
	}
	if maxTokens != float64(1000) {
		t.Fatalf("expected max_tokens 1000, got %v", maxTokens)
	}
}

func TestChatCompletionStream_WithTools(t *testing.T) {
	var requestBody map[string]any
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		bodyBytes, _ := io.ReadAll(r.Body)
		json.Unmarshal(bodyBytes, &requestBody)
		w.Header().Set("Content-Type", "text/event-stream")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("data: [DONE]\n\n"))
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	tools := []provider.ToolDefinition{
		{
			Name:        "get_weather",
			Description: "Get current weather",
			Parameters:  "{}",
		},
	}

	it, err := c.ChatCompletionStream(context.Background(), provider.ChatRequest{
		Model:    "test/model",
		Messages: []types.Message{{Role: "user", Content: "hi"}},
		Tools:    tools,
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	defer it.Close()

	if requestBody == nil {
		t.Fatal("expected request body to be captured")
	}
	_, ok := requestBody["tools"]
	if !ok {
		t.Fatal("expected tools in request body")
	}
}

func TestHealthCheck_Offline(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusUnauthorized)
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	status := c.HealthCheck(context.Background())
	if status.Status != "offline" {
		t.Fatalf("expected status %q, got %q", "offline", status.Status)
	}
	if status.LatencyMs < 0 {
		t.Fatalf("expected non-negative latency, got %d", status.LatencyMs)
	}
}

func TestHealthCheck_Slow(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/models" {
			time.Sleep(1000 * time.Millisecond) // Between live (500ms) and slow threshold (2000ms)
			w.WriteHeader(http.StatusOK)
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	status := c.HealthCheck(context.Background())
	if status.Status != "slow" {
		t.Fatalf("expected status %q, got %q", "slow", status.Status)
	}
	if status.LatencyMs < 500 {
		t.Fatalf("expected latency >= 500ms for slow status, got %d", status.LatencyMs)
	}
}

func TestHealthCheck_Degraded(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/models" {
			time.Sleep(5500 * time.Millisecond)
			w.WriteHeader(http.StatusOK)
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	status := c.HealthCheck(context.Background())
	if status.Status != "degraded" {
		t.Fatalf("expected status %q, got %q", "degraded", status.Status)
	}
	if status.LatencyMs < 5000 {
		t.Fatalf("expected latency >= 5000ms for degraded status, got %d", status.LatencyMs)
	}
}

func TestHealthCheck_NetworkError(t *testing.T) {
	c, _ := New("test-key", Options{})
	c.BaseURLField = "http://127.0.0.1:1" // unreachable port

	status := c.HealthCheck(context.Background())
	if status.Status != "offline" {
		t.Fatalf("expected status %q, got %q", "offline", status.Status)
	}
	if status.Error == "" {
		t.Fatal("expected error message for network failure")
	}
}

func TestGetModel_NotFound(t *testing.T) {
	c, _ := New("test-key", Options{})

	_, err := c.GetModel("nonexistent/model")
	if err != m31errors.ErrModelNotFound {
		t.Fatalf("expected ErrModelNotFound, got %v", err)
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
	c.BaseURLField = ts.URL

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
	c.BaseURLField = ts.URL

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

func TestAPIKey_Masking(t *testing.T) {
	c, _ := New("sk-zen-v1-abcdefgh", Options{})
	got := c.APIKey()
	if got != "****efgh" {
		t.Errorf("APIKey() = %q, want '****efgh'", got)
	}
}

func TestAPIKey_ShortKey_Zen(t *testing.T) {
	c, _ := New("sk-a", Options{})
	got := c.APIKey()
	if got != "****" {
		t.Errorf("APIKey() = %q, want '****' for short key", got)
	}
}

func TestCachedModels_Empty_Zen(t *testing.T) {
	c, _ := New("test-key", Options{})
	models := c.CachedModels()
	if len(models) != 0 {
		t.Errorf("expected 0 cached models, got %d", len(models))
	}
}

func TestCachedModels_AfterFetch_Zen(t *testing.T) {
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/models" {
			resp := map[string]any{
				"object": "list",
				"data": []map[string]any{
					{"id": "test/zen-model"},
				},
			}
			json.NewEncoder(w).Encode(resp)
		}
	}))
	defer ts.Close()

	c, _ := New("test-key", Options{})
	c.BaseURLField = ts.URL

	_, err := c.FetchModels(context.Background())
	if err != nil {
		t.Fatalf("FetchModels failed: %v", err)
	}

	models := c.CachedModels()
	if len(models) != 1 {
		t.Fatalf("expected 1 cached model, got %d", len(models))
	}
	if models[0].ID != "test/zen-model" {
		t.Errorf("cached model ID = %q, want 'test/zen-model'", models[0].ID)
	}
}
