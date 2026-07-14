package types

// Provider name constants — single source of truth for provider identifiers.
// Used across the codebase to avoid magic strings and enable safe refactoring.
const (
	ProviderOpenRouter = "openrouter"
	ProviderZen        = "zen"
	ProviderNvidia     = "nvidia"
)