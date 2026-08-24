package provider

import (
	"reflect"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestReasoningConfig(t *testing.T) {
	t.Parallel()

	// Test 1: reasoningParamMap has entry for nvidia/nemotron-3-ultra-550b-a55b
	t.Run("ultra_model_entry", func(t *testing.T) {
		t.Skip("Requires ultra model reasoning config (Wave 1)")
		cfg, ok := GetReasoningConfig("nvidia/nemotron-3-ultra-550b-a55b")
		require.True(t, ok, "should have reasoning config for ultra model")

		assert.Equal(t, "nvidia", cfg.ModelFamily)
		assert.Empty(t, cfg.RequestParams)
		assert.NotNil(t, cfg.ExtraBodyParams)

		extraBody := cfg.ExtraBodyParams
		assert.Equal(t, float64(32768), extraBody["reasoning_budget"])

		kwargs, ok := extraBody["chat_template_kwargs"].(map[string]any)
		require.True(t, ok)
		assert.Equal(t, true, kwargs["enable_thinking"])
		assert.Equal(t, true, kwargs["force_nonempty_content"])

		assert.Equal(t, "choices.0.delta.reasoning_content", cfg.SSEField)
	})

	// Test 2: GetReasoningConfig matches model ID prefixes correctly (sorted keys handle specificity)
	t.Run("prefix_matching_specificity", func(t *testing.T) {
		tests := []struct {
			modelID     string
			expectedKey string
			shouldMatch bool
		}{
			// Exact matches (most specific)
			{"nvidia/nemotron-3-nano-omni", "nvidia/nemotron-3-nano-omni", true},
			// Prefix matches
			{"deepseek/deepseek-r1", "deepseek", true},
			{"openai/o3-mini", "openai/o-", true},
			{"openai/o1-preview", "openai/o-", true},
			{"anthropic/claude-3-opus", "anthropic", true},
			{"qwen/qwen-2.5", "qwen", true},
			// No match
			{"custom/unknown-model", "", false},
		}

		for _, tt := range tests {
			t.Run(tt.modelID, func(t *testing.T) {
				cfg, ok := GetReasoningConfig(tt.modelID)
				if tt.shouldMatch {
					assert.True(t, ok, "should match for %s", tt.modelID)
					if tt.expectedKey != "" {
						expectedCfg := reasoningParamMap[tt.expectedKey]
						assert.Equal(t, expectedCfg.ModelFamily, cfg.ModelFamily)
						assert.Equal(t, expectedCfg.SSEField, cfg.SSEField)
					}
				} else {
					assert.False(t, ok, "should not match for %s", tt.modelID)
				}
			})
		}

		// Ultra model tests (require Wave 1 implementation)
		t.Run("nvidia/nemotron-3-ultra-550b-a55b", func(t *testing.T) {
			t.Skip("Requires ultra model reasoning config (Wave 1)")
		})
		t.Run("openai/o4-mini", func(t *testing.T) {
			t.Skip("Requires openai/o4-mini prefix matching fix (Wave 1)")
		})
	})

	// Test 3: ApplyReasoningParams applies RequestParams at top level and ExtraBodyParams nested in extra_body
	t.Run("apply_reasoning_params_dual_path", func(t *testing.T) {
		// Test with RequestParams (OpenAI style)
		body := map[string]any{
			"model": "openai/o3-mini",
			"messages": []map[string]any{
				{"role": "user", "content": "test"},
			},
		}

		result := ApplyReasoningParams("openai/o3-mini", body)
		assert.Equal(t, "medium", result["reasoning_effort"])

		// Test with ExtraBodyParams (NVIDIA style) - requires ultra model config (Wave 1)
		t.Skip("Requires ultra model reasoning config (Wave 1)")
		body = map[string]any{
			"model": "nvidia/nemotron-3-ultra-550b-a55b",
			"messages": []map[string]any{
				{"role": "user", "content": "test"},
			},
		}

		result = ApplyReasoningParams("nvidia/nemotron-3-ultra-550b-a55b", body)
		extraBody, ok := result["extra_body"].(map[string]any)
		require.True(t, ok)
		assert.Equal(t, float64(32768), extraBody["reasoning_budget"])
		kwargs, ok := extraBody["chat_template_kwargs"].(map[string]any)
		require.True(t, ok)
		assert.Equal(t, true, kwargs["enable_thinking"])
		assert.Equal(t, true, kwargs["force_nonempty_content"])
	})

	// Test 4: SSEFieldParts pre-computed for all entries in init()
	t.Run("sse_field_parts_precomputed", func(t *testing.T) {
		for key, cfg := range reasoningParamMap {
			if cfg.SSEField != "" {
				assert.NotEmpty(t, cfg.SSEFieldParts, "SSEFieldParts should be precomputed for %s", key)
				// Verify parts are correct
				expectedParts := splitSSEField(cfg.SSEField)
				assert.Equal(t, expectedParts, cfg.SSEFieldParts, "SSEFieldParts mismatch for %s", key)
			}
		}

		// Also verify sorted configs have parts
		for _, cfg := range sortedReasoningConfigs {
			if cfg.SSEField != "" {
				assert.NotEmpty(t, cfg.SSEFieldParts)
			}
		}
	})

	// Test 5: NVIDIA ultra model entry uses correct SSEField
	t.Run("nvidia_ultra_sse_field", func(t *testing.T) {
		t.Skip("Requires ultra model reasoning config (Wave 1)")
		cfg, ok := GetReasoningConfig("nvidia/nemotron-3-ultra-550b-a55b")
		require.True(t, ok)
		assert.Equal(t, "choices.0.delta.reasoning_content", cfg.SSEField)
		assert.Equal(t, []string{"choices", "0", "delta", "reasoning_content"}, cfg.SSEFieldParts)
	})

	// Test 6: OpenAI-style reasoning parameter applied via RequestParams
	t.Run("openai_style_request_params", func(t *testing.T) {
		tests := []struct {
			modelID       string
			expectedParam string
			expectedValue any
		}{
			{"openai/o3-mini", "reasoning_effort", "medium"},
			{"openai/o1-preview", "reasoning_effort", "medium"},
			{"anthropic/claude-3-opus", "thinking", map[string]any{
				"type":          "enabled",
				"budget_tokens": 1024,
			}},
		}

		for _, tt := range tests {
			t.Run(tt.modelID, func(t *testing.T) {
				body := map[string]any{"model": tt.modelID}
				result := ApplyReasoningParams(tt.modelID, body)
				assert.Equal(t, tt.expectedValue, result[tt.expectedParam])
			})
		}

		// o4-mini test requires prefix matching fix (Wave 1)
		t.Run("openai/o4-mini", func(t *testing.T) {
			t.Skip("Requires openai/o4-mini prefix matching fix (Wave 1)")
		})
	})

	// Test 7: NVIDIA extra_body parameters applied via ExtraBodyParams
	t.Run("nvidia_extra_body_params", func(t *testing.T) {
		t.Skip("ExtraBodyParams are applied by provider-specific code (buildNvidiaBody), not ApplyReasoningParams")
		tests := []struct {
			modelID         string
			expectExtraBody bool
			reasoningBudget float64
			enableThinking  bool
			forceNonEmpty   bool
		}{
			{"nvidia/nemotron-3-nano-omni", true, 16384, true, false},
			{"nvidia/some-other-model", false, 0, false, false},
		}

		for _, tt := range tests {
			t.Run(tt.modelID, func(t *testing.T) {
				body := map[string]any{"model": tt.modelID}
				result := ApplyReasoningParams(tt.modelID, body)

				if tt.expectExtraBody {
					extraBody, ok := result["extra_body"].(map[string]any)
					require.True(t, ok)
					// reasoning_budget can be int or float64
					budget := extraBody["reasoning_budget"]
					switch v := budget.(type) {
					case int:
						assert.Equal(t, int(tt.reasoningBudget), v)
					case float64:
						assert.Equal(t, tt.reasoningBudget, v)
					default:
						t.Fatalf("unexpected type for reasoning_budget: %T", v)
					}
					kwargs, ok := extraBody["chat_template_kwargs"].(map[string]any)
					require.True(t, ok)
					assert.Equal(t, tt.enableThinking, kwargs["enable_thinking"])
					assert.Equal(t, tt.forceNonEmpty, kwargs["force_nonempty_content"])
				} else {
					_, hasExtraBody := result["extra_body"]
					assert.False(t, hasExtraBody)
				}
			})
		}

		// Ultra model test requires Wave 1 implementation
		t.Run("nvidia/nemotron-3-ultra-550b-a55b", func(t *testing.T) {
			t.Skip("Requires ultra model reasoning config (Wave 1)")
		})
	})

	// Test 8: Edge cases - unknown model returns empty config
	t.Run("unknown_model_empty_config", func(t *testing.T) {
		cfg, ok := GetReasoningConfig("completely/unknown-model")
		assert.False(t, ok)
		assert.Equal(t, ReasoningConfig{}, cfg)

		body := map[string]any{"model": "unknown"}
		result := ApplyReasoningParams("unknown", body)
		assert.Equal(t, body, result) // Should return unchanged
	})

	// Test 9: Prefix matching specificity - longer prefixes match first
	t.Run("prefix_specificity_ordering", func(t *testing.T) {
		// The sorted keys should have longer/more specific prefixes first
		// This is tested implicitly by the prefix matching tests above
		// but we can verify the sort order
		for i := 0; i < len(sortedReasoningKeys)-1; i++ {
			// Each key should be >= the next in reverse alphabetical order
			// which means longer/specific prefixes come first
			assert.GreaterOrEqual(t, sortedReasoningKeys[i], sortedReasoningKeys[i+1])
		}
	})

	// Test 10: Deepseek config
	t.Run("deepseek_config", func(t *testing.T) {
		cfg, ok := GetReasoningConfig("deepseek/deepseek-r1")
		require.True(t, ok)
		assert.Equal(t, "deepseek", cfg.ModelFamily)
		assert.Equal(t, "choices.0.delta.reasoning_content", cfg.SSEField)
		assert.Empty(t, cfg.RequestParams)
		assert.Nil(t, cfg.ExtraBodyParams)
	})

	// Test 11: Anthropic config
	t.Run("anthropic_config", func(t *testing.T) {
		cfg, ok := GetReasoningConfig("anthropic/claude-3-opus")
		require.True(t, ok)
		assert.Equal(t, "anthropic", cfg.ModelFamily)
		assert.Equal(t, "choices.0.delta.content", cfg.SSEField)
		assert.NotEmpty(t, cfg.RequestParams)
		thinking, ok := cfg.RequestParams["thinking"].(map[string]any)
		require.True(t, ok)
		assert.Equal(t, "enabled", thinking["type"])
		// budget_tokens is stored as int in the config
		budgetTokens := thinking["budget_tokens"]
		switch v := budgetTokens.(type) {
		case int:
			assert.Equal(t, 1024, v)
		case float64:
			assert.Equal(t, float64(1024), v)
		default:
			t.Fatalf("unexpected type for budget_tokens: %T", v)
		}
		assert.Nil(t, cfg.ExtraBodyParams)
	})

	// Test 12: Qwen config
	t.Run("qwen_config", func(t *testing.T) {
		cfg, ok := GetReasoningConfig("qwen/qwen-2.5")
		require.True(t, ok)
		assert.Equal(t, "qwen", cfg.ModelFamily)
		assert.Equal(t, "choices.0.delta.reasoning_content", cfg.SSEField)
		assert.Empty(t, cfg.RequestParams)
		assert.Nil(t, cfg.ExtraBodyParams)
	})

	// Test 13: nano-omni config
	t.Run("nano_omni_config", func(t *testing.T) {
		cfg, ok := GetReasoningConfig("nvidia/nemotron-3-nano-omni")
		require.True(t, ok)
		assert.Equal(t, "nvidia", cfg.ModelFamily)
		assert.Equal(t, "choices.0.delta.reasoning_content", cfg.SSEField)
		assert.NotNil(t, cfg.ExtraBodyParams)
		// reasoning_budget is stored as int in the config
		budget := cfg.ExtraBodyParams["reasoning_budget"]
		switch v := budget.(type) {
		case int:
			assert.Equal(t, 16384, v)
		case float64:
			assert.Equal(t, float64(16384), v)
		default:
			t.Fatalf("unexpected type for reasoning_budget: %T", v)
		}
		kwargs, ok := cfg.ExtraBodyParams["chat_template_kwargs"].(map[string]any)
		require.True(t, ok)
		assert.Equal(t, true, kwargs["enable_thinking"])
		forceNonEmpty, _ := kwargs["force_nonempty_content"].(bool)
		assert.False(t, forceNonEmpty) // nano-omni doesn't have force_nonempty_content
	})

	// Test 14: ReasoningConfig struct has expected fields
	t.Run("reasoning_config_struct", func(t *testing.T) {
		reasoningConfigType := reflect.TypeOf(ReasoningConfig{})
		expectedFields := []string{"ModelFamily", "RequestParams", "ExtraBodyParams", "SSEField", "SSEFieldParts"}
		for _, fieldName := range expectedFields {
			_, found := reasoningConfigType.FieldByName(fieldName)
			assert.True(t, found, "ReasoningConfig missing field: %s", fieldName)
		}
	})
}

// Helper to split SSE field for verification
func splitSSEField(field string) []string {
	var parts []string
	current := ""
	for _, c := range field {
		if c == '.' {
			parts = append(parts, current)
			current = ""
		} else {
			current += string(c)
		}
	}
	if current != "" {
		parts = append(parts, current)
	}
	return parts
}
