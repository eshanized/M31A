package tui

import "github.com/eshanized/M31A/internal/config"

// getProviderFieldValue reads provider section values from config.
func getProviderFieldValue(c *config.Config, key string) (string, bool) {
	switch key {
	case "provider.default":
		return c.Provider.Default, true
	case "provider.auto_fallback":
		return boolStr(c.Provider.AutoFallback), true
	case "provider.openrouter_base_url":
		return c.Provider.OpenRouterBaseURL, true
	case "provider.zen_base_url":
		return c.Provider.ZenBaseURL, true
	case "provider.nvidia_base_url":
		return c.Provider.NvidiaBaseURL, true
	case "provider.openrouter_referer":
		return c.Provider.OpenRouterReferer, true
	case "provider.openrouter_title":
		return c.Provider.OpenRouterTitle, true
	case "provider.openrouter.api_key":
		return c.Provider.OpenRouter.APIKey, true
	case "provider.zen.api_key":
		return c.Provider.Zen.APIKey, true
	case "provider.nvidia.api_key":
		return c.Provider.Nvidia.APIKey, true
	}
	return "", false
}

// setProviderFieldValue writes provider section values to config.
func setProviderFieldValue(c *config.Config, key string, val string) bool {
	switch key {
	case "provider.default":
		c.Provider.Default = val
	case "provider.auto_fallback":
		c.Provider.AutoFallback = val == "yes"
	case "provider.openrouter_base_url":
		c.Provider.OpenRouterBaseURL = val
	case "provider.zen_base_url":
		c.Provider.ZenBaseURL = val
	case "provider.nvidia_base_url":
		c.Provider.NvidiaBaseURL = val
	case "provider.openrouter_referer":
		c.Provider.OpenRouterReferer = val
	case "provider.openrouter_title":
		c.Provider.OpenRouterTitle = val
	case "provider.openrouter.api_key":
		c.Provider.OpenRouter.APIKey = val
	case "provider.zen.api_key":
		c.Provider.Zen.APIKey = val
	case "provider.nvidia.api_key":
		c.Provider.Nvidia.APIKey = val
	default:
		return false
	}
	return true
}
