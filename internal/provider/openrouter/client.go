package openrouter

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
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
	referer string
	title   string
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
	Version           string
}

func New(apiKey string, opts Options) (*Client, error) {
	if apiKey == "" {
		return nil, m31errors.ErrInvalidKey
	}
	if opts.BaseURL == "" {
		opts.BaseURL = types.DefaultOpenRouterBaseURL
	}
	if opts.Referer == "" {
		opts.Referer = types.DefaultReferer
	}
	if opts.Title == "" {
		opts.Title = "M31A"
	}
	if opts.Version == "" {
		opts.Version = "dev"
	}

	return &Client{
		BaseClient: provider.NewBaseClient(apiKey, opts.BaseURL, opts.Version, opts.CacheTTL, opts.CacheStaleTTL, opts.HealthCheckLiveMs, opts.HealthCheckSlowMs),
		referer:    opts.Referer,
		title:      opts.Title,
	}, nil
}

func (c *Client) Name() string {
	return "openrouter"
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
		return provider.StaleFallback(c.Cache)
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

func isRetryable(err error) bool {
	if err == nil {
		return false
	}
	var httpErr *provider.HTTPStatusError
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

func (c *Client) doChatStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
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
	httpReq.Header.Set("HTTP-Referer", c.referer)
	httpReq.Header.Set("X-Title", c.title)

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
			return nil, m31errors.ErrInvalidKey
		case http.StatusPaymentRequired:
			return nil, fmt.Errorf("%w: insufficient credits on OpenRouter", m31errors.ErrNoCredits)
		case http.StatusServiceUnavailable:
			return nil, m31errors.ErrProviderUnreachable
		default:
			if provider.IsContextExceeded(resp.StatusCode, bodyStr) {
				return nil, m31errors.ErrContextExceeded
			}
			msg := provider.SanitizeProviderError(resp.StatusCode, bodyStr, "openrouter")
			return nil, &provider.HTTPStatusError{StatusCode: resp.StatusCode, Message: msg}
		}
	}

	sse := provider.NewSSEParserWithContext(resp, ctx)
	return c.MakeIterator(sse, req.Model), nil
}

func (c *Client) HealthCheck(ctx context.Context) types.HealthStatus {
	start := time.Now()

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.BaseURLField+"/auth/key", nil)
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
