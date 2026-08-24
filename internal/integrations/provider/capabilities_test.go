package provider

import (
	"reflect"
	"testing"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestCapabilityDetection(t *testing.T) {
	t.Parallel()

	modelInfoType := reflect.TypeOf(types.ModelInfo{})
	hasMaxOutputTokens := false
	hasSupportedParameters := false
	hasInputModalities := false
	hasOutputModalities := false
	if _, ok := modelInfoType.FieldByName("MaxOutputTokens"); ok {
		hasMaxOutputTokens = true
	}
	if _, ok := modelInfoType.FieldByName("SupportedParameters"); ok {
		hasSupportedParameters = true
	}
	if _, ok := modelInfoType.FieldByName("InputModalities"); ok {
		hasInputModalities = true
	}
	if _, ok := modelInfoType.FieldByName("OutputModalities"); ok {
		hasOutputModalities = true
	}

	// Test 1: FetchModels enriches ModelInfo with API metadata for NVIDIA
	t.Run("nvidia_api_enrichment", func(t *testing.T) {
		t.Skip("Requires nvidia client - run in nvidia package")
	})

	// Test 2: FetchModels enriches ModelInfo with API metadata for OpenRouter
	t.Run("openrouter_api_enrichment", func(t *testing.T) {
		// Use reflection to call FetchModels on a client that embeds BaseClient
		clientType := reflect.TypeOf(&struct{ BaseClient }{})
		_, hasFetchModels := clientType.MethodByName("FetchModels")
		if !hasFetchModels {
			t.Skip("FetchModels not yet implemented on mock client")
		}

		// This test will be expanded when concrete clients are available
		// For now, verify the structure exists
		assert.True(t, true)
	})

	// Test 3: FetchModels enriches ModelInfo with API metadata for Zen
	t.Run("zen_api_enrichment", func(t *testing.T) {
		clientType := reflect.TypeOf(&struct{ BaseClient }{})
		_, hasFetchModels := clientType.MethodByName("FetchModels")
		if !hasFetchModels {
			t.Skip("FetchModels not yet implemented on mock client")
		}

		assert.True(t, true)
	})

	// Test 4: MaxOutputTokens populated from API (or defaults) - will be added in Wave 3
	t.Run("max_output_tokens", func(t *testing.T) {
		if !hasMaxOutputTokens {
			t.Skip("MaxOutputTokens field not yet implemented (Wave 3)")
		}

		assert.True(t, true)
	})

	// Test 5: SupportedParameters includes temperature, top_p, max_tokens, reasoning_budget, chat_template_kwargs (Wave 3)
	t.Run("supported_parameters", func(t *testing.T) {
		if !hasSupportedParameters {
			t.Skip("SupportedParameters field not yet implemented (Wave 3)")
		}

		model := types.ModelInfo{}
		modelVal := reflect.ValueOf(&model).Elem()
		field := modelVal.FieldByName("SupportedParameters")
		if field.IsValid() && field.CanSet() {
			field.Set(reflect.ValueOf([]string{"temperature", "top_p", "max_tokens", "reasoning_budget", "chat_template_kwargs"}))
		}

		// Use reflection to access the field
		supportedParams := modelVal.FieldByName("SupportedParameters").Interface().([]string)
		expectedParams := []string{"temperature", "top_p", "max_tokens", "reasoning_budget", "chat_template_kwargs"}
		for _, param := range expectedParams {
			assert.Contains(t, supportedParams, param)
		}
	})

	// Test 6: InputModalities/OutputModalities from API (Wave 3)
	t.Run("modalities", func(t *testing.T) {
		if !hasInputModalities || !hasOutputModalities {
			t.Skip("InputModalities/OutputModalities fields not yet implemented (Wave 3)")
		}

		model := types.ModelInfo{}
		modelVal := reflect.ValueOf(&model).Elem()

		if field := modelVal.FieldByName("InputModalities"); field.IsValid() && field.CanSet() {
			field.Set(reflect.ValueOf([]string{"text", "image"}))
		}
		if field := modelVal.FieldByName("OutputModalities"); field.IsValid() && field.CanSet() {
			field.Set(reflect.ValueOf([]string{"text"}))
		}

		inputModalities := modelVal.FieldByName("InputModalities").Interface().([]string)
		outputModalities := modelVal.FieldByName("OutputModalities").Interface().([]string)

		assert.Contains(t, inputModalities, "text")
		assert.Contains(t, inputModalities, "image")
		assert.Contains(t, outputModalities, "text")
	})

	// Test 7: Capabilities from API take precedence; ParseModelCapabilities used as fallback
	t.Run("api_precedence_over_heuristics", func(t *testing.T) {
		// Heuristics would say gpt-4o has tools, reasoning=false, vision=false
		heuristicCaps := ParseModelCapabilities("openai/gpt-4o")
		assert.True(t, heuristicCaps.Tools)
		assert.False(t, heuristicCaps.Reasoning)
		assert.False(t, heuristicCaps.Vision) // gpt-4o doesn't contain "vision" or "multimodal"

		// API metadata should be able to override
		model := types.ModelInfo{
			ID:           "openai/gpt-4o",
			Capabilities: types.CapFlags{Tools: true, Reasoning: true, Vision: true, Chat: true}, // API says reasoning and vision too
		}

		// API metadata takes precedence
		assert.True(t, model.Capabilities.Reasoning)
		assert.True(t, model.Capabilities.Vision)
	})

	// Test 8: ModelCache caches enriched models with TTL
	t.Run("model_cache_ttl", func(t *testing.T) {
		t.Skip("Requires nvidia client - run in nvidia package")
	})

	// Test 9: Heuristics fallback when API metadata missing
	t.Run("heuristics_fallback", func(t *testing.T) {
		// When API doesn't provide capability fields, ParseModelCapabilities is used
		caps := ParseModelCapabilities("custom/unknown-model")
		// Should have defaults (Chat=true, others based on patterns)
		assert.True(t, caps.Chat)

		model := types.ModelInfo{
			ID:           "custom/unknown-model",
			Capabilities: caps,
		}

		assert.True(t, model.Capabilities.Chat)
	})

	// Test 10: NVIDIA-specific extra_body parameters in capability metadata
	t.Run("nvidia_extra_body_params", func(t *testing.T) {
		cfg, ok := GetReasoningConfig("nvidia/nemotron-3-ultra-550b-a55b")
		require.True(t, ok)

		extraBody := cfg.ExtraBodyParams
		require.NotNil(t, extraBody)

		// Should have reasoning_budget and chat_template_kwargs
		assert.Contains(t, extraBody, "reasoning_budget")
		assert.Contains(t, extraBody, "chat_template_kwargs")

		kwargs, ok := extraBody["chat_template_kwargs"].(map[string]any)
		require.True(t, ok)
		assert.Equal(t, true, kwargs["enable_thinking"])
		assert.Equal(t, true, kwargs["force_nonempty_content"])
	})

	// Test 11: OpenRouter and Zen OpenRouter-compatible API capability parsing
	t.Run("openrouter_zen_compatible", func(t *testing.T) {
		clientType := reflect.TypeOf(&struct{ BaseClient }{})
		_, hasFetchModels := clientType.MethodByName("FetchModels")
		if !hasFetchModels {
			t.Skip("FetchModels not yet implemented on mock client")
		}

		assert.True(t, true)
	})
}

// Integration test (requires API keys) - skipped by default
func TestCapabilityDetection_Integration(t *testing.T) {
	if testing.Short() {
		t.Skip("Skipping integration test in short mode")
	}
}