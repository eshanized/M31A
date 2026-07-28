package config

import (
	"testing"
)

func TestMergeConfig_OverlayOverridesBase(t *testing.T) {
	t.Parallel()

	base := &Config{
		UI:       UIConfig{Theme: "dark"},
		Model:    ModelConfig{Default: "gpt-4o"},
		Features: FeaturesConfig{MetricsEnabled: true},
	}
	overlay := &Config{
		Model: ModelConfig{Default: "claude-3-opus"},
	}

	MergeConfig(base, overlay, nil)

	if base.Model.Default != "claude-3-opus" {
		t.Errorf("expected 'claude-3-opus', got %q", base.Model.Default)
	}
	if base.UI.Theme != "dark" {
		t.Errorf("expected 'dark' preserved, got %q", base.UI.Theme)
	}
	if !base.Features.MetricsEnabled {
		t.Error("expected MetricsEnabled preserved")
	}
}

func TestMergeConfig_NilOverlay(t *testing.T) {
	t.Parallel()

	base := &Config{UI: UIConfig{Theme: "dark"}}
	MergeConfig(base, nil, nil)

	if base.UI.Theme != "dark" {
		t.Errorf("expected 'dark' preserved, got %q", base.UI.Theme)
	}
}

func TestMergeConfig_EmptyOverlay(t *testing.T) {
	t.Parallel()

	base := &Config{UI: UIConfig{Theme: "dark"}}
	overlay := &Config{}

	MergeConfig(base, overlay, nil)

	if base.UI.Theme != "dark" {
		t.Errorf("expected 'dark' preserved, got %q", base.UI.Theme)
	}
}

func TestMergeConfig_BoolDefinedFalse(t *testing.T) {
	t.Parallel()

	base := &Config{UI: UIConfig{CompactMode: true}}
	overlay := &Config{UI: UIConfig{CompactMode: false}}

	defined := map[string]bool{"ui.compact_mode": true}
	MergeConfig(base, overlay, defined)

	if base.UI.CompactMode {
		t.Error("expected false when explicitly defined")
	}
}

func TestMergeConfig_BoolUndefinedFalse(t *testing.T) {
	t.Parallel()

	base := &Config{UI: UIConfig{CompactMode: true}}
	overlay := &Config{UI: UIConfig{CompactMode: false}}

	MergeConfig(base, overlay, nil)

	if !base.UI.CompactMode {
		t.Error("expected true preserved when overlay false and not defined")
	}
}

func TestMergeConfig_BoolTrueOverride(t *testing.T) {
	t.Parallel()

	base := &Config{UI: UIConfig{CompactMode: false}}
	overlay := &Config{UI: UIConfig{CompactMode: true}}

	MergeConfig(base, overlay, nil)

	if !base.UI.CompactMode {
		t.Error("expected true from overlay")
	}
}

func TestMergeConfig_IntOverride(t *testing.T) {
	t.Parallel()

	base := &Config{UI: UIConfig{MaxIterations: 50}}
	overlay := &Config{UI: UIConfig{MaxIterations: 100}}

	MergeConfig(base, overlay, nil)

	if base.UI.MaxIterations != 100 {
		t.Errorf("expected 100, got %d", base.UI.MaxIterations)
	}
}

func TestMergeConfig_IntZeroPreserved(t *testing.T) {
	t.Parallel()

	base := &Config{UI: UIConfig{MaxIterations: 50}}
	overlay := &Config{UI: UIConfig{MaxIterations: 0}}

	MergeConfig(base, overlay, nil)

	if base.UI.MaxIterations != 50 {
		t.Errorf("expected 50 preserved, got %d", base.UI.MaxIterations)
	}
}

func TestMergeConfig_FloatOverride(t *testing.T) {
	t.Parallel()

	base := &Config{Model: ModelConfig{ContextWarningThreshold: 0.8}}
	overlay := &Config{Model: ModelConfig{ContextWarningThreshold: 0.9}}

	MergeConfig(base, overlay, nil)

	if base.Model.ContextWarningThreshold != 0.9 {
		t.Errorf("expected 0.9, got %f", base.Model.ContextWarningThreshold)
	}
}

func TestMergeConfig_SliceOverride(t *testing.T) {
	t.Parallel()

	base := &Config{Tools: ToolsConfig{SkipDirs: []string{"node_modules", "vendor"}}}
	overlay := &Config{Tools: ToolsConfig{SkipDirs: []string{"dist", "build"}}}

	MergeConfig(base, overlay, nil)

	if len(base.Tools.SkipDirs) != 2 {
		t.Fatalf("expected 2 skip dirs, got %d", len(base.Tools.SkipDirs))
	}
	if base.Tools.SkipDirs[0] != "dist" {
		t.Errorf("expected 'dist', got %q", base.Tools.SkipDirs[0])
	}
}

func TestMergeConfig_SliceEmptyOverlayPreserved(t *testing.T) {
	t.Parallel()

	base := &Config{Tools: ToolsConfig{SkipDirs: []string{"node_modules"}}}
	overlay := &Config{Tools: ToolsConfig{SkipDirs: nil}}

	MergeConfig(base, overlay, nil)

	if len(base.Tools.SkipDirs) != 1 {
		t.Errorf("expected 1 skip dir preserved, got %d", len(base.Tools.SkipDirs))
	}
}

func TestMergeConfig_MapMerge(t *testing.T) {
	t.Parallel()

	base := &Config{
		Permissions: PermissionsConfig{
			Agents: map[string]PermissionsAgentConfig{
				"build": {DefaultAction: "allow"},
			},
		},
	}
	overlay := &Config{
		Permissions: PermissionsConfig{
			Agents: map[string]PermissionsAgentConfig{
				"plan": {DefaultAction: "deny"},
			},
		},
	}

	MergeConfig(base, overlay, nil)

	if len(base.Permissions.Agents) != 2 {
		t.Fatalf("expected 2 agents, got %d", len(base.Permissions.Agents))
	}
	if base.Permissions.Agents["build"].DefaultAction != "allow" {
		t.Error("expected build agent preserved")
	}
	if base.Permissions.Agents["plan"].DefaultAction != "deny" {
		t.Error("expected plan agent merged")
	}
}

func TestMergeConfig_PermissionRules(t *testing.T) {
	t.Parallel()

	base := &Config{
		Permissions: PermissionsConfig{
			Rules: []PermissionRule{
				{Tool: "Bash", Action: "ask"},
			},
		},
	}
	overlay := &Config{
		Permissions: PermissionsConfig{
			Rules: []PermissionRule{
				{Tool: "Edit", Action: "deny"},
			},
		},
	}

	MergeConfig(base, overlay, nil)

	if len(base.Permissions.Rules) != 1 {
		t.Fatalf("expected 1 rule, got %d", len(base.Permissions.Rules))
	}
	if base.Permissions.Rules[0].Tool != "Edit" {
		t.Errorf("expected 'Edit', got %q", base.Permissions.Rules[0].Tool)
	}
}

func TestMergeConfig_ProviderStrings(t *testing.T) {
	t.Parallel()

	base := &Config{Provider: ProviderConfig{Default: "openrouter"}}
	overlay := &Config{Provider: ProviderConfig{Default: "zen"}}

	MergeConfig(base, overlay, nil)

	if base.Provider.Default != "zen" {
		t.Errorf("expected 'zen', got %q", base.Provider.Default)
	}
}

func TestMergeConfig_ProviderBoolAutoFallback(t *testing.T) {
	t.Parallel()

	base := &Config{Provider: ProviderConfig{AutoFallback: false}}
	overlay := &Config{Provider: ProviderConfig{AutoFallback: true}}

	MergeConfig(base, overlay, nil)

	if !base.Provider.AutoFallback {
		t.Error("expected true")
	}
}

func TestMergeConfig_GitConfig(t *testing.T) {
	t.Parallel()

	base := &Config{Git: GitConfig{CommitPrefix: "feat", UserName: "M31A"}}
	overlay := &Config{Git: GitConfig{CommitPrefix: "chore"}}

	MergeConfig(base, overlay, nil)

	if base.Git.CommitPrefix != "chore" {
		t.Errorf("expected 'chore', got %q", base.Git.CommitPrefix)
	}
	if base.Git.UserName != "M31A" {
		t.Errorf("expected 'M31A' preserved, got %q", base.Git.UserName)
	}
}

func TestMergeConfig_CompactionConfig(t *testing.T) {
	t.Parallel()

	base := &Config{Compaction: CompactionConfig{Auto: true, Buffer: 20000}}
	overlay := &Config{Compaction: CompactionConfig{Buffer: 30000}}

	MergeConfig(base, overlay, nil)

	if base.Compaction.Buffer != 30000 {
		t.Errorf("expected 30000, got %d", base.Compaction.Buffer)
	}
	if !base.Compaction.Auto {
		t.Error("expected Auto preserved")
	}
}

func TestMergeConfig_FeaturesConfig(t *testing.T) {
	t.Parallel()

	base := &Config{Features: FeaturesConfig{
		MetricsEnabled:       true,
		ModelCacheTTLMinutes: 5,
		PlanResearch:         false,
	}}
	overlay := &Config{Features: FeaturesConfig{
		PlanResearch:         true,
		ModelCacheTTLMinutes: 10,
	}}

	defined := map[string]bool{
		"features.plan_research":           true,
		"features.model_cache_ttl_minutes": true,
	}
	MergeConfig(base, overlay, defined)

	if !base.Features.PlanResearch {
		t.Error("expected PlanResearch true")
	}
	if base.Features.ModelCacheTTLMinutes != 10 {
		t.Errorf("expected 10, got %d", base.Features.ModelCacheTTLMinutes)
	}
	if !base.Features.MetricsEnabled {
		t.Error("expected MetricsEnabled preserved")
	}
}

func TestMergeConfig_AgentsConfig(t *testing.T) {
	t.Parallel()

	base := &Config{Agents: AgentsConfig{Default: "general", Execute: "code"}}
	overlay := &Config{Agents: AgentsConfig{Plan: "planner"}}

	MergeConfig(base, overlay, nil)

	if base.Agents.Default != "general" {
		t.Errorf("expected 'general' preserved, got %q", base.Agents.Default)
	}
	if base.Agents.Plan != "planner" {
		t.Errorf("expected 'planner', got %q", base.Agents.Plan)
	}
	if base.Agents.Execute != "code" {
		t.Errorf("expected 'code' preserved, got %q", base.Agents.Execute)
	}
}

func TestMergeConfig_NestedSubagentProfiles(t *testing.T) {
	t.Parallel()

	base := &Config{Agents: AgentsConfig{
		Profiles: map[string]SubagentProfileConfig{
			"explore": {Mode: "fast", MaxTools: 5},
		},
	}}
	overlay := &Config{Agents: AgentsConfig{
		Profiles: map[string]SubagentProfileConfig{
			"explore":  {MaxTools: 10},
			"security": {Mode: "deep", MaxTools: 3},
		},
	}}

	MergeConfig(base, overlay, nil)

	if len(base.Agents.Profiles) != 2 {
		t.Fatalf("expected 2 profiles, got %d", len(base.Agents.Profiles))
	}
	if base.Agents.Profiles["explore"].MaxTools != 10 {
		t.Errorf("expected explore MaxTools 10, got %d", base.Agents.Profiles["explore"].MaxTools)
	}
	if base.Agents.Profiles["explore"].Mode != "fast" {
		t.Errorf("expected explore Mode 'fast' preserved, got %q", base.Agents.Profiles["explore"].Mode)
	}
	if base.Agents.Profiles["security"].Mode != "deep" {
		t.Errorf("expected security Mode 'deep', got %q", base.Agents.Profiles["security"].Mode)
	}
}

func TestMergeConfig_VerifyConfig(t *testing.T) {
	t.Parallel()

	base := &Config{Verify: VerifyConfig{BuildCommand: "go build"}}
	overlay := &Config{Verify: VerifyConfig{TestCommand: "go test"}}

	MergeConfig(base, overlay, nil)

	if base.Verify.BuildCommand != "go build" {
		t.Errorf("expected 'go build' preserved, got %q", base.Verify.BuildCommand)
	}
	if base.Verify.TestCommand != "go test" {
		t.Errorf("expected 'go test', got %q", base.Verify.TestCommand)
	}
}

func TestMergeConfig_InstructionsConfig(t *testing.T) {
	t.Parallel()

	base := &Config{Instructions: InstructionsConfig{Enabled: false}}
	overlay := &Config{Instructions: InstructionsConfig{Enabled: true}}

	defined := map[string]bool{"instructions.enabled": true}
	MergeConfig(base, overlay, defined)

	if !base.Instructions.Enabled {
		t.Error("expected Enabled true")
	}
}

func TestMergeConfig_SkillsConfig(t *testing.T) {
	t.Parallel()

	base := &Config{Skills: SkillsConfig{Sources: []string{"~/.config/skills"}}}
	overlay := &Config{Skills: SkillsConfig{Sources: []string{"./skills", "./team-skills"}}}

	MergeConfig(base, overlay, nil)

	if len(base.Skills.Sources) != 2 {
		t.Fatalf("expected 2 sources, got %d", len(base.Skills.Sources))
	}
	if base.Skills.Sources[0] != "./skills" {
		t.Errorf("expected './skills', got %q", base.Skills.Sources[0])
	}
}

// B18: Zero-value override works when key is explicitly defined
func TestMergeConfig_ZeroIntOverride(t *testing.T) {
	t.Parallel()

	base := &Config{UI: UIConfig{MaxIterations: 50}}
	overlay := &Config{UI: UIConfig{MaxIterations: 0}}
	defined := map[string]bool{"ui.max_iterations": true}

	MergeConfig(base, overlay, defined)

	if base.UI.MaxIterations != 0 {
		t.Errorf("expected 0 (explicit zero override), got %d", base.UI.MaxIterations)
	}
}

func TestMergeConfig_ZeroFloatOverride(t *testing.T) {
	t.Parallel()

	base := &Config{Model: ModelConfig{ArbitrageThreshold: 0.5}}
	overlay := &Config{Model: ModelConfig{ArbitrageThreshold: 0}}
	defined := map[string]bool{"model.arbitrage_threshold": true}

	MergeConfig(base, overlay, defined)

	if base.Model.ArbitrageThreshold != 0 {
		t.Errorf("expected 0 (explicit zero override), got %f", base.Model.ArbitrageThreshold)
	}
}

func TestMergeConfig_ZeroIntPreservedWhenUndefined(t *testing.T) {
	t.Parallel()

	base := &Config{UI: UIConfig{MaxIterations: 50}}
	overlay := &Config{UI: UIConfig{MaxIterations: 0}}
	// defined is nil — key not in overlay, so base should be preserved

	MergeConfig(base, overlay, nil)

	if base.UI.MaxIterations != 50 {
		t.Errorf("expected 50 preserved (undefined zero), got %d", base.UI.MaxIterations)
	}
}

func TestMergeConfig_ProviderFallbackPriority(t *testing.T) {
	t.Parallel()

	base := &Config{Provider: ProviderConfig{
		FallbackPriority:        []string{"openrouter", "zen"},
		HealthCheckTimeoutSecs:  5,
		RegistrationOrder:       []string{"zen", "nvidia"},
	}}
	overlay := &Config{Provider: ProviderConfig{
		FallbackPriority:        []string{"nvidia", "zen", "openrouter"},
		HealthCheckTimeoutSecs:  15,
		RegistrationOrder:       []string{"openrouter", "nvidia", "zen"},
	}}

	MergeConfig(base, overlay, nil)

	if len(base.Provider.FallbackPriority) != 3 {
		t.Fatalf("expected 3 fallback priorities, got %d", len(base.Provider.FallbackPriority))
	}
	if base.Provider.FallbackPriority[0] != "nvidia" {
		t.Errorf("expected 'nvidia' first, got %q", base.Provider.FallbackPriority[0])
	}
	if base.Provider.HealthCheckTimeoutSecs != 15 {
		t.Errorf("expected 15, got %d", base.Provider.HealthCheckTimeoutSecs)
	}
	if len(base.Provider.RegistrationOrder) != 3 {
		t.Fatalf("expected 3 registration order, got %d", len(base.Provider.RegistrationOrder))
	}
}

func TestMergeConfig_NarrativeConfig(t *testing.T) {
	t.Parallel()

	base := &Config{Narrative: NarrativeConfig{
		TemplateOverrides:       map[string]string{"task_complete": "Done!"},
		ClassificationOverrides: map[string]string{"tool_call": "expanded"},
	}}
	overlay := &Config{Narrative: NarrativeConfig{
		TemplateOverrides:       map[string]string{"task_complete": "Finished!", "error": "Oops!"},
		ClassificationOverrides: map[string]string{"tool_call": "hidden"},
	}}

	MergeConfig(base, overlay, nil)

	if base.Narrative.TemplateOverrides["task_complete"] != "Finished!" {
		t.Errorf("expected 'Finished!', got %q", base.Narrative.TemplateOverrides["task_complete"])
	}
	if base.Narrative.TemplateOverrides["error"] != "Oops!" {
		t.Errorf("expected 'Oops!' merged in, got %q", base.Narrative.TemplateOverrides["error"])
	}
	if base.Narrative.ClassificationOverrides["tool_call"] != "hidden" {
		t.Errorf("expected 'hidden', got %q", base.Narrative.ClassificationOverrides["tool_call"])
	}
}

func TestMergeConfig_ModelCapabilitiesConfig(t *testing.T) {
	t.Parallel()

	base := &Config{ModelCapabilities: ModelCapabilitiesConfig{
		ExtraReasoningPatterns: []string{"pattern_a"},
		KnownCapabilities: map[string]ModelCapabilityOverride{
			"gpt-4": {ContextLength: 8192},
		},
	}}
	overlay := &Config{ModelCapabilities: ModelCapabilitiesConfig{
		ExtraReasoningPatterns:     []string{"pattern_b"},
		ExtraToolCapablePatterns:   []string{"tool_pattern"},
		ExtraCompletionOnlyPatterns: []string{"completion_pattern"},
		ExtraNonChatPatterns:       []string{"non_chat_pattern"},
		KnownCapabilities: map[string]ModelCapabilityOverride{
			"claude-3": {ContextLength: 200000, SupportsReasoning: true},
		},
	}}

	MergeConfig(base, overlay, nil)

	if len(base.ModelCapabilities.ExtraReasoningPatterns) != 1 {
		t.Errorf("expected 1 reasoning pattern (overlay replaces), got %d", len(base.ModelCapabilities.ExtraReasoningPatterns))
	}
	if base.ModelCapabilities.ExtraReasoningPatterns[0] != "pattern_b" {
		t.Errorf("expected 'pattern_b', got %q", base.ModelCapabilities.ExtraReasoningPatterns[0])
	}
	if len(base.ModelCapabilities.ExtraToolCapablePatterns) != 1 {
		t.Errorf("expected 1 tool capable pattern, got %d", len(base.ModelCapabilities.ExtraToolCapablePatterns))
	}
	if len(base.ModelCapabilities.ExtraCompletionOnlyPatterns) != 1 {
		t.Errorf("expected 1 completion only pattern, got %d", len(base.ModelCapabilities.ExtraCompletionOnlyPatterns))
	}
	if len(base.ModelCapabilities.ExtraNonChatPatterns) != 1 {
		t.Errorf("expected 1 non chat pattern, got %d", len(base.ModelCapabilities.ExtraNonChatPatterns))
	}
	// KnownCapabilities should have both gpt-4 and claude-3
	if len(base.ModelCapabilities.KnownCapabilities) != 2 {
		t.Fatalf("expected 2 known capabilities, got %d", len(base.ModelCapabilities.KnownCapabilities))
	}
	if base.ModelCapabilities.KnownCapabilities["gpt-4"].ContextLength != 8192 {
		t.Error("expected gpt-4 preserved")
	}
	if base.ModelCapabilities.KnownCapabilities["claude-3"].ContextLength != 200000 {
		t.Error("expected claude-3 merged")
	}
}

func TestMergeConfig_PromptConfig(t *testing.T) {
	t.Parallel()

	base := &Config{Prompts: PromptConfig{
		SystemPromptFile:  "/base/system.md",
		ProjectPromptDir:  "/base/prompts",
		Overrides:         map[string]string{"execute-task": "/base/execute.md"},
	}}
	overlay := &Config{Prompts: PromptConfig{
		SystemPromptFile:          "/overlay/system.md",
		GlobalPromptDir:           "/overlay/global",
		Overrides:                 map[string]string{"execute-task": "/overlay/execute.md", "plan": "/overlay/plan.md"},
		ModelTemplateOverrides:    map[string]string{"mistral": "/overlay/mistral.txt"},
	}}

	MergeConfig(base, overlay, nil)

	if base.Prompts.SystemPromptFile != "/overlay/system.md" {
		t.Errorf("expected '/overlay/system.md', got %q", base.Prompts.SystemPromptFile)
	}
	if base.Prompts.ProjectPromptDir != "/base/prompts" {
		t.Errorf("expected '/base/prompts' preserved, got %q", base.Prompts.ProjectPromptDir)
	}
	if base.Prompts.GlobalPromptDir != "/overlay/global" {
		t.Errorf("expected '/overlay/global', got %q", base.Prompts.GlobalPromptDir)
	}
	if base.Prompts.Overrides["execute-task"] != "/overlay/execute.md" {
		t.Errorf("expected override merged, got %q", base.Prompts.Overrides["execute-task"])
	}
	if base.Prompts.Overrides["plan"] != "/overlay/plan.md" {
		t.Errorf("expected new override merged, got %q", base.Prompts.Overrides["plan"])
	}
	if len(base.Prompts.ModelTemplateOverrides) != 1 {
		t.Errorf("expected 1 model template override, got %d", len(base.Prompts.ModelTemplateOverrides))
	}
}

func TestMergeConfig_TemplateConfig(t *testing.T) {
	t.Parallel()

	base := &Config{Templates: TemplateConfig{
		ExternalDir:     "/base/templates",
		WebsiteFramework: "vue",
		CustomPalettes: map[string]map[string]string{
			"my_palette": {"primary": "#000"},
		},
	}}
	overlay := &Config{Templates: TemplateConfig{
		ExternalDir:     "/overlay/templates",
		WebsiteFramework: "svelte",
		CustomPalettes: map[string]map[string]string{
			"my_palette":  {"primary": "#FFF", "secondary": "#CCC"},
			"new_palette": {"accent": "#00F"},
		},
	}}

	MergeConfig(base, overlay, nil)

	if base.Templates.ExternalDir != "/overlay/templates" {
		t.Errorf("expected '/overlay/templates', got %q", base.Templates.ExternalDir)
	}
	if base.Templates.WebsiteFramework != "svelte" {
		t.Errorf("expected 'svelte', got %q", base.Templates.WebsiteFramework)
	}
	if base.Templates.CustomPalettes["my_palette"]["primary"] != "#FFF" {
		t.Errorf("expected '#FFF' merged, got %q", base.Templates.CustomPalettes["my_palette"]["primary"])
	}
	if base.Templates.CustomPalettes["my_palette"]["secondary"] != "#CCC" {
		t.Errorf("expected '#CCC' merged, got %q", base.Templates.CustomPalettes["my_palette"]["secondary"])
	}
	if base.Templates.CustomPalettes["new_palette"]["accent"] != "#00F" {
		t.Errorf("expected new palette merged, got %q", base.Templates.CustomPalettes["new_palette"]["accent"])
	}
}

func TestMergeConfig_FeaturesMissingFields(t *testing.T) {
	t.Parallel()

	base := &Config{Features: FeaturesConfig{
		MaxHealAttempts:           3,
		MaxPlanRetries:            2,
		ContextTruncationThreshold: 0.8,
		RetryMaxAttempts:          5,
		RetryBaseDelayMs:          100,
		RetryMaxDelayMs:           5000,
		RetryBackoffMultiplier:    2.0,
		MaxRetryAfterSecs:         30,
		MaxParallelTasks:          4,
		CoordinatorTimeoutSecs:    60,
	}}
	overlay := &Config{Features: FeaturesConfig{
		MaxHealAttempts:           5,
		MaxPlanRetries:            3,
		ContextTruncationThreshold: 0.9,
		RetryMaxAttempts:          10,
		RetryBaseDelayMs:          200,
		RetryMaxDelayMs:           10000,
		RetryBackoffMultiplier:    3.0,
		MaxRetryAfterSecs:         60,
		MaxParallelTasks:          8,
		CoordinatorTimeoutSecs:    120,
	}}

	defined := map[string]bool{
		"features.max_heal_attempts":            true,
		"features.max_plan_retries":             true,
		"features.context_truncation_threshold": true,
		"features.retry_max_attempts":           true,
		"features.retry_base_delay_ms":          true,
		"features.retry_max_delay_ms":           true,
		"features.retry_backoff_multiplier":     true,
		"features.max_retry_after_secs":         true,
		"features.max_parallel_tasks":           true,
		"features.coordinator_timeout_secs":     true,
	}
	MergeConfig(base, overlay, defined)

	if base.Features.MaxHealAttempts != 5 {
		t.Errorf("expected 5, got %d", base.Features.MaxHealAttempts)
	}
	if base.Features.MaxPlanRetries != 3 {
		t.Errorf("expected 3, got %d", base.Features.MaxPlanRetries)
	}
	if base.Features.ContextTruncationThreshold != 0.9 {
		t.Errorf("expected 0.9, got %f", base.Features.ContextTruncationThreshold)
	}
	if base.Features.RetryMaxAttempts != 10 {
		t.Errorf("expected 10, got %d", base.Features.RetryMaxAttempts)
	}
	if base.Features.RetryBackoffMultiplier != 3.0 {
		t.Errorf("expected 3.0, got %f", base.Features.RetryBackoffMultiplier)
	}
	if base.Features.MaxParallelTasks != 8 {
		t.Errorf("expected 8, got %d", base.Features.MaxParallelTasks)
	}
}

func TestMergeConfig_ToolsMissingFields(t *testing.T) {
	t.Parallel()

	base := &Config{Tools: ToolsConfig{
		RateLimitBurst:           10,
		RateLimitPerSec:          5,
		DangerousRateLimitBurst:  3,
		DangerousRateLimitPerSec: 1,
		MaxConcurrent:            4,
		OutputRetentionDays:      7,
		DnsCacheTTLSecs:          300,
		FuzzyThreshold:           0.8,
		MinLinesForFuzzy:         50,
		BashMaxTimeoutSecs:       60,
		WebfetchMaxRetries:       3,
		WebfetchRetryDelayMs:     1000,
		MaxToolConcurrency:       4,
		LoopDetectWindow:         10,
	}}
	overlay := &Config{Tools: ToolsConfig{
		RateLimitBurst:           20,
		RateLimitPerSec:          10,
		DangerousRateLimitBurst:  5,
		DangerousRateLimitPerSec: 2,
		MaxConcurrent:            8,
		OutputRetentionDays:      30,
		DnsCacheTTLSecs:          600,
		FuzzyThreshold:           0.9,
		MinLinesForFuzzy:         100,
		BashMaxTimeoutSecs:       120,
		WebfetchMaxRetries:       5,
		WebfetchRetryDelayMs:     2000,
		MaxToolConcurrency:       8,
		LoopDetectWindow:         20,
		AdditionalBlockedCommands: []string{"docker rm"},
		AdditionalObfuscationPatterns: []string{"base64"},
	}}

	defined := map[string]bool{
		"tools.rate_limit_burst":                 true,
		"tools.rate_limit_per_sec":              true,
		"tools.dangerous_rate_limit_burst":      true,
		"tools.dangerous_rate_limit_per_sec":    true,
		"tools.max_concurrent":                  true,
		"tools.output_retention_days":           true,
		"tools.dns_cache_ttl_secs":              true,
		"tools.fuzzy_threshold":                 true,
		"tools.min_lines_for_fuzzy":             true,
		"tools.bash_max_timeout_secs":           true,
		"tools.webfetch_max_retries":            true,
		"tools.webfetch_retry_delay_ms":         true,
		"tools.max_tool_concurrency":            true,
		"tools.loop_detect_window":              true,
		"tools.additional_blocked_commands":     true,
		"tools.additional_obfuscation_patterns": true,
	}
	MergeConfig(base, overlay, defined)

	if base.Tools.RateLimitBurst != 20 {
		t.Errorf("expected 20, got %d", base.Tools.RateLimitBurst)
	}
	if base.Tools.MaxConcurrent != 8 {
		t.Errorf("expected 8, got %d", base.Tools.MaxConcurrent)
	}
	if base.Tools.FuzzyThreshold != 0.9 {
		t.Errorf("expected 0.9, got %f", base.Tools.FuzzyThreshold)
	}
	if len(base.Tools.AdditionalBlockedCommands) != 1 {
		t.Errorf("expected 1 blocked command, got %d", len(base.Tools.AdditionalBlockedCommands))
	}
	if base.Tools.AdditionalBlockedCommands[0] != "docker rm" {
		t.Errorf("expected 'docker rm', got %q", base.Tools.AdditionalBlockedCommands[0])
	}
}

func TestMergeConfig_CompactionMissingFields(t *testing.T) {
	t.Parallel()

	base := &Config{Compaction: CompactionConfig{
		SummaryTemplate:     "old template",
		SummaryTemplateFile: "/old/path.md",
	}}
	overlay := &Config{Compaction: CompactionConfig{
		SummaryTemplate:     "new template",
		SummaryTemplateFile: "/new/path.md",
	}}

	MergeConfig(base, overlay, nil)

	if base.Compaction.SummaryTemplate != "new template" {
		t.Errorf("expected 'new template', got %q", base.Compaction.SummaryTemplate)
	}
	if base.Compaction.SummaryTemplateFile != "/new/path.md" {
		t.Errorf("expected '/new/path.md', got %q", base.Compaction.SummaryTemplateFile)
	}
}

func TestMergeConfig_VerifyLintCommand(t *testing.T) {
	t.Parallel()

	base := &Config{Verify: VerifyConfig{
		BuildCommand: "go build",
		TestCommand:  "go test",
		LintCommand:  "golangci-lint",
	}}
	overlay := &Config{Verify: VerifyConfig{
		LintCommand: "staticcheck",
	}}

	MergeConfig(base, overlay, nil)

	if base.Verify.LintCommand != "staticcheck" {
		t.Errorf("expected 'staticcheck', got %q", base.Verify.LintCommand)
	}
	if base.Verify.BuildCommand != "go build" {
		t.Errorf("expected 'go build' preserved, got %q", base.Verify.BuildCommand)
	}
}
