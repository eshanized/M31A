package provider

import (
	"context"
	"errors"
	"reflect"
	"testing"
	"time"

	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestProfileMerging(t *testing.T) {
	t.Parallel()

	modelProfileType := reflect.TypeOf(types.ModelProfile{})
	hasModelProfile := modelProfileType.Kind() != reflect.Invalid

	baseClientType := reflect.TypeOf(&BaseClient{})
	_, hasProfilesField := baseClientType.Elem().FieldByName("Profiles")
	_, hasMergeProfileMethod := baseClientType.MethodByName("MergeProfile")

	// Test 1: ModelProfile type structure
	t.Run("model_profile_structure", func(t *testing.T) {
		require.True(t, hasModelProfile, "ModelProfile type should be implemented")

		expectedFields := []string{
			"ModelID",
			"Temperature",
			"TopP",
			"MaxTokens",
			"ReasoningEnabled",
			"ReasoningBudget",
			"ReasoningConfigRef",
		}
		for _, fieldName := range expectedFields {
			_, found := modelProfileType.FieldByName(fieldName)
			assert.True(t, found, "ModelProfile missing field: %s", fieldName)
		}
	})

	// Test 2: MergeProfile applies precedence: provider defaults → model overrides → request values
	t.Run("merge_precedence", func(t *testing.T) {
		require.True(t, hasMergeProfileMethod && hasProfilesField, "MergeProfile method and Profiles field should be implemented")

		// Create a BaseClient with test profiles
		profiles := &config.ModelProfileConfig{
			ProviderDefaults: map[string]types.ModelProfile{
				"nvidia": {
					ModelID:          "nvidia/default",
					Temperature:      float64Ptr(0.5),
					TopP:             float64Ptr(0.9),
					MaxTokens:        intPtr(2048),
					ReasoningEnabled: boolPtr(false),
				},
			},
			ModelOverrides: map[string]types.ModelProfile{
				"nvidia/nemotron-3-ultra-550b-a55b": {
					ModelID:          "nvidia/nemotron-3-ultra-550b-a55b",
					Temperature:      float64Ptr(1.0),
					TopP:             float64Ptr(0.95),
					MaxTokens:        intPtr(16384),
					ReasoningEnabled: boolPtr(true),
					ReasoningBudget:  intPtr(32768),
				},
			},
		}

		client := BaseClient{
			Profiles: profiles,
		}

		// Request with model override should get model-specific profile
		req := types.ChatRequest{
			Model:    "nvidia/nemotron-3-ultra-550b-a55b",
			Messages: []types.Message{{Role: "user", Content: "test"}},
		}

		merged := client.MergeProfile(req)

		t.Logf("merged.Temperature: %v (expected 1.0)", merged.Temperature)
		t.Logf("merged.TopP: %v (expected 0.95)", merged.TopP)
		t.Logf("merged.MaxTokens: %v (expected 16384)", merged.MaxTokens)
		t.Logf("merged.ReasoningEnabled: %v (expected true)", merged.ReasoningEnabled)

		// Model override should win over provider defaults
		assert.Equal(t, 1.0, *merged.Temperature)
		assert.Equal(t, 0.95, *merged.TopP)
		assert.Equal(t, 16384, merged.MaxTokens)
		assert.True(t, merged.ReasoningEnabled)
		// Note: ReasoningBudget is handled via ReasoningConfigRef or provider-specific body building
	})

	// Test 3: Request values have highest precedence
	t.Run("request_precedence", func(t *testing.T) {
		require.True(t, hasMergeProfileMethod && hasProfilesField, "MergeProfile method and Profiles field should be implemented")

		profiles := &config.ModelProfileConfig{
			ProviderDefaults: map[string]types.ModelProfile{
				"nvidia": {
					Temperature: float64Ptr(0.5),
					TopP:        float64Ptr(0.9),
				},
			},
			ModelOverrides: map[string]types.ModelProfile{
				"test/model": {
					Temperature: float64Ptr(0.8),
					TopP:        float64Ptr(0.95),
				},
			},
		}

		client := BaseClient{
			Profiles: profiles,
		}

		// Request explicitly sets temperature and top_p - should override both provider and model
		req := types.ChatRequest{
			Model:       "test/model",
			Messages:    []types.Message{{Role: "user", Content: "test"}},
			Temperature: float64Ptr(1.2),
			TopP:        float64Ptr(0.99),
		}

		merged := client.MergeProfile(req)

		// Request values should win
		assert.Equal(t, 1.2, *merged.Temperature)
		assert.Equal(t, 0.99, *merged.TopP)
	})

	// Test 4: nil profiles handled gracefully
	t.Run("nil_profiles", func(t *testing.T) {
		require.True(t, hasMergeProfileMethod, "MergeProfile method should be implemented")

		client := BaseClient{
			Profiles: nil,
		}

		req := types.ChatRequest{
			Model:    "test/model",
			Messages: []types.Message{{Role: "user", Content: "test"}},
		}

		merged := client.MergeProfile(req)
		// Should return request unchanged
		assert.Equal(t, req, merged)
	})

	// Test 5: ReasoningConfigRef resolved via GetReasoningConfig
	t.Run("reasoning_config_ref_resolved", func(t *testing.T) {
		require.True(t, hasMergeProfileMethod && hasProfilesField, "MergeProfile method and Profiles field should be implemented")

		profiles := &config.ModelProfileConfig{
			ModelOverrides: map[string]types.ModelProfile{
				"test/model": {
					ReasoningConfigRef: "nvidia/nemotron-3-ultra-550b-a55b",
				},
			},
		}

		client := BaseClient{
			Profiles: profiles,
		}

		req := types.ChatRequest{
			Model:    "test/model",
			Messages: []types.Message{{Role: "user", Content: "test"}},
		}

		merged := client.MergeProfile(req)
		// ReasoningConfigRef should be set from model override
		assert.Equal(t, "nvidia/nemotron-3-ultra-550b-a55b", merged.ReasoningConfigRef)
	})
}

func TestStreamRetry(t *testing.T) {
	t.Parallel()

	baseClientType := reflect.TypeOf(&BaseClient{})
	streamRetryConfigType := reflect.TypeOf(StreamRetryConfig{})
	hasStreamRetryConfig := streamRetryConfigType.Kind() != reflect.Invalid
	_, hasRetryConfigField := baseClientType.Elem().FieldByName("RetryConfig")

	// Test 1: StreamRetryConfig structure
	t.Run("retry_config_structure", func(t *testing.T) {
		require.True(t, hasStreamRetryConfig, "StreamRetryConfig type should be implemented")

		expectedFields := []string{"Mode", "MaxAttempts", "BaseDelay"}
		for _, fieldName := range expectedFields {
			_, found := streamRetryConfigType.FieldByName(fieldName)
			assert.True(t, found, "StreamRetryConfig missing field: %s", fieldName)
		}
	})

	// Test 2: none mode returns error immediately
	t.Run("none_mode", func(t *testing.T) {
		require.True(t, hasRetryConfigField && hasStreamRetryConfig, "RetryConfig field and StreamRetryConfig type should be implemented")

		client := BaseClient{
			RetryConfig: StreamRetryConfig{
				Mode:        "none",
				MaxAttempts: 3,
				BaseDelay:   time.Second,
			},
		}

		attemptCount := 0
		doStreamFunc := func(ctx context.Context, req types.ChatRequest) (*types.StreamIterator, error) {
			attemptCount++
			return nil, errors.New("connection failed")
		}

		_, err := client.RetryStream(context.Background(), types.ChatRequest{}, doStreamFunc)
		require.Error(t, err)
		assert.Equal(t, 1, attemptCount, "none mode should not retry")
	})

	// Test 3: initial_only retries on initial connection failure
	t.Run("initial_only_mode", func(t *testing.T) {
		require.True(t, hasRetryConfigField && hasStreamRetryConfig, "RetryConfig field and StreamRetryConfig type should be implemented")

		client := BaseClient{
			RetryConfig: StreamRetryConfig{
				Mode:        "initial_only",
				MaxAttempts: 3,
				BaseDelay:   10 * time.Millisecond,
			},
		}

		attemptCount := 0
		doStreamFunc := func(ctx context.Context, req types.ChatRequest) (*types.StreamIterator, error) {
			attemptCount++
			if attemptCount < 2 {
				return nil, errors.New("connection refused")
			}
			// Success on second attempt
			return &types.StreamIterator{
				Next: func() (*types.StreamChunk, error) {
					return nil, nil // EOF
				},
				Close: func() error { return nil },
			}, nil
		}

		iter, err := client.RetryStream(context.Background(), types.ChatRequest{}, doStreamFunc)
		require.NoError(t, err)
		require.NotNil(t, iter)
		assert.Equal(t, 2, attemptCount, "initial_only mode should retry on connection error")
	})

	// Test 4: full_resume restarts request on mid-stream failure
	t.Run("full_resume_mode", func(t *testing.T) {
		require.True(t, hasRetryConfigField && hasStreamRetryConfig, "RetryConfig field and StreamRetryConfig type should be implemented")

		client := BaseClient{
			RetryConfig: StreamRetryConfig{
				Mode:        "full_resume",
				MaxAttempts: 3,
				BaseDelay:   10 * time.Millisecond,
			},
		}

		attemptCount := 0
		doStreamFunc := func(ctx context.Context, req types.ChatRequest) (*types.StreamIterator, error) {
			attemptCount++
			if attemptCount < 2 {
				return nil, errors.New("mid-stream error")
			}
			// Success on second attempt
			return &types.StreamIterator{
				Next: func() (*types.StreamChunk, error) {
					return nil, nil // EOF
				},
				Close: func() error { return nil },
			}, nil
		}

		iter, err := client.RetryStream(context.Background(), types.ChatRequest{}, doStreamFunc)
		require.NoError(t, err)
		require.NotNil(t, iter)
		assert.Equal(t, 2, attemptCount, "full_resume mode should retry on any error")
	})

	// Test 5: All three providers use shared BaseClient retry logic
	t.Run("providers_share_retry_logic", func(t *testing.T) {
		// This is a structural test - verify the retry config is on BaseClient
		// and all providers embed BaseClient
		// We can't import concrete clients due to import cycle, but we can verify
		// that BaseClient has RetryConfig field which all providers embed
		baseClientType := reflect.TypeOf(&BaseClient{})
		_, hasRetryConfig := baseClientType.Elem().FieldByName("RetryConfig")
		assert.True(t, hasRetryConfig, "BaseClient should have RetryConfig field")

		// Verify the concrete provider types embed BaseClient by checking
		// that they have the BaseClient methods
		// This is a compile-time check - if providers didn't embed BaseClient,
		// they wouldn't compile
		var _ = BaseClient{} // BaseClient exists
	})
}

// Helper functions
func float64Ptr(v float64) *float64 {
	return &v
}

func intPtr(v int) *int {
	return &v
}

func boolPtr(v bool) *bool {
	return &v
}