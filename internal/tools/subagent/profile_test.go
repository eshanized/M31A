package subagent

import (
	"testing"

	"github.com/eshanized/M31A/internal/config"
)

// --- profile.go ---

func TestBuiltinProfiles_Count(t *testing.T) {
	t.Parallel()
	profiles := BuiltinProfiles()
	if len(profiles) != 6 {
		t.Errorf("expected 6 builtin profiles, got %d", len(profiles))
	}
}

func TestBuiltinProfiles_ExpectedNames(t *testing.T) {
	t.Parallel()
	profiles := BuiltinProfiles()
	expected := []string{"build", "plan", "general", "explore", "security", "review"}
	for _, name := range expected {
		if _, ok := profiles[name]; !ok {
			t.Errorf("missing builtin profile: %s", name)
		}
	}
}

func TestBuiltinProfiles_BuildMode(t *testing.T) {
	t.Parallel()
	profiles := BuiltinProfiles()
	b := profiles["build"]
	if b.Mode != ModePrimary {
		t.Errorf("build mode = %q, want %q", b.Mode, ModePrimary)
	}
	if b.SystemPrompt == "" {
		t.Error("build should have a system prompt")
	}
}

func TestBuiltinProfiles_PlanDeniedTools(t *testing.T) {
	t.Parallel()
	profiles := BuiltinProfiles()
	p := profiles["plan"]
	if len(p.DeniedTools) == 0 {
		t.Error("plan should have denied tools")
	}
	for _, dt := range p.DeniedTools {
		if dt == "" {
			t.Error("plan has empty denied tool name")
		}
	}
}

func TestBuiltinProfiles_ExploreAllowedTools(t *testing.T) {
	t.Parallel()
	profiles := BuiltinProfiles()
	e := profiles["explore"]
	if len(e.AllowedTools) == 0 {
		t.Error("explore should have allowed tools")
	}
	if e.Mode != ModeSubagent {
		t.Errorf("explore mode = %q, want %q", e.Mode, ModeSubagent)
	}
}

func TestBuiltinProfiles_SecurityMode(t *testing.T) {
	t.Parallel()
	profiles := BuiltinProfiles()
	s := profiles["security"]
	if s.Mode != ModeSubagent {
		t.Errorf("security mode = %q, want %q", s.Mode, ModeSubagent)
	}
	if len(s.AllowedTools) == 0 {
		t.Error("security should have allowed tools")
	}
	if len(s.DeniedTools) == 0 {
		t.Error("security should have denied tools")
	}
}

func TestBuiltinProfiles_ReviewMode(t *testing.T) {
	t.Parallel()
	profiles := BuiltinProfiles()
	r := profiles["review"]
	if r.Mode != ModeSubagent {
		t.Errorf("review mode = %q, want %q", r.Mode, ModeSubagent)
	}
}

// --- ResolveProfile ---

func TestResolveProfile_BuiltinDefault(t *testing.T) {
	t.Parallel()
	profile, ok := ResolveProfile("build", nil)
	if !ok {
		t.Fatal("expected to find build profile")
	}
	if profile.Name != "build" {
		t.Errorf("name = %q, want build", profile.Name)
	}
	if profile.Mode != ModePrimary {
		t.Errorf("mode = %q, want %q", profile.Mode, ModePrimary)
	}
}

func TestResolveProfile_UnknownNoOverride(t *testing.T) {
	t.Parallel()
	_, ok := ResolveProfile("nonexistent", nil)
	if ok {
		t.Error("expected false for unknown profile with no override")
	}
}

func TestResolveProfile_DisabledOverride(t *testing.T) {
	t.Parallel()
	overrides := map[string]config.SubagentProfileConfig{
		"build": {Disabled: true},
	}
	_, ok := ResolveProfile("build", overrides)
	if ok {
		t.Error("expected false for disabled profile")
	}
}

func TestResolveProfile_CustomProfileFromOverride(t *testing.T) {
	t.Parallel()
	overrides := map[string]config.SubagentProfileConfig{
		"myagent": {
			Description:  "My custom agent",
			Mode:         "subagent",
			SystemPrompt: "custom prompt",
			Model:        "gpt-4",
		},
	}
	profile, ok := ResolveProfile("myagent", overrides)
	if !ok {
		t.Fatal("expected true for custom profile from override")
	}
	if profile.Name != "myagent" {
		t.Errorf("name = %q, want myagent", profile.Name)
	}
	if profile.Mode != ModeSubagent {
		t.Errorf("mode = %q, want %q", profile.Mode, ModeSubagent)
	}
	if profile.Description != "My custom agent" {
		t.Errorf("description = %q", profile.Description)
	}
	if profile.SystemPrompt != "custom prompt" {
		t.Errorf("system prompt = %q", profile.SystemPrompt)
	}
	if profile.Model != "gpt-4" {
		t.Errorf("model = %q", profile.Model)
	}
}

func TestResolveProfile_OverrideBuiltinFields(t *testing.T) {
	t.Parallel()
	hidden := true
	overrides := map[string]config.SubagentProfileConfig{
		"build": {
			Description:  "Overridden description",
			Mode:         "subagent",
			SystemPrompt: "overridden prompt",
			Model:        "claude-3",
			Hidden:       &hidden,
			AllowedTools: []string{"Glob", "Grep"},
			DeniedTools:  []string{"Bash"},
			MaxTools:     5,
			MaxTokens:    1000,
			MaxTurns:     3,
		},
	}
	profile, ok := ResolveProfile("build", overrides)
	if !ok {
		t.Fatal("expected true")
	}
	if profile.Description != "Overridden description" {
		t.Errorf("description = %q", profile.Description)
	}
	if profile.Mode != ModeSubagent {
		t.Errorf("mode = %q", profile.Mode)
	}
	if profile.SystemPrompt != "overridden prompt" {
		t.Errorf("system prompt = %q", profile.SystemPrompt)
	}
	if profile.Model != "claude-3" {
		t.Errorf("model = %q", profile.Model)
	}
	if !profile.Hidden {
		t.Error("expected hidden=true")
	}
	if len(profile.AllowedTools) != 2 {
		t.Errorf("allowed tools = %d, want 2", len(profile.AllowedTools))
	}
	if len(profile.DeniedTools) != 1 {
		t.Errorf("denied tools = %d, want 1", len(profile.DeniedTools))
	}
	if profile.MaxTools != 5 {
		t.Errorf("max tools = %d, want 5", profile.MaxTools)
	}
	if profile.MaxTokens != 1000 {
		t.Errorf("max tokens = %d, want 1000", profile.MaxTokens)
	}
	if profile.MaxTurns != 3 {
		t.Errorf("max turns = %d, want 3", profile.MaxTurns)
	}
}

func TestResolveProfile_OverrideNoChanges(t *testing.T) {
	t.Parallel()
	overrides := map[string]config.SubagentProfileConfig{
		"build": {}, // empty override, no changes
	}
	profile, ok := ResolveProfile("build", overrides)
	if !ok {
		t.Fatal("expected true")
	}
	if profile.Mode != ModePrimary {
		t.Errorf("mode changed unexpectedly: %q", profile.Mode)
	}
}

// --- ListSubagentProfiles ---

func TestListSubagentProfiles_IncludesSubagentMode(t *testing.T) {
	t.Parallel()
	profiles := ListSubagentProfiles(nil)
	names := make(map[string]bool)
	for _, p := range profiles {
		names[p.Name] = true
	}
	// general, explore, security, review are ModeSubagent
	for _, expected := range []string{"general", "explore", "security", "review"} {
		if !names[expected] {
			t.Errorf("expected subagent profile %q in list", expected)
		}
	}
	// build and plan are ModePrimary, should be excluded
	for _, excluded := range []string{"build", "plan"} {
		if names[excluded] {
			t.Errorf("primary profile %q should not appear in subagent list", excluded)
		}
	}
}

func TestListSubagentProfiles_SortedByName(t *testing.T) {
	t.Parallel()
	profiles := ListSubagentProfiles(nil)
	for i := 1; i < len(profiles); i++ {
		if profiles[i-1].Name >= profiles[i].Name {
			t.Errorf("not sorted: %q >= %q at index %d", profiles[i-1].Name, profiles[i].Name, i)
		}
	}
}

func TestListSubagentProfiles_HiddenExcluded(t *testing.T) {
	t.Parallel()
	hidden := true
	overrides := map[string]config.SubagentProfileConfig{
		"explore": {Hidden: &hidden},
	}
	profiles := ListSubagentProfiles(overrides)
	for _, p := range profiles {
		if p.Name == "explore" {
			t.Error("hidden profile 'explore' should be excluded")
		}
	}
}

func TestListSubagentProfiles_DisabledExcluded(t *testing.T) {
	t.Parallel()
	overrides := map[string]config.SubagentProfileConfig{
		"general": {Disabled: true},
	}
	profiles := ListSubagentProfiles(overrides)
	for _, p := range profiles {
		if p.Name == "general" {
			t.Error("disabled profile 'general' should be excluded")
		}
	}
}

func TestListSubagentProfiles_CustomProfileIncluded(t *testing.T) {
	t.Parallel()
	overrides := map[string]config.SubagentProfileConfig{
		"myagent": {Mode: "subagent", Description: "custom"},
	}
	profiles := ListSubagentProfiles(overrides)
	found := false
	for _, p := range profiles {
		if p.Name == "myagent" {
			found = true
			break
		}
	}
	if !found {
		t.Error("custom subagent profile should be included")
	}
}

func TestListSubagentProfiles_EmptyOverrides(t *testing.T) {
	t.Parallel()
	profiles := ListSubagentProfiles(nil)
	if len(profiles) == 0 {
		t.Error("expected non-empty list")
	}
}

// --- profile_filter.go ---

func TestApplyToolFilter_EmptyFilters(t *testing.T) {
	t.Parallel()
	disp := &mockDispatcher{
		tools: []ToolDescriptor{
			{Name: "Bash"}, {Name: "FileRead"}, {Name: "Grep"},
		},
	}
	ApplyToolFilter(disp, AgentProfile{})
	if len(disp.tools) != 3 {
		t.Errorf("expected 3 tools (no filter), got %d", len(disp.tools))
	}
}

func TestApplyToolFilter_AllowedTools(t *testing.T) {
	t.Parallel()
	disp := &mockDispatcher{
		tools: []ToolDescriptor{
			{Name: "Bash"}, {Name: "FileRead"}, {Name: "Grep"}, {Name: "FileWrite"},
		},
	}
	ApplyToolFilter(disp, AgentProfile{
		AllowedTools: []string{"Bash", "Grep"},
	})
	names := make(map[string]bool)
	for _, td := range disp.tools {
		names[td.Name] = true
	}
	if !names["Bash"] {
		t.Error("Bash should be kept")
	}
	if !names["Grep"] {
		t.Error("Grep should be kept")
	}
	if names["FileRead"] {
		t.Error("FileRead should be removed by allowed filter")
	}
	if names["FileWrite"] {
		t.Error("FileWrite should be removed by allowed filter")
	}
}

func TestApplyToolFilter_DeniedTools(t *testing.T) {
	t.Parallel()
	disp := &mockDispatcher{
		tools: []ToolDescriptor{
			{Name: "Bash"}, {Name: "FileRead"}, {Name: "Grep"},
		},
	}
	ApplyToolFilter(disp, AgentProfile{
		DeniedTools: []string{"Bash"},
	})
	names := make(map[string]bool)
	for _, td := range disp.tools {
		names[td.Name] = true
	}
	if names["Bash"] {
		t.Error("Bash should be removed by denied filter")
	}
	if !names["FileRead"] {
		t.Error("FileRead should be kept")
	}
	if !names["Grep"] {
		t.Error("Grep should be kept")
	}
}

func TestApplyToolFilter_AllowedAndDenied(t *testing.T) {
	t.Parallel()
	disp := &mockDispatcher{
		tools: []ToolDescriptor{
			{Name: "Bash"}, {Name: "FileRead"}, {Name: "Grep"}, {Name: "FileWrite"},
		},
	}
	ApplyToolFilter(disp, AgentProfile{
		AllowedTools: []string{"Bash", "Grep", "FileWrite"},
		DeniedTools:  []string{"Bash"},
	})
	// After apply: Bash removed (denied), FileRead removed (not allowed),
	// Grep kept, FileWrite kept
	names := make(map[string]bool)
	for _, td := range disp.tools {
		names[td.Name] = true
	}
	if names["Bash"] {
		t.Error("Bash should be removed (denied overrides allowed)")
	}
	if names["FileRead"] {
		t.Error("FileRead should be removed (not in allowed)")
	}
	if !names["Grep"] {
		t.Error("Grep should be kept")
	}
	if !names["FileWrite"] {
		t.Error("FileWrite should be kept")
	}
}

func TestApplyToolFilter_AllRemoved(t *testing.T) {
	t.Parallel()
	disp := &mockDispatcher{
		tools: []ToolDescriptor{
			{Name: "Bash"}, {Name: "FileRead"},
		},
	}
	ApplyToolFilter(disp, AgentProfile{
		AllowedTools: []string{"Nonexistent"},
	})
	if len(disp.tools) != 0 {
		t.Errorf("expected 0 tools, got %d", len(disp.tools))
	}
}

func TestApplyToolFilter_EmptyDispatcher(t *testing.T) {
	t.Parallel()
	disp := &mockDispatcher{tools: []ToolDescriptor{}}
	ApplyToolFilter(disp, AgentProfile{
		AllowedTools: []string{"Bash"},
	})
	if len(disp.tools) != 0 {
		t.Errorf("expected 0 tools, got %d", len(disp.tools))
	}
}

// --- Mode constants ---

func TestModeConstants(t *testing.T) {
	t.Parallel()
	if ModePrimary != "primary" {
		t.Errorf("ModePrimary = %q", ModePrimary)
	}
	if ModeSubagent != "subagent" {
		t.Errorf("ModeSubagent = %q", ModeSubagent)
	}
	if ModeAll != "all" {
		t.Errorf("ModeAll = %q", ModeAll)
	}
}
