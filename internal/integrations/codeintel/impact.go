package codeintel

import (
	"strings"
)

// RiskCategory classifies the risk level of a symbol for impact analysis.
type RiskCategory string

const (
	// RiskAPI - Exported/public symbols that are part of the public API.
	// Changes to these may break external consumers.
	RiskAPI RiskCategory = "API"

	// RiskRuntime - Internal/unexported symbols used in runtime code.
	// Changes may break internal behavior but not public API.
	RiskRuntime RiskCategory = "runtime"

	// RiskTest - Symbols only used in test files (_test.go).
	// Changes only affect test code.
	RiskTest RiskCategory = "test"
)

// CallerInfo represents a direct caller of a symbol with location and risk category.
type CallerInfo struct {
	File     string       // relative file path where the call occurs
	Line     int          // line number of the call
	Symbol   string       // name of the calling symbol
	Category RiskCategory // risk category of the caller
}

// ImpactResult contains the full impact analysis for a symbol.
type ImpactResult struct {
	Symbol        string       // the analyzed symbol
	DirectCallers []CallerInfo // direct callers with file:line and category
	Indirect      []string     // transitive dependent files (unique)
	AffectedTests []string     // test files that transitively depend on the symbol
	Risk          RiskCategory // risk category of the symbol itself
}

// CategorizeSymbolRisk determines the risk category of a symbol based on its definitions.
func CategorizeSymbolRisk(index *SymbolIndex, symbol string) RiskCategory {
	locs := index.Define(symbol)
	if len(locs) == 0 {
		return RiskRuntime // unknown symbol defaults to runtime
	}

	// Check if any definition is exported
	for _, loc := range locs {
		fileSymbols := index.FileSymbols(loc.File)
		for _, sym := range fileSymbols {
			if sym.Name == symbol && sym.Exported {
				return RiskAPI
			}
		}
	}

	// Check if all definitions are in test files
	allInTestFiles := true
	for _, loc := range locs {
		if !strings.HasSuffix(loc.File, "_test.go") {
			allInTestFiles = false
			break
		}
	}
	if allInTestFiles {
		return RiskTest
	}

	return RiskRuntime
}

// categorizeCallerRisk determines the risk category of a caller based on its file and symbol.
func categorizeCallerRisk(index *SymbolIndex, callerFile, callerSymbol string) RiskCategory {
	// Test file check
	if strings.HasSuffix(callerFile, "_test.go") {
		return RiskTest
	}

	// Check if caller symbol is exported
	locs := index.Define(callerSymbol)
	for _, loc := range locs {
		if loc.File == callerFile {
			fileSymbols := index.FileSymbols(loc.File)
			for _, sym := range fileSymbols {
				if sym.Name == callerSymbol && sym.Exported {
					return RiskAPI
				}
			}
		}
	}

	return RiskRuntime
}

// transitiveCallers returns all transitive callers of a symbol up to maxDepth.
// Uses the call graph (Callees) to find callers of callers.
func transitiveCallers(graph *CodeGraph, symbol string, maxDepth int) map[string]struct{} {
	if maxDepth <= 0 {
		return nil
	}

	visited := make(map[string]struct{})
	type entry struct {
		symbol string
		depth  int
	}
	queue := []entry{{symbol: symbol, depth: 0}}
	result := make(map[string]struct{})

	for front := 0; front < len(queue); front++ {
		cur := queue[front]
		if cur.depth >= maxDepth {
			continue
		}

		// Find direct callers of current symbol
		callers := graph.Callers(cur.symbol)
		for _, edge := range callers {
			key := edge.CallerName + "@" + edge.CallerFile
			if _, seen := visited[key]; seen {
				continue
			}
			visited[key] = struct{}{}
			result[edge.CallerFile] = struct{}{}
			queue = append(queue, entry{symbol: edge.CallerName, depth: cur.depth + 1})
		}
	}

	return result
}

// AnalyzeImpact performs impact analysis on a symbol using the code graph and symbol index.
// It returns direct callers (with file:line), indirect dependents (transitive callers),
// affected tests, and the symbol's risk category.
func AnalyzeImpact(graph *CodeGraph, index *SymbolIndex, symbol string, depth int) *ImpactResult {
	result := &ImpactResult{
		Symbol:        symbol,
		DirectCallers: []CallerInfo{},
		Indirect:      []string{},
		AffectedTests: []string{},
		Risk:          CategorizeSymbolRisk(index, symbol),
	}

	// 1. Find direct callers using the call graph
	callEdges := graph.Callers(symbol)
	callerFilesSet := make(map[string]struct{})
	for _, edge := range callEdges {
		callerFilesSet[edge.CallerFile] = struct{}{}
		category := categorizeCallerRisk(index, edge.CallerFile, edge.CallerName)
		result.DirectCallers = append(result.DirectCallers, CallerInfo{
			File:     edge.CallerFile,
			Line:     edge.CallerLine,
			Symbol:   edge.CallerName,
			Category: category,
		})
	}

	// 2. Find indirect dependents (transitive callers) using call graph traversal
	indirectFiles := transitiveCallers(graph, symbol, depth)
	for file := range indirectFiles {
		// Exclude direct caller files from indirect
		if _, isDirect := callerFilesSet[file]; !isDirect {
			result.Indirect = append(result.Indirect, file)
		}
	}

	// Also include files that import the symbol's definition file (import graph downstream)
	// as they may directly use the symbol
	symbolLocs := index.Define(symbol)
	for _, loc := range symbolLocs {
		downstream := graph.Downstream(loc.File, depth)
		for _, dep := range downstream {
			if dep != loc.File {
				if _, isDirect := callerFilesSet[dep]; !isDirect {
					if _, isIndirect := indirectFiles[dep]; !isIndirect {
						result.Indirect = append(result.Indirect, dep)
					}
				}
			}
		}
	}

	// 3. Find affected tests: test files that transitively depend on the symbol
	testFilesSet := make(map[string]struct{})

	// Test files that directly call the symbol
	for _, edge := range callEdges {
		if strings.HasSuffix(edge.CallerFile, "_test.go") {
			testFilesSet[edge.CallerFile] = struct{}{}
		}
	}

	// Test files in transitive callers
	for file := range indirectFiles {
		if strings.HasSuffix(file, "_test.go") {
			testFilesSet[file] = struct{}{}
		}
	}

	// Test files that import the symbol's definition file
	for _, loc := range symbolLocs {
		downstream := graph.Downstream(loc.File, 0)
		for _, dep := range downstream {
			if strings.HasSuffix(dep, "_test.go") {
				testFilesSet[dep] = struct{}{}
			}
		}

		// Also check for corresponding test file
		testFile := strings.TrimSuffix(loc.File, ".go") + "_test.go"
		if graph.HasNode(testFile) {
			testFilesSet[testFile] = struct{}{}
		}

		// Check for test files in same directory
		dir := ""
		lastSlash := strings.LastIndex(loc.File, "/")
		if lastSlash >= 0 {
			dir = loc.File[:lastSlash+1]
		}
		for nodePath := range graph.nodes {
			if strings.HasPrefix(nodePath, dir) && strings.HasSuffix(nodePath, "_test.go") {
				testFilesSet[nodePath] = struct{}{}
			}
		}
	}

	// Convert test files set to slice
	for tf := range testFilesSet {
		result.AffectedTests = append(result.AffectedTests, tf)
	}

	return result
}