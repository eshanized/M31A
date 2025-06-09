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
	apiKey              string
	baseURL             string
	httpClient          *http.Client
	cache               *provider.ModelCache
	referer             string
	title               string
	healthCheckLiveMs   int64
	healthCheckSlowMs   int64
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

// openrouterModelCapabilities holds explicit capability flags for known OpenRouter models.
var openrouterModelCapabilities = map[string]types.CapFlags{
	"anthropic/claude-sonnet-4":         {Tools: true, Reasoning: false},
	"anthropic/claude-opus-4":           {Tools: true, Reasoning: false},
	"anthropic/claude-3.5-sonnet":       {Tools: true, Reasoning: false},
	"openai/gpt-4o":                     {Tools: true, Reasoning: false},
	"openai/o1":                         {Tools: true, Reasoning: true},
	"openai/o3":                         {Tools: true, Reasoning: true},
	"deepseek/deepseek-r1":              {Tools: false, Reasoning: true},
	"deepseek/deepseek-v3":              {Tools: true, Reasoning: false},
	"google/gemini-2.5-pro":             {Tools: true, Reasoning: false},
	"google/gemini-2.5-pro-exp":         {Tools: true, Reasoning: false},
	"meta-llama/llama-3.3-70b-instruct": {Tools: true, Reasoning: false},
}

func openrouterModelCaps(modelID string) (types.CapFlags, bool) {
	if caps, ok := openrouterModelCapabilities[modelID]; ok {
		return caps, true
	}
	return types.CapFlags{}, false
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
		opts.HealthCheckLiveMs = 2000
	}
	if opts.HealthCheckSlowMs == 0 {
		opts.HealthCheckSlowMs = 5000
	}

	cache := provider.NewModelCacheWithStale(opts.CacheTTL, opts.CacheStaleTTL)
	return &Client{
		apiKey:              apiKey,
		baseURL:             opts.BaseURL,
		httpClient: &http.Client{
			Transport: &http.Transport{
				DialContext: (&net.Dialer{Timeout: types.HTTPDialTimeout}).DialContext,
			},
		},
		cache:               cache,
		referer:             opts.Referer,
		title:               opts.Title,
		healthCheckLiveMs:   opts.HealthCheckLiveMs,
		healthCheckSlowMs:   opts.HealthCheckSlowMs,
	}, nil
}

func (c *Client) Name() string {
	return "openrouter"
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
	TopProvider string `json:"top_provider"`
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
				if caps, ok := openrouterModelCaps(m.ID); ok {
					return caps
				}
				// Fallback to string-sniff for unknown models
				return types.CapFlags{
					Tools:     strings.Contains(m.Architecture.Tokenizer, "tools") || strings.Contains(m.Architecture.Modality, "tool"),
					Reasoning: strings.Contains(m.Description, "reasoning") || strings.Contains(m.ID, "r1"),
				}
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
		bodyBytes, _ := io.ReadAll(resp.Body)
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
			if strings.Contains(bodyStr, "context_length") || strings.Contains(bodyStr, "context") {
				return nil, m31errors.ErrContextExceeded
			}
			return nil, fmt.Errorf("unexpected status %d: %s", resp.StatusCode, bodyStr)
		}
	}

	sse := provider.NewSSEParser(resp)
	return c.makeIterator(sse, req.Model), nil
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
