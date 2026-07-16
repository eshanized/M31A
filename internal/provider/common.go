package provider

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"regexp"
	"strings"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/pkg/types"
)

// HTTPStatusError carries the HTTP status code from a provider response,
// enabling typed retry classification via errors.As instead of fragile
// string matching on error messages.
type HTTPStatusError struct {
	StatusCode int
	Message    string
}

func (e *HTTPStatusError) Error() string {
	return e.Message
}

// IsRetryable reports whether the HTTP status code indicates a transient
// server error that should be retried (500, 502, 503).
func (e *HTTPStatusError) IsRetryable() bool {
	switch e.StatusCode {
	case http.StatusInternalServerError,
		http.StatusBadGateway,
		http.StatusServiceUnavailable:
		return true
	}
	return false
}

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
// Matches HTTP 400 and 413 with context-related patterns to avoid false positives.
// Uses containsFold for case-insensitive matching without allocating a lowered copy.
func IsContextExceeded(statusCode int, body string) bool {
	if statusCode != http.StatusBadRequest && statusCode != http.StatusRequestEntityTooLarge {
		return false
	}
	return containsFold(body, "context_length_exceeded") ||
		containsFold(body, "maximum context length") ||
		containsFold(body, "request too large") ||
		containsFold(body, "context window exceeded") ||
		containsFold(body, "context length") ||
		containsFold(body, "too many tokens") ||
		(containsFold(body, "input") && containsFold(body, "exceeds")) ||
		containsFold(body, "token limit") ||
		containsFold(body, "max tokens") ||
		containsFold(body, "token_count") ||
		(containsFold(body, "context_length") && containsFold(body, "exceed")) ||
		(containsFold(body, "context") && containsFold(body, "limit"))
}

// containsFold reports whether s contains substr using case-insensitive comparison.
// Avoids allocating a lowered copy of s by scanning character-by-character.
func containsFold(s, substr string) bool {
	if len(substr) > len(s) {
		return false
	}
	if len(substr) == 0 {
		return true
	}
	for i := 0; i <= len(s)-len(substr); i++ {
		match := true
		for j := 0; j < len(substr); j++ {
			if toLowerByte(s[i+j]) != toLowerByte(substr[j]) {
				match = false
				break
			}
		}
		if match {
			return true
		}
	}
	return false
}

func toLowerByte(c byte) byte {
	if c >= 'A' && c <= 'Z' {
		return c + ('a' - 'A')
	}
	return c
}

// messagesToWire projects storage-shaped messages into the OpenAI-compatible
// wire format expected by chat completion APIs. Storage-only fields (Segments,
// CreatedAt, Usage, SkipForLLM) are dropped; assistant tool_calls are reshaped
// to {id, type:"function", function:{name, arguments}}; tool-role messages
// retain only role/tool_call_id/content.
func messagesToWire(msgs []types.Message) []map[string]any {
	out := make([]map[string]any, 0, len(msgs))
	for _, m := range msgs {
		if m.SkipForLLM {
			continue
		}
		w := map[string]any{
			"role":    m.Role,
			"content": m.Content,
		}
		switch m.Role {
		case "assistant":
			if len(m.ToolCalls) > 0 {
				tcs := make([]map[string]any, 0, len(m.ToolCalls))
				for _, tc := range m.ToolCalls {
					tcs = append(tcs, toolCallToWire(tc))
				}
				w["tool_calls"] = tcs
			}
		case "tool":
			if m.ToolCallID != "" {
				w["tool_call_id"] = m.ToolCallID
			}
		}
		out = append(out, w)
	}
	return out
}

// toolCallToWire converts a storage ToolCall to the OpenAI wire shape.
// Input is a json.RawMessage; it must be serialized as a JSON string in the
// "arguments" field (not as a nested object).
func toolCallToWire(tc types.ToolCall) map[string]any {
	args := ""
	if len(tc.Input) > 0 {
		args = string(tc.Input)
	}
	id := tc.ID
	if id == "" {
		id = tc.Name
	}
	return map[string]any{
		"id":   id,
		"type": "function",
		"function": map[string]any{
			"name":      tc.Name,
			"arguments": args,
		},
	}
}

// BuildChatBody constructs the standard chat completion request body.
// Tool definitions are sent as native provider tools in OpenAI function-calling
// format when req.Tools is populated. The engine uses native-first tool dispatch
// with text-based JSON extraction as a fallback for models that do not support
// native function calling.
func BuildChatBody(req ChatRequest) map[string]any {
	body := map[string]any{
		"model":    req.Model,
		"messages": messagesToWire(req.Messages),
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
			if td.ParametersParsed != nil {
				fn["parameters"] = td.ParametersParsed
			} else if td.Parameters != "" {
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

// IsRetryable reports whether an error is a transient server error that should be retried.
// It checks for HTTP status-based errors first, then falls back to string matching
// for network-level errors (connection resets, unexpected EOF, etc.).
func IsRetryable(err error) bool {
	if err == nil {
		return false
	}
	var httpErr *HTTPStatusError
	if errors.As(err, &httpErr) {
		return httpErr.IsRetryable()
	}
	// Fallback: string matching for network-level errors that don't
	// carry an HTTP status code (connection resets, unexpected EOF, etc.)
	msg := err.Error()
	return strings.Contains(msg, "connection reset") ||
		strings.Contains(msg, "unexpected EOF") ||
		strings.Contains(msg, "server error") ||
		strings.Contains(msg, "gateway error") ||
		strings.Contains(msg, "temporarily unavailable")
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
		if providerName == types.ProviderZen && cleaned != "" {
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
		if providerName == types.ProviderZen {
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
