package types

import (
	"reflect"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestChatResponse(t *testing.T) {
	t.Parallel()

	// Test ChatResponse type exists and has expected fields
	chatResponseType := reflect.TypeOf(ChatResponse{})
	assert.NotEqual(t, reflect.Invalid, chatResponseType.Kind(), "ChatResponse type should exist")

	expectedFields := []string{
		"Content",
		"Usage",
		"Model",
		"FinishReason",
	}
	for _, fieldName := range expectedFields {
		field, found := chatResponseType.FieldByName(fieldName)
		assert.True(t, found, "ChatResponse missing field: %s", fieldName)
		if found {
			t.Logf("Field %s: %v", fieldName, field.Type)
		}
	}

	// Test ChatResponse serialization
	response := ChatResponse{
		Content:      "test content",
		Usage:        &Usage{PromptTokens: 10, CompletionTokens: 20, TotalTokens: 30},
		Model:        "test-model",
		FinishReason: "stop",
	}

	assert.Equal(t, "test content", response.Content)
	assert.Equal(t, 10, response.Usage.PromptTokens)
	assert.Equal(t, 20, response.Usage.CompletionTokens)
	assert.Equal(t, 30, response.Usage.TotalTokens)
	assert.Equal(t, "test-model", response.Model)
	assert.Equal(t, "stop", response.FinishReason)
}

func TestModelInfoExtensions(t *testing.T) {
	t.Parallel()

	// Test ModelInfo has extended capability fields
	modelInfoType := reflect.TypeOf(ModelInfo{})
	expectedFields := []string{
		"MaxOutputTokens",
		"SupportedParameters",
		"InputModalities",
		"OutputModalities",
		"Variant",
	}
	for _, fieldName := range expectedFields {
		field, found := modelInfoType.FieldByName(fieldName)
		assert.True(t, found, "ModelInfo missing field: %s", fieldName)
		if found {
			t.Logf("Field %s: %v", fieldName, field.Type)
		}
	}

	// Test ModelInfo with extended fields
	model := ModelInfo{
		ID:                  "test/model",
		Provider:            "test",
		Name:                "Test Model",
		ContextLength:       8192,
		MaxOutputTokens:     4096,
		SupportedParameters: []string{"temperature", "top_p", "max_tokens"},
		InputModalities:     []string{"text"},
		OutputModalities:    []string{"text"},
		Capabilities:        CapFlags{Chat: true, Tools: true},
		Variant:             nil,
	}

	assert.Equal(t, int64(4096), model.MaxOutputTokens)
	assert.Contains(t, model.SupportedParameters, "temperature")
	assert.Contains(t, model.SupportedParameters, "top_p")
	assert.Contains(t, model.SupportedParameters, "max_tokens")
	assert.Contains(t, model.InputModalities, "text")
	assert.Contains(t, model.OutputModalities, "text")
	assert.Nil(t, model.Variant)

	// Test with variant
	variant := "thinking"
	modelWithVariant := ModelInfo{
		ID:       "test/model-thinking",
		Provider: "test",
		Variant:  &variant,
	}
	assert.NotNil(t, modelWithVariant.Variant)
	assert.Equal(t, "thinking", *modelWithVariant.Variant)
}

func TestCapFlagsSerialization(t *testing.T) {
	t.Parallel()

	caps := CapFlags{
		Tools:     true,
		Reasoning: true,
		Vision:    false,
		Chat:      true,
	}

	// Test that all fields are present
	assert.True(t, caps.Chat)
	assert.True(t, caps.Tools)
	assert.True(t, caps.Reasoning)
	assert.False(t, caps.Vision)
}

func TestModelProfileStructure(t *testing.T) {
	t.Parallel()

	// Test ModelProfile type exists and has expected fields
	modelProfileType := reflect.TypeOf(ModelProfile{})
	assert.NotEqual(t, reflect.Invalid, modelProfileType.Kind(), "ModelProfile type should exist")

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
		field, found := modelProfileType.FieldByName(fieldName)
		assert.True(t, found, "ModelProfile missing field: %s", fieldName)
		if found {
			t.Logf("Field %s: %v", fieldName, field.Type)
		}
	}

	// Test ModelProfile with pointer fields for optional params
	profile := ModelProfile{
		ModelID:            "test/model",
		Temperature:        float64Ptr(0.7),
		TopP:               float64Ptr(0.9),
		MaxTokens:          intPtr(4096),
		ReasoningEnabled:   boolPtr(true),
		ReasoningBudget:    intPtr(32768),
		ReasoningConfigRef: "nvidia/nemotron-3-ultra-550b-a55b",
	}

	assert.Equal(t, "test/model", profile.ModelID)
	assert.Equal(t, 0.7, *profile.Temperature)
	assert.Equal(t, 0.9, *profile.TopP)
	assert.Equal(t, 4096, *profile.MaxTokens)
	assert.Equal(t, true, *profile.ReasoningEnabled)
	assert.Equal(t, 32768, *profile.ReasoningBudget)
	assert.Equal(t, "nvidia/nemotron-3-ultra-550b-a55b", profile.ReasoningConfigRef)

	// Test with nil pointers (not set)
	emptyProfile := ModelProfile{
		ModelID: "test/model",
	}
	assert.Nil(t, emptyProfile.Temperature)
	assert.Nil(t, emptyProfile.TopP)
	assert.Nil(t, emptyProfile.MaxTokens)
	assert.Nil(t, emptyProfile.ReasoningEnabled)
	assert.Nil(t, emptyProfile.ReasoningBudget)
	assert.Equal(t, "", emptyProfile.ReasoningConfigRef)
}

func TestChatRequestExtensions(t *testing.T) {
	t.Parallel()

	// Test ChatRequest has extended fields
	chatRequestType := reflect.TypeOf(ChatRequest{})
	expectedFields := []string{
		"Provider",
		"Temperature",
		"TopP",
		"ReasoningConfigRef",
	}
	for _, fieldName := range expectedFields {
		field, found := chatRequestType.FieldByName(fieldName)
		assert.True(t, found, "ChatRequest missing field: %s", fieldName)
		if found {
			t.Logf("Field %s: %v", fieldName, field.Type)
		}
	}

	// Test helper methods
	req := ChatRequest{
		Model:              "test/model",
		Messages:           []Message{{Role: "user", Content: "test"}},
		Provider:           "nvidia",
		Temperature:        float64Ptr(0.7),
		TopP:               float64Ptr(0.9),
		ReasoningConfigRef: "nvidia/nemotron-3-ultra-550b-a55b",
		ReasoningEnabled:   true,
		MaxTokens:          4096,
	}

	assert.True(t, req.HasProvider())
	assert.Equal(t, "nvidia", req.Provider)
	assert.True(t, req.HasTemperature())
	assert.Equal(t, 0.7, *req.Temperature)
	assert.True(t, req.HasTopP())
	assert.Equal(t, 0.9, *req.TopP)
	assert.True(t, req.HasReasoningConfigRef())
	assert.Equal(t, "nvidia/nemotron-3-ultra-550b-a55b", req.ReasoningConfigRef)
	assert.True(t, req.HasReasoningEnabled())
	assert.True(t, req.HasMaxTokens())

	// Test with empty request
	emptyReq := ChatRequest{
		Model:    "test/model",
		Messages: []Message{{Role: "user", Content: "test"}},
	}
	assert.False(t, emptyReq.HasProvider())
	assert.False(t, emptyReq.HasTemperature())
	assert.False(t, emptyReq.HasTopP())
	assert.False(t, emptyReq.HasReasoningConfigRef())
	assert.False(t, emptyReq.HasReasoningEnabled())
	assert.False(t, emptyReq.HasMaxTokens())
}

// Helper functions for pointer creation
func float64Ptr(v float64) *float64 {
	return &v
}

func intPtr(v int) *int {
	return &v
}

func boolPtr(v bool) *bool {
	return &v
}
