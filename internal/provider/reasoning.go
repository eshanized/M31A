package provider

import (
	"encoding/json"
	"fmt"
	"strconv"
	"strings"

	"github.com/eshanized/M31A/internal/types"
)

type ReasoningConfig struct {
	ModelFamily   string         `json:"model_family"`
	RequestParams map[string]any `json:"request_params"`
	SSEField      string         `json:"sse_field"`
}

var reasoningParamMap = map[string]ReasoningConfig{
	"deepseek": {
		ModelFamily:   "deepseek",
		RequestParams: map[string]any{},
		SSEField:      "choices.0.delta.reasoning_content",
	},
	"openai/o-": {
		ModelFamily:   "openai",
		RequestParams: map[string]any{"reasoning_effort": "medium"},
		SSEField:      "choices.0.delta.reasoning",
	},
	"anthropic": {
		ModelFamily:   "anthropic",
		RequestParams: map[string]any{
			"thinking": map[string]any{
				"type":          "enabled",
				"budget_tokens": 1024,
			},
		},
		SSEField: "choices.0.delta.content",
	},
	"qwen": {
		ModelFamily:   "qwen",
		RequestParams: map[string]any{},
		SSEField:      "choices.0.delta.reasoning_content",
	},
}

func GetReasoningConfig(modelID string) (ReasoningConfig, bool) {
	keys := make([]string, 0, len(reasoningParamMap))
	for k := range reasoningParamMap {
		keys = append(keys, k)
	}

	for _, prefix := range keys {
		if strings.HasPrefix(modelID, prefix) {
			return reasoningParamMap[prefix], true
		}
	}

	if strings.HasPrefix(modelID, "openai/") {
		modelName := strings.TrimPrefix(modelID, "openai/")
		if strings.HasPrefix(modelName, "o-") || strings.HasPrefix(modelName, "o1") || strings.HasPrefix(modelName, "o3") {
			cfg := reasoningParamMap["openai/o-"]
			return cfg, true
		}
	}

	return ReasoningConfig{}, false
}

func ApplyReasoningParams(modelID string, body map[string]any) map[string]any {
	cfg, ok := GetReasoningConfig(modelID)
	if !ok {
		return body
	}
	for k, v := range cfg.RequestParams {
		body[k] = v
	}
	return body
}

func getNestedField(m map[string]any, path string) (any, bool) {
	parts := strings.Split(path, ".")
	current := any(m)
	for _, part := range parts {
		switch v := current.(type) {
		case map[string]any:
			var exists bool
			current, exists = v[part]
			if !exists {
				return nil, false
			}
		case []any:
			idx, err := strconv.Atoi(part)
			if err != nil {
				return nil, false
			}
			if idx < 0 || idx >= len(v) {
				return nil, false
			}
			current = v[idx]
		default:
			return nil, false
		}
	}
	return current, true
}

func ParseSSEChunk(data string, modelID string) (*types.StreamChunk, error) {
	var raw map[string]any
	if err := json.Unmarshal([]byte(data), &raw); err != nil {
		return nil, err
	}

	cfg, _ := GetReasoningConfig(modelID)

	choices, ok := getNestedField(raw, "choices")
	if !ok {
		return nil, fmt.Errorf("missing 'choices' field in SSE payload")
	}

	choicesArr, ok := choices.([]any)
	if !ok || len(choicesArr) == 0 {
		return nil, fmt.Errorf("empty or malformed 'choices' array in SSE payload")
	}

	firstChoice, ok := choicesArr[0].(map[string]any)
	if !ok {
		return nil, fmt.Errorf("first choice in 'choices' array is not an object")
	}

	if finishReason, exists := firstChoice["finish_reason"]; exists && finishReason != nil {
		return &types.StreamChunk{Type: "done"}, nil
	}

	delta, ok := firstChoice["delta"]
	if !ok {
		return nil, nil
	}

	deltaMap, ok := delta.(map[string]any)
	if !ok {
		return nil, nil
	}

	if cfg.ModelFamily == "anthropic" {
		if contentType, exists := deltaMap["type"]; exists {
			if contentTypeStr, ok := contentType.(string); ok && contentTypeStr == "thinking" {
				content, _ := deltaMap["content"].(string)
				return &types.StreamChunk{Type: "thinking", Delta: content}, nil
			}
		}
	}

	if cfg.SSEField != "" {
		parts := strings.Split(cfg.SSEField, ".")
		if len(parts) >= 4 {
			fieldName := parts[len(parts)-1]
			if val, exists := deltaMap[fieldName]; exists {
				if str, ok := val.(string); ok && str != "" {
					return &types.StreamChunk{Type: "thinking", Delta: str}, nil
				}
			}
		}
	}

	content, _ := deltaMap["content"].(string)
	return &types.StreamChunk{Type: "content", Delta: content}, nil
}
