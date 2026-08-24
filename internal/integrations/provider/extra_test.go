//go:build ignore

package provider

import (
	"context"
	"io"
	"net/http"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/tests/testutil"
)

// ── getSharedTransport ───────────────────────────────────────────────────────

func TestGetSharedTransport(t *testing.T) {
	t.Parallel()
	a := getSharedTransport()
	b := getSharedTransport()
	if a != b {
		t.Error("expected same transport instance")
	}
	if a == nil {
		t.Error("expected non-nil transport")
	}
}

func TestGetSharedTransport_Idempotent(t *testing.T) {
	t.Parallel()
	for i := 0; i < 10; i++ {
		transport := getSharedTransport()
		if transport == nil {
			t.Fatal("transport nil on iteration", i)
		}
	}
}

// ── NewBaseClient ────────────────────────────────────────────────────────────

func TestNewBaseClient_Defaults(t *testing.T) {
	t.Parallel()
	testutil.LoadTestDotEnv(t)

	apiKey := os.Getenv("OPENROUTER_API_KEY")
	if apiKey == "" {
		apiKey = "sk-test12345678"
	}
	c := NewBaseClient(apiKey, "https://api.example.com", "1.0.0", 0, 0, 0, 0)

	if c.APIKeyField != apiKey {
		t.Errorf("APIKeyField = %q", c.APIKeyField)
	}
	if c.BaseURLField != "https://api.example.com" {
		t.Errorf("BaseURLField = %q", c.BaseURLField)
	}
	if c.Version != "1.0.0" {
		t.Errorf("Version = %q", c.Version)
	}
	if c.HTTPClient == nil {
		t.Error("HTTPClient is nil")
	}
	if c.HTTPClient.Transport != getSharedTransport() {
		t.Error("expected shared transport")
	}
	if c.Cache == nil {
		t.Error("Cache is nil")
	}
	if c.HealthLiveMs == 0 {
		t.Error("HealthLiveMs should default to non-zero")
	}
	if c.HealthSlowMs == 0 {
		t.Error("HealthSlowMs should default to non-zero")
	}
}

func TestNewBaseClient_CustomValues(t *testing.T) {
	t.Parallel()
	c := NewBaseClient("key", "url", "2.0", 10*time.Minute, 30*time.Minute, 500, 2000)

	if c.HealthLiveMs != 500 {
		t.Errorf("HealthLiveMs = %d, want 500", c.HealthLiveMs)
	}
	if c.HealthSlowMs != 2000 {
		t.Errorf("HealthSlowMs = %d, want 2000", c.HealthSlowMs)
	}
}

// ── APIKey ───────────────────────────────────────────────────────────────────

func TestAPIKey_Masked(t *testing.T) {
	t.Parallel()
	testutil.LoadTestDotEnv(t)

	apiKey := os.Getenv("OPENROUTER_API_KEY")
	if apiKey == "" {
		apiKey = "sk-abc123def456ghi7"
	}
	c := NewBaseClient(apiKey, "", "", 0, 0, 0, 0)
	masked := c.APIKey()
	if !strings.HasPrefix(masked, "****") {
		t.Errorf("APIKey() = %q, should start with ****", masked)
	}
	if strings.Contains(masked, "abc") {
		t.Error("API key not properly masked")
	}
}

func TestAPIKey_ShortKey(t *testing.T) {
	t.Parallel()
	c := NewBaseClient("abc", "", "", 0, 0, 0, 0)
	masked := c.APIKey()
	if masked != "****" {
		t.Errorf("APIKey() = %q, want '****'", masked)
	}
}

func TestAPIKey_EmptyKey(t *testing.T) {
	t.Parallel()
	c := NewBaseClient("", "", "", 0, 0, 0, 0)
	masked := c.APIKey()
	if masked != "****" {
		t.Errorf("APIKey() = %q, want '****'", masked)
	}
}

func TestAPIKey_ExactFourChars(t *testing.T) {
	t.Parallel()
	c := NewBaseClient("abcd", "", "", 0, 0, 0, 0)
	masked := c.APIKey()
	if masked != "****" {
		t.Errorf("APIKey() = %q, want '****'", masked)
	}
}

// ── EstimateCost (BaseClient method) ────────────────────────────────────────

func TestBaseClient_EstimateCost(t *testing.T) {
	t.Parallel()
	c := NewBaseClient("", "", "", 0, 0, 0, 0)
	c.Cache.Set([]types.ModelInfo{
		{ID: "m1", Pricing: types.Pricing{InputPerMToken: 3.0, OutputPerMToken: 15.0}},
	})

	cost := c.EstimateCost("m1", types.Usage{PromptTokens: 1000, CompletionTokens: 200})
	expected := (1000.0/1_000_000)*3.0 + (200.0/1_000_000)*15.0
	if cost != expected {
		t.Errorf("EstimateCost = %f, want %f", cost, expected)
	}
}

func TestBaseClient_EstimateCost_MissingModel(t *testing.T) {
	t.Parallel()
	c := NewBaseClient("", "", "", 0, 0, 0, 0)
	cost := c.EstimateCost("nonexistent", types.Usage{PromptTokens: 100})
	if cost != 0 {
		t.Errorf("expected 0 for missing model, got %f", cost)
	}
}

// ── GetModel (BaseClient method) ────────────────────────────────────────────

func TestBaseClient_GetModel_Found(t *testing.T) {
	t.Parallel()
	c := NewBaseClient("", "", "", 0, 0, 0, 0)
	c.Cache.Set([]types.ModelInfo{{ID: "m1", Name: "Model 1"}})

	m, err := c.GetModel("m1")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if m.Name != "Model 1" {
		t.Errorf("Name = %q, want Model 1", m.Name)
	}
}

func TestBaseClient_GetModel_NotFound(t *testing.T) {
	t.Parallel()
	c := NewBaseClient("", "", "", 0, 0, 0, 0)
	_, err := c.GetModel("missing")
	if err == nil {
		t.Fatal("expected error for missing model")
	}
}

// ── CachedModels (BaseClient method) ────────────────────────────────────────

func TestBaseClient_CachedModels(t *testing.T) {
	t.Parallel()
	c := NewBaseClient("", "", "", 0, 0, 0, 0)
	c.Cache.Set([]types.ModelInfo{{ID: "a"}, {ID: "b"}, {ID: "c"}})

	models := c.CachedModels()
	if len(models) != 3 {
		t.Fatalf("expected 3, got %d", len(models))
	}
}

func TestBaseClient_CachedModels_Empty(t *testing.T) {
	t.Parallel()
	c := NewBaseClient("", "", "", 0, 0, 0, 0)
	models := c.CachedModels()
	if len(models) != 0 {
		t.Errorf("expected 0, got %d", len(models))
	}
}

// ── MakeIterator ─────────────────────────────────────────────────────────────

func TestBaseClient_MakeIterator(t *testing.T) {
	t.Parallel()
	resp := bodyReader("data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n")
	sse := NewSSEParserWithContext(resp, context.Background())

	c := NewBaseClient("", "", "", 0, 0, 0, 0)
	iter := c.MakeIterator(sse, "any/model")

	if iter == nil {
		t.Fatal("expected non-nil iterator")
	}
	if iter.Next == nil {
		t.Fatal("expected non-nil Next")
	}
	if iter.Close == nil {
		t.Fatal("expected non-nil Close")
	}

	chunk, err := iter.Next()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if chunk == nil {
		t.Fatal("expected non-nil chunk")
	}
	if chunk.Type != "content" {
		t.Errorf("expected 'content', got %q", chunk.Type)
	}
	if chunk.Delta != "hi" {
		t.Errorf("expected 'hi', got %q", chunk.Delta)
	}

	iter.Close()
}

func TestBaseClient_MakeIterator_EmptyData(t *testing.T) {
	t.Parallel()
	// Send an event with only event type but no data lines, followed by EOF.
	// The parser now recursively skips empty events, so this results in EOF.
	resp := bodyReader("event: ping\n\n")
	sse := NewSSEParserWithContext(resp, context.Background())

	c := NewBaseClient("", "", "", 0, 0, 0, 0)
	iter := c.MakeIterator(sse, "any/model")

	_, err := iter.Next()
	if err == nil {
		t.Fatal("expected EOF for empty event stream, got nil error")
	}
}

// ── Cache FetchTime ──────────────────────────────────────────────────────────

func TestCache_FetchTime(t *testing.T) {
	t.Parallel()
	c := NewModelCache(5 * time.Minute)
	before := time.Now()
	c.Set([]types.ModelInfo{{ID: "m1"}})
	after := time.Now()

	ft := c.FetchTime()
	if ft.Before(before) || ft.After(after) {
		t.Errorf("FetchTime %v not between %v and %v", ft, before, after)
	}
}

func TestCache_FetchTime_Empty(t *testing.T) {
	t.Parallel()
	c := NewModelCache(5 * time.Minute)
	ft := c.FetchTime()
	if !ft.IsZero() {
		t.Errorf("expected zero time for empty cache, got %v", ft)
	}
}

// ── Cache Models (snapshot) ──────────────────────────────────────────────────

func TestCache_Models_Snapshot(t *testing.T) {
	t.Parallel()
	c := NewModelCache(5 * time.Minute)
	c.Set([]types.ModelInfo{{ID: "x"}, {ID: "y"}})

	snapshot := c.Models()
	if len(snapshot) != 2 {
		t.Fatalf("expected 2, got %d", len(snapshot))
	}

	// Mutating snapshot should not affect cache
	delete(snapshot, "x")
	if c.Len() != 2 {
		t.Error("mutating snapshot affected cache")
	}
}

// ── Cache Refresh ────────────────────────────────────────────────────────────

func TestCache_Refresh(t *testing.T) {
	t.Parallel()
	c := NewModelCache(5 * time.Minute)

	fetchFn := func(ctx context.Context) ([]types.ModelInfo, error) {
		return []types.ModelInfo{{ID: "m1"}, {ID: "m2"}}, nil
	}

	models, err := c.Refresh(context.Background(), fetchFn)
	if err != nil {
		t.Fatalf("Refresh failed: %v", err)
	}
	if len(models) != 2 {
		t.Fatalf("expected 2, got %d", len(models))
	}
	if c.Len() != 2 {
		t.Errorf("cache length = %d, want 2", c.Len())
	}
}

func TestCache_Refresh_Error(t *testing.T) {
	t.Parallel()
	c := NewModelCache(5 * time.Minute)

	fetchFn := func(ctx context.Context) ([]types.ModelInfo, error) {
		return nil, io.ErrUnexpectedEOF
	}

	_, err := c.Refresh(context.Background(), fetchFn)
	if err != io.ErrUnexpectedEOF {
		t.Fatalf("expected io.ErrUnexpectedEOF, got %v", err)
	}
	if c.Len() != 0 {
		t.Error("cache should be empty after error")
	}
}

// ── Registry additional ──────────────────────────────────────────────────────

func TestRegistry_EmptyList(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	names := r.List()
	if len(names) != 0 {
		t.Errorf("expected empty list, got %d", len(names))
	}
}

func TestRegistry_ActiveEmpty(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	if active := r.Active(); active != "" {
		t.Errorf("expected empty active, got %q", active)
	}
}

func TestRegistry_ActiveProvider_Empty(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	if ap := r.ActiveProvider(); ap != nil {
		t.Errorf("expected nil ActiveProvider, got %v", ap)
	}
}

func TestRegistry_SetActive(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a"})
	r.Register("b", &mockProvider{name: "b"})

	if err := r.SetActive("b"); err != nil {
		t.Fatalf("SetActive failed: %v", err)
	}
	if r.Active() != "b" {
		t.Errorf("Active = %q, want 'b'", r.Active())
	}
}

// ── UserAgent additional ────────────────────────────────────────────────────

func TestUserAgent_EmptyVersion(t *testing.T) {
	t.Parallel()
	got := UserAgent("")
	if got != "M31A/" {
		t.Errorf("UserAgent('') = %q, want 'M31A/'", got)
	}
}

// ── SetCommonHeaders additional ──────────────────────────────────────────────

func TestSetCommonHeaders_EmptyApiKey(t *testing.T) {
	t.Parallel()
	req, _ := http.NewRequest("GET", "http://example.com", nil)
	SetCommonHeaders(req, "", "1.0")

	if got := req.Header.Get("Authorization"); got != "Bearer " {
		t.Errorf("Authorization = %q, want 'Bearer '", got)
	}
}

// ── IsContextExceeded additional ─────────────────────────────────────────────

func TestIsContextExceeded_EmptyBody(t *testing.T) {
	t.Parallel()
	if IsContextExceeded(400, "") {
		t.Error("expected false for empty body")
	}
}

func TestIsContextExceeded_Non400(t *testing.T) {
	t.Parallel()
	if IsContextExceeded(500, "context_length_exceeded") {
		t.Error("expected false for non-400")
	}
}

// ── BuildChatBody additional ────────────────────────────────────────────────

func TestBuildChatBody_ReasoningEnabled(t *testing.T) {
	t.Parallel()
	body := BuildChatBody(ChatRequest{
		Model:            "anthropic/claude-sonnet-4",
		ReasoningEnabled: true,
	})
	thinking, ok := body["thinking"]
	if !ok {
		t.Fatal("expected thinking param when reasoning enabled")
	}
	thinkingMap, ok := thinking.(map[string]any)
	if !ok {
		t.Fatal("expected thinking to be map")
	}
	if thinkingMap["type"] != "enabled" {
		t.Errorf("expected type=enabled, got %v", thinkingMap["type"])
	}
}

// ── SanitizeProviderError additional ────────────────────────────────────────

func TestSanitizeProviderError_EmptyBody(t *testing.T) {
	t.Parallel()
	got := SanitizeProviderError(400, "", "openrouter")
	if !strings.Contains(got, "Bad request") {
		t.Errorf("expected 'Bad request', got %q", got)
	}
}

func TestSanitizeProviderError_HTMLStripped(t *testing.T) {
	t.Parallel()
	got := SanitizeProviderError(400, "<p>error</p>", "openrouter")
	if strings.Contains(got, "<p>") {
		t.Error("HTML not stripped")
	}
	if !strings.Contains(got, "error") {
		t.Error("content text lost")
	}
}

// ── IsRateLimited additional ────────────────────────────────────────────────

func TestIsRateLimited_200(t *testing.T) {
	t.Parallel()
	resp := &http.Response{StatusCode: 200}
	if IsRateLimited(resp) {
		t.Error("expected false for 200")
	}
}

// ── GetRetryAfter additional ────────────────────────────────────────────────

func TestGetRetryAfter_EmptyHeader(t *testing.T) {
	t.Parallel()
	resp := &http.Response{Header: http.Header{}}
	got := GetRetryAfter(resp)
	if got != "" {
		t.Errorf("expected empty, got %q", got)
	}
}

// ── InspectResponse additional ──────────────────────────────────────────────

func TestInspectResponse_InvalidRetryAfterString(t *testing.T) {
	t.Parallel()
	resp := &http.Response{
		StatusCode: 429,
		Header:     http.Header{"Retry-After": {"abc"}},
	}
	info := InspectResponse(resp)
	if info.Wait != 0 {
		t.Errorf("expected 0 wait for invalid retry-after, got %v", info.Wait)
	}
}

func TestInspectResponse_PositiveRetryAfter(t *testing.T) {
	t.Parallel()
	resp := &http.Response{
		StatusCode: 429,
		Header:     http.Header{"Retry-After": {"0"}},
	}
	info := InspectResponse(resp)
	if info.Wait != 0 {
		t.Errorf("expected 0 wait for zero retry-after, got %v", info.Wait)
	}
}

// ── FindFallbackWithRetryAfter ──────────────────────────────────────────────

func TestFindFallbackWithRetryAfter_WithHeader(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a", healthStatus: "offline"})
	r.Register("b", &mockProvider{name: "b", healthStatus: "live"})
	r.SetActive("a")

	result := FindFallbackWithRetryAfter(r, "a", "30", nil, 0)
	if result.Err != nil {
		t.Fatalf("unexpected error: %v", result.Err)
	}
	if result.Event == nil {
		t.Fatal("expected event")
	}
	if result.Wait != 30*time.Second {
		t.Errorf("Wait = %v, want 30s", result.Wait)
	}
	if result.Event.Reason != "rate_limited" {
		// rate_limited is set by FindFallbackWithRetryAfter when a Retry-After header
		// is present; FindFallbackProvider's own reason (fallback_live) is overwritten.
		t.Errorf("Reason = %q, want 'rate_limited' (overwrite from FindFallbackWithRetryAfter)", result.Event.Reason)
	}
}

func TestFindFallbackWithRetryAfter_NoHeader(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a", healthStatus: "offline"})
	r.Register("b", &mockProvider{name: "b", healthStatus: "live"})
	r.SetActive("a")

	result := FindFallbackWithRetryAfter(r, "a", "", nil, 0)
	if result.Err != nil {
		t.Fatalf("unexpected error: %v", result.Err)
	}
	if result.Wait != 0 {
		t.Errorf("Wait = %v, want 0", result.Wait)
	}
}

func TestFindFallbackWithRetryAfter_NoAlternative(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a", healthStatus: "offline"})
	r.SetActive("a")

	result := FindFallbackWithRetryAfter(r, "a", "30", nil, 0)
	if result.Err == nil {
		t.Fatal("expected error when no fallback available")
	}
}

func TestFindFallbackWithRetryAfter_CapWait(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a", healthStatus: "offline"})
	r.Register("b", &mockProvider{name: "b", healthStatus: "live"})
	r.SetActive("a")

	result := FindFallbackWithRetryAfter(r, "a", "9999", nil, 0)
	if result.Err != nil {
		t.Fatalf("unexpected error: %v", result.Err)
	}
	if result.Wait > maxRetryAfter {
		t.Errorf("Wait = %v exceeds maxRetryAfter %v", result.Wait, maxRetryAfter)
	}
}

// ── Fallback slow status ────────────────────────────────────────────────────

func TestFindFallbackProvider_SlowStatus(t *testing.T) {
	t.Parallel()
	r := NewRegistry()
	r.Register("a", &mockProvider{name: "a", healthStatus: "offline"})
	r.Register("b", &mockProvider{name: "b", healthStatus: "slow"})
	r.SetActive("a")

	_, event, err := FindFallbackProvider(r, "a", nil, 0)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if event.Reason != "fallback_slow" {
		t.Errorf("Reason = %q, want 'fallback_slow'", event.Reason)
	}
}

// ── ParseModelCapabilities additional ────────────────────────────────────────

func TestParseModelCapabilities_ExtraReasoningPatterns(t *testing.T) {
	t.Parallel()
	caps := ParseModelCapabilities("custom/model-r1-large", "-r1")
	if !caps.Reasoning {
		t.Error("expected Reasoning=true with extra pattern")
	}
}

func TestParseModelCapabilities_ReasonWord(t *testing.T) {
	t.Parallel()
	caps := ParseModelCapabilities("custom/reason-model")
	if !caps.Reasoning {
		t.Error("expected Reasoning=true for model with 'reason' in name")
	}
}

func TestParseModelCapabilities_ThinkingWord(t *testing.T) {
	t.Parallel()
	caps := ParseModelCapabilities("custom/thinking-model")
	if !caps.Reasoning {
		t.Error("expected Reasoning=true for model with 'thinking' in name")
	}
}

func TestParseModelCapabilities_LlamaTools(t *testing.T) {
	t.Parallel()
	caps := ParseModelCapabilities("meta-llama/llama-3-70b")
	if !caps.Tools {
		t.Error("expected Tools=true for llama model")
	}
}

func TestParseModelCapabilities_MistralTools(t *testing.T) {
	t.Parallel()
	caps := ParseModelCapabilities("mistralai/mistral-large")
	if !caps.Tools {
		t.Error("expected Tools=true for mistral model")
	}
}

func TestParseModelCapabilities_CohereCommandR(t *testing.T) {
	t.Parallel()
	caps := ParseModelCapabilities("cohere/command-r-plus")
	if !caps.Tools {
		t.Error("expected Tools=true for command-r")
	}
}

func TestParseModelCapabilities_CommandA(t *testing.T) {
	t.Parallel()
	caps := ParseModelCapabilities("cohere/command-a")
	if !caps.Tools {
		t.Error("expected Tools=true for command-a")
	}
}

// ── GetReasoningConfig additional ────────────────────────────────────────────

func TestGetReasoningConfig_Qwen(t *testing.T) {
	cfg, ok := GetReasoningConfig("qwen/qwen-2.5")
	if !ok {
		t.Fatal("expected config for qwen")
	}
	if cfg.ModelFamily != "qwen" {
		t.Errorf("ModelFamily = %q, want 'qwen'", cfg.ModelFamily)
	}
	if cfg.SSEField != "choices.0.delta.reasoning_content" {
		t.Errorf("SSEField = %q", cfg.SSEField)
	}
}

func TestGetReasoningConfig_OpenAIo3(t *testing.T) {
	cfg, ok := GetReasoningConfig("openai/o3-mini")
	if !ok {
		t.Fatal("expected config for openai/o3-mini")
	}
	if cfg.ModelFamily != "openai" {
		t.Errorf("ModelFamily = %q, want 'openai'", cfg.ModelFamily)
	}
}

func TestGetReasoningConfig_OpenAIo1(t *testing.T) {
	cfg, ok := GetReasoningConfig("openai/o1-preview")
	if !ok {
		t.Fatal("expected config for openai/o1-preview")
	}
	if cfg.ModelFamily != "openai" {
		t.Errorf("ModelFamily = %q, want 'openai'", cfg.ModelFamily)
	}
}

func TestGetReasoningConfig_OpenAIo4(t *testing.T) {
	// o4 models don't match the openai/o- prefix pattern, so no reasoning config
	_, ok := GetReasoningConfig("openai/o4-mini")
	if ok {
		t.Error("expected no config for openai/o4-mini (not in reasoning map)")
	}
}

func TestGetReasoningConfig_OpenRouterPrefix(t *testing.T) {
	cfg, ok := GetReasoningConfig("openai/o1-mini")
	if !ok {
		t.Fatal("expected config for openai/o1-mini via OpenRouter prefix")
	}
	if cfg.ModelFamily != "openai" {
		t.Errorf("ModelFamily = %q, want 'openai'", cfg.ModelFamily)
	}
}

func TestApplyReasoningParams_UnknownModel(t *testing.T) {
	body := map[string]any{"model": "unknown"}
	result := ApplyReasoningParams("unknown/model", body)
	if _, ok := result["thinking"]; ok {
		t.Error("unexpected thinking param for unknown model")
	}
}

// ── ParseSSEChunk additional ────────────────────────────────────────────────

func TestParseSSEChunk_InvalidJSON(t *testing.T) {
	_, err := ParseSSEChunk("not json", "any/model")
	if err == nil {
		t.Fatal("expected error for invalid JSON")
	}
}

func TestParseSSEChunk_MissingChoices(t *testing.T) {
	data := `{"something":"else"}`
	chunk, err := ParseSSEChunk(data, "any/model")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if chunk != nil {
		t.Errorf("expected nil chunk for missing choices without usage, got %+v", chunk)
	}
}

func TestParseSSEChunk_EmptyChoices(t *testing.T) {
	data := `{"choices":[]}`
	chunk, err := ParseSSEChunk(data, "any/model")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if chunk != nil {
		t.Errorf("expected nil chunk for empty choices without usage, got %+v", chunk)
	}
}

func TestParseSSEChunk_ToolCall(t *testing.T) {
	data := `{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"get_weather","arguments":"{\"city\":\"NYC\"}"}}]}}]}`
	chunk, err := ParseSSEChunk(data, "any/model")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if chunk.Type != "tool_call" {
		t.Errorf("Type = %q, want 'tool_call'", chunk.Type)
	}
	if chunk.ToolCallID != "call_1" {
		t.Errorf("ToolCallID = %q, want 'call_1'", chunk.ToolCallID)
	}
	if chunk.ToolName != "get_weather" {
		t.Errorf("ToolName = %q, want 'get_weather'", chunk.ToolName)
	}
	if chunk.ToolInput != `{"city":"NYC"}` {
		t.Errorf("ToolInput = %q", chunk.ToolInput)
	}
}

func TestParseSSEChunk_UsageOnly(t *testing.T) {
	data := `{"usage":{"prompt_tokens":100,"completion_tokens":50,"total_tokens":150}}`
	chunk, err := ParseSSEChunk(data, "any/model")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if chunk.Type != "usage" {
		t.Errorf("Type = %q, want 'usage'", chunk.Type)
	}
	if chunk.Usage == nil {
		t.Fatal("expected non-nil usage")
	}
	if chunk.Usage.PromptTokens != 100 {
		t.Errorf("PromptTokens = %d, want 100", chunk.Usage.PromptTokens)
	}
	if chunk.Usage.CompletionTokens != 50 {
		t.Errorf("CompletionTokens = %d, want 50", chunk.Usage.CompletionTokens)
	}
}

func TestParseSSEChunk_DoneToolCalls(t *testing.T) {
	data := `{"choices":[{"finish_reason":"tool_calls"}]}`
	chunk, err := ParseSSEChunk(data, "any/model")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if chunk.Type != "done" {
		t.Errorf("Type = %q, want 'done'", chunk.Type)
	}
}

func TestParseSSEChunk_AnthropicThinking(t *testing.T) {
	data := `{"choices":[{"delta":{"type":"thinking","content":"Let me think..."}}]}`
	chunk, err := ParseSSEChunk(data, "anthropic/claude-sonnet-4")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if chunk.Type != "thinking" {
		t.Errorf("Type = %q, want 'thinking'", chunk.Type)
	}
	if chunk.Delta != "Let me think..." {
		t.Errorf("Delta = %q", chunk.Delta)
	}
}

func TestParseSSEChunk_WithUsage(t *testing.T) {
	data := `{"choices":[{"delta":{"content":"Hi"}}],"usage":{"prompt_tokens":10,"completion_tokens":5}}`
	chunk, err := ParseSSEChunk(data, "any/model")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if chunk.Usage == nil {
		t.Fatal("expected non-nil usage")
	}
	if chunk.Usage.PromptTokens != 10 {
		t.Errorf("PromptTokens = %d, want 10", chunk.Usage.PromptTokens)
	}
}

func TestParseSSEChunk_NonObjectFirstChoice(t *testing.T) {
	data := `{"choices":["string"]}`
	chunk, err := ParseSSEChunk(data, "any/model")
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if chunk != nil {
		t.Errorf("expected nil chunk for non-object first choice, got %+v", chunk)
	}
}

// ── SSEParser additional ────────────────────────────────────────────────────

func TestSSEParser_CarriageReturn(t *testing.T) {
	resp := bodyReader("data: {\"key\":\"val\"}\r\n\r\n")
	p := NewSSEParserWithContext(resp, context.Background())
	defer p.Close()

	_, data, err := p.Next()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if data != `{"key":"val"}` {
		t.Errorf("data = %q", data)
	}
}

func TestSSEParser_KeepAlive(t *testing.T) {
	// A comment line followed by empty line is a keep-alive (no data).
	// The actual data event follows after.
	resp := bodyReader(":\n\ndata: {\"ok\":true}\n\n")
	p := NewSSEParserWithContext(resp, context.Background())
	defer p.Close()

	// The parser now recursively skips keep-alive events, so the first
	// call to Next() returns the actual data event directly.
	_, data1, err := p.Next()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if data1 != `{"ok":true}` {
		t.Errorf("data = %q, want %q", data1, `{"ok":true}`)
	}
}

func TestSSEParser_IdField(t *testing.T) {
	resp := bodyReader("id: 123\ndata: {\"ok\":true}\n\n")
	p := NewSSEParserWithContext(resp, context.Background())
	defer p.Close()

	_, data, err := p.Next()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if data != `{"ok":true}` {
		t.Errorf("data = %q", data)
	}
}

func TestSSEParser_RetryField(t *testing.T) {
	resp := bodyReader("retry: 5000\ndata: {\"ok\":true}\n\n")
	p := NewSSEParserWithContext(resp, context.Background())
	defer p.Close()

	_, data, err := p.Next()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if data != `{"ok":true}` {
		t.Errorf("data = %q", data)
	}
}

func TestSSEParser_CloseIdempotent(t *testing.T) {
	resp := bodyReader("data: {}\n\n")
	p := NewSSEParserWithContext(resp, context.Background())

	if err := p.Close(); err != nil {
		t.Fatalf("first Close failed: %v", err)
	}
	if err := p.Close(); err != nil {
		t.Fatalf("second Close failed: %v", err)
	}
}

func TestSSEParserWithContext_CancelBetweenEvents(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())

	// Write both events before creating parser
	data := "data: {\"first\":true}\n\ndata: {\"second\":true}\n\n"
	resp := bodyReader(data)

	p := NewSSEParserWithContext(resp, ctx)
	defer p.Close()

	// Read first event
	_, _, err := p.Next()
	if err != nil {
		t.Fatalf("unexpected error reading first event: %v", err)
	}

	// Cancel before reading second event
	cancel()

	// Next should return context error
	_, _, err = p.Next()
	if err == nil {
		t.Fatal("expected error after context cancellation")
	}
}

func TestDefaultStreamTimeout(t *testing.T) {
	if DefaultStreamTimeout != 30*time.Second {
		t.Errorf("DefaultStreamTimeout = %v, want 30s", DefaultStreamTimeout)
	}
}

// ── ReadBodyLimited ──────────────────────────────────────────────────────────

func TestReadBodyLimited(t *testing.T) {
	t.Parallel()
	resp := &http.Response{
		Body: io.NopCloser(strings.NewReader("hello world")),
	}
	data, err := ReadBodyLimited(resp, 1024)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if string(data) != "hello world" {
		t.Errorf("data = %q, want 'hello world'", data)
	}
}

func TestReadBodyLimited_Truncates(t *testing.T) {
	t.Parallel()
	content := strings.Repeat("x", 200)
	resp := &http.Response{
		Body: io.NopCloser(strings.NewReader(content)),
	}
	data, err := ReadBodyLimited(resp, 50)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(data) != 50 {
		t.Errorf("expected 50 bytes, got %d", len(data))
	}
}
