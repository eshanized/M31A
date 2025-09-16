package zen

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
	healthCheckLiveMs int64
	healthCheckSlowMs int64
	defaultContextLen int64
}

// Options holds configurable settings for the Zen client.
type Options struct {
	BaseURL           string
	CacheTTL          time.Duration
	CacheStaleTTL     time.Duration
	HealthCheckLiveMs int64
	HealthCheckSlowMs int64
	DefaultContextLen int64
}

func New(apiKey string, opts Options) (*Client, error) {
	if apiKey == "" {
		return nil, m31errors.ErrInvalidKey
	}
	if opts.BaseURL == "" {
		opts.BaseURL = types.DefaultZenBaseURL
	}
	if opts.CacheTTL == 0 {
		opts.CacheTTL = types.ModelCacheTTL
	}
	if opts.CacheStaleTTL == 0 {
		opts.CacheStaleTTL = types.StaleCacheTTL
	}
	if opts.HealthCheckLiveMs == 0 {
		opts.HealthCheckLiveMs = types.DefaultHealthLiveMs
	}
	if opts.HealthCheckSlowMs == 0 {
		opts.HealthCheckSlowMs = types.DefaultHealthSlowMs
	}
	if opts.DefaultContextLen == 0 {
		opts.DefaultContextLen = types.DefaultContextLength
	}

	cache := provider.NewModelCacheWithStale(opts.CacheTTL, opts.CacheStaleTTL)
	return &Client{
		apiKey:  apiKey,
		baseURL: opts.BaseURL,
		httpClient: &http.Client{
			Transport: &http.Transport{
				DialContext:           (&net.Dialer{Timeout: types.HTTPDialTimeout}).DialContext,
				ResponseHeaderTimeout: types.HTTPDialTimeout, // C-17 fix: was missing
			},
		},
		cache:             cache,
		healthCheckLiveMs: opts.HealthCheckLiveMs,
		healthCheckSlowMs: opts.HealthCheckSlowMs,
		defaultContextLen: opts.DefaultContextLen,
	}, nil
}

func (c *Client) Name() string {
	return "zen"
}

func (c *Client) APIKey() string {
	return c.apiKey
}

type zenModel struct {
	ID      string `json:"id"`
	Object  string `json:"object"`
	Created int64  `json:"created"`
	OwnedBy string `json:"owned_by"`
}

type zenModelsResp struct {
	Object string     `json:"object"`
	Data   []zenModel `json:"data"`
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
		var apiResp zenModelsResp
		if err := json.NewDecoder(resp.Body).Decode(&apiResp); err != nil {
			return nil, err
		}
		models := make([]types.ModelInfo, 0, len(apiResp.Data))
		for _, m := range apiResp.Data {
			info := types.ModelInfo{
				ID:            m.ID,
				Name:          m.ID,
				Description:   m.OwnedBy,
				ContextLength: c.defaultContextLen,
				Pricing: types.Pricing{
					InputPerMToken:  0,
					OutputPerMToken: 0,
				},
				TopProvider:  "zen",
				Capabilities: provider.ParseModelCapabilities(m.ID, "-r1"),
			}
			models = append(models, info)
		}
		return models, nil
	})
	if err != nil {
		slog.Warn("zen failed to refresh models", "error", err)
		return provider.StaleFallback(c.cache)
	}
	return models, nil
}

func (c *Client) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
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

	resp, err := c.httpClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("send request: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		bodyBytes, _ := io.ReadAll(io.LimitReader(resp.Body, types.MaxLLMResponseBytes)) // C-16 fix: was unbounded io.ReadAll
		resp.Body.Close()
		bodyStr := string(bodyBytes)
		switch resp.StatusCode {
		case http.StatusTooManyRequests:
			return nil, m31errors.ErrRateLimited
		case http.StatusUnauthorized:
			// Check if it's a credits/billing issue vs actual invalid key
			if strings.Contains(bodyStr, "CreditsError") || strings.Contains(bodyStr, "payment") || strings.Contains(bodyStr, "billing") || strings.Contains(bodyStr, "credit") {
				return nil, fmt.Errorf("no credits: %s: %w", provider.SanitizeProviderError(resp.StatusCode, bodyStr, "zen"), m31errors.ErrNoCredits)
			}
			return nil, m31errors.ErrInvalidKey
		case http.StatusServiceUnavailable:
			return nil, m31errors.ErrProviderUnreachable
		default:
			if provider.IsContextExceeded(resp.StatusCode, bodyStr) {
				return nil, m31errors.ErrContextExceeded
			}
			return nil, fmt.Errorf("%s", provider.SanitizeProviderError(resp.StatusCode, bodyStr, "zen"))
		}
	}

	sse := provider.NewSSEParserWithContext(resp, ctx)
	return c.makeIterator(sse, req.Model), nil
}

func (c *Client) makeIterator(sse *provider.SSEParser, modelID string) *types.StreamIterator {
	return &types.StreamIterator{
		Next: func() (*types.StreamChunk, error) {
			_, data, err := sse.Next()
			if err != nil {
				return nil, err
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

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.baseURL+"/models", nil)
	if err != nil {
		return types.HealthStatus{Status: types.HealthStatusOffline, Error: err.Error()}
	}
	provider.SetCommonHeaders(req, c.apiKey, Version)

	resp, err := c.httpClient.Do(req)
	latency := time.Since(start).Milliseconds()
	if err != nil {
		return types.HealthStatus{Status: types.HealthStatusOffline, LatencyMs: latency, Error: err.Error()}
	}
	io.Copy(io.Discard, io.LimitReader(resp.Body, types.MaxLLMResponseBytes))
	defer resp.Body.Close()

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
