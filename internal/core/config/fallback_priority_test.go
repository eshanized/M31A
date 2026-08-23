package config

import (
	"strings"
	"testing"
)

func TestValidateConfig_FallbackPriority(t *testing.T) {
	tests := []struct {
		name     string
		priority []string
		wantErr  bool
	}{
		{
			name:     "valid providers",
			priority: []string{"openrouter", "zen", "nvidia"},
			wantErr:  false,
		},
		{
			name:     "single valid provider",
			priority: []string{"openrouter"},
			wantErr:  false,
		},
		{
			name:     "empty list",
			priority: []string{},
			wantErr:  false,
		},
		{
			name:     "nil list",
			priority: nil,
			wantErr:  false,
		},
		{
			name:     "invalid provider name",
			priority: []string{"openrouter", "invalid-provider"},
			wantErr:  true,
		},
		{
			name:     "typo in provider name",
			priority: []string{"openrouterr"},
			wantErr:  true,
		},
		{
			name:     "case sensitive",
			priority: []string{"OpenRouter"},
			wantErr:  true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cfg := DefaultConfig()
			cfg.Provider.FallbackPriority = tt.priority

			err := validateConfig(cfg)
			if tt.wantErr && err == nil {
				t.Error("expected validation error, got nil")
			}
			if !tt.wantErr && err != nil {
				t.Errorf("expected no error, got %v", err)
			}
			if tt.wantErr && err != nil && !strings.Contains(err.Error(), "fallback_priority") {
				t.Errorf("error should mention fallback_priority, got %v", err)
			}
		})
	}
}
