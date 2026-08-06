package tools

import (
	"context"
	"fmt"
	"log/slog"

	"github.com/eshanized/M31A/pkg/extensions"
)

// RegisterExternalTools registers all external tools from the extension registry with the dispatcher.
func RegisterExternalTools(ctx context.Context, dispatcher *Dispatcher, extRegistry *extensions.ExtensionRegistry) error {
	toolConfigs := extRegistry.GetToolConfigs()

	for name, cfg := range toolConfigs {
		timeout, err := cfg.ParsedTimeout()
		if err != nil {
			return fmt.Errorf("invalid timeout for tool %s: %w", name, err)
		}

		// Create and start subprocess manager
		proc := extensions.NewSubprocessManager(cfg.Command, cfg.Args, cfg.Env, timeout)
		proc.SetWorkDir(".") // TODO: use project root from context

		if err := proc.Start(ctx); err != nil {
			return fmt.Errorf("start tool subprocess %s: %w", name, err)
		}

		// Create adapter
		adapter := extensions.NewExternalToolAdapter(name, proc)

		// Register with dispatcher
		if err := dispatcher.Register(adapter); err != nil {
			_ = proc.Stop()
			return fmt.Errorf("register external tool %s: %w", name, err)
		}

		slog.Debug("registered external tool", "name", name)
	}

	return nil
}

// StopExternalTools stops all external tool subprocesses.
func StopExternalTools(extRegistry *extensions.ExtensionRegistry) error {
	// The registry's Stop() method handles stopping all subprocesses
	return extRegistry.Stop()
}