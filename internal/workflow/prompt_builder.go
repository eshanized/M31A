package workflow

import (
	"embed"
	"fmt"
	"strings"
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

// LoadPrompts reads all embedded prompt files and returns a registry.
func LoadPrompts() (*PromptRegistry, error) {
	r := &PromptRegistry{}
	files := map[string]*string{
		"prompts/base.md":                 &r.Base,
		"prompts/tool-use.md":             &r.ToolUse,
		"prompts/plan-format.md":          &r.PlanFormat,
		"prompts/execute-task.md":         &r.ExecuteTask,
		"prompts/discuss-questions.md":    &r.Discuss,
		"prompts/self-heal.md":            &r.SelfHeal,
		"prompts/demonstration-format.md": &r.Demonstration,
		"prompts/autonomous.md":           &r.Autonomous,
		"prompts/context-awareness.md":    &r.ContextAwareness,
		"prompts/code-quality.md":         &r.CodeQuality,
		"prompts/code-intelligence.md":    &r.CodeIntelligence,
		"prompts/research.md":             &r.Research,
		"prompts/plan-check.md":           &r.PlanCheck,
		"prompts/plan-revise.md":          &r.PlanRevise,
		"prompts/plan-outline.md":         &r.PlanOutline,
		"prompts/discuss-followup.md":     &r.DiscussFollowup,
		"prompts/intent-classify.md":      &r.IntentClassify,
		"prompts/website-build.md":        &r.WebsiteBuild,
	}
	for path, ptr := range files {
		data, err := promptFS.ReadFile(path)
		if err != nil {
			return nil, fmt.Errorf("load prompt %s: %w", path, err)
		}
		*ptr = strings.TrimSpace(string(data))
	}
	return r, nil
}



// PromptBuilder wraps a PromptRegistry and provides named prompt access.
type PromptBuilder struct {
	registry *PromptRegistry
}

// NewPromptBuilder creates a PromptBuilder by loading all prompt templates.
func NewPromptBuilder() (*PromptBuilder, error) {
	r, err := LoadPrompts()
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
