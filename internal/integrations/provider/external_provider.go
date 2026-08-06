package provider

import (
	"context"
	"log/slog"

	"github.com/eshanized/M31A/pkg/extensions"
)

// RegisterExternalProviders registers all external providers from the extension registry with the provider registry.
func RegisterExternalProviders(ctx context.Context, registry *Registry, extRegistry *extensions.ExtensionRegistry) error {
	providerConfigs := extRegistry.GetProviderConfigs()

	for name, cfg := range providerConfigs {
		timeout, err := cfg.ParsedTimeout()
		if err != nil {
			return err
		}

		proc := extensions.NewSubprocessManager(cfg.Command, cfg.Args, cfg.Env, timeout)
		proc.SetWorkDir(".") // TODO: use project root

		if err := proc.Start(ctx); err != nil {
			return err
		}

		// Create adapter
		adapter := extensions.NewExternalProviderAdapter(name, proc)
		if err := registry.Register(name, adapter); err != nil {
			_ = proc.Stop()
			return err
		}

		slog.Debug("registered external provider", "name", name)
	}

	return nil
}