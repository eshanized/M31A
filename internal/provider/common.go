package provider

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"regexp"
	"strings"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// UserAgent returns the standard M31A User-Agent string.
func UserAgent(version string) string {
	return fmt.Sprintf("M31A/%s", version)
}

// SetCommonHeaders sets Authorization and User-Agent headers on an HTTP request.
func SetCommonHeaders(req *http.Request, apiKey string, version string) {
	req.Header.Set("Authorization", "Bearer "+apiKey)
	req.Header.Set("User-Agent", UserAgent(version))
}

// IsContextExceeded checks if an HTTP error indicates context window overflow.
// Only matches HTTP 400 with specific context-related patterns to avoid false positives.
func IsContextExceeded(statusCode int, body string) bool {
	if statusCode != http.StatusBadRequest {
		return false
	}
	lower := strings.ToLower(body)
	return strings.Contains(lower, "context_length_exceeded") ||
		strings.Contains(lower, "maximum context length") ||
		strings.Contains(lower, "request too large") ||
		strings.Contains(lower, "context window exceeded") ||
		strings.Contains(lower, "context_length") && strings.Contains(lower, "exceed")
}

// BuildChatBody constructs the standard chat completion request body.
// Tool definitions are sent as native provider tools in OpenAI function-calling
// format when req.Tools is populated. The engine uses native-first tool dispatch
// with text-based JSON extraction as a fallback for models that do not support
// native function calling.
func BuildChatBody(req ChatRequest) map[string]any {
	body := map[string]any{
		"model":    req.Model,
		"messages": req.Messages,
		"stream":   true,
	}
	if req.MaxTokens > 0 {
		body["max_tokens"] = req.MaxTokens
	}
	if req.ReasoningEnabled {
		body = ApplyReasoningParams(req.Model, body)
	}
	if len(req.Tools) > 0 {
		tools := make([]map[string]any, 0, len(req.Tools))
		for _, td := range req.Tools {
			fn := map[string]any{
				"name":        td.Name,
				"description": td.Description,
			}
			if td.Parameters != "" {
				var params any
				if err := json.Unmarshal([]byte(td.Parameters), &params); err == nil {
					fn["parameters"] = params
				} else {
					fn["parameters"] = map[string]any{}
				}
			} else {
				fn["parameters"] = map[string]any{}
			}
			tools = append(tools, map[string]any{
				"type":     "function",
				"function": fn,
			})
		}
		body["tools"] = tools
	}
	return body
}

// EstimateCost calculates the cost of a usage sample against the model cache.
func EstimateCost(modelID string, usage types.Usage, cache *ModelCache) float64 {
	model, ok := cache.Get(modelID)
	if !ok {
		return 0
	}
	return (float64(usage.PromptTokens)/1_000_000)*model.Pricing.InputPerMToken +
		(float64(usage.CompletionTokens)/1_000_000)*model.Pricing.OutputPerMToken
}

// GetModel retrieves a model from the cache by ID.
func GetModel(id string, cache *ModelCache) (*types.ModelInfo, error) {
	m, ok := cache.Get(id)
	if !ok {
		return nil, m31errors.ErrModelNotFound
	}
	return m, nil
}

// CachedModels returns all models from the cache as a slice.
func CachedModels(cache *ModelCache) []types.ModelInfo {
	all := cache.Models()
	models := make([]types.ModelInfo, 0, len(all))
	for _, m := range all {
		models = append(models, *m)
	}
	return models
}

// ReadBodyLimited reads an HTTP response body up to maxBytes, preventing OOM
// from unbounded responses. Returns the body bytes and any error.
func ReadBodyLimited(resp *http.Response, maxBytes int64) ([]byte, error) {
	data, err := io.ReadAll(io.LimitReader(resp.Body, maxBytes))
	if err != nil {
		return nil, err
	}
	return data, nil
}

// StaleFallback returns cached models if the cache is not yet stale,
// otherwise returns ErrProviderUnreachable.
func StaleFallback(cache *ModelCache) ([]types.ModelInfo, error) {
	if !cache.IsStale() && cache.Len() > 0 {
		return CachedModels(cache), nil
	}
	return nil, m31errors.ErrProviderUnreachable
}

// stripHTMLTags removes HTML tags from a string using a single-pass scanner.
func stripHTMLTags(s string) string {
	var b strings.Builder
	inTag := false
	for _, ch := range s {
		if ch == '<' {
			inTag = true
			continue
		}
		if ch == '>' {
			inTag = false
			continue
		}
		if !inTag {
			b.WriteRune(ch)
		}
	}
	return b.String()
}

var apiKeyPattern = regexp.MustCompile(`(?i)(sk-[a-zA-Z0-9]{8,}|key-[a-zA-Z0-9]{8,}|api[_-]?key[_\s:=]+["']?)([a-zA-Z0-9]{4,})`)

// maskAPIKeys redacts potential API keys from error messages to prevent leakage.
func maskAPIKeys(s string) string {
	return apiKeyPattern.ReplaceAllStringFunc(s, func(match string) string {
		if len(match) > 8 {
			return match[:4] + "****" + match[len(match)-4:]
		}
		return "****"
	})
}

// SanitizeProviderError maps HTTP status codes to friendly messages and
// truncates/strips the response body to prevent raw HTML/JSON leaking to users.
// providerName controls minor behavioral differences: Zen appends body text for
// 401/502 errors while OpenRouter does not.
func SanitizeProviderError(statusCode int, body string, providerName string) string {
	cleaned := stripHTMLTags(body)
	cleaned = maskAPIKeys(cleaned)

	// Truncate to MaxProviderErrorChars
	if len(cleaned) > types.MaxProviderErrorChars {
		cleaned = cleaned[:types.MaxProviderErrorChars] + "…"
	}

	switch statusCode {
	case http.StatusBadRequest:
		msg := "Bad request — invalid parameters"
		if cleaned != "" {
			msg += ": " + cleaned
		}
		return msg
	case http.StatusUnauthorized:
		msg := "Invalid API key"
		if providerName == "zen" && cleaned != "" {
			msg += ": " + cleaned
		}
		return msg
	case http.StatusPaymentRequired:
		return "Payment required — check your billing"
	case http.StatusTooManyRequests:
		return "Rate limited — retry in a moment"
	case http.StatusInternalServerError:
		return "Provider server error — try again later"
	case http.StatusBadGateway:
		if providerName == "zen" {
			msg := fmt.Sprintf("Provider gateway error (HTTP %d) — try again later", statusCode)
			if cleaned != "" {
				msg += ": " + cleaned
			}
			return msg
		}
		return "Provider gateway error — try again later"
	case http.StatusServiceUnavailable:
		return "Provider temporarily unavailable"
	default:
		msg := fmt.Sprintf("Unexpected error (HTTP %d)", statusCode)
		if cleaned != "" {
			msg += ": " + cleaned
		}
		return msg
	}
}
