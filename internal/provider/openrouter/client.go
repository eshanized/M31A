package openrouter

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net"
	"net/http"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

var Version = "dev"

// Compile-time interface check
var _ provider.LLMProvider = (*Client)(nil)

type Client struct {
	apiKey            string
	baseURL           string
	httpClient        *http.Client
	cache             *provider.ModelCache
	referer           string
	title             string
	healthCheckLiveMs int64
	healthCheckSlowMs int64
}

// Options holds configurable settings for the OpenRouter client.
type Options struct {
	BaseURL           string
	CacheTTL          time.Duration
	CacheStaleTTL     time.Duration
	Referer           string
	Title             string
	HealthCheckLiveMs int64
	HealthCheckSlowMs int64
}

// parseModelCapabilities infers capability flags from the model ID using heuristics.
// TODO: Query provider API for actual capabilities when available (e.g. OpenRouter model metadata).
// Tools capability defaults to true for all models since most modern LLMs support function calling.
func parseModelCapabilities(modelID string) types.CapFlags {
	id := strings.ToLower(modelID)
	caps := types.CapFlags{
		Tools: true, // Default: assume tool support for all models
	}
	// Detect reasoning/thinking models by ID patterns
	if strings.Contains(id, "reason") || strings.Contains(id, "thinking") ||
		strings.Contains(id, "/o1") || strings.Contains(id, "/o3") || strings.Contains(id, "/o4") {
		caps.Reasoning = true
	}
	// Detect vision/multimodal models by ID patterns
	if strings.Contains(id, "vision") || strings.Contains(id, "multimodal") {
		caps.Vision = true
	}
	return caps
}

func New(apiKey string, opts Options) (*Client, error) {
	if apiKey == "" {
		return nil, m31errors.ErrInvalidKey
	}
	if opts.BaseURL == "" {
		opts.BaseURL = "https://openrouter.ai/api/v1"
	}
	if opts.CacheTTL == 0 {
		opts.CacheTTL = types.ModelCacheTTL
	}
	if opts.CacheStaleTTL == 0 {
		opts.CacheStaleTTL = 24 * time.Hour
	}
	if opts.Referer == "" {
		opts.Referer = "https://github.com/eshanized/M31A"
	}
	if opts.Title == "" {
		opts.Title = "M31A"
	}
	if opts.HealthCheckLiveMs == 0 {
		opts.HealthCheckLiveMs = 500 // Match config default (FeaturesConfig.HealthCheckLiveMs)
	}
	if opts.HealthCheckSlowMs == 0 {
		opts.HealthCheckSlowMs = 2000 // Match config default (FeaturesConfig.HealthCheckSlowMs)
	}

	cache := provider.NewModelCacheWithStale(opts.CacheTTL, opts.CacheStaleTTL)
	return &Client{
		apiKey:  apiKey,
		baseURL: opts.BaseURL,
		httpClient: &http.Client{
			Transport: &http.Transport{
				DialContext:           (&net.Dialer{Timeout: types.HTTPDialTimeout}).DialContext,
				ResponseHeaderTimeout: 30 * time.Second,
			},
		},
		cache:             cache,
		referer:           opts.Referer,
		title:             opts.Title,
		healthCheckLiveMs: opts.HealthCheckLiveMs,
		healthCheckSlowMs: opts.HealthCheckSlowMs,
	}, nil
}

func (c *Client) Name() string {
	return "openrouter"
}

func (c *Client) APIKey() string {
	return c.apiKey
}

func (c *Client) userAgent() string {
	return fmt.Sprintf("M31A/%s", Version)
}

type openRouterModel struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Description string `json:"description"`
	ContextLen  int64  `json:"context_length"`
	Pricing     struct {
		PromptToken     float64 `json:"prompt_token"`
		CompletionToken float64 `json:"completion_token"`
	} `json:"pricing"`
	TopProvider  string `json:"top_provider"`
	Architecture struct {
		Modality  string `json:"modality"`
		Tokenizer string `json:"tokenizer"`
	} `json:"architecture"`
}

type openRouterModelsResponse struct {
	Data []openRouterModel `json:"data"`
}

func (c *Client) FetchModels(ctx context.Context) ([]types.ModelInfo, error) {
	if !c.cache.IsExpired() && c.cache.Len() > 0 {
		return c.cachedModels(), nil
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.baseURL+"/models", nil)
	if err != nil {
		slog.Warn("openrouter failed to create models request", "error", err)
		return c.staleFallback()
	}
	c.setCommonHeaders(req)

	resp, err := c.httpClient.Do(req)
	if err != nil {
		slog.Warn("openrouter failed to fetch models", "error", err)
		return c.staleFallback()
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		slog.Warn("openrouter unexpected status fetching models", "status", resp.StatusCode)
		return c.staleFallback()
	}

	var apiResp openRouterModelsResponse
	if err := json.NewDecoder(resp.Body).Decode(&apiResp); err != nil {
		slog.Warn("openrouter failed to decode models response", "error", err)
		return c.staleFallback()
	}

	models := make([]types.ModelInfo, 0, len(apiResp.Data))
	for _, m := range apiResp.Data {
		info := types.ModelInfo{
			ID:            m.ID,
			Name:          m.Name,
			Description:   m.Description,
			ContextLength: m.ContextLen,
			Pricing: types.Pricing{
				InputPerMToken:  m.Pricing.PromptToken * 1_000_000,
				OutputPerMToken: m.Pricing.CompletionToken * 1_000_000,
			},
			TopProvider: m.TopProvider,
			Capabilities: func() types.CapFlags {
				// Infer capabilities from model ID heuristics
				return parseModelCapabilities(m.ID)
			}(),
		}
		models = append(models, info)
	}

	c.cache.Set(models)
	return models, nil
}

func (c *Client) staleFallback() ([]types.ModelInfo, error) {
	if !c.cache.IsStale() && c.cache.Len() > 0 {
		return c.cachedModels(), nil
	}
	return nil, m31errors.ErrProviderUnreachable
}

func (c *Client) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
	body := map[string]any{
		"model":    req.Model,
		"messages": req.Messages,
		"stream":   true,
	}
	if req.MaxTokens > 0 {
		body["max_tokens"] = req.MaxTokens
	}
	if len(req.Tools) > 0 {
		body["tools"] = req.Tools
	}
	if req.ReasoningEnabled {
		body = provider.ApplyReasoningParams(req.Model, body)
	}

	jsonBody, err := json.Marshal(body)
	if err != nil {
		return nil, fmt.Errorf("marshal request: %w", err)
	}

	httpReq, err := http.NewRequestWithContext(ctx, http.MethodPost, c.baseURL+"/chat/completions", bytes.NewReader(jsonBody))
	if err != nil {
		return nil, fmt.Errorf("create request: %w", err)
	}

	c.setCommonHeaders(httpReq)
	httpReq.Header.Set("Content-Type", "application/json")
	httpReq.Header.Set("Accept", "application/json")
	httpReq.Header.Set("HTTP-Referer", c.referer)
	httpReq.Header.Set("X-Title", c.title)

	resp, err := c.httpClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("send request: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		bodyBytes, _ := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
		resp.Body.Close()
		bodyStr := string(bodyBytes)
		switch resp.StatusCode {
		case http.StatusTooManyRequests:
			return nil, m31errors.ErrRateLimited
		case http.StatusUnauthorized:
			return nil, m31errors.ErrInvalidKey
		case http.StatusServiceUnavailable:
			return nil, m31errors.ErrProviderUnreachable
		default:
			if isContextExceeded(resp.StatusCode, bodyStr) {
				return nil, m31errors.ErrContextExceeded
			}
			return nil, fmt.Errorf("%s", sanitizeProviderError(resp.StatusCode, bodyStr))
		}
	}

	sse := provider.NewSSEParser(resp)
	return c.makeIterator(sse, req.Model), nil
}

// isContextExceeded checks if an HTTP error indicates context window overflow.
// Only matches HTTP 400 with specific context-related patterns to avoid false positives.
func isContextExceeded(statusCode int, body string) bool {
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

func (c *Client) makeIterator(sse *provider.SSEParser, modelID string) *types.StreamIterator {
	return &types.StreamIterator{
		Next: func() (*types.StreamChunk, error) {
			eventType, data, err := sse.Next()
			if err != nil {
				return nil, err
			}
			if eventType == "message" && data != "" {
				chunk, err := provider.ParseSSEChunk(data, modelID)
				if err != nil {
					return nil, err
				}
				return chunk, nil
			}
			if data == "" {
				return nil, nil
			}
			chunk, err := provider.ParseSSEChunk(data, modelID)
			if err != nil {
				return nil, err
			}
			return chunk, nil
		},
		Close: func() error {
			return sse.Close()
		},
	}
}

func (c *Client) EstimateCost(modelID string, usage types.Usage) float64 {
	model, ok := c.cache.Get(modelID)
	if !ok {
		return 0
	}
	return (float64(usage.PromptTokens)/1_000_000)*model.Pricing.InputPerMToken +
		(float64(usage.CompletionTokens)/1_000_000)*model.Pricing.OutputPerMToken
}

func (c *Client) HealthCheck(ctx context.Context) types.HealthStatus {
	start := time.Now()

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.baseURL+"/auth/key", nil)
	if err != nil {
		return types.HealthStatus{Status: "offline", Error: err.Error()}
	}
	c.setCommonHeaders(req)

	resp, err := c.httpClient.Do(req)
	latency := time.Since(start).Milliseconds()
	if err != nil {
		return types.HealthStatus{Status: "offline", LatencyMs: latency, Error: err.Error()}
	}
	io.Copy(io.Discard, io.LimitReader(resp.Body, 1<<20))
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return types.HealthStatus{Status: "offline", LatencyMs: latency, Error: fmt.Sprintf("status %d", resp.StatusCode)}
	}

	switch {
	case latency < c.healthCheckLiveMs:
		return types.HealthStatus{Status: "live", LatencyMs: latency}
	case latency < c.healthCheckSlowMs:
		return types.HealthStatus{Status: "slow", LatencyMs: latency}
	default:
		return types.HealthStatus{Status: "degraded", LatencyMs: latency}
	}
}

func (c *Client) GetModel(id string) (*types.ModelInfo, error) {
	m, ok := c.cache.Get(id)
	if !ok {
		return nil, m31errors.ErrModelNotFound
	}
	return m, nil
}

func (c *Client) cachedModels() []types.ModelInfo {
	all := c.cache.Models()
	models := make([]types.ModelInfo, 0, len(all))
	for _, m := range all {
		models = append(models, *m)
	}
	return models
}

func (c *Client) setCommonHeaders(req *http.Request) {
	req.Header.Set("Authorization", "Bearer "+c.apiKey)
	req.Header.Set("User-Agent", c.userAgent())
}

// sanitizeProviderError maps HTTP status codes to friendly messages and
// truncates/strips the response body to prevent raw HTML/JSON leaking to users.
func sanitizeProviderError(statusCode int, body string) string {
	// Single-pass HTML stripping
	var b strings.Builder
	inTag := false
	for _, ch := range body {
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
	cleaned := b.String()

	// Truncate to 200 chars
	if len(cleaned) > 200 {
		cleaned = cleaned[:200] + "…"
	}

	switch statusCode {
	case http.StatusBadRequest:
		msg := "Bad request — invalid parameters"
		if cleaned != "" {
			msg += ": " + cleaned
		}
		return msg
	case http.StatusUnauthorized:
		return "Invalid API key"
	case http.StatusPaymentRequired:
		return "Payment required — check your billing"
	case http.StatusTooManyRequests:
		return "Rate limited — retry in a moment"
	case http.StatusInternalServerError:
		return "Provider server error — try again later"
	case http.StatusBadGateway:
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
