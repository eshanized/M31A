package provider

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"net/http"
	"strings"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

// ModelMetadata holds enrichment data for a model from an external source.
type ModelMetadata struct {
	ContextLength int64
	Pricing       types.Pricing
	Source        string // "openrouter", "local"
}

var (
	openRouterMetadataCache   map[string]ModelMetadata
	openRouterMetadataOnce    sync.Once
	openRouterMetadataFetched time.Time
	openRouterMetadataMu      sync.RWMutex

	// openRouterMetadataTTL controls how long the OpenRouter metadata cache lives.
	// Context windows and pricing change infrequently, so 1 hour is safe.
	openRouterMetadataTTL = 1 * time.Hour
)

// openRouterModelResponse is the wire format for OpenRouter's /models endpoint.
type openRouterModelResponse struct {
	Data []openRouterModelEntry `json:"data"`
}

type openRouterModelEntry struct {
	ID            string `json:"id"`
	Name          string `json:"name"`
	ContextLength int64  `json:"context_length"`
	Pricing       struct {
		Prompt     string `json:"prompt"`
		Completion string `json:"completion"`
	} `json:"pricing"`
}

// FetchOpenRouterMetadata fetches model metadata from OpenRouter's public API.
// Results are cached for openRouterMetadataTTL. Uses a dedicated HTTP client
// with a short timeout to avoid blocking provider operations.
func FetchOpenRouterMetadata(ctx context.Context) map[string]ModelMetadata {
	openRouterMetadataMu.RLock()
	if openRouterMetadataCache != nil && time.Since(openRouterMetadataFetched) < openRouterMetadataTTL {
		defer openRouterMetadataMu.RUnlock()
		return openRouterMetadataCache
	}
	openRouterMetadataMu.RUnlock()

	openRouterMetadataOnce.Do(func() {
		metadata, err := fetchOpenRouterMetadataUncached(ctx)
		if err != nil {
			slog.Warn("model_metadata: failed to fetch OpenRouter metadata", "error", err)
			return
		}
		openRouterMetadataMu.Lock()
		openRouterMetadataCache = metadata
		openRouterMetadataFetched = time.Now()
		openRouterMetadataMu.Unlock()
	})

	openRouterMetadataMu.RLock()
	defer openRouterMetadataMu.RUnlock()
	return openRouterMetadataCache
}

func fetchOpenRouterMetadataUncached(ctx context.Context) (map[string]ModelMetadata, error) {
	client := &http.Client{Timeout: types.FetchModelsTimeout}
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, types.DefaultOpenRouterBaseURL+"/models", nil)
	if err != nil {
		return nil, fmt.Errorf("create request: %w", err)
	}
	req.Header.Set("User-Agent", "M31A/metadata")

	resp, err := client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("fetch models: %w", err)
	}
	defer resp.Body.Close() //nolint:errcheck

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("status %d", resp.StatusCode)
	}

	var apiResp openRouterModelResponse
	if err := json.NewDecoder(resp.Body).Decode(&apiResp); err != nil {
		return nil, fmt.Errorf("decode: %w", err)
	}

	metadata := make(map[string]ModelMetadata, len(apiResp.Data))
	for _, m := range apiResp.Data {
		promptPrice := parseFloatSafe(m.Pricing.Prompt)
		compPrice := parseFloatSafe(m.Pricing.Completion)

		metadata[m.ID] = ModelMetadata{
			ContextLength: m.ContextLength,
			Pricing: types.Pricing{
				InputPerMToken:  promptPrice * 1_000_000,
				OutputPerMToken: compPrice * 1_000_000,
			},
			Source: "openrouter",
		}
	}

	slog.Info("model_metadata: fetched OpenRouter metadata", "count", len(metadata))
	return metadata, nil
}

// parseFloatSafe parses a string to float64, returning 0 on failure.
// OpenRouter pricing values can be strings like "0.000005" or "-1" (meaning unknown).
func parseFloatSafe(s string) float64 {
	if s == "" || s == "-1" {
		return 0
	}
	var v float64
	_, err := fmt.Sscanf(s, "%f", &v)
	if err != nil {
		return 0
	}
	return v
}

// LocalMetadataFallback returns a hardcoded database of known model metadata.
// Used as a fallback when OpenRouter API is unreachable. Context windows and
// pricing are derived from public provider documentation.
func LocalMetadataFallback() map[string]ModelMetadata {
	return map[string]ModelMetadata{
		// Anthropic Claude models
		"claude-fable-5":      {ContextLength: 1_000_000, Pricing: types.Pricing{InputPerMToken: 10, OutputPerMToken: 50}, Source: "local"},
		"claude-opus-4-8":     {ContextLength: 1_000_000, Pricing: types.Pricing{InputPerMToken: 5, OutputPerMToken: 25}, Source: "local"},
		"claude-opus-4-7":     {ContextLength: 1_000_000, Pricing: types.Pricing{InputPerMToken: 5, OutputPerMToken: 25}, Source: "local"},
		"claude-opus-4-6":     {ContextLength: 1_000_000, Pricing: types.Pricing{InputPerMToken: 5, OutputPerMToken: 25}, Source: "local"},
		"claude-opus-4-5":     {ContextLength: 200_000, Pricing: types.Pricing{InputPerMToken: 5, OutputPerMToken: 25}, Source: "local"},
		"claude-opus-4-1":     {ContextLength: 200_000, Pricing: types.Pricing{InputPerMToken: 5, OutputPerMToken: 25}, Source: "local"},
		"claude-sonnet-4-6":   {ContextLength: 1_000_000, Pricing: types.Pricing{InputPerMToken: 3, OutputPerMToken: 15}, Source: "local"},
		"claude-sonnet-4-5":   {ContextLength: 200_000, Pricing: types.Pricing{InputPerMToken: 3, OutputPerMToken: 15}, Source: "local"},
		"claude-sonnet-4":     {ContextLength: 200_000, Pricing: types.Pricing{InputPerMToken: 3, OutputPerMToken: 15}, Source: "local"},
		"claude-haiku-4-5":    {ContextLength: 200_000, Pricing: types.Pricing{InputPerMToken: 1, OutputPerMToken: 5}, Source: "local"},
		"claude-3-5-sonnet":   {ContextLength: 200_000, Pricing: types.Pricing{InputPerMToken: 3, OutputPerMToken: 15}, Source: "local"},
		"claude-3-5-haiku":    {ContextLength: 200_000, Pricing: types.Pricing{InputPerMToken: 0.8, OutputPerMToken: 4}, Source: "local"},
		"claude-3-opus":       {ContextLength: 200_000, Pricing: types.Pricing{InputPerMToken: 15, OutputPerMToken: 75}, Source: "local"},

		// Google Gemini models
		"gemini-3.5-flash":  {ContextLength: 1_048_576, Pricing: types.Pricing{InputPerMToken: 1.5, OutputPerMToken: 9}, Source: "local"},
		"gemini-3.1-pro":    {ContextLength: 1_048_576, Pricing: types.Pricing{InputPerMToken: 2, OutputPerMToken: 12}, Source: "local"},
		"gemini-3-flash":    {ContextLength: 1_048_576, Pricing: types.Pricing{InputPerMToken: 0.5, OutputPerMToken: 3}, Source: "local"},
		"gemini-2.5-pro":    {ContextLength: 1_048_576, Pricing: types.Pricing{InputPerMToken: 2.5, OutputPerMToken: 15}, Source: "local"},
		"gemini-2.5-flash":  {ContextLength: 1_048_576, Pricing: types.Pricing{InputPerMToken: 0.5, OutputPerMToken: 3}, Source: "local"},

		// OpenAI GPT models
		"gpt-5.5":      {ContextLength: 1_050_000, Pricing: types.Pricing{InputPerMToken: 5, OutputPerMToken: 30}, Source: "local"},
		"gpt-5.5-pro":  {ContextLength: 1_050_000, Pricing: types.Pricing{InputPerMToken: 30, OutputPerMToken: 180}, Source: "local"},
		"gpt-5.4":      {ContextLength: 1_050_000, Pricing: types.Pricing{InputPerMToken: 2.5, OutputPerMToken: 15}, Source: "local"},
		"gpt-5.4-pro":  {ContextLength: 1_050_000, Pricing: types.Pricing{InputPerMToken: 30, OutputPerMToken: 180}, Source: "local"},
		"gpt-5.4-mini": {ContextLength: 400_000, Pricing: types.Pricing{InputPerMToken: 0.75, OutputPerMToken: 4.5}, Source: "local"},
		"gpt-4o":       {ContextLength: 128_000, Pricing: types.Pricing{InputPerMToken: 2.5, OutputPerMToken: 10}, Source: "local"},
		"gpt-4o-mini":  {ContextLength: 128_000, Pricing: types.Pricing{InputPerMToken: 0.15, OutputPerMToken: 0.6}, Source: "local"},

		// DeepSeek models
		"deepseek-r1":  {ContextLength: 128_000, Pricing: types.Pricing{InputPerMToken: 0.55, OutputPerMToken: 2.19}, Source: "local"},
		"deepseek-v3":  {ContextLength: 128_000, Pricing: types.Pricing{InputPerMToken: 0.27, OutputPerMToken: 1.1}, Source: "local"},

		// Qwen models
		"qwen-3-235b": {ContextLength: 131_072, Pricing: types.Pricing{InputPerMToken: 0.25, OutputPerMToken: 1.0}, Source: "local"},

		// Meta Llama models
		"llama-4-maverick": {ContextLength: 1_048_576, Pricing: types.Pricing{InputPerMToken: 0.2, OutputPerMToken: 0.6}, Source: "local"},
		"llama-4-scout":    {ContextLength: 1_048_576, Pricing: types.Pricing{InputPerMToken: 0.08, OutputPerMToken: 0.3}, Source: "local"},
	}
}

// normalizeModelID strips provider prefixes (e.g., "anthropic/claude-fable-5" -> "claude-fable-5")
// and lowercases the result for matching against the local metadata database.
func normalizeModelID(id string) string {
	// Strip provider prefix if present (e.g., "anthropic/claude-fable-5" -> "claude-fable-5")
	if idx := strings.IndexByte(id, '/'); idx >= 0 {
		id = id[idx+1:]
	}
	return strings.ToLower(id)
}

// lookupMetadata finds metadata for a model ID by trying multiple matching strategies:
//  1. Exact match in OpenRouter metadata
//  2. Normalized match in OpenRouter metadata (strip provider prefix)
//  3. Exact match in local fallback
//  4. Normalized match in local fallback
func lookupMetadata(modelID string, openRouterMeta map[string]ModelMetadata) (*ModelMetadata, bool) {
	// Try OpenRouter metadata first (exact match)
	if meta, ok := openRouterMeta[modelID]; ok {
		return &meta, true
	}

	// Try OpenRouter with normalized ID
	normalized := normalizeModelID(modelID)
	if meta, ok := openRouterMeta[normalized]; ok {
		return &meta, true
	}

	// Try OpenRouter with "provider/model" format (e.g., "anthropic/claude-fable-5")
	for prefix := range map[string]bool{"anthropic": true, "openai": true, "google": true, "meta-llama": true, "deepseek": true, "mistralai": true, "qwen": true} {
		fullID := prefix + "/" + normalized
		if meta, ok := openRouterMeta[fullID]; ok {
			return &meta, true
		}
	}

	// Fall back to local database (exact match)
	localDB := LocalMetadataFallback()
	if meta, ok := localDB[modelID]; ok {
		return &meta, true
	}

	// Try local database with normalized ID
	if meta, ok := localDB[normalized]; ok {
		return &meta, true
	}

	return nil, false
}

// EnrichModelInfo enriches a slice of ModelInfo with context_length and pricing
// data from OpenRouter (preferred) or the local fallback database.
//
// This is designed for providers like Zen that don't return pricing or context
// length in their API response. The enrichment only modifies fields that are
// currently zero/default — it never overwrites non-zero values from the provider.
//
// The providerName parameter controls enrichment behavior:
//   - "zen": enriches both context_length and pricing
//   - Other providers: only enriches context_length (pricing comes from their API)
func EnrichModelInfo(models []types.ModelInfo, providerName string) []types.ModelInfo {
	if len(models) == 0 {
		return models
	}

	ctx, cancel := context.WithTimeout(context.Background(), types.FetchModelsTimeout)
	defer cancel()

	openRouterMeta := FetchOpenRouterMetadata(ctx)
	localDB := LocalMetadataFallback()

	enriched := make([]types.ModelInfo, len(models))
	for i, m := range models {
		// Try OpenRouter metadata first
		meta, found := lookupMetadata(m.ID, openRouterMeta)

		// Fall back to local database
		if !found {
			if localMeta, ok := lookupMetadata(m.ID, localDB); ok {
				meta = localMeta
			}
		}

		if meta != nil {
			// Enrich context_length if the provider didn't set it (still at default)
			if m.ContextLength == types.DefaultContextLength || m.ContextLength == 0 {
				m.ContextLength = meta.ContextLength
			}

			// Enrich pricing for providers that don't report it (e.g., Zen)
			if providerName == "zen" {
				if m.Pricing.InputPerMToken == 0 && m.Pricing.OutputPerMToken == 0 {
					m.Pricing = meta.Pricing
				}
			}
		}

		enriched[i] = m
	}

	return enriched
}
