package zen

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log"
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
	apiKey     string
	baseURL    string
	httpClient *http.Client
	cache      *provider.ModelCache
}

func New(apiKey string) (*Client, error) {
	if apiKey == "" {
		return nil, m31errors.ErrInvalidKey
	}
	return &Client{
		apiKey:  apiKey,
		baseURL: "https://opencode.ai/zen/v1",
		httpClient: &http.Client{
			Transport: &http.Transport{
				DialContext: (&net.Dialer{Timeout: 30 * time.Second}).DialContext,
			},
		},
		cache: provider.NewModelCache(types.ModelCacheTTL),
	}, nil
}

func (c *Client) Name() string {
	return "zen"
}

func (c *Client) userAgent() string {
	return fmt.Sprintf("M31A/%s", Version)
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
		return c.cachedModels(), nil
	}

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.baseURL+"/models", nil)
	if err != nil {
		log.Printf("[zen] failed to create models request: %v; falling back to cache", err)
		return c.staleFallback()
	}
	c.setCommonHeaders(req)

	resp, err := c.httpClient.Do(req)
	if err != nil {
		log.Printf("[zen] failed to fetch models: %v; falling back to cache", err)
		return c.staleFallback()
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		log.Printf("[zen] unexpected status %d fetching models; falling back to cache", resp.StatusCode)
		return c.staleFallback()
	}

	var apiResp zenModelsResp
	if err := json.NewDecoder(resp.Body).Decode(&apiResp); err != nil {
		log.Printf("[zen] failed to decode models response: %v; falling back to cache", err)
		return c.staleFallback()
	}

	models := make([]types.ModelInfo, 0, len(apiResp.Data))
	for _, m := range apiResp.Data {
		info := types.ModelInfo{
			ID:            m.ID,
			Name:          m.ID,
			Description:   m.OwnedBy,
			ContextLength: 128_000,
			Pricing: types.Pricing{
				InputPerMToken:  0,
				OutputPerMToken: 0,
			},
			TopProvider: "zen",
			Capabilities: types.CapFlags{
				Tools:     true,
				Reasoning: false,
			},
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
	// NOTE: HTTP-Referer and X-Title headers are intentionally omitted here.
	// Unlike OpenRouter, the Zen gateway does not require or use these headers
	// for rate limiting, analytics, or request routing.

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
			// Check if it's a credits/billing issue vs actual invalid key
			if strings.Contains(bodyStr, "CreditsError") || strings.Contains(bodyStr, "payment") || strings.Contains(bodyStr, "billing") || strings.Contains(bodyStr, "credit") {
				return nil, fmt.Errorf("no credits: %s", bodyStr)
			}
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
	model, ok := c.cache.Get(modelID)
	if !ok {
		return 0
	}
	return (float64(usage.PromptTokens)/1_000_000)*model.Pricing.InputPerMToken +
		(float64(usage.CompletionTokens)/1_000_000)*model.Pricing.OutputPerMToken
}

func (c *Client) HealthCheck(ctx context.Context) types.HealthStatus {
	start := time.Now()

	req, err := http.NewRequestWithContext(ctx, http.MethodGet, c.baseURL+"/models", nil)
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
	case latency < 2000:
		return types.HealthStatus{Status: "live", LatencyMs: latency}
	case latency < 5000:
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
