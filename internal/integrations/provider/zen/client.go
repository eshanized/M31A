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

	m31errors "github.com/eshanized/M31A/internal/core/errors"
	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/provider"
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
	Profiles          *config.ModelProfileConfig         // model profiles for parameter merging (D-09/D-10/D-11)
	RetryConfig       provider.StreamRetryConfig         // streaming retry configuration (D-13/D-16)
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
		BaseClient:        provider.NewBaseClient(apiKey, opts.BaseURL, opts.Version, opts.CacheTTL, opts.CacheStaleTTL, opts.HealthCheckLiveMs, opts.HealthCheckSlowMs, opts.Profiles, opts.RetryConfig),
		defaultContextLen: opts.DefaultContextLen,
	}, nil
}

func (c *Client) Name() string {
	return types.ProviderZen
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
		defer func() {
			if err := resp.Body.Close(); err != nil {
				slog.Debug("close response body", "error", err, "resource", "zen_models")
			}
		}()
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
				Provider:     types.ProviderZen,
				TopProvider:  types.ProviderZen,
				Capabilities: provider.ParseModelCapabilities(m.ID, "-r1"),
			}
			models = append(models, info)
		}
		// Enrich models with context_length and pricing from OpenRouter or local database.
		// Zen API does not return pricing or per-model context_length.
		models = provider.EnrichModelInfo(models, types.ProviderZen)
		return models, nil
	})
	if err != nil {
		slog.Warn("zen failed to refresh models", "error", err)
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

	resp, err := c.HTTPClient.Do(httpReq)
	if err != nil {
		return nil, fmt.Errorf("send request: %w", err)
	}

	if resp.StatusCode != http.StatusOK {
		// Zen has special credit detection on 401
		if resp.StatusCode == http.StatusUnauthorized {
			bodyBytes, _ := provider.ReadBodyLimited(resp, types.MaxLLMResponseBytes)
			_ = resp.Body.Close()
			bodyStr := string(bodyBytes)
			if strings.Contains(bodyStr, "CreditsError") || strings.Contains(bodyStr, "payment") || strings.Contains(bodyStr, "billing") || strings.Contains(bodyStr, "credit") {
				return nil, fmt.Errorf("no credits: %s: %w", provider.SanitizeProviderError(resp.StatusCode, bodyStr, types.ProviderZen), m31errors.ErrNoCredits)
			}
			return nil, m31errors.ErrInvalidKey
		}
		return nil, c.HandleChatHTTPErrorWithCredits(resp, types.ProviderZen, nil)
	}

	sse := provider.NewSSEParserWithContext(resp, ctx)
	return c.MakeIterator(sse, req.Model), nil
}

func (c *Client) HealthCheck(ctx context.Context) types.HealthStatus {
	return c.BaseClient.HealthCheck(ctx, "/models")
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
