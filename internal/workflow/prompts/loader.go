package prompts

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"

	"github.com/eshanized/M31A/internal/config"
)

// LoadPrompt loads a prompt by name with layered override resolution.
// Priority order (highest to lowest):
//  1. Config override (prompts.overrides[name])
//  2. Project-level override (.m31a/prompts/<name>.md)
//  3. Global override (~/.m31a/prompts/<name>.md)
//  4. Embedded default (prompts/<name>.md from go:embed)
//
// Returns the prompt content, whether it was loaded from an override, and any error.
func LoadPrompt(name string, cfg config.PromptConfig, projectRoot string, embeddedFS fs.FS) (string, bool, error) {
	// 1. Check config override (explicit per-prompt file path)
	if overridePath, ok := cfg.Overrides[name]; ok && overridePath != "" {
		data, err := os.ReadFile(overridePath)
		if err != nil {
			return "", false, fmt.Errorf("reading prompt override %q: %w", overridePath, err)
		}
		return string(data), true, nil
	}

	// 2. Check project-level override
	if cfg.ProjectPromptDir != "" && projectRoot != "" {
		projectPath := filepath.Join(projectRoot, cfg.ProjectPromptDir, name+".md")
		if data, err := os.ReadFile(projectPath); err == nil {
			return string(data), true, nil
		}
	}

	// 3. Check global override
	if cfg.GlobalPromptDir != "" {
		globalPath := filepath.Join(cfg.GlobalPromptDir, name+".md")
		if data, err := os.ReadFile(globalPath); err == nil {
			return string(data), true, nil
		}
	}

	// 4. Fall back to embedded default
	data, err := fs.ReadFile(embeddedFS, "prompts/"+name+".md")
	if err != nil {
		return "", false, fmt.Errorf("reading embedded prompt %q: %w", name, err)
	}
	return string(data), false, nil
}

// LoadModelTemplate loads a model-specific template with override support.
// Priority order:
//  1. Config override (prompts.model_template_overrides[prefix])
//  2. Embedded default (prompts/models/<prefix>.txt)
//  3. Embedded fallback (prompts/models/default.txt)
func LoadModelTemplate(modelID string, cfg config.PromptConfig, embeddedFS fs.FS) string {
	prefix := DetectModelPrefix(modelID)

	// 1. Check config override
	if overridePath, ok := cfg.ModelTemplateOverrides[prefix]; ok && overridePath != "" {
		if data, err := os.ReadFile(overridePath); err == nil {
			return strings.TrimSpace(string(data))
		}
	}

	// 2. Check embedded template for this prefix
	path := "prompts/models/" + prefix + ".txt"
	if data, err := fs.ReadFile(embeddedFS, path); err == nil {
		return strings.TrimSpace(string(data))
	}

	// 3. Fall back to default
	data, err := fs.ReadFile(embeddedFS, "prompts/models/default.txt")
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(data))
}

// DetectModelPrefix determines the model family prefix for template selection.
func DetectModelPrefix(modelID string) string {
	lower := strings.ToLower(modelID)
	switch {
	case strings.Contains(lower, "claude") || strings.Contains(lower, "anthropic"):
		return "anthropic"
	case strings.Contains(lower, "gpt") || strings.Contains(lower, "o1") ||
		strings.Contains(lower, "o3") || strings.Contains(lower, "o4"):
		return "openai"
	case strings.Contains(lower, "gemini") || strings.Contains(lower, "google"):
		return "google"
	default:
		return "default"
	}
}
