package extensions

import (
	"time"
)

// ExtensionsConfig holds all extension configuration sections.
// This is the top-level config structure that gets merged from multiple sources.
type ExtensionsConfig struct {
	// Tools maps tool names to their configurations.
	Tools map[string]ExternalToolConfig `toml:"tools" json:"tools"`
	// Providers maps provider names to their configurations.
	Providers map[string]ExternalProviderConfig `toml:"providers" json:"providers"`
	// Hooks maps hook names to their configurations.
	Hooks map[string]PhaseHookConfig `toml:"hooks" json:"hooks"`
}

// ExternalToolConfig configures an external tool extension.
type ExternalToolConfig struct {
	// Command is the path to the extension executable.
	// Must be an absolute path or resolvable via PATH.
	Command string `toml:"command" json:"command"`
	// Args are command-line arguments passed to the executable.
	Args []string `toml:"args" json:"args"`
	// Env are environment variables to set for the subprocess.
	Env map[string]string `toml:"env" json:"env"`
	// Timeout is the maximum duration for tool execution (e.g., "30s").
	Timeout string `toml:"timeout" json:"timeout"`
}

// ExternalProviderConfig configures an external LLM provider extension.
type ExternalProviderConfig struct {
	// Command is the path to the extension executable.
	// Must be an absolute path or resolvable via PATH.
	Command string `toml:"command" json:"command"`
	// Args are command-line arguments passed to the executable.
	Args []string `toml:"args" json:"args"`
	// Env are environment variables to set for the subprocess.
	Env map[string]string `toml:"env" json:"env"`
	// Timeout is the maximum duration for provider operations (e.g., "120s").
	Timeout string `toml:"timeout" json:"timeout"`
}

// PhaseHookConfig configures a workflow phase hook extension.
type PhaseHookConfig struct {
	// Command is the path to the extension executable.
	// Must be an absolute path or resolvable via PATH.
	Command string `toml:"command" json:"command"`
	// Args are command-line arguments passed to the executable.
	Args []string `toml:"args" json:"args"`
	// Env are environment variables to set for the subprocess.
	Env map[string]string `toml:"env" json:"env"`
	// Phases lists the workflow phases this hook applies to.
	Phases []string `toml:"phases" json:"phases"`
	// HookTypes specifies "pre", "post", or both.
	HookTypes []string `toml:"hook_types" json:"hook_types"`
	// Timeout is the maximum duration for hook execution (e.g., "30s").
	Timeout string `toml:"timeout" json:"timeout"`
}

// ParsedTimeout parses the timeout string into a time.Duration.
func (c *ExternalToolConfig) ParsedTimeout() (time.Duration, error) {
	if c.Timeout == "" {
		return 30 * time.Second, nil
	}
	return time.ParseDuration(c.Timeout)
}

// ParsedTimeout parses the timeout string into a time.Duration.
func (c *ExternalProviderConfig) ParsedTimeout() (time.Duration, error) {
	if c.Timeout == "" {
		return 120 * time.Second, nil
	}
	return time.ParseDuration(c.Timeout)
}

// ParsedTimeout parses the timeout string into a time.Duration.
func (c *PhaseHookConfig) ParsedTimeout() (time.Duration, error) {
	if c.Timeout == "" {
		return 30 * time.Second, nil
	}
	return time.ParseDuration(c.Timeout)
}