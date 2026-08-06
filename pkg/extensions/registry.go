package extensions

import (
	"context"
	"errors"
	"fmt"
	"log/slog"
	"sync"

	"github.com/eshanized/M31A/internal/core/types"
)

// MsgEmitter is the interface for emitting messages to the TUI.
// This mirrors the internal/tui channelEmitter interface.
type MsgEmitter interface {
	Emit(msg any)
}

// ExtensionRegistry manages the lifecycle of all registered extensions.
// It loads extensions from config, starts their subprocesses, and provides
// lookup methods for tools, providers, and hooks.
type ExtensionRegistry struct {
	mu          sync.RWMutex
	tools       map[string]ExternalTool
	providers   map[string]ExternalProvider
	preHooks    map[types.WorkflowPhase][]PhaseHookHandler
	postHooks   map[types.WorkflowPhase][]PhaseHookHandler
	processes   map[string]*SubprocessManager
	toolConfigs     map[string]ExternalToolConfig
	providerConfigs map[string]ExternalProviderConfig
	hookConfigs     map[string]PhaseHookConfig
	emitter         MsgEmitter
	started         bool
}

// NewExtensionRegistry creates a new ExtensionRegistry from extension config.
func NewExtensionRegistry(cfg *ExtensionsConfig, emitter MsgEmitter) *ExtensionRegistry {
	return &ExtensionRegistry{
		tools:           make(map[string]ExternalTool),
		providers:       make(map[string]ExternalProvider),
		preHooks:        make(map[types.WorkflowPhase][]PhaseHookHandler),
		postHooks:       make(map[types.WorkflowPhase][]PhaseHookHandler),
		processes:       make(map[string]*SubprocessManager),
		toolConfigs:     cfg.Tools,
		providerConfigs: cfg.Providers,
		hookConfigs:     cfg.Hooks,
		emitter:         emitter,
	}
}

// Start initializes and starts all configured extensions.
func (r *ExtensionRegistry) Start(ctx context.Context) error {
	r.mu.Lock()
	defer r.mu.Unlock()

	if r.started {
		return errors.New("registry already started")
	}

	// Start tool extensions
	for name, cfg := range r.toolConfigs {
		if err := r.startTool(ctx, name, cfg); err != nil {
			// Clean up already started extensions
			r.stopAllLocked()
			return fmt.Errorf("start tool %s: %w", name, err)
		}
	}

	// Start provider extensions
	for name, cfg := range r.providerConfigs {
		if err := r.startProvider(ctx, name, cfg); err != nil {
			r.stopAllLocked()
			return fmt.Errorf("start provider %s: %w", name, err)
		}
	}

	// Start hook extensions
	for name, cfg := range r.hookConfigs {
		if err := r.startHook(ctx, name, cfg); err != nil {
			r.stopAllLocked()
			return fmt.Errorf("start hook %s: %w", name, err)
		}
	}

	r.started = true
	slog.Info("extension registry started", "tools", len(r.tools), "providers", len(r.providers), "hooks", len(r.preHooks)+len(r.postHooks))
	return nil
}

// startTool starts a single tool extension.
func (r *ExtensionRegistry) startTool(ctx context.Context, name string, cfg ExternalToolConfig) error {
	timeout, err := cfg.ParsedTimeout()
	if err != nil {
		return fmt.Errorf("invalid timeout: %w", err)
	}

	proc := NewSubprocessManager(cfg.Command, cfg.Args, cfg.Env, timeout)
	proc.SetWorkDir(".") // TODO: use project root

	if err := proc.Start(ctx); err != nil {
		return fmt.Errorf("start subprocess: %w", err)
	}

	// Create adapter
	adapter := NewExternalToolAdapter(name, proc)
	r.tools[name] = adapter
	r.processes[name] = proc

	slog.Debug("registered external tool", "name", name)
	return nil
}

// startProvider starts a single provider extension.
func (r *ExtensionRegistry) startProvider(ctx context.Context, name string, cfg ExternalProviderConfig) error {
	timeout, err := cfg.ParsedTimeout()
	if err != nil {
		return fmt.Errorf("invalid timeout: %w", err)
	}

	proc := NewSubprocessManager(cfg.Command, cfg.Args, cfg.Env, timeout)
	proc.SetWorkDir(".")

	if err := proc.Start(ctx); err != nil {
		return fmt.Errorf("start subprocess: %w", err)
	}

	// Create adapter
	adapter := NewExternalProviderAdapter(name, proc)
	r.providers[name] = adapter
	r.processes[name] = proc

	slog.Debug("registered external provider", "name", name)
	return nil
}

// startHook starts a single hook extension.
func (r *ExtensionRegistry) startHook(ctx context.Context, name string, cfg PhaseHookConfig) error {
	timeout, err := cfg.ParsedTimeout()
	if err != nil {
		return fmt.Errorf("invalid timeout: %w", err)
	}

	proc := NewSubprocessManager(cfg.Command, cfg.Args, cfg.Env, timeout)
	proc.SetWorkDir(".")

	if err := proc.Start(ctx); err != nil {
		return fmt.Errorf("start subprocess: %w", err)
	}

	// Create adapter
	adapter := NewPhaseHookAdapter(name, proc, r.emitter)

	// Register for each phase
	for _, phaseStr := range cfg.Phases {
		phase := types.WorkflowPhase(phaseStr)
		for _, hookType := range cfg.HookTypes {
			switch hookType {
			case "pre":
				r.preHooks[phase] = append(r.preHooks[phase], adapter)
			case "post":
				r.postHooks[phase] = append(r.postHooks[phase], adapter)
			}
		}
	}

	r.processes[name] = proc
	slog.Debug("registered external hook", "name", name, "phases", cfg.Phases, "types", cfg.HookTypes)
	return nil
}

// Stop stops all extension subprocesses.
func (r *ExtensionRegistry) Stop() error {
	r.mu.Lock()
	defer r.mu.Unlock()

	return r.stopAllLocked()
}

// stopAllLocked stops all subprocesses (must hold lock).
func (r *ExtensionRegistry) stopAllLocked() error {
	var errs []error

	for name, proc := range r.processes {
		if err := proc.Stop(); err != nil {
			errs = append(errs, fmt.Errorf("stop %s: %w", name, err))
		}
	}

	r.tools = make(map[string]ExternalTool)
	r.providers = make(map[string]ExternalProvider)
	r.preHooks = make(map[types.WorkflowPhase][]PhaseHookHandler)
	r.postHooks = make(map[types.WorkflowPhase][]PhaseHookHandler)
	r.processes = make(map[string]*SubprocessManager)
	r.started = false

	if len(errs) > 0 {
		return fmt.Errorf("stop errors: %v", errs)
	}

	slog.Info("extension registry stopped")
	return nil
}

// GetTool returns an external tool by name.
func (r *ExtensionRegistry) GetTool(name string) (ExternalTool, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	tool, ok := r.tools[name]
	return tool, ok
}

// GetProvider returns an external provider by name.
func (r *ExtensionRegistry) GetProvider(name string) (ExternalProvider, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	provider, ok := r.providers[name]
	return provider, ok
}

// GetHooks returns all hook handlers for a given phase.
func (r *ExtensionRegistry) GetHooks(phase types.WorkflowPhase) []PhaseHookHandler {
	r.mu.RLock()
	defer r.mu.RUnlock()

	var hooks []PhaseHookHandler
	hooks = append(hooks, r.preHooks[phase]...)
	hooks = append(hooks, r.postHooks[phase]...)
	return hooks
}

// GetPreHooks returns pre-phase hooks for a given phase.
func (r *ExtensionRegistry) GetPreHooks(phase types.WorkflowPhase) []PhaseHookHandler {
	r.mu.RLock()
	defer r.mu.RUnlock()
	hooks := make([]PhaseHookHandler, len(r.preHooks[phase]))
	copy(hooks, r.preHooks[phase])
	return hooks
}

// GetPostHooks returns post-phase hooks for a given phase.
func (r *ExtensionRegistry) GetPostHooks(phase types.WorkflowPhase) []PhaseHookHandler {
	r.mu.RLock()
	defer r.mu.RUnlock()
	hooks := make([]PhaseHookHandler, len(r.postHooks[phase]))
	copy(hooks, r.postHooks[phase])
	return hooks
}

// GetToolConfigs returns all tool configurations.
func (r *ExtensionRegistry) GetToolConfigs() map[string]ExternalToolConfig {
	r.mu.RLock()
	defer r.mu.RUnlock()
	configs := make(map[string]ExternalToolConfig, len(r.toolConfigs))
	for k, v := range r.toolConfigs {
		configs[k] = v
	}
	return configs
}

// GetProviderConfigs returns all provider configurations.
func (r *ExtensionRegistry) GetProviderConfigs() map[string]ExternalProviderConfig {
	r.mu.RLock()
	defer r.mu.RUnlock()
	configs := make(map[string]ExternalProviderConfig, len(r.providerConfigs))
	for k, v := range r.providerConfigs {
		configs[k] = v
	}
	return configs
}

// GetHookConfigs returns all hook configurations.
func (r *ExtensionRegistry) GetHookConfigs() map[string]PhaseHookConfig {
	r.mu.RLock()
	defer r.mu.RUnlock()
	configs := make(map[string]PhaseHookConfig, len(r.hookConfigs))
	for k, v := range r.hookConfigs {
		configs[k] = v
	}
	return configs
}

// IsStarted returns true if the registry has been started.
func (r *ExtensionRegistry) IsStarted() bool {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.started
}