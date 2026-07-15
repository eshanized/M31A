package tools

import (
	"testing"

	"github.com/eshanized/M31A/internal/types"
)

func TestBuildToolDefs_IncludesAllRegistered(t *testing.T) {
	t.Parallel()
	d := testDispatcher(t)
	d.Register(&mockTool{name: "Alpha", riskLevel: types.RiskSafe})
	d.Register(&mockTool{name: "Beta", riskLevel: types.RiskMedium})
	d.Register(&mockTool{name: "Gamma", riskLevel: types.RiskDangerous})

	defs := BuildToolDefs(d)
	if len(defs) != 3 {
		t.Fatalf("expected 3 defs, got %d", len(defs))
	}

	nameSet := map[string]bool{}
	for _, def := range defs {
		nameSet[def.Name] = true
		if def.Description == "" {
			t.Errorf("tool %q has empty description", def.Name)
		}
	}
	for _, name := range []string{"Alpha", "Beta", "Gamma"} {
		if !nameSet[name] {
			t.Errorf("missing tool %q in defs", name)
		}
	}
}

func TestBuildToolDefs_WithSchemaProvider(t *testing.T) {
	t.Parallel()
	d := testDispatcher(t)
	bash := NewBash(t.TempDir(), 1800, nil, nil)
	d.Register(bash)

	defs := BuildToolDefs(d)
	if len(defs) != 1 {
		t.Fatalf("expected 1 def, got %d", len(defs))
	}
	def := defs[0]
	if def.Name != "Bash" {
		t.Errorf("expected 'Bash', got %q", def.Name)
	}
	// Bash implements SchemaProvider
	if def.Parameters == "" || def.Parameters == "{}" {
		t.Error("expected Bash to have parameter schema")
	}
}

func TestBuildToolDefs_DefinitionType_Implicit(t *testing.T) {
	t.Parallel()
	d := testDispatcher(t)
	d.Register(&mockTool{name: "Test", riskLevel: types.RiskSafe})
	defs := BuildToolDefs(d)
	if len(defs) != 1 {
		t.Fatalf("expected 1 def, got %d", len(defs))
	}
	// Verify it's a ToolDefinition
	var _ = defs[0]
}
