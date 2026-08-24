

package nvidia

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"reflect"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestNVIDIAClient(t *testing.T) {
	t.Parallel()

	chatResponseType := reflect.TypeOf(types.ChatResponse{})
	hasChatResponse := chatResponseType.Kind() != reflect.Invalid

	// Test 1: Client implements LLMProvider interface
	t.Run("implements_interface", func(t *testing.T) {
		client, err := New("test-key", Options{})
		require.NoError(t, err)
		require.NotNil(t, client)

		var _ provider.LLMProvider = client
		assert.Equal(t, types.ProviderNvidia, client.Name())
	})

	// Test 2: ChatCompletionStream works with ultra model reasoning config
	t.Run("chat_completion_stream_ultra_reasoning", func(t *testing.T) {
		client, err := New("test-key", Options{})
		require.NoError(t, err)

		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// Verify request body includes reasoning config
			var reqBody map[string]any
			json.NewDecoder(r.Body).Decode(&reqBody)

			// Check for extra_body with reasoning params
			if extraBody, ok := reqBody["extra_body"].(map[string]any); ok {
				assert.Equal(t, float64(32768), extraBody["reasoning_budget"])
				if kwargs, ok := extraBody["chat_template_kwargs"].(map[string]any); ok {
					assert.Equal(t, true, kwargs["enable_thinking"])
					assert.Equal(t, true, kwargs["force_nonempty_content"])
				}
			}

			// Return mock SSE stream
			w.Header().Set("Content-Type", "text/event-stream")
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"thinking\"}}]}\n\n"))
			w.Write([]byte("data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n"))
			w.Write([]byte("data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":20,\"total_tokens\":30}}\n\n"))
			w.Write([]byte("data: [DONE]\n\n"))
		}))
		defer server.Close()

		client.BaseURLField = server.URL

		req := provider.ChatRequest{
			Model:    "nvidia/nemotron-3-ultra-550b-a55b",
			Messages: []types.Message{{Role: "user", Content: "Hello"}},
		}

		iter, err := client.ChatCompletionStream(context.Background(), req)
		require.NoError(t, err)
		require.NotNil(t, iter)

		// Collect chunks
		var chunks []*types.StreamChunk
		for {
			chunk, err := iter.Next()
			if err != nil {
				t.Fatalf("stream error: %v", err)
			}
			if chunk == nil {
				break
			}
			chunks = append(chunks, chunk)
		}

		// Verify we got thinking and content chunks
		foundThinking := false
		foundContent := false
		for _, chunk := range chunks {
			if chunk.Type == "thinking" {
				foundThinking = true
			}
			if chunk.Type == "content" {
				foundContent = true
			}
		}
		assert.True(t, foundThinking, "expected thinking chunk")
		assert.True(t, foundContent, "expected content chunk")
	})

	// Test 3: ChatCompletion collects stream chunks and returns ChatResponse (Wave 1)
	t.Run("chat_completion_collects_chunks", func(t *testing.T) {
		// Check if ChatCompletion method exists
		clientType := reflect.TypeOf(&Client{})
		method, hasMethod := clientType.MethodByName("ChatCompletion")
		if !hasMethod {
			t.Skip("ChatCompletion method not yet implemented (Wave 1)")
		}

		if !hasChatResponse {
			t.Skip("ChatResponse type not yet implemented (Wave 1)")
		}

		client, err := New("test-key", Options{})
		require.NoError(t, err)

		server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Content-Type", "text/event-stream")
			w.WriteHeader(http.StatusOK)
			w.Write([]byte("data: {\"choices\":[{\"delta\":{\"content\":\"Hello \"}}]}\n\n"))
			w.Write([]byte("data: {\"choices\":[{\"delta\":{\"content\":\"world\"}}]}\n\n"))
			w.Write([]byte("data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":10,\"total_tokens\":15}}\n\n"))
			w.Write([]byte("data: [DONE]\n\n"))
		}))
		defer server.Close()

		client.BaseURLField = server.URL

		req := provider.ChatRequest{
			Model:    "nvidia/nemotron-3-ultra-550b-a55b",
			Messages: []types.Message{{Role: "user", Content: "Hello"}},
		}

		// Use reflection to call ChatCompletion
		results := method.Func.Call([]reflect.Value{
			reflect.ValueOf(client),
			reflect.ValueOf(context.Background()),
			reflect.ValueOf(req),
		})

		respVal := results[0]
		errVal := results[1]

		if !errVal.IsNil() {
			t.Fatalf("ChatCompletion error: %v", errVal.Interface())
		}

		resp := respVal.Interface().(*types.ChatResponse)
		require.NotNil(t, resp)

		assert.Equal(t, "Hello world", resp.Content)
		assert.Equal(t, "nvidia/nemotron-3-ultra-550b-a55b", resp.Model)
		assert.Equal(t, "stop", resp.FinishReason)
		assert.Equal(t, 5, resp.Usage.PromptTokens)
		assert.Equal(t, 10, resp.Usage.CompletionTokens)
		assert.Equal(t, 15, resp.Usage.TotalTokens)
	})

	// Test 4: buildNvidiaBody includes extra_body with reasoning config
	t.Run("build_nvidia_body_reasoning_config", func(t *testing.T) {
		client, err := New("test-key", Options{})
		require.NoError(t, err)

		req := provider.ChatRequest{
			Model:    "nvidia/nemotron-3-ultra-550b-a55b",
			Messages: []types.Message{{Role: "user", Content: "test"}},
		}

		body := client.BuildNvidiaBodyForTest(req)

		extraBody, ok := body["extra_body"].(map[string]any)
		require.True(t, ok, "extra_body should be present")
		assert.Equal(t, float64(32768), extraBody["reasoning_budget"])

		kwargs, ok := extraBody["chat_template_kwargs"].(map[string]any)
		require.True(t, ok, "chat_template_kwargs should be present")
		assert.Equal(t, true, kwargs["enable_thinking"])
		assert.Equal(t, true, kwargs["force_nonempty_content"])
	})

	// Test 5: reasoning config applied for ultra model
	t.Run("reasoning_config_ultra_model", func(t *testing.T) {
		cfg, ok := provider.GetReasoningConfig("nvidia/nemotron-3-ultra-550b-a55b")
		require.True(t, ok, "should have reasoning config for ultra model")
		assert.Equal(t, "nvidia", cfg.ModelFamily)
		assert.Equal(t, "choices.0.delta.reasoning_content", cfg.SSEField)

		extraBody := cfg.ExtraBodyParams
		require.NotNil(t, extraBody)
		assert.Equal(t, float64(32768), extraBody["reasoning_budget"])

		kwargs, ok := extraBody["chat_template_kwargs"].(map[string]any)
		require.True(t, ok)
		assert.Equal(t, true, kwargs["enable_thinking"])
		assert.Equal(t, true, kwargs["force_nonempty_content"])
	})

	// Test 6: Table-driven tests for different model configs
	t.Run("model_configs", func(t *testing.T) {
		tests := []struct {
			modelID          string
			hasReasoning     bool
			expectThinking   bool
			expectForceNonEmpty bool
		}{
			{"nvidia/nemotron-3-ultra-550b-a55b", true, true, true},
			{"nvidia/nemotron-3-nano-omni", true, true, false},
			{"nvidia/some-other-model", false, false, false},
		}

		for _, tt := range tests {
			t.Run(tt.modelID, func(t *testing.T) {
				cfg, ok := provider.GetReasoningConfig(tt.modelID)
				if tt.hasReasoning {
					assert.True(t, ok, "should have reasoning config for %s", tt.modelID)
					if tt.expectThinking {
						assert.NotEmpty(t, cfg.ExtraBodyParams)
						if kwargs, ok := cfg.ExtraBodyParams["chat_template_kwargs"].(map[string]any); ok {
							if tt.expectForceNonEmpty {
								assert.Equal(t, true, kwargs["force_nonempty_content"])
							}
							assert.Equal(t, true, kwargs["enable_thinking"])
						}
					}
				} else {
					assert.False(t, ok, "should not have reasoning config for %s", tt.modelID)
				}
			})
		}
	})
}
