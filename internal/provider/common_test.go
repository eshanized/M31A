package provider

import (
	"net/http"
	"os"
	"strings"
	"testing"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/testutil"
	"github.com/eshanized/M31A/pkg/types"
)

func TestMaskAPIKeys(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "standard OpenAI sk- key",
			input:    "sk-abc123def456ghi7",
			expected: "sk-a****ghi7",
		},
		{
			name:     "key- prefix",
			input:    "key-abc123def456ghi7",
			expected: "key-****ghi7",
		},
		{
			name:     "api_key= assignment",
			input:    "api_key=abc123def456ghi7",
			expected: "api_****ghi7",
		},
		{
			name:     "api-key= assignment (dash variant)",
			input:    "api-key=abc123def456ghi7",
			expected: "api-****ghi7",
		},
		{
			name:     "Bearer JWT token does not match",
			input:    "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N",
			expected: "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N",
		},
		{
			name:     "empty string",
			input:    "",
			expected: "",
		},
		{
			name:     "no keys present",
			input:    "hello world no api keys here",
			expected: "hello world no api keys here",
		},
		{
			name:     "multiple keys in one string",
			input:    "sk-abc123def456ghi7 and key-xyz789abc012def3",
			expected: "sk-a****ghi7 and key-****def3",
		},
		{
			name:     "short key below minimum length does not match",
			input:    "sk-abcd1234",
			expected: "sk-abcd1234",
		},
		{
			name:     "key in error message context",
			input:    "401 Unauthorized: invalid sk-abc123def456ghi7",
			expected: "401 Unauthorized: invalid sk-a****ghi7",
		},
		{
			name:     "case insensitive SK- prefix",
			input:    "SK-ABC123DEF456GHI7",
			expected: "SK-A****GHI7",
		},
		{
			name:     "api_key with colon and space",
			input:    "api_key: abc123def456ghi7",
			expected: "api_****ghi7",
		},
		{
			name:     "api_key with quotes",
			input:    `api_key="abc123def456ghi7"`,
			expected: `api_****ghi7"`,
		},
		{
			name:     "long key in JSON body",
			input:    `{"error":"Invalid api_key=sk-abc123def456ghi7 was rejected"}`,
			expected: `{"error":"Invalid api_key=sk-a****ghi7 was rejected"}`,
		},
		{
			name:     "three keys in sequence",
			input:    "sk-aaa111bbb222ccc3 key-zzz999yyy888xxx7 api_key=ppp555qqq666rrr8",
			expected: "sk-a****ccc3 key-****xxx7 api_****rrr8",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := maskAPIKeys(tt.input)
			if got != tt.expected {
				t.Errorf("maskAPIKeys(%q) = %q, want %q", tt.input, got, tt.expected)
			}
		})
	}
}

func TestMaskAPIKeys_Idempotent(t *testing.T) {
	input := "error: sk-abc123def456ghi7 failed"
	once := maskAPIKeys(input)
	twice := maskAPIKeys(once)
	if once != twice {
		t.Errorf("maskAPIKeys is not idempotent: first=%q, second=%q", once, twice)
	}
}

func TestMaskAPIKeys_PreservesNonKeyContent(t *testing.T) {
	input := "GET /v1/chat/completions returned 401 for model sk-abc123def456ghi7"
	got := maskAPIKeys(input)

	if strings.Contains(got, "sk-abc123def456ghi7") {
		t.Errorf("key was not redacted: %q", got)
	}
	if !strings.HasPrefix(got, "GET /v1/chat/completions returned 401 for model ") {
		t.Errorf("non-key content was altered: %q", got)
	}
}

func TestUserAgent(t *testing.T) {
	t.Parallel()
	got := UserAgent("1.2.3")
	if got != "M31A/1.2.3" {
		t.Errorf("UserAgent('1.2.3') = %q, want %q", got, "M31A/1.2.3")
	}
}

func TestSetCommonHeaders(t *testing.T) {
	t.Parallel()
	testutil.LoadTestDotEnv(t)

	apiKey := os.Getenv("OPENROUTER_API_KEY")
	if apiKey == "" {
		apiKey = "sk-test123"
	}
	req, _ := http.NewRequest("GET", "http://example.com", nil)
	SetCommonHeaders(req, apiKey, "1.0")

	if got := req.Header.Get("Authorization"); got != "Bearer "+apiKey {
		t.Errorf("Authorization = %q, want %q", got, "Bearer "+apiKey)
	}
	if got := req.Header.Get("User-Agent"); got != "M31A/1.0" {
		t.Errorf("User-Agent = %q, want %q", got, "M31A/1.0")
	}
}

func TestBuildChatBody_Basic(t *testing.T) {
	t.Parallel()
	body := BuildChatBody(ChatRequest{Model: "gpt-4"})
	if body["model"] != "gpt-4" {
		t.Errorf("model = %v, want gpt-4", body["model"])
	}
	if body["stream"] != true {
		t.Error("expected stream=true")
	}
	if _, ok := body["max_tokens"]; ok {
		t.Error("max_tokens should be absent when 0")
	}
	if _, ok := body["tools"]; ok {
		t.Error("tools should be absent when empty")
	}
}

func TestBuildChatBody_WithMaxTokens(t *testing.T) {
	t.Parallel()
	body := BuildChatBody(ChatRequest{Model: "gpt-4", MaxTokens: 1024})
	if body["max_tokens"] != 1024 {
		t.Errorf("max_tokens = %v, want 1024", body["max_tokens"])
	}
}

func TestBuildChatBody_WithTools(t *testing.T) {
	t.Parallel()
	tools := []ToolDefinition{
		{Name: "get_weather", Description: "Get weather", Parameters: `{"type":"object","properties":{"city":{"type":"string"}}}`},
		{Name: "no_params", Description: "No params", Parameters: ""},
		{Name: "bad_json", Description: "Bad JSON", Parameters: "not-json"},
	}
	body := BuildChatBody(ChatRequest{Model: "gpt-4", Tools: tools})
	toolsArr, ok := body["tools"].([]map[string]any)
	if !ok {
		t.Fatal("expected tools to be []map[string]any")
	}
	if len(toolsArr) != 3 {
		t.Fatalf("expected 3 tools, got %d", len(toolsArr))
	}
	// First tool: valid parameters
	fn0 := toolsArr[0]["function"].(map[string]any)
	if fn0["name"] != "get_weather" {
		t.Errorf("tool[0].name = %v, want get_weather", fn0["name"])
	}
	if fn0["parameters"] == nil {
		t.Error("tool[0].parameters should not be nil")
	}
	// Second tool: empty parameters → empty object
	fn1 := toolsArr[1]["function"].(map[string]any)
	params1, ok := fn1["parameters"].(map[string]any)
	if !ok || len(params1) != 0 {
		t.Errorf("tool[1].parameters = %v, want empty map", fn1["parameters"])
	}
	// Third tool: bad JSON → empty object
	fn2 := toolsArr[2]["function"].(map[string]any)
	params2, ok := fn2["parameters"].(map[string]any)
	if !ok || len(params2) != 0 {
		t.Errorf("tool[2].parameters = %v, want empty map", fn2["parameters"])
	}
}

func TestSanitizeProviderError_AllStatusCodes(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name       string
		status     int
		body       string
		provider   string
		wantSubstr string
	}{
		{"400", 400, "bad param", "openrouter", "Bad request"},
		{"401 openrouter", 401, "invalid key", "openrouter", "Invalid API key"},
		{"401 zen with body", 401, "CreditsError", "zen", "Invalid API key"},
		{"402", 402, "", "openrouter", "Payment required"},
		{"429", 429, "", "openrouter", "Rate limited"},
		{"500", 500, "", "openrouter", "server error"},
		{"502 openrouter", 502, "gw error", "openrouter", "gateway error"},
		{"502 zen", 502, "gw error", "zen", "HTTP 502"},
		{"503", 503, "", "openrouter", "temporarily unavailable"},
		{"default 418", 418, "teapot", "openrouter", "HTTP 418"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := SanitizeProviderError(tt.status, tt.body, tt.provider)
			if !strings.Contains(got, tt.wantSubstr) {
				t.Errorf("SanitizeProviderError(%d, %q, %q) = %q, want substring %q", tt.status, tt.body, tt.provider, got, tt.wantSubstr)
			}
		})
	}
}

func TestSanitizeProviderError_ScrubsAPIKeys(t *testing.T) {
	t.Parallel()
	got := SanitizeProviderError(400, "error: sk-abc123def456ghi7 rejected", "openrouter")
	if strings.Contains(got, "sk-abc123def456ghi7") {
		t.Errorf("API key leaked in sanitized output: %q", got)
	}
}

func TestSanitizeProviderError_TruncatesLongBody(t *testing.T) {
	t.Parallel()
	longBody := strings.Repeat("x", 500)
	got := SanitizeProviderError(400, longBody, "openrouter")
	if len(got) > 300 {
		t.Errorf("sanitized output too long: %d chars", len(got))
	}
}

func TestEstimateCost_CacheHit(t *testing.T) {
	t.Parallel()
	cache := NewModelCache(5 * time.Minute)
	cache.Set([]types.ModelInfo{
		{ID: "m1", Pricing: types.Pricing{InputPerMToken: 1.0, OutputPerMToken: 2.0}},
	})
	cost := EstimateCost("m1", types.Usage{PromptTokens: 1000, CompletionTokens: 500}, cache)
	expected := (1000.0/1_000_000)*1.0 + (500.0/1_000_000)*2.0
	if cost != expected {
		t.Errorf("EstimateCost = %f, want %f", cost, expected)
	}
}

func TestEstimateCost_CacheMiss(t *testing.T) {
	t.Parallel()
	cache := NewModelCache(5 * time.Minute)
	cost := EstimateCost("nonexistent", types.Usage{PromptTokens: 1000}, cache)
	if cost != 0 {
		t.Errorf("EstimateCost for missing model = %f, want 0", cost)
	}
}

func TestGetModel_Found(t *testing.T) {
	t.Parallel()
	cache := NewModelCache(5 * time.Minute)
	cache.Set([]types.ModelInfo{{ID: "m1", Name: "Model 1"}})
	m, err := GetModel("m1", cache)
	if err != nil {
		t.Fatalf("GetModel failed: %v", err)
	}
	if m.Name != "Model 1" {
		t.Errorf("Name = %q, want Model 1", m.Name)
	}
}

func TestGetModel_NotFound(t *testing.T) {
	t.Parallel()
	cache := NewModelCache(5 * time.Minute)
	_, err := GetModel("nonexistent", cache)
	if err != m31errors.ErrModelNotFound {
		t.Errorf("expected ErrModelNotFound, got %v", err)
	}
}

func TestCachedModels(t *testing.T) {
	t.Parallel()
	cache := NewModelCache(5 * time.Minute)
	cache.Set([]types.ModelInfo{{ID: "a"}, {ID: "b"}})
	models := CachedModels(cache)
	if len(models) != 2 {
		t.Fatalf("expected 2 models, got %d", len(models))
	}
}

func TestStaleFallback_FreshCache(t *testing.T) {
	t.Parallel()
	cache := NewModelCacheWithStale(5*time.Minute, 24*time.Hour)
	cache.Set([]types.ModelInfo{{ID: "m1"}})
	models, err := StaleFallback(cache)
	if err != nil {
		t.Fatalf("StaleFallback failed: %v", err)
	}
	if len(models) != 1 {
		t.Errorf("expected 1 model, got %d", len(models))
	}
}

func TestStaleFallback_EmptyCache(t *testing.T) {
	t.Parallel()
	cache := NewModelCacheWithStale(5*time.Minute, 24*time.Hour)
	_, err := StaleFallback(cache)
	if err != m31errors.ErrProviderUnreachable {
		t.Errorf("expected ErrProviderUnreachable, got %v", err)
	}
}

func TestStripHTMLTags(t *testing.T) {
	t.Parallel()
	tests := []struct {
		input string
		want  string
	}{
		{"<p>hello</p>", "hello"},
		{"no tags", "no tags"},
		{"<b>bold</b> and <i>italic</i>", "bold and italic"},
		{"<div><p>nested</p></div>", "nested"},
		{"", ""},
	}
	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			got := stripHTMLTags(tt.input)
			if got != tt.want {
				t.Errorf("stripHTMLTags(%q) = %q, want %q", tt.input, got, tt.want)
			}
		})
	}
}

func TestIsContextExceeded_OperatorPrecedence(t *testing.T) {
	tests := []struct {
		name       string
		statusCode int
		body       string
		expected   bool
	}{
		{"context_length_exceeded", 400, `{"error":"context_length_exceeded"}`, true},
		{"maximum context length", 400, `{"error":"maximum context length is 128000"}`, true},
		{"request too large", 400, `{"error":"request too large for model"}`, true},
		{"context window exceeded", 400, `{"error":"context window exceeded"}`, true},
		{"context_length with exceed keyword", 400, `{"error":"context_length must not exceed limit"}`, true},
		{"context_length without exceed keyword", 400, `{"error":"context_length is required"}`, false},
		{"non-400 with context keyword", 500, `{"error":"context_length_exceeded"}`, false},
		{"400 without context keywords", 400, `{"error":"invalid parameter"}`, false},
		{"case insensitive", 400, `{"error":"CONTEXT_LENGTH_EXCEEDED"}`, true},
		{"mixed case exceed keyword", 400, `{"error":"context_length must not Exceed limit"}`, true},
		{"input exceeds (compound)", 400, `{"error":"Input token count exceeds maximum"}`, true},
		{"input length exceeds limit", 400, `{"error":"input length exceeds limit"}`, true},
		{"INPUT EXCEEDS case insensitive", 400, `{"error":"INPUT TOKEN EXCEEDS MAX"}`, true},
		{"input without exceeds", 400, `{"error":"input is required"}`, false},
		{"exceeds without input", 400, `{"error":"token count exceeds maximum"}`, false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := IsContextExceeded(tt.statusCode, tt.body)
			if got != tt.expected {
				t.Errorf("IsContextExceeded(%d, %q) = %v, want %v", tt.statusCode, tt.body, got, tt.expected)
			}
		})
	}
}

// TestBuildChatBody_WireFormat verifies that the agent loop's storage-shaped
// messages (assistant with ToolCalls, tool-role with ToolCallID, messages with
// Segments/CreatedAt/Usage/SkipForLLM) are projected to the OpenAI wire shape
// expected by Zen/OpenRouter. A regression here causes HTTP 400 "Bad request"
// on agent iteration 2 once tool results are fed back to the LLM.
func TestBuildChatBody_WireFormat(t *testing.T) {
	t.Parallel()
	msgs := []types.Message{
		{
			Role:      "system",
			Content:   "sys",
			Segments:  []types.MessageSegment{{Type: "content", Content: "sys"}},
			CreatedAt: time.Now(),
		},
		{
			Role:    "user",
			Content: "hi",
		},
		{
			Role:    "assistant",
			Content: "",
			ToolCalls: []types.ToolCall{{
				ID:    "call_42",
				Name:  "glob",
				Input: []byte(`{"pattern":"*.go"}`),
			}},
		},
		{
			Role:       "tool",
			ToolCallID: "call_42",
			Content:    "file.go",
			Segments:   []types.MessageSegment{{Type: "content", Content: "file.go"}},
			CreatedAt:  time.Now(),
		},
		{
			Role:       "assistant",
			Content:    "done",
			SkipForLLM: true,
		},
	}

	body := BuildChatBody(ChatRequest{Model: "m", Messages: msgs})
	wire, ok := body["messages"].([]map[string]any)
	if !ok {
		t.Fatalf("messages should be []map[string]any, got %T", body["messages"])
	}
	if len(wire) != 4 {
		t.Fatalf("expected 4 wire messages (SkipForLLM excluded), got %d", len(wire))
	}

	// system: role+content only (no segments, created_at)
	sys := wire[0]
	if sys["role"] != "system" || sys["content"] != "sys" {
		t.Errorf("sys = %+v", sys)
	}
	for _, bad := range []string{"segments", "created_at", "usage", "skip_for_llm"} {
		if _, hasKey := sys[bad]; hasKey {
			t.Errorf("sys message leaked storage field %q", bad)
		}
	}

	// assistant with tool_calls: OpenAI shape
	asst := wire[2]
	tcs, ok := asst["tool_calls"].([]map[string]any)
	if !ok || len(tcs) != 1 {
		t.Fatalf("assistant tool_calls = %v", asst["tool_calls"])
	}
	tc := tcs[0]
	if tc["id"] != "call_42" {
		t.Errorf("tc.id = %v", tc["id"])
	}
	if tc["type"] != "function" {
		t.Errorf("tc.type = %v, want function", tc["type"])
	}
	fn, ok := tc["function"].(map[string]any)
	if !ok {
		t.Fatalf("tc.function = %T, want map", tc["function"])
	}
	if fn["name"] != "glob" {
		t.Errorf("fn.name = %v", fn["name"])
	}
	if args, ok := fn["arguments"].(string); !ok || args != `{"pattern":"*.go"}` {
		t.Errorf("fn.arguments = %v (%T), want JSON string", fn["arguments"], fn["arguments"])
	}
	// storage fields "name" and "input" must NOT appear at the top level
	if _, ok := tc["name"]; ok {
		t.Error("tool_call leaked storage field 'name' at top level")
	}
	if _, ok := tc["input"]; ok {
		t.Error("tool_call leaked storage field 'input' at top level")
	}

	// tool message: role/tool_call_id/content only
	tool := wire[3]
	if tool["role"] != "tool" || tool["tool_call_id"] != "call_42" || tool["content"] != "file.go" {
		t.Errorf("tool msg = %+v", tool)
	}
	for _, bad := range []string{"segments", "created_at", "usage"} {
		if _, ok := tool[bad]; ok {
			t.Errorf("tool message leaked storage field %q", bad)
		}
	}
}
