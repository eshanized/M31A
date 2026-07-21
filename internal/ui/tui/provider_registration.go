package tui

import (
	"fmt"
	"log/slog"
	"time"

	"github.com/eshanized/M31A/internal/core/config"
	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/integrations/provider"
	"github.com/eshanized/M31A/internal/integrations/provider/nvidia"
	"github.com/eshanized/M31A/internal/integrations/provider/openrouter"
	"github.com/eshanized/M31A/internal/integrations/provider/zen"
)

// RegisterProvider creates and registers a provider client in the registry
// using the given API key and configuration. If the provider is already
// registered, it is replaced with the new client.
func RegisterProvider(registry *provider.Registry, cfg *config.Config, providerID, apiKey, version string) error {
	if registry == nil || apiKey == "" {
		return nil
	}

	cacheTTL := types.ModelCacheTTL
	if cfg != nil && cfg.Features.ModelCacheTTLMinutes > 0 {
		cacheTTL = time.Duration(cfg.Features.ModelCacheTTLMinutes) * time.Minute
	}
	cacheStaleTTL := types.StaleCacheTTL
	if cfg != nil && cfg.Features.ModelCacheStaleHours > 0 {
		cacheStaleTTL = time.Duration(cfg.Features.ModelCacheStaleHours) * time.Hour
	}

	switch providerID {
	case types.ProviderOpenRouter:
		baseURL := ""
		referer := ""
		title := ""
		var healthLiveMs, healthSlowMs int
		if cfg != nil {
			baseURL = cfg.Provider.OpenRouterBaseURL
			referer = cfg.Provider.OpenRouterReferer
			title = cfg.Provider.OpenRouterTitle
			healthLiveMs = cfg.Features.HealthCheckLiveMs
			healthSlowMs = cfg.Features.HealthCheckSlowMs
		}
		client, err := openrouter.New(apiKey, openrouter.Options{
			BaseURL:           baseURL,
			CacheTTL:          cacheTTL,
			CacheStaleTTL:     cacheStaleTTL,
			Referer:           referer,
			Title:             title,
			HealthCheckLiveMs: int64(healthLiveMs),
			HealthCheckSlowMs: int64(healthSlowMs),
			Version:           version,
		})
		if err != nil {
			return fmt.Errorf("create OpenRouter client: %w", err)
		}
		if err := registry.Register(types.ProviderOpenRouter, client); err != nil {
			return fmt.Errorf("register OpenRouter provider: %w", err)
		}
		slog.Info("OpenRouter provider registered")

	case types.ProviderZen:
		baseURL := ""
		var defaultCtxLen int
		var healthLiveMs, healthSlowMs int
		if cfg != nil {
			baseURL = cfg.Provider.ZenBaseURL
			defaultCtxLen = cfg.Model.DefaultContextLength
			healthLiveMs = cfg.Features.HealthCheckLiveMs
			healthSlowMs = cfg.Features.HealthCheckSlowMs
		}
		client, err := zen.New(apiKey, zen.Options{
			BaseURL:           baseURL,
			CacheTTL:          cacheTTL,
			CacheStaleTTL:     cacheStaleTTL,
			HealthCheckLiveMs: int64(healthLiveMs),
			HealthCheckSlowMs: int64(healthSlowMs),
			DefaultContextLen: int64(defaultCtxLen),
			Version:           version,
		})
		if err != nil {
			return fmt.Errorf("create Zen client: %w", err)
		}
		if err := registry.Register(types.ProviderZen, client); err != nil {
			return fmt.Errorf("register Zen provider: %w", err)
		}
		slog.Info("Zen provider registered")

	case types.ProviderNvidia:
		baseURL := ""
		var defaultCtxLen int
		var healthLiveMs, healthSlowMs int
		if cfg != nil {
			baseURL = cfg.Provider.NvidiaBaseURL
			defaultCtxLen = cfg.Model.DefaultContextLength
			healthLiveMs = cfg.Features.HealthCheckLiveMs
			healthSlowMs = cfg.Features.HealthCheckSlowMs
		}
		client, err := nvidia.New(apiKey, nvidia.Options{
			BaseURL:           baseURL,
			CacheTTL:          cacheTTL,
			CacheStaleTTL:     cacheStaleTTL,
			HealthCheckLiveMs: int64(healthLiveMs),
			HealthCheckSlowMs: int64(healthSlowMs),
			DefaultContextLen: int64(defaultCtxLen),
			Version:           version,
		})
		if err != nil {
			return fmt.Errorf("create NVIDIA client: %w", err)
		}
		if err := registry.Register(types.ProviderNvidia, client); err != nil {
			return fmt.Errorf("register NVIDIA provider: %w", err)
		}
		slog.Info("NVIDIA NIM provider registered")
	}

	return nil
}
