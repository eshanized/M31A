package openrouter

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"strings"
	"time"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/provider"
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
	Profiles          *config.ModelProfileConfig         // model profiles for parameter merging (D-09/D-10/D-11)
	RetryConfig       provider.StreamRetryConfig         // streaming retry configuration (D-13/D-16)
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
		BaseClient: provider.NewBaseClient(apiKey, opts.BaseURL, opts.Version, opts.CacheTTL, opts.CacheStaleTTL, opts.HealthCheckLiveMs, opts.HealthCheckSlowMs, opts.Profiles, opts.RetryConfig),
		referer:    opts.Referer,
		title:      opts.Title,
	}, nil
}

func (c *Client) Name() string {
	return types.ProviderOpenRouter
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
		defer func() {
			if err := resp.Body.Close(); err != nil {
				slog.Debug("close response body", "error", err, "resource", "openrouter_models")
			}
		}()
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
				Provider:     types.ProviderOpenRouter,
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
	// Apply model profile merging (D-09/D-10/D-11)
	req = c.MergeProfile(req)

	// Use BaseClient's RetryStream with configured retry strategy (D-13/D-16)
	return c.RetryStream(ctx, req, c.doChatStream)
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
		return nil, c.HandleChatHTTPErrorWithCredits(resp, types.ProviderOpenRouter, nil)
	}

	sse := provider.NewSSEParserWithContext(resp, ctx)
	return c.MakeIterator(sse, req.Model), nil
}

func (c *Client) HealthCheck(ctx context.Context) types.HealthStatus {
	return c.BaseClient.HealthCheck(ctx, "/auth/key")
}

// ChatCompletion implements the non-streaming chat completion by collecting
// chunks from ChatCompletionStream internally (per D-04).
func (c *Client) ChatCompletion(ctx context.Context, req provider.ChatRequest) (*types.ChatResponse, error) {
	stream, err := c.ChatCompletionStream(ctx, req)
	if err != nil {
		return nil, err
	}
	defer func() {
		if closeErr := stream.Close(); closeErr != nil {
			slog.Debug("close stream iterator", "error", closeErr, "resource", "chat_completion")
		}
	}()

	var content strings.Builder
	var finalUsage *types.Usage
	var finishReason string

	for {
		chunk, chunkErr := stream.Next()
		if chunkErr != nil {
			return nil, chunkErr
		}
		if chunk == nil {
			break
		}
		if chunk.Type == "content" || chunk.Type == "thinking" {
			content.WriteString(chunk.Delta)
		}
		if chunk.Usage != nil {
			finalUsage = chunk.Usage
		}
		if chunk.Type == "done" {
			finishReason = "stop"
		}
	}

	if finishReason == "" {
		finishReason = "stop"
	}

	return &types.ChatResponse{
		Content:      content.String(),
		Usage:        finalUsage,
		Model:        req.Model,
		FinishReason: finishReason,
	}, nil
}

// ListModels returns the list of available models, delegating to FetchModels.
func (c *Client) ListModels(ctx context.Context) ([]types.ModelInfo, error) {
	return c.FetchModels(ctx)
}
