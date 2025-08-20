package tools

import (
	"testing"

	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

// ---------------------------------------------------------------------------
// Agent-based permission tests (complementary to dispatcher_test.go)
// ---------------------------------------------------------------------------

func TestCheckPermission_AgentDefaultAllow(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(&config.PermissionsConfig{
		Agents: map[string]config.PermissionsAgentConfig{
			"autonomous": {DefaultAction: "allow"},
		},
	})
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskDangerous})

	if err := d.SelectAgent("autonomous"); err != nil {
		t.Fatalf("SelectAgent failed: %v", err)
	}
	defer d.SelectAgent("default")

	allowed, pctx, err := d.checkPermission("Bash", types.ToolInput{
		Name:   "Bash",
		Params: map[string]any{"command": "echo test"},
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !allowed {
		t.Error("expected agent default to allow")
	}
	if pctx.Source != "agent_default" {
		t.Errorf("expected source 'agent_default', got %q", pctx.Source)
	}
}

func TestCheckPermission_AgentDefaultDeny(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(&config.PermissionsConfig{
		Agents: map[string]config.PermissionsAgentConfig{
			"restricted": {DefaultAction: "deny"},
		},
	})
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskDangerous})

	if err := d.SelectAgent("restricted"); err != nil {
		t.Fatalf("SelectAgent failed: %v", err)
	}
	defer d.SelectAgent("default")

	allowed, _, err := d.checkPermission("Bash", types.ToolInput{
		Name:   "Bash",
		Params: map[string]any{"command": "echo test"},
	})
	if allowed {
		t.Error("expected agent default to deny")
	}
	if err != m31errors.ErrPermissionDenied {
		t.Errorf("expected ErrPermissionDenied, got %v", err)
	}
}

func TestCheckPermission_RuleOverridesAgentDefault(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(&config.PermissionsConfig{
		Rules: []config.PermissionRule{
			{Tool: "Bash", Pattern: "**", Action: "deny"},
		},
		Agents: map[string]config.PermissionsAgentConfig{
			"autonomous": {
				DefaultAction: "allow",
				Rules: []config.PermissionRule{
					{Tool: "Bash", Pattern: "**", Action: "deny"},
				},
			},
		},
	})
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskDangerous})

	if err := d.SelectAgent("autonomous"); err != nil {
		t.Fatalf("SelectAgent failed: %v", err)
	}
	defer d.SelectAgent("default")

	// Agent's own rule should apply (deny overrides agent's default allow)
	allowed, _, err := d.checkPermission("Bash", types.ToolInput{
		Name:   "Bash",
		Params: map[string]any{"path": "main.go"},
	})
	if allowed {
		t.Error("expected agent rule to override agent default")
	}
	if err != m31errors.ErrPermissionDenied {
		t.Errorf("expected ErrPermissionDenied from agent rule, got %v", err)
	}
}

func TestSelectAgent_UnknownAgent(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	err := d.SelectAgent("nonexistent")
	if err == nil {
		t.Error("expected error for unknown agent")
	}
}

func TestSelectAgent_ResetToDefault(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(&config.PermissionsConfig{
		Rules: []config.PermissionRule{
			{Tool: "Bash", Action: "allow"},
		},
		Agents: map[string]config.PermissionsAgentConfig{
			"custom": {DefaultAction: "deny"},
		},
	})
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskDangerous})

	// Switch to custom agent — custom deny should apply
	d.SelectAgent("custom")
	allowed, _, _ := d.checkPermission("Bash", types.ToolInput{
		Name: "Bash", Params: map[string]any{"command": "echo"},
	})
	if allowed {
		t.Error("expected deny from custom agent default")
	}

	// Reset to default — original rule should be restored
	d.SelectAgent("default")
	allowed2, _, _ := d.checkPermission("Bash", types.ToolInput{
		Name: "Bash", Params: map[string]any{"command": "echo"},
	})
	if !allowed2 {
		t.Error("expected allow from original rule after reset")
	}
}

// ---------------------------------------------------------------------------
// Glob pattern matching edge cases
// ---------------------------------------------------------------------------

func TestMatchToolName_GlobEdgeCases(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name     string
		pattern  string
		toolName string
		want     bool
	}{
		{"exact match", "Bash", "Bash", true},
		{"case sensitive", "bash", "Bash", false},
		{"wildcard matches all", "*", "Anything", true},
		{"doublestar prefix", "**/Bash", "Bash", true},
		{"question mark single char", "Ba?h", "Bash", true},
		{"question mark no match", "Ba??h", "Bas", false},
		{"bracket pattern", "[BF]*", "Bash", true},
		{"bracket pattern match FileRead", "[BF]*", "FileRead", true},
		{"bracket no match", "[A-Z]ash", "bash", false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			got := matchToolName(tt.pattern, tt.toolName)
			if got != tt.want {
				t.Errorf("matchToolName(%q, %q) = %v, want %v", tt.pattern, tt.toolName, got, tt.want)
			}
		})
	}
}

func TestMatchAnyParamValue_CommandGlob(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name    string
		pattern string
		params  map[string]any
		want    bool
	}{
		{"rm command matches **", "**", map[string]any{"command": "rm -rf /tmp"}, true},
		{"echo does not match rm pattern", "rm*", map[string]any{"command": "echo hello"}, false},
		{"rm with no args", "rm", map[string]any{"command": "rm"}, true},
		{"empty command matches *", "*", map[string]any{"command": ""}, true},
		{"git command with spaces", "git *", map[string]any{"command": "git status"}, true},
		{"int param stringified", "*42*", map[string]any{"command": "42"}, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			got := matchAnyParamValue(tt.pattern, tt.params)
			if got != tt.want {
				t.Errorf("matchAnyParamValue(%q, %v) = %v, want %v", tt.pattern, tt.params, got, tt.want)
			}
		})
	}
}

// ---------------------------------------------------------------------------
// Permission check flow with agent profiles
// ---------------------------------------------------------------------------

func TestCheckPermission_AgentWithRules(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(&config.PermissionsConfig{
		Rules: []config.PermissionRule{
			{Tool: "Bash", Action: "ask"},
		},
		Agents: map[string]config.PermissionsAgentConfig{
			"safe-agent": {
				DefaultAction: "allow",
				Rules: []config.PermissionRule{
					{Tool: "Bash", Action: "allow"},
				},
			},
		},
	})
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskDangerous})

	// Without agent: should ask
	allowed, _, _ := d.checkPermission("Bash", types.ToolInput{
		Name: "Bash", Params: map[string]any{"command": "echo"},
	})
	if allowed {
		t.Error("expected ask (not allowed) without agent")
	}

	// With safe-agent: agent rules should apply
	d.SelectAgent("safe-agent")
	defer d.SelectAgent("default")

	allowed2, pctx2, _ := d.checkPermission("Bash", types.ToolInput{
		Name: "Bash", Params: map[string]any{"command": "echo"},
	})
	if !allowed2 {
		t.Error("expected allow from agent rule")
	}
	if pctx2.Source != "rule" {
		t.Errorf("expected source 'rule' from agent rule, got %q", pctx2.Source)
	}
}

func TestCheckPermission_ConcurrentAccess(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(&config.PermissionsConfig{
		Rules: []config.PermissionRule{
			{Tool: "Bash", Action: "allow"},
		},
	})
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskDangerous})

	// Run concurrent permission checks
	done := make(chan struct{}, 10)
	for i := 0; i < 10; i++ {
		go func() {
			defer func() { done <- struct{}{} }()
			allowed, _, err := d.checkPermission("Bash", types.ToolInput{
				Name: "Bash", Params: map[string]any{"command": "echo"},
			})
			if err != nil {
				t.Errorf("unexpected error: %v", err)
			}
			if !allowed {
				t.Error("expected allowed")
			}
		}()
	}
	for i := 0; i < 10; i++ {
		<-done
	}
}

func TestExtractCommandString(t *testing.T) {
	t.Parallel()
	tests := []struct {
		name     string
		toolName string
		input    string
		want     string
	}{
		{"bash command", "Bash", `{"name":"Bash","params":{"command":"echo hello"}}`, "echo hello"},
		{"fileread path", "FileRead", `{"name":"FileRead","params":{"path":"main.go"}}`, "read main.go"},
		{"filewrite path", "FileWrite", `{"name":"FileWrite","params":{"path":"out.txt"}}`, "write out.txt"},
		{"glob pattern", "Glob", `{"name":"Glob","params":{"pattern":"*.go"}}`, "glob *.go"},
		{"grep pattern", "Grep", `{"name":"Grep","params":{"pattern":"TODO"}}`, "grep TODO"},
		{"empty input", "Bash", "", ""},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			got := extractCommandString(tt.toolName, []byte(tt.input))
			if got != tt.want {
				t.Errorf("extractCommandString(%q, %q) = %q, want %q", tt.toolName, tt.input, got, tt.want)
			}
		})
	}
}
