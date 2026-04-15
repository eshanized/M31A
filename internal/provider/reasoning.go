package provider

import (
	"encoding/json"
	"fmt"
	"sort"
	"strconv"
	"strings"

	"github.com/eshanized/M31A/internal/types"
)

type ReasoningConfig struct {
	ModelFamily   string         `json:"model_family"`
	RequestParams map[string]any `json:"request_params"`
	SSEField      string         `json:"sse_field"`
	// Pre-computed field path parts to avoid per-chunk strings.Split
	SSEFieldParts []string `json:"-"`
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
		ModelFamily: "anthropic",
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

// Pre-computed sorted keys and SSE field parts to avoid per-chunk allocations.
// Initialized once at package load time.
var (
	sortedReasoningKeys    []string
	sortedReasoningConfigs []ReasoningConfig
)

func init() {
	sortedReasoningKeys = make([]string, 0, len(reasoningParamMap))
	for k := range reasoningParamMap {
		sortedReasoningKeys = append(sortedReasoningKeys, k)
	}
	// Sort by length descending so longer prefixes match first
	sort.Sort(sort.Reverse(sort.StringSlice(sortedReasoningKeys)))
	// Pre-compute SSEFieldParts on both the map entries (for fallback path)
	// and the sorted configs slice (for fast path).
	for k := range reasoningParamMap {
		cfg := reasoningParamMap[k]
		if cfg.SSEField != "" {
			cfg.SSEFieldParts = strings.Split(cfg.SSEField, ".")
			reasoningParamMap[k] = cfg
		}
	}
	sortedReasoningConfigs = make([]ReasoningConfig, len(sortedReasoningKeys))
	for i, k := range sortedReasoningKeys {
		sortedReasoningConfigs[i] = reasoningParamMap[k]
	}
}

func GetReasoningConfig(modelID string) (ReasoningConfig, bool) {
	// Use pre-computed sorted keys (H7 fix)
	for i, prefix := range sortedReasoningKeys {
		if strings.HasPrefix(modelID, prefix) {
			return sortedReasoningConfigs[i], true
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

	// Extract usage from the final SSE chunk if present.
	var usage *types.Usage
	if usageRaw, exists := raw["usage"]; exists {
		if usageMap, ok := usageRaw.(map[string]any); ok {
			u := &types.Usage{}
			if pt, ok := usageMap["prompt_tokens"].(float64); ok {
				u.PromptTokens = int(pt)
			}
			if ct, ok := usageMap["completion_tokens"].(float64); ok {
				u.CompletionTokens = int(ct)
			}
			if tt, ok := usageMap["total_tokens"].(float64); ok {
				u.TotalTokens = int(tt)
			}
			usage = u
		}
	}

	choices, ok := getNestedField(raw, "choices")
	if !ok {
		// Some providers send usage without choices in the final chunk.
		if usage != nil {
			return &types.StreamChunk{Type: "usage", Usage: usage}, nil
		}
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

	delta, deltaOk := firstChoice["delta"]
	if deltaOk {
		deltaMap, ok := delta.(map[string]any)
		if ok {
			if cfg.ModelFamily == "anthropic" {
				if contentType, exists := deltaMap["type"]; exists {
					if contentTypeStr, ok := contentType.(string); ok && contentTypeStr == "thinking" {
						content, _ := deltaMap["content"].(string)
						return &types.StreamChunk{Type: "thinking", Delta: content, Usage: usage}, nil
					}
				}
			}

			if cfg.SSEField != "" {
				// Use pre-split parts (H4 fix) instead of strings.Split per chunk
				parts := cfg.SSEFieldParts
				if len(parts) >= 4 {
					fieldName := parts[len(parts)-1]
					if val, exists := deltaMap[fieldName]; exists {
						if str, ok := val.(string); ok && str != "" {
							return &types.StreamChunk{Type: "thinking", Delta: str, Usage: usage}, nil
						}
					}
				}
			}

			if tcRaw, exists := deltaMap["tool_calls"]; exists {
				if tcArr, ok := tcRaw.([]any); ok && len(tcArr) > 0 {
					if tc, ok := tcArr[0].(map[string]any); ok {
						chunk := &types.StreamChunk{Type: "tool_call", Usage: usage}
						if idx, ok := tc["index"].(float64); ok {
							chunk.Index = int(idx)
						}
						if id, ok := tc["id"].(string); ok {
							chunk.ToolCallID = id
						}
						if fn, ok := tc["function"].(map[string]any); ok {
							if name, ok := fn["name"].(string); ok {
								chunk.ToolName = name
							}
							if args, ok := fn["arguments"].(string); ok {
								chunk.ToolInput = args
							}
						}
						return chunk, nil
					}
				}
			}

			content, _ := deltaMap["content"].(string)
			if content != "" {
				return &types.StreamChunk{Type: "content", Delta: content, Usage: usage}, nil
			}
		}
	}

	if finishReason, exists := firstChoice["finish_reason"]; exists && finishReason != nil {
		if fr, ok := finishReason.(string); ok && fr == "tool_calls" {
			return &types.StreamChunk{Type: "done", Usage: usage}, nil
		}
		return &types.StreamChunk{Type: "done", Usage: usage}, nil
	}

	return &types.StreamChunk{Type: "content", Delta: "", Usage: usage}, nil
}
