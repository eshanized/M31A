package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"log/slog"
	"os"

	"github.com/eshanized/M31A/internal/integrations/codeintel"
)

// runImpact handles the `m31a impact` command.
func runImpact(args []string, workDir string, logger *slog.Logger) int {
	fs := flag.NewFlagSet("impact", flag.ExitOnError)
	depth := fs.Int("depth", 3, "Traversal depth for transitive dependents")
	format := fs.String("format", "table", "Output format: table, json, graphviz")
	fs.Usage = func() {
		fmt.Fprintln(os.Stderr, "Usage: m31a impact <symbol> [--depth N] [--format table|json|graphviz]")
		fmt.Fprintln(os.Stderr, "  --depth N       Traversal depth (default: 3)")
		fmt.Fprintln(os.Stderr, "  --format FMT    Output format: table, json, graphviz (default: table)")
	}
	fs.Parse(args)

	if fs.NArg() < 1 {
		fmt.Fprintln(os.Stderr, "Error: symbol argument required")
		fs.Usage()
		return 1
	}

	symbol := fs.Arg(0)

	// Create indexer and build
	indexer := codeintel.NewIndexerWithStore(workDir, nil)
	if err := indexer.Build(context.Background()); err != nil {
		logger.Error("index build failed", "error", err)
		fmt.Fprintf(os.Stderr, "Error building index: %v\n", err)
		return 1
	}

	// Get graph and analyze impact
	graph := indexer.Projection().Graph()
	result := codeintel.AnalyzeImpact(graph, indexer.SymbolIndex(), symbol, *depth)

	if result == nil {
		fmt.Printf("No impact data found for symbol %q\n", symbol)
		return 0
	}

	// Format output
	switch *format {
	case "json":
		return outputImpactJSON(result)
	case "graphviz":
		return outputImpactGraphviz(result)
	case "table":
		fallthrough
	default:
		return outputImpactTable(result)
	}
}

// outputImpactTable outputs impact results in table format.
func outputImpactTable(result *codeintel.ImpactResult) int {
	fmt.Printf("Impact Analysis for: %s\n", result.Symbol)
	fmt.Printf("Risk Category: %s\n\n", result.Risk)

	if len(result.DirectCallers) > 0 {
		fmt.Println("Direct Callers:")
		fmt.Println("  FILE:LINE        SYMBOL           CATEGORY")
		for _, caller := range result.DirectCallers {
			fmt.Printf("  %-20s %-15s %s\n", fmt.Sprintf("%s:%d", caller.File, caller.Line), caller.Symbol, caller.Category)
		}
		fmt.Println()
	}

	if len(result.Indirect) > 0 {
		fmt.Println("Indirect Dependents (transitive):")
		for _, dep := range result.Indirect {
			fmt.Printf("  %s\n", dep)
		}
		fmt.Println()
	}

	if len(result.AffectedTests) > 0 {
		fmt.Println("Affected Tests:")
		for _, test := range result.AffectedTests {
			fmt.Printf("  %s\n", test)
		}
		fmt.Println()
	}

	if len(result.DirectCallers) == 0 && len(result.Indirect) == 0 && len(result.AffectedTests) == 0 {
		fmt.Println("No dependents found.")
	}

	return 0
}

// outputImpactJSON outputs impact results as JSON.
func outputImpactJSON(result *codeintel.ImpactResult) int {
	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(result); err != nil {
		fmt.Fprintf(os.Stderr, "Error encoding JSON: %v\n", err)
		return 1
	}
	return 0
}

// outputImpactGraphviz outputs impact results in Graphviz DOT format.
func outputImpactGraphviz(result *codeintel.ImpactResult) int {
	fmt.Println("digraph impact {")
	fmt.Println("  rankdir=LR;")
	fmt.Println("  node [shape=box, style=filled];")

	// Symbol node
	riskColor := "lightblue"
	switch result.Risk {
	case codeintel.RiskAPI:
		riskColor = "lightcoral"
	case codeintel.RiskRuntime:
		riskColor = "lightyellow"
	case codeintel.RiskTest:
		riskColor = "lightgreen"
	}
	fmt.Printf("  \"%s\" [fillcolor=%s, label=\"%s\\n(%s)\"];\n", result.Symbol, riskColor, result.Symbol, result.Risk)

	// Direct callers
	for _, caller := range result.DirectCallers {
		callerColor := "lightblue"
		switch caller.Category {
		case codeintel.RiskAPI:
			callerColor = "lightcoral"
		case codeintel.RiskRuntime:
			callerColor = "lightyellow"
		case codeintel.RiskTest:
			callerColor = "lightgreen"
		}
		nodeName := fmt.Sprintf("%s:%d", caller.File, caller.Line)
		fmt.Printf("  \"%s\" [fillcolor=%s, label=\"%s\\n%s:%d\"];\n", nodeName, callerColor, caller.Symbol, caller.File, caller.Line)
		fmt.Printf("  \"%s\" -> \"%s\";\n", nodeName, result.Symbol)
	}

	// Indirect dependents
	for _, dep := range result.Indirect {
		fmt.Printf("  \"%s\" [fillcolor=lightgray, label=\"%s\"];\n", dep, dep)
		// We don't know the exact edges for indirect, so just connect to symbol
		fmt.Printf("  \"%s\" -> \"%s\" [style=dashed];\n", dep, result.Symbol)
	}

	// Affected tests
	for _, test := range result.AffectedTests {
		fmt.Printf("  \"%s\" [fillcolor=lightgreen, shape=ellipse, label=\"%s\"];\n", test, test)
		fmt.Printf("  \"%s\" -> \"%s\" [style=dotted];\n", test, result.Symbol)
	}

	fmt.Println("}")
	return 0
}