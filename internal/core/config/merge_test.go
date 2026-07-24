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
