package workflow

import (
	"embed"
	"fmt"
	"strings"
	"time"

	"github.com/eshanized/M31A/internal/config"
	"github.com/eshanized/M31A/internal/types"
	"github.com/eshanized/M31A/internal/workflow/prompts"
)

//go:embed prompts/*.md
var promptFS embed.FS

// PromptRegistry holds all loaded prompt templates.
type PromptRegistry struct {
	Base             string
	ToolUse          string
	PlanFormat       string
	ExecuteTask      string
	Discuss          string
	SelfHeal         string
	Demonstration    string
	Autonomous       string
	ContextAwareness string
	CodeQuality      string
	CodeIntelligence string
	Research         string
	PlanCheck        string
	PlanRevise       string
	PlanOutline      string
	DiscussFollowup  string
	IntentClassify   string
	WebsiteBuild     string
}

// LoadPrompts reads all prompt files with override support and returns a registry.
// Uses the 4-level priority chain: config override > project > global > embedded.
func LoadPrompts(cfg config.PromptConfig, projectRoot string) (*PromptRegistry, error) {
	r := &PromptRegistry{}
	registryFields := map[string]*string{
		"base":                 &r.Base,
		"tool-use":             &r.ToolUse,
		"plan-format":          &r.PlanFormat,
		"execute-task":         &r.ExecuteTask,
		"discuss-questions":    &r.Discuss,
		"self-heal":            &r.SelfHeal,
		"demonstration-format": &r.Demonstration,
		"autonomous":           &r.Autonomous,
		"context-awareness":    &r.ContextAwareness,
		"code-quality":         &r.CodeQuality,
		"code-intelligence":    &r.CodeIntelligence,
		"research":             &r.Research,
		"plan-check":           &r.PlanCheck,
		"plan-revise":          &r.PlanRevise,
		"plan-outline":         &r.PlanOutline,
		"discuss-followup":     &r.DiscussFollowup,
		"intent-classify":      &r.IntentClassify,
		"website-build":        &r.WebsiteBuild,
	}

	for fileKey, ptr := range registryFields {
		data, _, err := prompts.LoadPrompt(fileKey, cfg, projectRoot, promptFS)
		if err != nil {
			return nil, fmt.Errorf("load prompt %s: %w", fileKey, err)
		}
		*ptr = strings.TrimSpace(data)
	}

	// Inject dynamic limits into the tool-use prompt (F-010).
	// The prompt must reflect actual runtime limits, not hardcoded values.
	r.ToolUse = injectToolUseLimits(r.ToolUse)

	return r, nil
}

// injectToolUseLimits replaces hardcoded limit values in the tool-use prompt
// with actual runtime constants. This ensures the prompt accurately describes
// the real constraints the AI will encounter.
func injectToolUseLimits(prompt string) string {
	timeoutMins := int(types.BashTimeout / time.Minute)
	outputLimit := types.BashOutputLimit
	maxFileSizeMB := types.MaxFileSize / (1024 * 1024)

	prompt = strings.ReplaceAll(prompt, "30 minutes maximum",
		fmt.Sprintf("%d minutes maximum", timeoutMins))
	prompt = strings.ReplaceAll(prompt, "50,000 characters",
		fmt.Sprintf("%d characters", outputLimit))
	prompt = strings.ReplaceAll(prompt, "5MB",
		fmt.Sprintf("%dMB", maxFileSizeMB))

	return prompt
}

// PromptBuilder wraps a PromptRegistry and provides named prompt access.
type PromptBuilder struct {
	registry *PromptRegistry
}

// NewPromptBuilder creates a PromptBuilder by loading all prompt templates
// with override support from the provided config.
func NewPromptBuilder(cfg config.PromptConfig, projectRoot string) (*PromptBuilder, error) {
	r, err := LoadPrompts(cfg, projectRoot)
	if err != nil {
		return nil, fmt.Errorf("create prompt builder: %w", err)
	}
	return &PromptBuilder{registry: r}, nil
}

// GetPrompt returns the prompt template for the given name.
// Returns an error if the name is not recognized.
func (pb *PromptBuilder) GetPrompt(name string) (string, error) {
	if pb == nil || pb.registry == nil {
		return "", fmt.Errorf("prompt builder not initialized")
	}
	switch name {
	case "base":
		return pb.registry.Base, nil
	case "tool-use":
		return pb.registry.ToolUse, nil
	case "plan-format":
		return pb.registry.PlanFormat, nil
	case "execute-task":
		return pb.registry.ExecuteTask, nil
	case "discuss":
		return pb.registry.Discuss, nil
	case "self-heal":
		return pb.registry.SelfHeal, nil
	case "demonstration":
		return pb.registry.Demonstration, nil
	case "autonomous":
		return pb.registry.Autonomous, nil
	case "context-awareness":
		return pb.registry.ContextAwareness, nil
	case "code-quality":
		return pb.registry.CodeQuality, nil
	case "code-intelligence":
		return pb.registry.CodeIntelligence, nil
	case "research":
		return pb.registry.Research, nil
	case "plan-check":
		return pb.registry.PlanCheck, nil
	case "plan-revise":
		return pb.registry.PlanRevise, nil
	case "plan-outline":
		return pb.registry.PlanOutline, nil
	case "discuss-followup":
		return pb.registry.DiscussFollowup, nil
	case "intent-classify":
		return pb.registry.IntentClassify, nil
	case "website-build":
		return pb.registry.WebsiteBuild, nil
	default:
		return "", fmt.Errorf("unknown prompt: %s", name)
	}
}

// Prompt returns the prompt template for the given name.
// Returns an error if the name is not recognized.
// Callers should prefer GetPrompt for explicit error handling.
func (pb *PromptBuilder) Prompt(name string) (string, error) {
	return pb.GetPrompt(name)
}

// BuildSystemPrompt concatenates the base prompt with optional extras.
func (pb *PromptBuilder) BuildSystemPrompt(base string, extras ...string) string {
	parts := []string{base}
	for _, e := range extras {
		if e != "" {
			parts = append(parts, e)
		}
	}
	return strings.Join(parts, "\n\n---\n\n")
}
