package nvidia

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
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/provider"
)

var _ provider.LLMProvider = (*Client)(nil)

type Client struct {
	provider.BaseClient
	defaultContextLen int64
}

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
		opts.BaseURL = types.DefaultNvidiaBaseURL
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
	return types.ProviderNvidia
}

type nvidiaModel struct {
	ID      string `json:"id"`
	Object  string `json:"object"`
	Created int64  `json:"created"`
	OwnedBy string `json:"owned_by"`
}

type nvidiaModelsResp struct {
	Object string        `json:"object"`
	Data   []nvidiaModel `json:"data"`
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
		var apiResp nvidiaModelsResp
		if err := json.NewDecoder(resp.Body).Decode(&apiResp); err != nil {
			return nil, err
		}
		models := make([]types.ModelInfo, 0, len(apiResp.Data))
		for _, m := range apiResp.Data {
			if provider.IsNonChatModel(m.ID) {
				continue
			}
			if provider.IsLikelyBrokenOnNvidia(m.ID) {
				continue
			}
			info := types.ModelInfo{
				ID:            m.ID,
				Name:          m.ID,
				Description:   m.OwnedBy,
				ContextLength: c.defaultContextLen,
				Pricing: types.Pricing{
					InputPerMToken:  0,
					OutputPerMToken: 0,
				},
				Provider:     types.ProviderNvidia,
				TopProvider:  types.ProviderNvidia,
				Capabilities: provider.ParseModelCapabilities(m.ID),
			}
			if !info.Capabilities.Chat {
				continue
			}
			models = append(models, info)
		}
		models = provider.EnrichModelInfo(models, types.ProviderNvidia)
		return models, nil
	})
	if err != nil {
		slog.Warn("nvidia failed to refresh models", "error", err)
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

		if attempt < maxRetries && provider.IsRetryable(err) {
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

// isMultimodalModel checks whether a model ID indicates multimodal capabilities
// (image, video, or audio input). These models return 400 on text-only requests.
func isMultimodalModel(modelID string) bool {
	id := strings.ToLower(modelID)
	return strings.Contains(id, "multimodal") || strings.Contains(id, "vision")
}

// hasImageContent reports whether any message in the request contains
// image content (base64 or URL). Used to decide whether a multimodal
// model needs force_text hints.
func hasImageContent(msgs []types.Message) bool {
	for _, m := range msgs {
		if strings.Contains(m.Content, "image_url") ||
			strings.Contains(m.Content, "data:image/") ||
			strings.Contains(m.Content, "[image") {
			return true
		}
	}
	return false
}

// buildNvidiaBody constructs a chat completion request body tailored for
// NVIDIA NIM. It starts from the standard OpenAI-compatible body and adds
// NVIDIA-specific fields:
//   - ExtraBodyParams from the reasoning config are nested under "extra_body"
//   - Multimodal models with text-only messages get force_text hints
func (c *Client) buildNvidiaBody(req provider.ChatRequest) map[string]any {
	body := provider.BuildChatBody(req)

	cfg, hasCfg := provider.GetReasoningConfig(req.Model)
	if hasCfg && len(cfg.ExtraBodyParams) > 0 {
		extraBody, _ := body["extra_body"].(map[string]any)
		if extraBody == nil {
			extraBody = make(map[string]any, len(cfg.ExtraBodyParams))
		}
		for k, v := range cfg.ExtraBodyParams {
			extraBody[k] = v
		}
		body["extra_body"] = extraBody
	}

	if isMultimodalModel(req.Model) && !hasImageContent(req.Messages) {
		extraBody, _ := body["extra_body"].(map[string]any)
		if extraBody == nil {
			extraBody = make(map[string]any)
		}
		kwargs, _ := extraBody["chat_template_kwargs"].(map[string]any)
		if kwargs == nil {
			kwargs = make(map[string]any)
		}
		kwargs["force_text"] = true
		extraBody["chat_template_kwargs"] = kwargs
		body["extra_body"] = extraBody
	}

	return body
}

func (c *Client) doChatStream(ctx context.Context, req provider.ChatRequest) (*types.StreamIterator, error) {
	body := c.buildNvidiaBody(req)

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
		// Handle NVIDIA-specific status codes first
		switch resp.StatusCode {
		case http.StatusNotFound:
			c.Cache.Remove(req.Model)
			slog.Info("nvidia: evicted unavailable model", "model", req.Model, "status", resp.StatusCode)
			return nil, fmt.Errorf("%w: model %q is unavailable or deprecated on NVIDIA NIM", m31errors.ErrModelNotFound, req.Model)
		case http.StatusBadRequest:
			bodyBytes, _ := provider.ReadBodyLimited(resp, types.MaxLLMResponseBytes)
			_ = resp.Body.Close()
			bodyStr := string(bodyBytes)
			if !provider.IsContextExceeded(resp.StatusCode, bodyStr) {
				c.Cache.Remove(req.Model)
				slog.Info("nvidia: evicted incompatible model", "model", req.Model, "body", bodyStr)
			}
			msg := provider.SanitizeProviderError(resp.StatusCode, bodyStr, types.ProviderNvidia)
			if isMultimodalModel(req.Model) {
				msg += " — this model requires image, video, or audio input"
			}
			return nil, &provider.HTTPStatusError{StatusCode: resp.StatusCode, Message: msg}
		}
		// Fall through to shared handler for 429, 401, 402, 503, and others
		return nil, c.HandleChatHTTPErrorWithCredits(resp, types.ProviderNvidia, nil)
	}

	sse := provider.NewSSEParserWithContext(resp, ctx)
	return c.MakeIterator(sse, req.Model), nil
}

func (c *Client) HealthCheck(ctx context.Context) types.HealthStatus {
	return c.BaseClient.HealthCheck(ctx, "/models")
}
