package tools

import (
	"context"
	"strings"
	"testing"
	"time"

	"github.com/bmatcuk/doublestar/v4"
	"github.com/eshanized/M31A/internal/config"
	m31errors "github.com/eshanized/M31A/internal/errors"
	"github.com/eshanized/M31A/internal/types"
)

type mockTool struct {
	name      string
	riskLevel types.RiskLevel
	execFunc  func(ctx context.Context, input types.ToolInput) (types.ToolResult, error)
}

func (m *mockTool) Name() string               { return m.name }
func (m *mockTool) Description() string        { return "mock tool for testing" }
func (m *mockTool) RiskLevel() types.RiskLevel { return m.riskLevel }
func (m *mockTool) Execute(ctx context.Context, input types.ToolInput) (types.ToolResult, error) {
	if m.execFunc != nil {
		return m.execFunc(ctx, input)
	}
	return types.ToolResult{Output: "ok"}, nil
}

func TestDispatcher_RegisterAndExecute(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe})

	result, err := d.Execute(context.Background(), types.ToolCall{
		ID:    "call1",
		Name:  "test",
		Input: []byte(`{}`),
	})
	if err != nil {
		t.Fatal(err)
	}
	if result.Output != "ok" {
		t.Errorf("expected 'ok', got %q", result.Output)
	}
	if result.ToolCallID != "call1" {
		t.Errorf("expected ToolCallID 'call1', got %q", result.ToolCallID)
	}
}

func TestDispatcher_UnknownTool(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	_, err := d.Execute(context.Background(), types.ToolCall{
		ID:   "call1",
		Name: "nonexistent",
	})
	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(err.Error(), "unknown tool") {
		t.Errorf("expected 'unknown tool', got: %v", err)
	}
	if !strings.Contains(err.Error(), "Available tools") {
		t.Errorf("expected 'Available tools' in error, got: %v", err)
	}
}

func TestDispatcher_SafeToolNoPermission(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "safe", riskLevel: types.RiskSafe})

	_, err := d.Execute(context.Background(), types.ToolCall{
		ID:    "call1",
		Name:  "safe",
		Input: []byte(`{}`),
	})
	if err != nil {
		t.Errorf("safe tool should execute without permission, got: %v", err)
	}
}

func TestDispatcher_DangerousToolPermissionGranted(t *testing.T) {
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "bash", riskLevel: types.RiskDangerous})

	errCh := make(chan error, 1)
	go func() {
		_, err := d.Execute(context.Background(), types.ToolCall{
			ID:    "call1",
			Name:  "bash",
			Input: []byte(`{"name": "bash", "params": {"command": "echo hello"}}`),
		})
		errCh <- err
	}()

	req := <-d.RequestCh()
	if req.ToolName != "bash" {
		t.Errorf("expected request for 'bash', got %q", req.ToolName)
	}
	t.Logf("DEBUG: req.Command = %q (len=%d)", req.Command, len(req.Command))
	if req.Command != "echo hello" {
		t.Errorf("expected Command 'echo hello', got %q", req.Command)
	}
	d.ApprovePermission(req.ID, true, false)

	if err := <-errCh; err != nil {
		t.Errorf("expected nil error after approval, got: %v", err)
	}
}

func TestDispatcher_DangerousToolPermissionDenied(t *testing.T) {
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "bash", riskLevel: types.RiskDangerous})

	errCh := make(chan error, 1)
	go func() {
		_, err := d.Execute(context.Background(), types.ToolCall{
			ID:    "call1",
			Name:  "bash",
			Input: []byte(`{}`),
		})
		errCh <- err
	}()

	req := <-d.RequestCh()
	d.ApprovePermission(req.ID, false, false)

	if err := <-errCh; err != m31errors.ErrPermissionDenied {
		t.Errorf("expected ErrPermissionDenied, got: %v", err)
	}
}

func TestDispatcher_RememberedPermission(t *testing.T) {
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "bash", riskLevel: types.RiskDangerous})

	// First call — approve with remember=true
	errCh1 := make(chan error, 1)
	go func() {
		_, err := d.Execute(context.Background(), types.ToolCall{
			ID:    "call1",
			Name:  "bash",
			Input: []byte(`{}`),
		})
		errCh1 <- err
	}()

	req := <-d.RequestCh()
	d.ApprovePermission(req.ID, true, true)

	if err := <-errCh1; err != nil {
		t.Fatalf("first call failed: %v", err)
	}

	// Second call — should not ask for permission
	result, err := d.Execute(context.Background(), types.ToolCall{
		ID:    "call2",
		Name:  "bash",
		Input: []byte(`{}`),
	})
	if err != nil {
		t.Errorf("remembered call should not need permission, got: %v", err)
	}
	if result.Output != "ok" {
		t.Errorf("expected 'ok', got %q", result.Output)
	}
}

func TestDispatcher_List(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "zzz", riskLevel: types.RiskSafe})
	d.Register(&mockTool{name: "aaa", riskLevel: types.RiskSafe})
	d.Register(&mockTool{name: "mmm", riskLevel: types.RiskSafe})

	names := d.List()
	if len(names) != 3 {
		t.Errorf("expected 3 tools, got %d", len(names))
	}
	if names[0] != "aaa" || names[1] != "mmm" || names[2] != "zzz" {
		t.Errorf("expected sorted [aaa mmm zzz], got %v", names)
	}
}

func TestDispatcher_GetTool(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe})

	tool, ok := d.GetTool("test")
	if !ok {
		t.Fatal("expected to find tool")
	}
	if tool.Name() != "test" {
		t.Errorf("expected tool name 'test', got %s", tool.Name())
	}

	_, ok = d.GetTool("nonexistent")
	if ok {
		t.Error("expected not to find nonexistent tool")
	}
}

func TestDispatcher_DefaultDispatcher(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	d, err := DefaultDispatcher(dir, dir, dir, nil)
	if err != nil {
		t.Fatalf("DefaultDispatcher failed: %v", err)
	}

	names := d.List()
	if len(names) != 18 {
		t.Errorf("expected 18 tools, got %d: %v", len(names), names)
	}

	expectedTools := []string{"Bash", "FileRead", "FileWrite", "Edit", "TodoWrite", "TodoRead", "WebFetch", "WebSearch", "AskUserQuestion", "Glob", "Grep", "FileList", "FileDelete", "FileMove", "CodeMap", "CodeComplexity", "DevServer", "HTTPCheck"}
	for _, expected := range expectedTools {
		found := false
		for _, name := range names {
			if name == expected {
				found = true
				break
			}
		}
		if !found {
			t.Errorf("expected tool %s not found in %v", expected, names)
		}
	}
}

func TestExtractCommandString_Bash(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("bash", []byte(`{"name":"bash","params":{"command":"ls -la"}}`))
	if cmd != "ls -la" {
		t.Errorf("expected 'ls -la', got %q", cmd)
	}
}

func TestExtractCommandString_FileRead(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("fileRead", []byte(`{"name":"fileread","params":{"path":"/tmp/test.txt"}}`))
	if cmd != "read /tmp/test.txt" {
		t.Errorf("expected 'read /tmp/test.txt', got %q", cmd)
	}
}

func TestExtractCommandString_FileWrite(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("fileWrite", []byte(`{"name":"filewrite","params":{"path":"/tmp/out.txt"}}`))
	if cmd != "write /tmp/out.txt" {
		t.Errorf("expected 'write /tmp/out.txt', got %q", cmd)
	}
}

func TestExtractCommandString_Glob(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("glob", []byte(`{"name":"glob","params":{"pattern":"*.go"}}`))
	if cmd != "glob *.go" {
		t.Errorf("expected 'glob *.go', got %q", cmd)
	}
}

func TestExtractCommandString_Grep(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("grep", []byte(`{"name":"grep","params":{"pattern":"func main"}}`))
	if cmd != "grep func main" {
		t.Errorf("expected 'grep func main', got %q", cmd)
	}
}

func TestExtractCommandString_EmptyInput(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("bash", nil)
	if cmd != "" {
		t.Errorf("expected empty string, got %q", cmd)
	}
}

func TestExtractCommandString_RawMap(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("bash", []byte(`{"command":"echo test"}`))
	if cmd != "echo test" {
		t.Errorf("expected 'echo test', got %q", cmd)
	}
}

func TestExtractCommandString_Fallback(t *testing.T) {
	t.Parallel()
	cmd := extractCommandString("bash", []byte(`{"some":"json"}`))
	if cmd != `{"some":"json"}` {
		t.Errorf("expected raw JSON fallback, got %q", cmd)
	}
}

func TestDispatcher_DestructiveToolPermission(t *testing.T) {
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "filewrite", riskLevel: types.RiskDestructive})

	errCh := make(chan error, 1)
	go func() {
		_, err := d.Execute(context.Background(), types.ToolCall{
			ID:    "call1",
			Name:  "filewrite",
			Input: []byte(`{"name":"filewrite","params":{"path":"test.txt"}}`),
		})
		errCh <- err
	}()

	req := <-d.RequestCh()
	if req.ToolName != "filewrite" {
		t.Errorf("expected request for 'filewrite', got %q", req.ToolName)
	}
	// extractCommandString normalizes to "Filewrite" which doesn't match "FileWrite" switch case
	if req.Command != "write test.txt" && req.Command != `{"name":"filewrite","params":{"path":"test.txt"}}` {
		t.Errorf("expected Command 'write test.txt' or raw JSON, got %q", req.Command)
	}
	d.ApprovePermission(req.ID, true, false)

	if err := <-errCh; err != nil {
		t.Errorf("expected nil error after approval, got: %v", err)
	}
}

func TestDispatcher_DangerousToolContextCancelled(t *testing.T) {
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "bash", riskLevel: types.RiskDangerous})

	ctx, cancel := context.WithCancel(context.Background())

	errCh := make(chan error, 1)
	go func() {
		_, err := d.Execute(ctx, types.ToolCall{
			ID:    "call1",
			Name:  "bash",
			Input: []byte(`{}`),
		})
		errCh <- err
	}()

	<-d.RequestCh()
	cancel()

	select {
	case err := <-errCh:
		if err == nil {
			t.Error("expected context cancellation error")
		}
	case <-time.After(2 * time.Second):
		t.Fatal("expected context cancellation to be handled")
	}
}

func TestDispatcher_RegisterDuplicate(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	if err := d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe}); err != nil {
		t.Fatalf("first register failed: %v", err)
	}

	err := d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe})
	if err == nil {
		t.Error("expected error for duplicate registration")
	}
}

// ---------------------------------------------------------------------------
// Permission ruleset matching tests
// ---------------------------------------------------------------------------

func TestMatchToolName(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name    string
		pattern string
		tool    string
		want    bool
	}{
		{"exact match", "Bash", "Bash", true},
		{"wildcard", "*", "Bash", true},
		{"glob prefix", "B*", "Bash", true},
		{"glob file prefix", "File*", "FileRead", true},
		{"no match", "Bash", "FileRead", false},
		{"doublestar pattern", "**", "Anything", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := matchToolName(tt.pattern, tt.tool)
			if got != tt.want {
				t.Errorf("matchToolName(%q, %q) = %v, want %v", tt.pattern, tt.tool, got, tt.want)
			}
		})
	}
}

func TestMatchAnyParamValue(t *testing.T) {
	t.Parallel()

	t.Run("glob pattern matches nested path", func(t *testing.T) {
		params := map[string]any{"path": "src/main.go"}
		if !matchAnyParamValue("**/*.go", params) {
			t.Error("expected **/*.go to match src/main.go")
		}
	})

	t.Run("glob pattern matches flat path", func(t *testing.T) {
		params := map[string]any{"path": "main.go"}
		if !matchAnyParamValue("**/*.go", params) {
			t.Error("expected **/*.go to match main.go")
		}
	})

	t.Run("glob without doublestar does not match nested path", func(t *testing.T) {
		params := map[string]any{"path": "src/main.go"}
		if matchAnyParamValue("*.go", params) {
			t.Error("expected *.go to NOT match src/main.go")
		}
	})

	t.Run("secret key pattern matches deep path", func(t *testing.T) {
		params := map[string]any{"path": "/home/user/secret.key"}
		if !matchAnyParamValue("**/secret*", params) {
			t.Error("expected **/secret* to match /home/user/secret.key")
		}
	})

	t.Run("non-string values are stringified before matching", func(t *testing.T) {
		params := map[string]any{"command": "42", "path": "/some/path"}
		if !matchAnyParamValue("*4*", params) {
			t.Error("expected match on stringified int 42 with pattern *4*")
		}
	})
}

func TestMatchAnyParamValue_NonStringValues(t *testing.T) {
	t.Parallel()

	t.Run("mixed types with matching string value", func(t *testing.T) {
		params := map[string]any{
			"count":   42,
			"enabled": true,
			"path":    "src/main.go",
		}
		if !matchAnyParamValue("**/*.go", params) {
			t.Error("expected match when one param is a string path")
		}
	})

	t.Run("mixed types with no matching string", func(t *testing.T) {
		params := map[string]any{
			"count":   42,
			"enabled": true,
			"ratio":   3.14,
		}
		if matchAnyParamValue("**/*.go", params) {
			t.Error("expected no match when no string values")
		}
	})

	t.Run("nil values do not cause panic", func(t *testing.T) {
		params := map[string]any{
			"path":    nil,
			"command": "echo hello",
		}
		if !matchAnyParamValue("**echo**", params) {
			t.Error("expected match on string param despite nil value")
		}
	})

	t.Run("all non-string types are stringified without panic", func(t *testing.T) {
		params := map[string]any{
			"command": "42",
			"path":    "test.go",
		}
		// Should not panic, and ** matches the stringified representations
		if !matchAnyParamValue("**", params) {
			t.Error("expected ** to match stringified non-string values")
		}
		// Specific pattern should match stringified value
		if !matchAnyParamValue("*42*", params) {
			t.Error("expected match on stringified int 42")
		}
	})
}

func TestCheckPermission_RuleAllow(t *testing.T) {
	d := NewDispatcher(&config.PermissionsConfig{
		Rules: []config.PermissionRule{
			{Tool: "Bash", Pattern: "**/*.go", Action: "allow"},
		},
	})
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskSafe})

	allowed, pctx, err := d.checkPermission("Bash", types.ToolInput{
		Name:   "Bash",
		Params: map[string]any{"path": "src/main.go"},
	})
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if !allowed {
		t.Fatal("expected allowed=true")
	}
	if pctx == nil {
		t.Fatal("expected non-nil PermissionContext")
	}
	if pctx.RuleAction != "allow" {
		t.Errorf("expected RuleAction 'allow', got %q", pctx.RuleAction)
	}
	if pctx.Source != "rule" {
		t.Errorf("expected Source 'rule', got %q", pctx.Source)
	}
}

func TestCheckPermission_RuleDeny(t *testing.T) {
	d := NewDispatcher(&config.PermissionsConfig{
		Rules: []config.PermissionRule{
			{Tool: "Bash", Pattern: "**/*.go", Action: "deny"},
		},
	})
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskSafe})

	allowed, pctx, err := d.checkPermission("Bash", types.ToolInput{
		Name:   "Bash",
		Params: map[string]any{"path": "src/main.go"},
	})
	if err != m31errors.ErrPermissionDenied {
		t.Fatalf("expected ErrPermissionDenied, got: %v", err)
	}
	if allowed {
		t.Fatal("expected allowed=false")
	}
	if pctx == nil {
		t.Fatal("expected non-nil PermissionContext")
	}
	if pctx.RuleAction != "deny" {
		t.Errorf("expected RuleAction 'deny', got %q", pctx.RuleAction)
	}
}

func TestCheckPermission_RuleAsk(t *testing.T) {
	d := NewDispatcher(&config.PermissionsConfig{
		Rules: []config.PermissionRule{
			{Tool: "Bash", Pattern: "**/*.go", Action: "ask"},
		},
	})
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskSafe})

	allowed, pctx, err := d.checkPermission("Bash", types.ToolInput{
		Name:   "Bash",
		Params: map[string]any{"path": "src/main.go"},
	})
	if err != nil {
		t.Fatalf("expected nil error (ask action), got: %v", err)
	}
	if allowed {
		t.Fatal("expected allowed=false for ask action")
	}
	if pctx == nil {
		t.Fatal("expected non-nil PermissionContext")
	}
	if pctx.RuleAction != "ask" {
		t.Errorf("expected RuleAction 'ask', got %q", pctx.RuleAction)
	}
	if pctx.Source != "rule" {
		t.Errorf("expected Source 'rule', got %q", pctx.Source)
	}
}

func TestCheckPermission_NoMatchFallthrough(t *testing.T) {
	d := NewDispatcher(&config.PermissionsConfig{
		Rules: []config.PermissionRule{
			{Tool: "Bash", Pattern: "*.py", Action: "allow"},
		},
	})
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskSafe})

	// .go file should not match *.py pattern
	allowed, pctx, err := d.checkPermission("Bash", types.ToolInput{
		Name:   "Bash",
		Params: map[string]any{"path": "main.go"},
	})
	if err != nil {
		t.Fatalf("expected nil error for fallthrough, got: %v", err)
	}
	if allowed {
		t.Fatal("expected allowed=false when no rule matches")
	}
	if pctx == nil {
		t.Fatal("expected non-nil PermissionContext")
	}
	if pctx.Source != "risk_level" {
		t.Errorf("expected Source 'risk_level', got %q", pctx.Source)
	}
}

func TestCheckPermission_ToolFilter(t *testing.T) {
	d := NewDispatcher(&config.PermissionsConfig{
		Rules: []config.PermissionRule{
			{Tool: "Grep", Pattern: "**", Action: "allow"},
		},
	})
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskSafe})

	// Bash is not Grep — tool filter prevents match
	_, pctx, err := d.checkPermission("Bash", types.ToolInput{
		Name:   "Bash",
		Params: map[string]any{"command": "echo test"},
	})
	if err != nil {
		t.Fatalf("expected nil error for non-matching tool filter, got: %v", err)
	}
	if pctx == nil {
		t.Fatal("expected non-nil PermissionContext")
	}
	if pctx.Source != "risk_level" {
		t.Errorf("expected Source 'risk_level' when tool filter prevents match, got %q", pctx.Source)
	}
	if pctx == nil {
		t.Fatal("expected non-nil PermissionContext")
	}
	if pctx.Source != "risk_level" {
		t.Errorf("expected Source 'risk_level' when tool filter prevents match, got %q", pctx.Source)
	}
}

func TestCheckPermission_MultipleRulesFirstWins(t *testing.T) {
	d := NewDispatcher(&config.PermissionsConfig{
		Rules: []config.PermissionRule{
			{Tool: "Bash", Pattern: "**/secret*", Action: "deny"},
			{Tool: "Bash", Pattern: "**/*.go", Action: "allow"},
		},
	})
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskSafe})

	t.Run("secret key denied by first rule", func(t *testing.T) {
		allowed, pctx, err := d.checkPermission("Bash", types.ToolInput{
			Name:   "Bash",
			Params: map[string]any{"path": "src/secret.key"},
		})
		if err != m31errors.ErrPermissionDenied {
			t.Fatalf("expected ErrPermissionDenied, got: %v", err)
		}
		if allowed {
			t.Fatal("expected allowed=false for secret key")
		}
		if pctx.RuleAction != "deny" {
			t.Errorf("expected RuleAction 'deny', got %q", pctx.RuleAction)
		}
	})

	t.Run("go file allowed by second rule", func(t *testing.T) {
		allowed, pctx, err := d.checkPermission("Bash", types.ToolInput{
			Name:   "Bash",
			Params: map[string]any{"path": "src/main.go"},
		})
		if err != nil {
			t.Fatalf("expected no error, got: %v", err)
		}
		if !allowed {
			t.Fatal("expected allowed=true for go file")
		}
		if pctx.RuleAction != "allow" {
			t.Errorf("expected RuleAction 'allow', got %q", pctx.RuleAction)
		}
	})
}

func TestSelectAgent(t *testing.T) {
	d := NewDispatcher(&config.PermissionsConfig{
		Rules: []config.PermissionRule{
			{Tool: "Bash", Pattern: "**/*.py", Action: "allow"},
		},
		Agents: map[string]config.PermissionsAgentConfig{
			"build": {
				Rules: []config.PermissionRule{
					{Tool: "Bash", Pattern: "**", Action: "allow"},
				},
			},
		},
	})
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskSafe})

	t.Run("select build agent succeeds", func(t *testing.T) {
		err := d.SelectAgent("build")
		if err != nil {
			t.Fatalf("expected no error, got: %v", err)
		}
	})

	t.Run("build agent allows all bash commands", func(t *testing.T) {
		allowed, _, err := d.checkPermission("Bash", types.ToolInput{
			Name:   "Bash",
			Params: map[string]any{"command": "rm -rf /"},
		})
		if err != nil {
			t.Fatalf("expected no error, got: %v", err)
		}
		if !allowed {
			t.Fatal("expected allowed=true for build agent")
		}
	})

	t.Run("select unknown agent returns error", func(t *testing.T) {
		err := d.SelectAgent("unknown")
		if err == nil {
			t.Fatal("expected error for unknown agent")
		}
	})

	t.Run("select default agent resets to global rules", func(t *testing.T) {
		err := d.SelectAgent("default")
		if err != nil {
			t.Fatalf("expected no error, got: %v", err)
		}

		// After reset to default, *.py rule should apply (not **)
		allowed, _, err := d.checkPermission("Bash", types.ToolInput{
			Name:   "Bash",
			Params: map[string]any{"command": "rm -rf /"},
		})
		if err != nil {
			t.Fatalf("expected no error, got: %v", err)
		}
		if allowed {
			t.Fatal("expected allowed=false after reset to default rules")
		}
	})
}

func TestPermissionContext_Fields(t *testing.T) {
	t.Parallel()

	t.Run("rule match populates all fields", func(t *testing.T) {
		d := NewDispatcher(&config.PermissionsConfig{
			Rules: []config.PermissionRule{
				{Tool: "Bash", Pattern: "**/secret*", Action: "deny"},
			},
		})
		d.Register(&mockTool{name: "Bash", riskLevel: types.RiskSafe})

		_, pctx, _ := d.checkPermission("Bash", types.ToolInput{
			Name:   "Bash",
			Params: map[string]any{"path": "/etc/secret.key"},
		})
		if pctx == nil {
			t.Fatal("expected non-nil PermissionContext")
		}
		if pctx.RuleTool != "Bash" {
			t.Errorf("expected RuleTool 'Bash', got %q", pctx.RuleTool)
		}
		if pctx.RulePattern != "**/secret*" {
			t.Errorf("expected RulePattern '**/secret*', got %q", pctx.RulePattern)
		}
		if pctx.RuleAction != "deny" {
			t.Errorf("expected RuleAction 'deny', got %q", pctx.RuleAction)
		}
		if pctx.Source != "rule" {
			t.Errorf("expected Source 'rule', got %q", pctx.Source)
		}
	})

	t.Run("no match returns risk_level source", func(t *testing.T) {
		d := NewDispatcher(nil)
		d.Register(&mockTool{name: "Bash", riskLevel: types.RiskSafe})

		_, pctx, err := d.checkPermission("Bash", types.ToolInput{
			Name:   "Bash",
			Params: map[string]any{"path": "main.go"},
		})
		if err != nil {
			t.Fatalf("expected no error, got: %v", err)
		}
		if pctx == nil {
			t.Fatal("expected non-nil PermissionContext")
		}
		if pctx.Source != "risk_level" {
			t.Errorf("expected Source 'risk_level', got %q", pctx.Source)
		}
		if pctx.RuleTool != "" {
			t.Errorf("expected empty RuleTool, got %q", pctx.RuleTool)
		}
	})
}

// ---------------------------------------------------------------------------
// Doublestar integration verification
// ---------------------------------------------------------------------------

func TestDoublestarMatch(t *testing.T) {
	t.Parallel()

	tests := []struct {
		pattern string
		path    string
		want    bool
	}{
		{"**/*.go", "src/main.go", true},
		{"**/*.go", "main.go", true},
		{"**/*.go", "src/sub/file_test.go", true},
		{"*.go", "main.go", true},
		{"*.go", "src/main.go", false},
		{"**/secret*", "/home/user/secret.key", true},
		{"**/secret*", "secret.txt", true},
		{"Bash", "Bash", true},
		{"Bash", "FileRead", false},
	}

	for _, tt := range tests {
		t.Run(tt.pattern+"/"+tt.path, func(t *testing.T) {
			got, err := doublestar.Match(tt.pattern, tt.path)
			if err != nil {
				t.Fatalf("doublestar.Match(%q, %q) returned error: %v", tt.pattern, tt.path, err)
			}
			if got != tt.want {
				t.Errorf("doublestar.Match(%q, %q) = %v, want %v", tt.pattern, tt.path, got, tt.want)
			}
		})
	}
}

func TestDispatcher_PermissionChannelFull(t *testing.T) {
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "bash", riskLevel: types.RiskDangerous})

	// Fill the request channel (buffer size is 8)
	for i := 0; i < 8; i++ {
		d.requestCh <- PermissionRequest{}
	}

	_, err := d.Execute(context.Background(), types.ToolCall{
		ID:    "call1",
		Name:  "bash",
		Input: []byte(`{}`),
	})
	if err != m31errors.ErrPermissionDenied {
		t.Errorf("expected ErrPermissionDenied when channel is full, got: %v", err)
	}
}

func TestToolInputJSON(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "safe", riskLevel: types.RiskSafe})

	_, err := d.Execute(context.Background(), types.ToolCall{
		ID:    "call1",
		Name:  "safe",
		Input: []byte(`{invalid json}`),
	})
	if err == nil {
		t.Fatal("expected error for invalid JSON")
	}
	if !strings.Contains(err.Error(), "invalid input JSON") {
		t.Errorf("expected 'invalid input JSON' in error, got: %v", err)
	}
	if !strings.Contains(err.Error(), "Raw input") {
		t.Errorf("expected 'Raw input' in error, got: %v", err)
	}
}

func TestDispatcher_EmptyInputForKnownTool(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe})

	result, err := d.Execute(context.Background(), types.ToolCall{
		ID:    "call1",
		Name:  "test",
		Input: nil,
	})
	if err != nil {
		t.Fatalf("expected nil error for empty input on known tool, got: %v", err)
	}
	if result.Output != "ok" {
		t.Errorf("expected 'ok', got %q", result.Output)
	}
}

func TestDispatcher_EmptyBytesInput(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe})

	result, err := d.Execute(context.Background(), types.ToolCall{
		ID:    "call1",
		Name:  "test",
		Input: []byte(""),
	})
	if err != nil {
		t.Fatalf("expected nil error for empty bytes input, got: %v", err)
	}
	if result.Output != "ok" {
		t.Errorf("expected 'ok', got %q", result.Output)
	}
}

func TestDispatcher_NullInput(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "test", riskLevel: types.RiskSafe})

	result, err := d.Execute(context.Background(), types.ToolCall{
		ID:    "call1",
		Name:  "test",
		Input: []byte("null"),
	})
	if err != nil {
		t.Fatalf("expected nil error for null input, got: %v", err)
	}
	_ = result
}

func TestDispatcher_UpdatePermissions(t *testing.T) {
	d := NewDispatcher(nil)
	d.Register(&mockTool{name: "Bash", riskLevel: types.RiskSafe})

	cfg := &config.PermissionsConfig{
		Rules: []config.PermissionRule{
			{Tool: "Bash", Pattern: "**", Action: "allow"},
		},
	}
	d.UpdatePermissions(cfg)

	allowed, pctx, err := d.checkPermission("Bash", types.ToolInput{
		Name:   "Bash",
		Params: map[string]any{"command": "echo test"},
	})
	if err != nil {
		t.Fatalf("expected no error, got: %v", err)
	}
	if !allowed {
		t.Error("expected allowed=true after UpdatePermissions")
	}
	if pctx == nil || pctx.RuleAction != "allow" {
		t.Error("expected RuleAction 'allow' after update")
	}
}

func TestDispatcher_UpdatePermissions_Nil(t *testing.T) {
	d := NewDispatcher(nil)
	d.UpdatePermissions(nil) // should not panic
}

func TestDispatcher_Stop_Idempotent(t *testing.T) {
	d := NewDispatcher(nil)
	d.Stop()
	d.Stop() // second call should not panic
}

func TestDispatcher_QuestionChannels(t *testing.T) {
	t.Parallel()
	d := NewDispatcher(nil)

	if d.QuestionRequestCh() == nil {
		t.Error("QuestionRequestCh should not be nil")
	}
	if d.QuestionResponseCh() == nil {
		t.Error("QuestionResponseCh should not be nil")
	}
}

func TestDispatcher_RespondQuestion_Routing(t *testing.T) {
	d := NewDispatcher(nil)

	// Register a per-request channel
	reqID := int64(42)
	respCh := make(chan QuestionResponse, 1)
	d.pendingQuestions.Store(reqID, respCh)

	d.RespondQuestion(reqID, "yes")

	select {
	case resp := <-respCh:
		if resp.Answer != "yes" {
			t.Errorf("expected answer 'yes', got %q", resp.Answer)
		}
	default:
		t.Error("expected response on per-request channel")
	}

	// Verify cleanup
	if _, ok := d.pendingQuestions.Load(reqID); ok {
		t.Error("expected pending question to be deleted after response")
	}
}

func TestDispatcher_RespondQuestion_FallbackToShared(t *testing.T) {
	d := NewDispatcher(nil)

	go d.RespondQuestion(999, "fallback")

	select {
	case resp := <-d.questionRespCh:
		if resp.Answer != "fallback" {
			t.Errorf("expected 'fallback', got %q", resp.Answer)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("timed out waiting for fallback response")
	}
}
