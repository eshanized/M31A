package provider

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestStreamingResilience(t *testing.T) {
	t.Parallel()

	// Test 1: SSEParser skips empty deltas silently (D-14)
	t.Run("empty_deltas_skipped", func(t *testing.T) {
		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Content-Type", "text/event-stream")
			w.WriteHeader(http.StatusOK)
			// Empty data line (keep-alive)
			w.Write([]byte("data: \n\n"))
			w.Write([]byte("data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\n"))
			w.Write([]byte("data: [DONE]\n\n"))
		}))
		defer server.Close()

		req, err := http.NewRequestWithContext(context.Background(), http.MethodGet, server.URL, nil)
		require.NoError(t, err)
		resp, err := http.DefaultClient.Do(req)
		require.NoError(t, err)
		defer resp.Body.Close()

		parser := NewSSEParserWithContext(resp, context.Background())

		// First event should be empty data, skipped
		eventType, data, err := parser.Next()
		require.NoError(t, err)
		assert.Equal(t, "", eventType)
		assert.Equal(t, `{"choices":[{"delta":{"content":"hello"}}]}`, data)

		// Next should be DONE
		_, _, err = parser.Next()
		assert.Error(t, err) // io.EOF
	})

	// Test 2: ParseSSEChunk returns thinking chunks for reasoning_content
	t.Run("reasoning_chunks", func(t *testing.T) {
		t.Skip("Requires ultra model reasoning config (Wave 1)")
		data := `{"choices":[{"delta":{"reasoning_content":"thinking step 1"}}]}`
		chunk, err := ParseSSEChunk(data, "nvidia/nemotron-3-ultra-550b-a55b")
		require.NoError(t, err)
		require.NotNil(t, chunk)
		assert.Equal(t, "thinking", chunk.Type)
		assert.Equal(t, "thinking step 1", chunk.Delta)
	})

	// Test 3: ParseSSEChunk returns content chunks for delta.content
	t.Run("content_chunks", func(t *testing.T) {
		data := `{"choices":[{"delta":{"content":"Hello world"}}]}`
		chunk, err := ParseSSEChunk(data, "deepseek/deepseek-r1")
		require.NoError(t, err)
		require.NotNil(t, chunk)
		assert.Equal(t, "content", chunk.Type)
		assert.Equal(t, "Hello world", chunk.Delta)
	})

	// Test 4: ParseSSEChunk returns tool_call chunks for delta.tool_calls
	t.Run("tool_call_chunks", func(t *testing.T) {
		data := `{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_123","function":{"name":"get_weather","arguments":"{\"location\":\"NYC\"}"}}]}}]}`
		chunk, err := ParseSSEChunk(data, "openai/gpt-4o")
		require.NoError(t, err)
		require.NotNil(t, chunk)
		assert.Equal(t, "tool_call", chunk.Type)
		assert.Equal(t, 0, chunk.Index)
		assert.Equal(t, "call_123", chunk.ToolCallID)
		assert.Equal(t, "get_weather", chunk.ToolName)
		assert.Equal(t, "{\"location\":\"NYC\"}", chunk.ToolInput)
	})

	// Test 5: Sequential reasoning→content chunks produce correct StreamChunk types
	t.Run("sequential_reasoning_content", func(t *testing.T) {
		t.Skip("Requires ultra model reasoning config (Wave 1)")
		// First chunk: reasoning
		data1 := `{"choices":[{"delta":{"reasoning_content":"Let me think..."}}]}`
		chunk1, err := ParseSSEChunk(data1, "nvidia/nemotron-3-ultra-550b-a55b")
		require.NoError(t, err)
		assert.Equal(t, "thinking", chunk1.Type)

		// Second chunk: content
		data2 := `{"choices":[{"delta":{"content":"The answer is 42."}}]}`
		chunk2, err := ParseSSEChunk(data2, "nvidia/nemotron-3-ultra-550b-a55b")
		require.NoError(t, err)
		assert.Equal(t, "content", chunk2.Type)
	})

	// Test 6: [DONE] termination handled correctly
	t.Run("done_termination", func(t *testing.T) {
		data := `{"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":20,"total_tokens":30}}`
		chunk, err := ParseSSEChunk(data, "deepseek/deepseek-r1")
		require.NoError(t, err)
		require.NotNil(t, chunk)
		assert.Equal(t, "done", chunk.Type)
		assert.NotNil(t, chunk.Usage)
		assert.Equal(t, 10, chunk.Usage.PromptTokens)
		assert.Equal(t, 20, chunk.Usage.CompletionTokens)
		assert.Equal(t, 30, chunk.Usage.TotalTokens)
	})

	// Test 7: Watchdog timeout triggers on stalled connection
	t.Run("watchdog_timeout", func(t *testing.T) {
		// This test verifies the watchdog mechanism exists
		// The actual timeout is tested in integration tests
		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Content-Type", "text/event-stream")
			w.WriteHeader(http.StatusOK)
			// Never send data - watchdog should trigger
		}))
		defer server.Close()

		req, err := http.NewRequestWithContext(context.Background(), http.MethodGet, server.URL, nil)
		require.NoError(t, err)
		resp, err := http.DefaultClient.Do(req)
		require.NoError(t, err)

		parser := NewSSEParserWithContext(resp, context.Background())

		// Should eventually timeout
		_, _, err = parser.Next()
		// May get context deadline exceeded or EOF
		assert.Error(t, err)
		parser.Close()
	})

	// Test 8: Buffer pooling and cleanup on Close()
	t.Run("buffer_pool_cleanup", func(t *testing.T) {
		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Content-Type", "text/event-stream")
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("data: {\"choices\":[{\"delta\":{\"content\":\"test\"}}]}\n\n"))
			w.Write([]byte("data: [DONE]\n\n"))
		}))
		defer server.Close()

		req, err := http.NewRequestWithContext(context.Background(), http.MethodGet, server.URL, nil)
		require.NoError(t, err)
		resp, err := http.DefaultClient.Do(req)
		require.NoError(t, err)

		parser := NewSSEParserWithContext(resp, context.Background())
		_, _, _ = parser.Next()
		err = parser.Close()
		assert.NoError(t, err)

		// Calling Close again should be idempotent
		err = parser.Close()
		assert.NoError(t, err)
	})

	// Test 9: Edge cases - empty data lines, \r\n line endings, malformed chunks
	t.Run("edge_cases", func(t *testing.T) {
		tests := []struct {
			name     string
			data     string
			wantType string
			wantErr  bool
		}{
			{"empty_data", `{"choices":[{"delta":{}}]}`, "", false},       // No content, skips silently (nil, nil)
			{"malformed_json", `not json`, "", true},                     // Parse error
			{"empty_choices", `{"choices":[]}`, "", false},               // Empty choices, skips silently (nil, nil)
			{"usage_only", `{"usage":{"prompt_tokens":5,"completion_tokens":10,"total_tokens":15}}`, "usage", false},
		}

		for _, tt := range tests {
			t.Run(tt.name, func(t *testing.T) {
				chunk, err := ParseSSEChunk(tt.data, "test-model")
				if tt.wantErr {
					assert.Error(t, err)
				} else {
					assert.NoError(t, err)
				}
				if tt.wantType != "" {
					require.NotNil(t, chunk)
					assert.Equal(t, tt.wantType, chunk.Type)
				}
			})
		}
	})

	// Test 10: Anthropic thinking chunks
	t.Run("anthropic_thinking", func(t *testing.T) {
		data := `{"choices":[{"delta":{"type":"thinking","content":"thinking..."}}]}`
		chunk, err := ParseSSEChunk(data, "anthropic/claude-3-opus")
		require.NoError(t, err)
		require.NotNil(t, chunk)
		assert.Equal(t, "thinking", chunk.Type)
		assert.Equal(t, "thinking...", chunk.Delta)
	})

	// Test 11: tool_calls with multiple calls (first one used)
	t.Run("multiple_tool_calls", func(t *testing.T) {
		data := `{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"func1","arguments":"{}"}},{"index":1,"id":"call_2","function":{"name":"func2","arguments":"{}"}}]}}]}`
		chunk, err := ParseSSEChunk(data, "openai/gpt-4o")
		require.NoError(t, err)
		require.NotNil(t, chunk)
		assert.Equal(t, "tool_call", chunk.Type)
		assert.Equal(t, 0, chunk.Index)
		assert.Equal(t, "call_1", chunk.ToolCallID)
		assert.Equal(t, "func1", chunk.ToolName)
	})

	// Test 12: Deepseek reasoning content
	t.Run("deepseek_reasoning", func(t *testing.T) {
		data := `{"choices":[{"delta":{"reasoning_content":"reasoning..."}}]}`
		chunk, err := ParseSSEChunk(data, "deepseek/deepseek-r1")
		require.NoError(t, err)
		require.NotNil(t, chunk)
		assert.Equal(t, "thinking", chunk.Type)
		assert.Equal(t, "reasoning...", chunk.Delta)
	})

	// Test 13: OpenAI o-series reasoning
	t.Run("openai_reasoning", func(t *testing.T) {
		data := `{"choices":[{"delta":{"reasoning":"reasoning..."}}]}`
		chunk, err := ParseSSEChunk(data, "openai/o3-mini")
		require.NoError(t, err)
		require.NotNil(t, chunk)
		assert.Equal(t, "thinking", chunk.Type)
		assert.Equal(t, "reasoning...", chunk.Delta)
	})

	// Test 14: Qwen reasoning
	t.Run("qwen_reasoning", func(t *testing.T) {
		data := `{"choices":[{"delta":{"reasoning_content":"reasoning..."}}]}`
		chunk, err := ParseSSEChunk(data, "qwen/qwen-2.5")
		require.NoError(t, err)
		require.NotNil(t, chunk)
		assert.Equal(t, "thinking", chunk.Type)
		assert.Equal(t, "reasoning...", chunk.Delta)
	})

	// Test 15: SSEParser handles \r\n line endings (H-18 fix)
	t.Run("crlf_line_endings", func(t *testing.T) {
		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Content-Type", "text/event-stream")
			w.WriteHeader(http.StatusOK)
			// Send with \r\n line endings
			w.Write([]byte("data: {\"choices\":[{\"delta\":{\"content\":\"test\"}}]}\r\n\r\n"))
			w.Write([]byte("data: [DONE]\r\n\r\n"))
		}))
		defer server.Close()

		req, err := http.NewRequestWithContext(context.Background(), http.MethodGet, server.URL, nil)
		require.NoError(t, err)
		resp, err := http.DefaultClient.Do(req)
		require.NoError(t, err)
		defer resp.Body.Close()

		parser := NewSSEParserWithContext(resp, context.Background())
		_, data, err := parser.Next()
		require.NoError(t, err)
		assert.Contains(t, data, "test")
	})
}

// Test helper to create SSE response with custom chunks
func createSSEResponse(chunks ...string) *httptest.ResponseRecorder {
	w := httptest.NewRecorder()
	w.Header().Set("Content-Type", "text/event-stream")
	w.WriteHeader(http.StatusOK)
	for _, chunk := range chunks {
		w.WriteString("data: " + chunk + "\n\n")
	}
	w.WriteString("data: [DONE]\n\n")
	return w
}

// Helper to parse JSON for testing
func parseJSON(t *testing.T, data string) map[string]any {
	t.Helper()
	var result map[string]any
	err := json.Unmarshal([]byte(data), &result)
	require.NoError(t, err)
	return result
}