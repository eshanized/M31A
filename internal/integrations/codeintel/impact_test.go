package codeintel

import (
	"testing"
)

func TestAnalyzeImpact_DirectCallers(t *testing.T) {
	// Build a CodeGraph with known call edges
	graph := NewCodeGraph()
	index := NewSymbolIndex()

	// Add nodes and call edges
	// File A calls function "Foo"
	graph.AddNode("a.go", []string{}, "go")
	graph.AddNode("b.go", []string{}, "go")
	graph.AddNode("c.go", []string{}, "go")

	// Add call edges: Bar (in b.go) calls Foo (in a.go), Baz (in c.go) calls Foo
	graph.AddCallEdge(CallEdge{
		CallerFile: "b.go", CallerLine: 10, CallerName: "Bar",
		CalleeFile: "a.go", CalleeLine: 5, CalleeName: "Foo",
	})
	graph.AddCallEdge(CallEdge{
		CallerFile: "c.go", CallerLine: 20, CallerName: "Baz",
		CalleeFile: "a.go", CalleeLine: 5, CalleeName: "Foo",
	})

	// Add symbols to index
	index.AddFile(&FileInfo{
		Path: "a.go", Language: "go",
		Exports: []SymbolInfo{{Name: "Foo", Kind: "func", Exported: true}},
	})
	index.AddFile(&FileInfo{
		Path: "b.go", Language: "go",
		Exports: []SymbolInfo{{Name: "Bar", Kind: "func", Exported: false}},
	})
	index.AddFile(&FileInfo{
		Path: "c.go", Language: "go",
		Exports: []SymbolInfo{{Name: "Baz", Kind: "func", Exported: false}},
	})

	result := AnalyzeImpact(graph, index, "Foo", 3)

	if len(result.DirectCallers) != 2 {
		t.Fatalf("expected 2 direct callers, got %d: %+v", len(result.DirectCallers), result.DirectCallers)
	}

	// Verify caller info
	foundBar := false
	foundBaz := false
	for _, c := range result.DirectCallers {
		if c.Symbol == "Bar" {
			foundBar = true
			if c.File != "b.go" || c.Line != 10 {
				t.Errorf("Bar caller: expected file=b.go line=10, got file=%s line=%d", c.File, c.Line)
			}
			if c.Category != RiskRuntime {
				t.Errorf("Bar caller category: expected runtime, got %s", c.Category)
			}
		}
		if c.Symbol == "Baz" {
			foundBaz = true
			if c.File != "c.go" || c.Line != 20 {
				t.Errorf("Baz caller: expected file=c.go line=20, got file=%s line=%d", c.File, c.Line)
			}
		}
	}
	if !foundBar || !foundBaz {
		t.Errorf("missing expected callers: foundBar=%v foundBaz=%v", foundBar, foundBaz)
	}
}

func TestAnalyzeImpact_IndirectDependents(t *testing.T) {
	// Build a call graph with transitive callers:
	// Main (main.go) -> Process (util.go) -> Helper (helper.go) -> Load (lib.go)
	//                                              -> Config (config.go)
	graph := NewCodeGraph()
	index := NewSymbolIndex()

	graph.AddNode("main.go", []string{}, "go")
	graph.AddNode("util.go", []string{}, "go")
	graph.AddNode("helper.go", []string{}, "go")
	graph.AddNode("lib.go", []string{}, "go")
	graph.AddNode("config.go", []string{}, "go")

	// Call edges forming a chain: Main calls Process, Process calls Helper, Helper calls Load and Config
	graph.AddCallEdge(CallEdge{
		CallerFile: "main.go", CallerLine: 10, CallerName: "Main",
		CalleeFile: "util.go", CalleeLine: 5, CalleeName: "Process",
	})
	graph.AddCallEdge(CallEdge{
		CallerFile: "util.go", CallerLine: 15, CallerName: "Process",
		CalleeFile: "helper.go", CalleeLine: 8, CalleeName: "Helper",
	})
	graph.AddCallEdge(CallEdge{
		CallerFile: "helper.go", CallerLine: 20, CallerName: "Helper",
		CalleeFile: "lib.go", CalleeLine: 3, CalleeName: "Load",
	})
	graph.AddCallEdge(CallEdge{
		CallerFile: "helper.go", CallerLine: 25, CallerName: "Helper",
		CalleeFile: "config.go", CalleeLine: 10, CalleeName: "Config",
	})

	index.AddFile(&FileInfo{Path: "main.go", Language: "go", Exports: []SymbolInfo{{Name: "Main", Kind: "func", Exported: true}}})
	index.AddFile(&FileInfo{Path: "util.go", Language: "go", Exports: []SymbolInfo{{Name: "Process", Kind: "func", Exported: false}}})
	index.AddFile(&FileInfo{Path: "helper.go", Language: "go", Exports: []SymbolInfo{{Name: "Helper", Kind: "func", Exported: false}, {Name: "Config", Kind: "func", Exported: false}}})
	index.AddFile(&FileInfo{Path: "lib.go", Language: "go", Exports: []SymbolInfo{{Name: "Load", Kind: "func", Exported: false}}})
	index.AddFile(&FileInfo{Path: "config.go", Language: "go", Exports: []SymbolInfo{{Name: "Config", Kind: "func", Exported: false}}})

	// Analyze impact on "Process" (in util.go)
	// Direct callers: Main (main.go)
	// Indirect (transitive callers): none, because nothing calls Main
	// But with depth=3, we should find transitive callers of Process
	// Process is called by Main. Main has no callers. So indirect should be empty.
	result := AnalyzeImpact(graph, index, "Process", 3)

	// Check direct callers
	if len(result.DirectCallers) != 1 {
		t.Fatalf("expected 1 direct caller for Process, got %d", len(result.DirectCallers))
	}
	if result.DirectCallers[0].Symbol != "Main" {
		t.Errorf("expected caller Main, got %s", result.DirectCallers[0].Symbol)
	}

	// For "Process", indirect should be empty since Main has no callers
	if len(result.Indirect) != 0 {
		t.Errorf("expected 0 indirect for Process (Main has no callers), got %v", result.Indirect)
	}

	// Now analyze "Helper" - called by Process, which is called by Main
	result = AnalyzeImpact(graph, index, "Helper", 3)
	// Direct: Process (util.go)
	// Indirect: Main (main.go) - caller of Process
	if len(result.DirectCallers) != 1 {
		t.Fatalf("expected 1 direct caller for Helper, got %d", len(result.DirectCallers))
	}
	if result.DirectCallers[0].Symbol != "Process" {
		t.Errorf("expected caller Process, got %s", result.DirectCallers[0].Symbol)
	}

	// Indirect should include main.go (transitive caller via Process -> Main)
	foundMain := false
	for _, f := range result.Indirect {
		if f == "main.go" {
			foundMain = true
			break
		}
	}
	if !foundMain {
		t.Errorf("expected main.go in indirect for Helper, got %v", result.Indirect)
	}

	// Analyze "Load" - called by Helper -> Process -> Main
	result = AnalyzeImpact(graph, index, "Load", 3)
	// Direct: Helper (helper.go)
	// Indirect: Process (util.go), Main (main.go)
	foundUtil := false
	foundMain = false
	for _, f := range result.Indirect {
		if f == "util.go" {
			foundUtil = true
		}
		if f == "main.go" {
			foundMain = true
		}
	}
	if !foundUtil || !foundMain {
		t.Errorf("expected util.go and main.go in indirect for Load, got %v", result.Indirect)
	}
}

func TestAnalyzeImpact_AffectedTests(t *testing.T) {
	// Graph with test files
	graph := NewCodeGraph()
	index := NewSymbolIndex()

	graph.AddNode("service.go", []string{}, "go")
	graph.AddNode("service_test.go", []string{}, "go")
	graph.AddNode("handler.go", []string{}, "go")
	graph.AddNode("handler_test.go", []string{}, "go")

	// Call edges
	graph.AddCallEdge(CallEdge{
		CallerFile: "handler.go", CallerLine: 10, CallerName: "Handle",
		CalleeFile: "service.go", CalleeLine: 5, CalleeName: "DoWork",
	})
	graph.AddCallEdge(CallEdge{
		CallerFile: "service_test.go", CallerLine: 20, CallerName: "TestDoWork",
		CalleeFile: "service.go", CalleeLine: 5, CalleeName: "DoWork",
	})
	graph.AddCallEdge(CallEdge{
		CallerFile: "handler_test.go", CallerLine: 15, CallerName: "TestHandle",
		CalleeFile: "handler.go", CalleeLine: 10, CalleeName: "Handle",
	})

	index.AddFile(&FileInfo{Path: "service.go", Language: "go", Exports: []SymbolInfo{{Name: "DoWork", Kind: "func", Exported: true}}})
	index.AddFile(&FileInfo{Path: "handler.go", Language: "go", Exports: []SymbolInfo{{Name: "Handle", Kind: "func", Exported: false}}})
	index.AddFile(&FileInfo{Path: "service_test.go", Language: "go", Exports: []SymbolInfo{{Name: "TestDoWork", Kind: "func", Exported: false}}})
	index.AddFile(&FileInfo{Path: "handler_test.go", Language: "go", Exports: []SymbolInfo{{Name: "TestHandle", Kind: "func", Exported: false}}})

	result := AnalyzeImpact(graph, index, "DoWork", 3)

	// Should find both test files as affected:
	// - service_test.go directly calls DoWork
	// - handler_test.go transitively calls DoWork via Handle
	expectedTests := map[string]bool{"service_test.go": true, "handler_test.go": true}
	for _, tf := range result.AffectedTests {
		if !expectedTests[tf] {
			t.Errorf("unexpected affected test: %s", tf)
		}
		delete(expectedTests, tf)
	}
	if len(expectedTests) > 0 {
		t.Errorf("missing affected tests: %v", expectedTests)
	}
}

func TestAnalyzeImpact_RiskCategories(t *testing.T) {
	graph := NewCodeGraph()
	index := NewSymbolIndex()

	// Exported symbol -> RiskAPI
	graph.AddNode("api.go", []string{}, "go")
	index.AddFile(&FileInfo{
		Path: "api.go", Language: "go",
		Exports: []SymbolInfo{{Name: "PublicAPI", Kind: "func", Exported: true}},
	})

	// Unexported symbol -> RiskRuntime
	graph.AddNode("internal.go", []string{}, "go")
	index.AddFile(&FileInfo{
		Path: "internal.go", Language: "go",
		Exports: []SymbolInfo{{Name: "internalHelper", Kind: "func", Exported: false}},
	})

	// Test-only symbol -> RiskTest
	graph.AddNode("test_utils_test.go", []string{}, "go")
	index.AddFile(&FileInfo{
		Path: "test_utils_test.go", Language: "go",
		Exports: []SymbolInfo{{Name: "testHelper", Kind: "func", Exported: false}},
	})

	// Test exported symbol
	result := AnalyzeImpact(graph, index, "PublicAPI", 1)
	if result.Risk != RiskAPI {
		t.Errorf("PublicAPI: expected RiskAPI, got %s", result.Risk)
	}

	// Test unexported symbol
	result = AnalyzeImpact(graph, index, "internalHelper", 1)
	if result.Risk != RiskRuntime {
		t.Errorf("internalHelper: expected RiskRuntime, got %s", result.Risk)
	}

	// Test test-only symbol
	result = AnalyzeImpact(graph, index, "testHelper", 1)
	if result.Risk != RiskTest {
		t.Errorf("testHelper: expected RiskTest, got %s", result.Risk)
	}
}

func TestAnalyzeImpact_EmptySymbol(t *testing.T) {
	graph := NewCodeGraph()
	index := NewSymbolIndex()

	graph.AddNode("a.go", []string{}, "go")
	index.AddFile(&FileInfo{Path: "a.go", Language: "go", Exports: []SymbolInfo{{Name: "Foo", Kind: "func", Exported: true}}})

	result := AnalyzeImpact(graph, index, "NonExistentSymbol", 3)

	if result.Symbol != "NonExistentSymbol" {
		t.Errorf("symbol name not preserved: %s", result.Symbol)
	}
	if len(result.DirectCallers) != 0 {
		t.Errorf("expected 0 direct callers for unknown symbol, got %d", len(result.DirectCallers))
	}
	if len(result.Indirect) != 0 {
		t.Errorf("expected 0 indirect for unknown symbol, got %d", len(result.Indirect))
	}
	if len(result.AffectedTests) != 0 {
		t.Errorf("expected 0 affected tests for unknown symbol, got %d", len(result.AffectedTests))
	}
	if result.Risk != RiskRuntime {
		t.Errorf("unknown symbol should default to RiskRuntime, got %s", result.Risk)
	}
}

func TestCategorizeSymbolRisk(t *testing.T) {
	index := NewSymbolIndex()

	// Exported
	index.AddFile(&FileInfo{
		Path: "api.go", Language: "go",
		Exports: []SymbolInfo{{Name: "ExportedFunc", Kind: "func", Exported: true}},
	})
	if CategorizeSymbolRisk(index, "ExportedFunc") != RiskAPI {
		t.Errorf("exported symbol should be RiskAPI")
	}

	// Unexported
	index.AddFile(&FileInfo{
		Path: "internal.go", Language: "go",
		Exports: []SymbolInfo{{Name: "unexportedFunc", Kind: "func", Exported: false}},
	})
	if CategorizeSymbolRisk(index, "unexportedFunc") != RiskRuntime {
		t.Errorf("unexported symbol should be RiskRuntime")
	}

	// Test file only
	index.AddFile(&FileInfo{
		Path: "foo_test.go", Language: "go",
		Exports: []SymbolInfo{{Name: "testOnly", Kind: "func", Exported: false}},
	})
	if CategorizeSymbolRisk(index, "testOnly") != RiskTest {
		t.Errorf("test-only symbol should be RiskTest")
	}

	// Unknown symbol
	if CategorizeSymbolRisk(index, "unknown") != RiskRuntime {
		t.Errorf("unknown symbol should default to RiskRuntime")
	}
}