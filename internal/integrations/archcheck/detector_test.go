package archcheck

import (
	"testing"

	"github.com/eshanized/M31A/internal/integrations/codeintel"
)

func TestDetectViolations_NoViolations(t *testing.T) {
	// Simple graph with no violations
	graph := codeintel.NewCodeGraph()
	graph.AddNode("internal/core/a.go", []string{"internal/core/b.go"}, "go")
	graph.AddNode("internal/core/b.go", []string{}, "go")

	rules := DefaultArchRules()
	violations := DetectViolations(graph, rules)

	if len(violations) != 0 {
		t.Errorf("expected no violations, got %d: %v", len(violations), violations)
	}
}

func TestDetectViolations_ForbiddenImport(t *testing.T) {
	// domain -> engine import (not allowed)
	graph := codeintel.NewCodeGraph()
	graph.AddNode("internal/core/domain.go", []string{"internal/engine/workflow.go"}, "go")
	graph.AddNode("internal/engine/workflow.go", []string{}, "go")

	rules := DefaultArchRules()
	violations := DetectViolations(graph, rules)

	found := false
	for _, v := range violations {
		if v.Type == ViolationForbiddenImport {
			found = true
			if v.Severity != SeverityError {
				t.Errorf("forbidden_import should be error severity, got %v", v.Severity)
			}
			if v.From != "internal/core/domain.go" {
				t.Errorf("from should be internal/core/domain.go, got %s", v.From)
			}
			if v.To != "internal/engine/workflow.go" {
				t.Errorf("to should be internal/engine/workflow.go, got %s", v.To)
			}
		}
	}
	if !found {
		t.Error("expected forbidden_import violation")
	}
}

func TestDetectViolations_AllowedCrossLayerImport(t *testing.T) {
	// engine -> domain import (explicitly allowed)
	graph := codeintel.NewCodeGraph()
	graph.AddNode("internal/engine/workflow.go", []string{"internal/core/domain.go"}, "go")
	graph.AddNode("internal/core/domain.go", []string{}, "go")

	rules := DefaultArchRules()
	violations := DetectViolations(graph, rules)

	hasForbidden := false
	for _, v := range violations {
		if v.Type == ViolationForbiddenImport {
			hasForbidden = true
		}
	}
	if hasForbidden {
		t.Error("engine -> domain should be allowed, but got forbidden_import violation")
	}
}

func TestDetectViolations_CircularDep(t *testing.T) {
	// Circular dependency: A -> B -> A
	graph := codeintel.NewCodeGraph()
	graph.AddNode("a.go", []string{"b.go"}, "go")
	graph.AddNode("b.go", []string{"a.go"}, "go")

	rules := DefaultArchRules()
	violations := DetectViolations(graph, rules)

	found := false
	for _, v := range violations {
		if v.Type == ViolationCircularDep {
			found = true
			if v.Severity != SeverityError {
				t.Errorf("circular_dep should be error severity, got %v", v.Severity)
			}
		}
	}
	if !found {
		t.Error("expected circular_dep violation")
	}
}

func TestDetectViolations_MultipleViolations(t *testing.T) {
	// Graph with both forbidden import and circular dep
	graph := codeintel.NewCodeGraph()
	// Circular: a.go <-> b.go
	graph.AddNode("a.go", []string{"b.go"}, "go")
	graph.AddNode("b.go", []string{"a.go"}, "go")
	// Forbidden: domain -> engine
	graph.AddNode("internal/core/domain.go", []string{"internal/engine/workflow.go"}, "go")
	graph.AddNode("internal/engine/workflow.go", []string{}, "go")

	rules := DefaultArchRules()
	violations := DetectViolations(graph, rules)

	forbiddenCount := 0
	circularCount := 0
	for _, v := range violations {
		switch v.Type {
		case ViolationForbiddenImport:
			forbiddenCount++
		case ViolationCircularDep:
			circularCount++
		}
	}

	if forbiddenCount != 1 {
		t.Errorf("expected 1 forbidden_import, got %d", forbiddenCount)
	}
	if circularCount != 1 {
		t.Errorf("expected 1 circular_dep, got %d", circularCount)
	}
}

func TestDetectViolations_CircularDepThreeNode(t *testing.T) {
	// Three-node cycle: A -> B -> C -> A
	graph := codeintel.NewCodeGraph()
	graph.AddNode("a.go", []string{"b.go"}, "go")
	graph.AddNode("b.go", []string{"c.go"}, "go")
	graph.AddNode("c.go", []string{"a.go"}, "go")

	rules := DefaultArchRules()
	violations := DetectViolations(graph, rules)

	circularCount := 0
	for _, v := range violations {
		if v.Type == ViolationCircularDep {
			circularCount++
		}
	}
	if circularCount != 1 {
		t.Errorf("expected 1 circular_dep for 3-node cycle, got %d", circularCount)
	}
}

func TestDetectViolations_SameLayerNoViolation(t *testing.T) {
	// Imports within the same layer should not trigger violations
	graph := codeintel.NewCodeGraph()
	graph.AddNode("internal/core/a.go", []string{"internal/core/b.go"}, "go")
	graph.AddNode("internal/core/b.go", []string{}, "go")

	rules := DefaultArchRules()
	violations := DetectViolations(graph, rules)

	forbiddenCount := 0
	for _, v := range violations {
		if v.Type == ViolationForbiddenImport {
			forbiddenCount++
		}
	}
	if forbiddenCount != 0 {
		t.Errorf("same-layer import should not trigger forbidden_import, got %d", forbiddenCount)
	}
}

func TestDetectViolations_IntelligenceToDomainAllowed(t *testing.T) {
	// intelligence -> domain is allowed
	graph := codeintel.NewCodeGraph()
	graph.AddNode("internal/integrations/codeintel/parser.go", []string{"internal/core/domain.go"}, "go")
	graph.AddNode("internal/core/domain.go", []string{}, "go")

	rules := DefaultArchRules()
	violations := DetectViolations(graph, rules)

	hasForbidden := false
	for _, v := range violations {
		if v.Type == ViolationForbiddenImport {
			hasForbidden = true
		}
	}
	if hasForbidden {
		t.Error("intelligence -> domain should be allowed")
	}
}

func TestDetectViolations_DomainToIntelligenceForbidden(t *testing.T) {
	// domain -> intelligence is NOT allowed (reverse)
	graph := codeintel.NewCodeGraph()
	graph.AddNode("internal/core/domain.go", []string{"internal/integrations/codeintel/parser.go"}, "go")
	graph.AddNode("internal/integrations/codeintel/parser.go", []string{}, "go")

	rules := DefaultArchRules()
	violations := DetectViolations(graph, rules)

	found := false
	for _, v := range violations {
		if v.Type == ViolationForbiddenImport {
			found = true
		}
	}
	if !found {
		t.Error("domain -> intelligence should be forbidden")
	}
}

func TestCircularDepViolation(t *testing.T) {
	cycle := []string{"a.go", "b.go", "c.go"}
	rules := DefaultArchRules()

	v := CircularDepViolation(cycle, rules)

	if v.Type != ViolationCircularDep {
		t.Errorf("type = %v, want %v", v.Type, ViolationCircularDep)
	}
	if v.Severity != SeverityError {
		t.Errorf("severity = %v, want %v", v.Severity, SeverityError)
	}
	if v.From != "a.go" {
		t.Errorf("from = %s, want a.go", v.From)
	}
	if v.To != "c.go" {
		t.Errorf("to = %s, want c.go", v.To)
	}
	expectedMsg := "circular dependency detected: a.go -> b.go -> c.go -> a.go"
	if v.Message != expectedMsg {
		t.Errorf("message = %s, want %s", v.Message, expectedMsg)
	}
}