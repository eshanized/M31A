package config

import (
	"testing"
)

func TestKnownConfigKeys_NoProviderLiterals(t *testing.T) {
	keys := knownConfigKeys()

	// These Go type literal strings should NOT be in the known keys
	badKeys := []string{
		"types.ProviderOpenRouter",
		"types.ProviderZen",
		"types.ProviderNvidia",
	}
	for _, k := range badKeys {
		if keys[k] {
			t.Errorf("knownConfigKeys contains invalid key %q (Go type literal, not TOML key)", k)
		}
	}

	// These valid TOML keys should be present
	validKeys := []string{
		"provider", "model", "ui", "permissions", "features", "tools",
		"git", "ledger", "agents", "verify", "compaction", "instructions",
		"skills", "model_capabilities", "prompts", "narrative", "templates",
	}
	for _, k := range validKeys {
		if !keys[k] {
			t.Errorf("knownConfigKeys missing valid key %q", k)
		}
	}
}
