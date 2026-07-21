package workflow

import (
	"testing"

	"github.com/eshanized/M31A/internal/core/config"
)

func TestSelectTemplate(t *testing.T) {
	tests := []struct {
		name      string
		modelID   string
		wantMin   int // minimum length of returned template
		wantEmpty bool
	}{
		{
			name:    "empty model ID returns default",
			modelID: "",
			wantMin: 1,
		},
		{
			name:    "GPT model",
			modelID: "gpt-4",
			wantMin: 1,
		},
		{
			name:    "O1 model",
			modelID: "o1-preview",
			wantMin: 1,
		},
		{
			name:    "O3 model",
			modelID: "o3-mini",
			wantMin: 1,
		},
		{
			name:    "Claude model",
			modelID: "claude-3-opus",
			wantMin: 1,
		},
		{
			name:    "Anthropic model",
			modelID: "anthropic-claude-3",
			wantMin: 1,
		},
		{
			name:    "Gemini model",
			modelID: "gemini-pro",
			wantMin: 1,
		},
		{
			name:    "Google model",
			modelID: "google-gemini",
			wantMin: 1,
		},
		{
			name:    "unknown model gets default",
			modelID: "some-random-model",
			wantMin: 1,
		},
		{
			name:    "case insensitive",
			modelID: "GPT-4",
			wantMin: 1,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := SelectTemplate(tt.modelID, config.PromptConfig{})
			if tt.wantEmpty {
				if got != "" {
					t.Errorf("SelectTemplate(%q) = non-empty string, want empty", tt.modelID)
				}
			} else {
				if len(got) < tt.wantMin {
					t.Errorf("SelectTemplate(%q) returned %d chars, want at least %d", tt.modelID, len(got), tt.wantMin)
				}
			}
		})
	}
}

func TestLoadDefaultTemplate(t *testing.T) {
	got := loadDefaultTemplate()
	if len(got) == 0 {
		t.Error("loadDefaultTemplate() returned empty string")
	}
}
