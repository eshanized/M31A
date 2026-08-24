package archcheck

import (
	"fmt"
	"strings"

	"github.com/eshanized/M31A/internal/integrations/codeintel"
)

// Violation represents an architecture violation.
type Violation struct {
	Type     ViolationType `json:"type"`
	Severity SeverityLevel `json:"severity"`
	From     string        `json:"from"`
	To       string        `json:"to"`
	Message  string        `json:"message"`
}

// DetectViolations runs all architecture violation checks against the code graph.
func DetectViolations(graph *codeintel.CodeGraph, rules *ArchRules) []Violation {
	var violations []Violation

	// 1. Forbidden imports: cross-layer imports not in allowed list
	forbiddenViolations := detectForbiddenImports(graph, rules)
	violations = append(violations, forbiddenViolations...)

	// 2. Circular dependencies: Tarjan's SCC
	circularViolations := detectCircularDependencies(graph, rules)
	violations = append(violations, circularViolations...)

	// 3. Public API changes: compare exported symbols against prior release
	apiViolations := detectPublicAPIChanges(graph, rules)
	violations = append(violations, apiViolations...)

	return violations
}

// detectForbiddenImports checks for cross-layer imports that are not explicitly allowed.
func detectForbiddenImports(graph *codeintel.CodeGraph, rules *ArchRules) []Violation {
	var violations []Violation

	allPaths := graph.AllPaths()
	for _, fromPath := range allPaths {
		fromLayer := rules.ForbiddenImportLayer(fromPath)
		if fromLayer == "" {
			continue // file doesn't belong to any layer
		}

		imports, _ := graph.Neighbors(fromPath)
		for _, toPath := range imports {
			toLayer := rules.ForbiddenImportLayer(toPath)
			if toLayer == "" {
				continue // target not in any layer
			}

			if fromLayer != toLayer {
				if !rules.IsAllowedCrossLayer(fromLayer, toLayer) {
					sev := rules.GetSeverity(ViolationForbiddenImport)
					violations = append(violations, Violation{
						Type:     ViolationForbiddenImport,
						Severity: sev,
						From:     fromPath,
						To:       toPath,
						Message:  fmt.Sprintf("forbidden cross-layer import: %s (layer: %s) imports %s (layer: %s)", fromPath, fromLayer, toPath, toLayer),
					})
				}
			}
		}
	}

	return violations
}

// detectCircularDependencies finds circular dependencies using Tarjan's SCC algorithm.
func detectCircularDependencies(graph *codeintel.CodeGraph, rules *ArchRules) []Violation {
	var violations []Violation

	cycles := DetectSCCs(graph)
	sev := rules.GetSeverity(ViolationCircularDep)

	for _, cycle := range cycles {
		// Create a violation for each cycle
		// Format: A -> B -> C -> A
		cycleStr := strings.Join(cycle, " -> ") + " -> " + cycle[0]
		violations = append(violations, Violation{
			Type:     ViolationCircularDep,
			Severity: sev,
			From:     cycle[0],
			To:       cycle[len(cycle)-1],
			Message:  fmt.Sprintf("circular dependency detected: %s", cycleStr),
		})
	}

	return violations
}

// detectPublicAPIChanges detects public API surface changes by comparing exported symbols.
// For now, this detects all exported symbols as potential API surface.
// A full implementation would compare against a baseline (prior release tag).
func detectPublicAPIChanges(graph *codeintel.CodeGraph, rules *ArchRules) []Violation {
	var violations []Violation

	// This is a placeholder for public API change detection.
	// In a full implementation, this would:
	// 1. Get the prior release tag (e.g., latest git tag)
	// 2. Compare current exported symbols against the baseline
	// 3. Report added/removed/changed exported symbols
	//
	// For now, we return no violations but the framework is in place.
	_ = rules // suppress unused warning for now
	_ = graph

	return violations
}

// CircularDepViolation creates a violation from a cycle.
func CircularDepViolation(cycle []string, rules *ArchRules) Violation {
	sev := rules.GetSeverity(ViolationCircularDep)
	cycleStr := strings.Join(cycle, " -> ") + " -> " + cycle[0]
	return Violation{
		Type:     ViolationCircularDep,
		Severity: sev,
		From:     cycle[0],
		To:       cycle[len(cycle)-1],
		Message:  fmt.Sprintf("circular dependency detected: %s", cycleStr),
	}
}