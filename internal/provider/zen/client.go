package zen

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/provider"
	"github.com/eshanized/M31A/internal/types"
)

// Compile-time interface check
var _ provider.LLMProvider = (*Client)(nil)

type Client struct {
	provider.BaseClient
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
	Version           string
}

func New(apiKey string, opts Options) (*Client, error) {
	if apiKey == "" {
		return nil, m31errors.ErrInvalidKey
	}
	if opts.BaseURL == "" {
		opts.BaseURL = types.DefaultZenBaseURL
	}
	if opts.DefaultContextLen == 0 {
		opts.DefaultContextLen = types.DefaultContextLength
	}
	if opts.Version == "" {
		opts.Version = "dev"
	}

	return &Client{
		BaseClient:        provider.NewBaseClient(apiKey, opts.BaseURL, opts.Version, opts.CacheTTL, opts.CacheStaleTTL, opts.HealthCheckLiveMs, opts.HealthCheckSlowMs),
		defaultContextLen: opts.DefaultContextLen,
	}, nil
}

func (c *Client) Name() string {
	return "zen"
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
	if !c.Cache.IsExpired() && c.Cache.Len() > 0 {
		return provider.CachedModels(c.Cache), nil
	}

	models, err := c.Cache.Refresh(ctx, func(ctx context.Context) ([]types.ModelInfo, error) {
		req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.BaseURLField+"/models", nil)
		if err != nil {
			return nil, err
		}
		provider.SetCommonHeaders(req, c.APIKeyField, c.Version)
		resp, err := c.CatalogClient.Do(req)
		if err != nil {
			return nil, err
		}
		defer resp.Body.Close() //nolint:errcheck
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
		// Enrich models with context_length and pricing from OpenRouter or local database.
		// Zen API does not return pricing or per-model context_length.
		models = provider.EnrichModelInfo(models, "zen")
		return models, nil
	})
	if err != nil {
		slog.Warn("zen failed to refresh models", "error", err)
		return provider.StaleFallback(c.Cache)
	}
	return models, nil
}

func (c *Client) ChatCompletionStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
	body := provider.BuildChatBody(req)

	jsonBody, err := json.Marshal(body)
	if err != nil {
		return nil, fmt.Errorf("marshal request: %w", err)
	}

	httpReq, err := http.NewRequestWithContext(ctx, http.MethodPost, c.BaseURLField+"/chat/completions", bytes.NewReader(jsonBody))
	if err != nil {
		return nil, fmt.Errorf("create request: %w", err)
	}

	provider.SetCommonHeaders(httpReq, c.APIKeyField, c.Version)
	httpReq.Header.Set("Content-Type", "application/json")
	httpReq.Header.Set("Accept", "application/json")

	resp, err := c.HTTPClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("send request: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		bodyBytes, _ := provider.ReadBodyLimited(resp, types.MaxLLMResponseBytes)
		retryAfter := provider.GetRetryAfter(resp)
		_ = resp.Body.Close()
		bodyStr := string(bodyBytes)
		switch resp.StatusCode {
		case http.StatusTooManyRequests:
			if retryAfter != "" {
				return nil, fmt.Errorf("%w (retry-after: %s)", m31errors.ErrRateLimited, retryAfter)
			}
			return nil, m31errors.ErrRateLimited
		case http.StatusUnauthorized:
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
	return c.MakeIterator(sse, req.Model), nil
}

func (c *Client) HealthCheck(ctx context.Context) types.HealthStatus {
	start := time.Now()

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.BaseURLField+"/models", nil)
	if err != nil {
		return types.HealthStatus{Status: types.HealthStatusOffline, Error: err.Error()}
	}
	provider.SetCommonHeaders(req, c.APIKeyField, c.Version)

	resp, err := c.CatalogClient.Do(req)
	latency := time.Since(start).Milliseconds()
	if err != nil {
		return types.HealthStatus{Status: types.HealthStatusOffline, LatencyMs: latency, Error: err.Error()}
	}
	defer resp.Body.Close() //nolint:errcheck
	_, _ = provider.ReadBodyLimited(resp, types.MaxLLMResponseBytes)

	if resp.StatusCode != http.StatusOK {
		return types.HealthStatus{Status: types.HealthStatusOffline, LatencyMs: latency, Error: fmt.Sprintf("status %d", resp.StatusCode)}
	}

	switch {
	case latency < c.HealthLiveMs:
		return types.HealthStatus{Status: types.HealthStatusLive, LatencyMs: latency}
	case latency < c.HealthSlowMs:
		return types.HealthStatus{Status: types.HealthStatusSlow, LatencyMs: latency}
	default:
		return types.HealthStatus{Status: types.HealthStatusDegraded, LatencyMs: latency}
	}
}
