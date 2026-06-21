package workflow

import (
	"embed"
	"strings"
)

//go:embed prompts/models/*.txt
var modelPromptFS embed.FS

// SelectTemplate returns the model-specific prompt template for the given model ID.
// Falls back to the default template if no model-specific match is found.
func SelectTemplate(modelID string) string {
	if modelID == "" {
		return loadDefaultTemplate()
	}
	id := strings.ToLower(modelID)

	var templateName string
	switch {
	case strings.Contains(id, "gpt") || strings.Contains(id, "o1") ||
		strings.Contains(id, "o3") || strings.Contains(id, "o4"):
		templateName = "openai.txt"
	case strings.Contains(id, "claude") || strings.Contains(id, "anthropic"):
		templateName = "anthropic.txt"
	case strings.Contains(id, "gemini") || strings.Contains(id, "google"):
		templateName = "google.txt"
	default:
		templateName = "default.txt"
	}

	data, err := modelPromptFS.ReadFile("prompts/models/" + templateName)
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(data))
}

func loadDefaultTemplate() string {
	data, err := modelPromptFS.ReadFile("prompts/models/default.txt")
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(data))
}
