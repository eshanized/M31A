package subagent

import (
	"sort"

	"github.com/eshanized/M31A/internal/config"
)

// AgentProfile defines a named subagent type with its own system prompt,
// tool access, model assignment, and resource budgets. Profiles can be
// built-in or user-defined via config overrides.
type AgentProfile struct {
	Name         string
	Description  string
	Mode         string // "primary", "subagent", "all"
	SystemPrompt string
	Model        string // override model ID; "" = inherit parent
	Hidden       bool

	// Tool scoping: AllowedTools is an allowlist (empty = all tools).
	// DeniedTools is a denylist applied after the allowlist.
	AllowedTools []string
	DeniedTools  []string

	// Resource budgets (0 = use manager defaults).
	MaxTools  int
	MaxTokens int
	MaxTurns  int
}

const (
	ModePrimary  = "primary"
	ModeSubagent = "subagent"
	ModeAll      = "all"
)

const (
	promptGeneral = `You are a general-purpose subagent running inside a parallel exploration swarm.
Your parent has given you a well-defined task. Execute it efficiently.

Guidelines:
- Use any tools available to accomplish the task.
- Do not ask the user questions; you run autonomously.
- When you have enough information, stop calling tools and write a clear summary.
- Keep tool calls minimal; every call costs tokens and time.
- Read files before editing them to understand existing code.`

	promptExplore = `You are a file search specialist. You excel at thoroughly navigating and exploring codebases.

Your strengths:
- Rapidly finding files using glob patterns
- Searching code and text with powerful regex patterns
- Reading and analyzing file contents

Guidelines:
- Use Glob for broad file pattern matching
- Use Grep for searching file contents with regex
- Use Read when you know the specific file path you need to read
- Adapt your search approach based on the thoroughness level specified by the caller
- Return file paths as absolute paths in your final response
- Do not create any files or run commands that modify the system state
- When you have enough information, write a clear summary of your findings.`

	promptSecurity = `You are a security audit specialist focused on identifying vulnerabilities and security issues in codebases.

Your strengths:
- Identifying common vulnerability patterns (injection, XSS, path traversal, etc.)
- Analyzing authentication and authorization logic
- Reviewing dependency security and configuration issues
- Detecting hardcoded secrets and credentials

Guidelines:
- Search for known vulnerability patterns using Grep
- Read relevant files to understand the full security context
- Check configuration files for insecure defaults
- Look for hardcoded secrets, API keys, and credentials
- Provide specific file paths and line references in your findings
- Rate findings by severity (critical, high, medium, low, informational)`

	promptReview = `You are a code review agent focused on quality, correctness, and maintainability.

Your strengths:
- Identifying bugs, logic errors, and edge cases
- Evaluating code structure, naming, and readability
- Checking for proper error handling and resource cleanup
- Assessing test coverage and test quality

Guidelines:
- Read the target files thoroughly before forming opinions
- Use Grep to find related code and understand call sites
- Use CodeMap to understand the dependency graph
- Provide specific, actionable feedback with file paths and line references
- Categorize findings (bug, style, performance, security, suggestion)`

	promptPlan = `You are a planning agent. Your job is to analyze tasks and create detailed implementation plans.

Guidelines:
- Read relevant files to understand the codebase structure
- Analyze requirements and break them into concrete steps
- Identify potential risks and edge cases
- Produce a clear, actionable plan with specific file changes
- Do not modify any files except plan documents`

	promptBuild = `You are a build agent with full tool access for executing software engineering tasks.

Guidelines:
- Use all available tools to accomplish the task efficiently
- Read files before modifying them
- Test your changes when possible
- Provide a clear summary of what was done`
)

// BuiltinProfiles returns the 6 built-in agent profiles.
func BuiltinProfiles() map[string]AgentProfile {
	return map[string]AgentProfile{
		"build": {
			Name:         "build",
			Description:  "Default agent. Full tool access for executing tasks.",
			Mode:         ModePrimary,
			SystemPrompt: promptBuild,
		},
		"plan": {
			Name:         "plan",
			Description:  "Planning agent. Read-only except plan documents.",
			Mode:         ModePrimary,
			SystemPrompt: promptPlan,
			DeniedTools: []string{
				"Bash", "FileWrite", "Edit", "FileDelete", "FileMove", "DevServer",
			},
		},
		"general": {
			Name:         "general",
			Description:  "General-purpose agent for multi-step tasks and parallel work.",
			Mode:         ModeSubagent,
			SystemPrompt: promptGeneral,
			DeniedTools:  []string{"TodoWrite", "Agent"},
		},
		"explore": {
			Name:         "explore",
			Description:  "Fast codebase exploration. Read-only search tools.",
			Mode:         ModeSubagent,
			SystemPrompt: promptExplore,
			AllowedTools: []string{
				"Glob", "Grep", "FileRead", "FileList",
				"CodeMap", "CodeComplexity", "WebFetch", "WebSearch",
			},
		},
		"security": {
			Name:         "security",
			Description:  "Security audit agent for vulnerability scanning and code security review.",
			Mode:         ModeSubagent,
			SystemPrompt: promptSecurity,
			AllowedTools: []string{
				"Glob", "Grep", "FileRead", "FileList",
				"CodeMap", "CodeComplexity", "Bash", "WebFetch", "WebSearch",
			},
			DeniedTools: []string{
				"FileWrite", "Edit", "FileDelete", "FileMove",
			},
		},
		"review": {
			Name:         "review",
			Description:  "Code review agent for quality and correctness analysis.",
			Mode:         ModeSubagent,
			SystemPrompt: promptReview,
			AllowedTools: []string{
				"Glob", "Grep", "FileRead", "FileList",
				"CodeMap", "CodeComplexity", "WebFetch", "WebSearch",
			},
			DeniedTools: []string{
				"FileWrite", "Edit", "FileDelete", "FileMove",
			},
		},
	}
}

// ResolveProfile returns the resolved AgentProfile for a given name.
// It starts with the built-in default (if one exists) and applies user
// overrides from the config. If the profile is disabled via config, the
// second return value is false.
// If no built-in profile exists for the name, a custom profile is created
// from the overrides alone (Mode defaults to "all").
func ResolveProfile(name string, overrides map[string]config.SubagentProfileConfig) (AgentProfile, bool) {
	builtins := BuiltinProfiles()
	profile, exists := builtins[name]

	ov, hasOverride := overrides[name]
	if hasOverride && ov.Disabled {
		return AgentProfile{}, false
	}

	if !exists {
		if !hasOverride {
			return AgentProfile{}, false
		}
		profile = AgentProfile{
			Name: name,
			Mode: ModeAll,
		}
	}

	if !hasOverride {
		return profile, true
	}

	if ov.Description != "" {
		profile.Description = ov.Description
	}
	if ov.Mode != "" {
		profile.Mode = ov.Mode
	}
	if ov.SystemPrompt != "" {
		profile.SystemPrompt = ov.SystemPrompt
	}
	if ov.Model != "" {
		profile.Model = ov.Model
	}
	if ov.Hidden != nil {
		profile.Hidden = *ov.Hidden
	}
	if len(ov.AllowedTools) > 0 {
		profile.AllowedTools = ov.AllowedTools
	}
	if len(ov.DeniedTools) > 0 {
		profile.DeniedTools = ov.DeniedTools
	}
	if ov.MaxTools > 0 {
		profile.MaxTools = ov.MaxTools
	}
	if ov.MaxTokens > 0 {
		profile.MaxTokens = ov.MaxTokens
	}
	if ov.MaxTurns > 0 {
		profile.MaxTurns = ov.MaxTurns
	}

	return profile, true
}

// ListSubagentProfiles returns all non-hidden profiles where Mode is
// "subagent" or "all", sorted by name. Used to populate the Agent tool
// description so the LLM knows which subagent types are available.
func ListSubagentProfiles(overrides map[string]config.SubagentProfileConfig) []AgentProfile {
	builtins := BuiltinProfiles()

	// Merge in user-defined profiles that don't match a built-in.
	for name := range overrides {
		if _, exists := builtins[name]; !exists {
			builtins[name] = AgentProfile{Name: name, Mode: ModeAll}
		}
	}

	var result []AgentProfile
	for name := range builtins {
		profile, ok := ResolveProfile(name, overrides)
		if !ok {
			continue
		}
		if profile.Hidden {
			continue
		}
		if profile.Mode != ModeSubagent && profile.Mode != ModeAll {
			continue
		}
		result = append(result, profile)
	}

	sort.Slice(result, func(i, j int) bool {
		return result[i].Name < result[j].Name
	})
	return result
}
