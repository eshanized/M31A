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

func New(apiKey string, opts Options) (*Client, error) {
	if apiKey == "" {
		return nil, m31errors.ErrInvalidKey
	}
	if opts.BaseURL == "" {
		opts.BaseURL = types.DefaultOpenRouterBaseURL
	}
	if opts.CacheTTL == 0 {
		opts.CacheTTL = types.ModelCacheTTL
	}
	if opts.CacheStaleTTL == 0 {
		opts.CacheStaleTTL = types.StaleCacheTTL
	}
	if opts.Referer == "" {
		opts.Referer = types.DefaultReferer
	}
	if opts.Title == "" {
		opts.Title = "M31A"
	}
	if opts.HealthCheckLiveMs == 0 {
		opts.HealthCheckLiveMs = types.DefaultHealthLiveMs
	}
	if opts.HealthCheckSlowMs == 0 {
		opts.HealthCheckSlowMs = types.DefaultHealthSlowMs
	}

	cache := provider.NewModelCacheWithStale(opts.CacheTTL, opts.CacheStaleTTL)
	return &Client{
		apiKey:  apiKey,
		baseURL: opts.BaseURL,
		httpClient: &http.Client{
			Transport: &http.Transport{
				DialContext: (&net.Dialer{Timeout: types.HTTPDialTimeout}).DialContext,
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
	if len(c.apiKey) <= 4 {
		return "****"
	}
	return "****" + c.apiKey[len(c.apiKey)-4:]
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
		return provider.CachedModels(c.cache), nil
	}

	models, err := c.cache.Refresh(ctx, func(ctx context.Context) ([]types.ModelInfo, error) {
		req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.baseURL+"/models", nil)
		if err != nil {
			return nil, err
		}
		provider.SetCommonHeaders(req, c.apiKey, Version)
		resp, err := c.httpClient.Do(req)
		if err != nil {
			return nil, err
		}
		defer resp.Body.Close()
		if resp.StatusCode != http.StatusOK {
			return nil, fmt.Errorf("models fetch returned status %d", resp.StatusCode)
		}
		var apiResp openRouterModelsResponse
		if err := json.NewDecoder(resp.Body).Decode(&apiResp); err != nil {
			return nil, err
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
				TopProvider:  m.TopProvider,
				Capabilities: provider.ParseModelCapabilities(m.ID),
			}
			models = append(models, info)
		}
		return models, nil
	})
	if err != nil {
		slog.Warn("openrouter failed to refresh models", "error", err)
		return provider.StaleFallback(c.cache)
	}
	return models, nil
}

func (c *Client) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
	const maxRetries = 2

	for attempt := 0; attempt <= maxRetries; attempt++ {
		iter, err := c.doChatStream(ctx, req)
		if err == nil {
			return iter, nil
		}

		// Retry on 5xx and connection errors
		if attempt < maxRetries && isRetryable(err) {
			delay := time.Duration(1<<uint(attempt)) * time.Second
			select {
			case <-ctx.Done():
				return nil, ctx.Err()
			case <-time.After(delay):
				continue
			}
		}
		return nil, err
	}
	return nil, fmt.Errorf("max retries exceeded")
}

// isRetryable returns true for errors that warrant automatic retry.
func isRetryable(err error) bool {
	if err == nil {
		return false
	}
	msg := err.Error()
	return strings.Contains(msg, "500") ||
		strings.Contains(msg, "502") ||
		strings.Contains(msg, "503") ||
		strings.Contains(msg, "connection reset") ||
		strings.Contains(msg, "unexpected EOF") ||
		strings.Contains(msg, "server error") ||
		strings.Contains(msg, "gateway error") ||
		strings.Contains(msg, "temporarily unavailable")
}

func (c *Client) doChatStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
	body := provider.BuildChatBody(req)

	jsonBody, err := json.Marshal(body)
	if err != nil {
		return nil, fmt.Errorf("marshal request: %w", err)
	}

	httpReq, err := http.NewRequestWithContext(ctx, http.MethodPost, c.baseURL+"/chat/completions", bytes.NewReader(jsonBody))
	if err != nil {
		return nil, fmt.Errorf("create request: %w", err)
	}

	provider.SetCommonHeaders(httpReq, c.apiKey, Version)
	httpReq.Header.Set("Content-Type", "application/json")
	httpReq.Header.Set("Accept", "application/json")
	httpReq.Header.Set("HTTP-Referer", c.referer)
	httpReq.Header.Set("X-Title", c.title)

	resp, err := c.httpClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("send request: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		bodyBytes, _ := io.ReadAll(io.LimitReader(resp.Body, types.MaxLLMResponseBytes))
		resp.Body.Close()
		bodyStr := string(bodyBytes)
		switch resp.StatusCode {
		case http.StatusTooManyRequests:
			return nil, m31errors.ErrRateLimited
		case http.StatusUnauthorized:
			return nil, m31errors.ErrInvalidKey
		case http.StatusPaymentRequired:
			return nil, fmt.Errorf("%w: insufficient credits on OpenRouter", m31errors.ErrNoCredits)
		case http.StatusServiceUnavailable:
			return nil, m31errors.ErrProviderUnreachable
		default:
			if provider.IsContextExceeded(resp.StatusCode, bodyStr) {
				return nil, m31errors.ErrContextExceeded
			}
			return nil, fmt.Errorf("%s", provider.SanitizeProviderError(resp.StatusCode, bodyStr, "openrouter"))
		}
	}

	sse := provider.NewSSEParserWithContext(resp, ctx)
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
	return provider.EstimateCost(modelID, usage, c.cache)
}

func (c *Client) HealthCheck(ctx context.Context) types.HealthStatus {
	start := time.Now()

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.baseURL+"/auth/key", nil)
	if err != nil {
		return types.HealthStatus{Status: types.HealthStatusOffline, Error: err.Error()}
	}
	provider.SetCommonHeaders(req, c.apiKey, Version)

	resp, err := c.httpClient.Do(req)
	latency := time.Since(start).Milliseconds()
	if err != nil {
		return types.HealthStatus{Status: types.HealthStatusOffline, LatencyMs: latency, Error: err.Error()}
	}
	defer resp.Body.Close()
	io.Copy(io.Discard, io.LimitReader(resp.Body, types.MaxLLMResponseBytes))

	if resp.StatusCode != http.StatusOK {
		return types.HealthStatus{Status: types.HealthStatusOffline, LatencyMs: latency, Error: fmt.Sprintf("status %d", resp.StatusCode)}
	}

	switch {
	case latency < c.healthCheckLiveMs:
		return types.HealthStatus{Status: types.HealthStatusLive, LatencyMs: latency}
	case latency < c.healthCheckSlowMs:
		return types.HealthStatus{Status: types.HealthStatusSlow, LatencyMs: latency}
	default:
		return types.HealthStatus{Status: types.HealthStatusDegraded, LatencyMs: latency}
	}
}

func (c *Client) GetModel(id string) (*types.ModelInfo, error) {
	return provider.GetModel(id, c.cache)
}

// CachedModels returns all models from the cache without a network call.
func (c *Client) CachedModels() []types.ModelInfo {
	return provider.CachedModels(c.cache)
}
