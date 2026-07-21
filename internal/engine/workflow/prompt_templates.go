package workflow

import (
	"embed"
	"strings"

	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/engine/workflow/prompts"
)

//go:embed prompts/models/*.txt
var modelPromptFS embed.FS

// SelectTemplate returns the model-specific prompt template for the given model ID.
// Falls back to the default template if no model-specific match is found.
// Uses the prompt loader for override support.
func SelectTemplate(modelID string, cfg config.PromptConfig) string {
	return prompts.LoadModelTemplate(modelID, cfg, modelPromptFS)
}

func loadDefaultTemplate() string {
	data, err := modelPromptFS.ReadFile("prompts/models/default.txt")
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(data))
}
