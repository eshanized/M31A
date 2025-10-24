package tui

import "strings"

// ProviderShortName returns a short display name for a provider.
func ProviderShortName(name string) string {
	switch strings.ToLower(name) {
	case "openrouter":
		return "OR"
	case "zen", "zen-gateway":
		return "Zen"
	case "openai":
		return "OAI"
	case "anthropic":
		return "AC"
	default:
		if len(name) > 4 {
			return name[:4]
		}
		return name
	}
}
