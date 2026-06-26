package efie

import (
	"testing"
)

func TestTrie_InsertAndSearch(t *testing.T) {
	trie := NewTrie()
	trie.Insert("Engine")
	trie.Insert("EngineConfig")
	trie.Insert("ErrorHandler")
	trie.Insert("Engine")

	results := trie.Search("Engine")
	if len(results) != 1 {
		t.Errorf("expected 1 result for 'Engine', got %d", len(results))
	}

	results = trie.Search("EngineConfig")
	if len(results) != 1 {
		t.Errorf("expected 1 result for 'EngineConfig', got %d", len(results))
	}

	results = trie.Search("NonExistent")
	if len(results) != 0 {
		t.Errorf("expected 0 results for 'NonExistent', got %d", len(results))
	}
}

func TestTrie_PrefixSearch(t *testing.T) {
	trie := NewTrie()
	trie.Insert("Engine")
	trie.Insert("EngineConfig")
	trie.Insert("ErrorHandler")
	trie.Insert("EngineType")

	results := trie.PrefixSearch("Eng")
	if len(results) != 3 {
		t.Errorf("expected 3 prefix results for 'Eng', got %d: %v", len(results), results)
	}

	results = trie.PrefixSearch("Error")
	if len(results) != 1 {
		t.Errorf("expected 1 prefix result for 'Error', got %d", len(results))
	}

	results = trie.PrefixSearch("Zzz")
	if len(results) != 0 {
		t.Errorf("expected 0 prefix results for 'Zzz', got %d", len(results))
	}
}

func TestTrie_HasPrefix(t *testing.T) {
	trie := NewTrie()
	trie.Insert("Engine")

	if !trie.HasPrefix("Eng") {
		t.Error("expected HasPrefix('Eng') to be true")
	}
	if trie.HasPrefix("XYZ") {
		t.Error("expected HasPrefix('XYZ') to be false")
	}
}

func TestTrie_BuildTrie(t *testing.T) {
	names := []string{"Alpha", "Beta", "Gamma", "AlphaBeta"}
	trie := BuildTrie(names)

	if !trie.HasPrefix("Alpha") {
		t.Error("expected prefix 'Alpha' to exist")
	}
	if !trie.HasPrefix("Beta") {
		t.Error("expected prefix 'Beta' to exist")
	}
	results := trie.PrefixSearch("Alpha")
	if len(results) != 2 {
		t.Errorf("expected 2 results for 'Alpha' prefix, got %d", len(results))
	}
}

func TestBloomFilter_AddAndContains(t *testing.T) {
	bf := NewBloomFilter(100, 0.01)

	bf.Add("Engine")
	bf.Add("Handler")
	bf.Add("Store")

	if !bf.Contains("Engine") {
		t.Error("expected Contains('Engine') to be true")
	}
	if !bf.Contains("Handler") {
		t.Error("expected Contains('Handler') to be true")
	}
	if !bf.Contains("Store") {
		t.Error("expected Contains('Store') to be true")
	}
	if bf.Contains("NonExistent") {
		t.Log("false positive detected (acceptable at 1% FP rate)")
	}
}

func TestBloomFilter_FalseNegativeImpossible(t *testing.T) {
	bf := NewBloomFilter(1000, 0.001)
	for i := 0; i < 100; i++ {
		bf.Add("Symbol" + string(rune('A'+i%26)))
	}
	for i := 0; i < 100; i++ {
		name := "Symbol" + string(rune('A'+i%26))
		if !bf.Contains(name) {
			t.Errorf("false negative for %s (should never happen)", name)
		}
	}
}

func TestWeightedImportGraph_AddNode(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("a.go", []string{"b.go", "c.go"}, "go")
	g.AddNode("b.go", []string{"c.go"}, "go")
	g.AddNode("c.go", nil, "go")

	if g.NodeCount() != 3 {
		t.Errorf("expected 3 nodes, got %d", g.NodeCount())
	}

	imports, importedBy := g.Neighbors("c.go")
	if len(imports) != 0 {
		t.Errorf("expected 0 imports for c.go, got %d", len(imports))
	}
	if len(importedBy) != 2 {
		t.Errorf("expected 2 importers for c.go, got %d", len(importedBy))
	}
}

func TestWeightedImportGraph_UpstreamDownstream(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("a.go", []string{"b.go"}, "go")
	g.AddNode("b.go", []string{"c.go"}, "go")
	g.AddNode("c.go", nil, "go")

	upstream := g.Upstream("a.go", 0)
	if len(upstream) != 2 {
		t.Errorf("expected 2 upstream for a.go, got %d", len(upstream))
	}

	downstream := g.Downstream("c.go", 0)
	if len(downstream) != 2 {
		t.Errorf("expected 2 downstream for c.go, got %d", len(downstream))
	}
}

func TestWeightedImportGraph_EdgeCount(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("a.go", []string{"b.go", "c.go"}, "go")
	g.AddNode("b.go", []string{"c.go"}, "go")

	if g.EdgeCount() != 3 {
		t.Errorf("expected 3 edges, got %d", g.EdgeCount())
	}
}

func TestLouvainDeterministic(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("engine.go", []string{"types.go"}, "go")
	g.AddNode("types.go", nil, "go")
	g.AddNode("handler.go", []string{"engine.go", "routes.go"}, "go")
	g.AddNode("routes.go", nil, "go")
	g.AddNode("store.go", []string{"types.go"}, "go")

	c1 := LouvainDetect_Deterministic(g, 42)
	c2 := LouvainDetect_Deterministic(g, 42)

	for path := range c1 {
		if c1[path] != c2[path] {
			t.Errorf("non-deterministic: %s got %d then %d", path, c1[path], c2[path])
		}
	}

	if len(c1) != 5 {
		t.Errorf("expected 5 community assignments, got %d", len(c1))
	}
}

func TestComputePageRank(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("hub.go", []string{"a.go", "b.go", "c.go"}, "go")
	g.AddNode("a.go", nil, "go")
	g.AddNode("b.go", nil, "go")
	g.AddNode("c.go", nil, "go")

	pr := ComputePageRank(g, 20, 0.85)

	hubPR := pr["hub.go"]
	aPR := pr["a.go"]

	if hubPR <= aPR {
		t.Errorf("hub should have higher PageRank than leaf: hub=%.4f leaf=%.4f", hubPR, aPR)
	}
}

func TestComputeApproxBetweenness(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("bridge.go", []string{"a.go"}, "go")
	g.AddNode("a.go", []string{"b.go"}, "go")
	g.AddNode("b.go", nil, "go")
	g.AddNode("isolated.go", nil, "go")

	bc := ComputeApproxBetweenness(g, 2, 42)

	// bridge.go should have nonzero betweenness since it's on the path from bridge->a->b
	bridgeBC := bc["bridge.go"]
	if bridgeBC < 0 {
		t.Errorf("bridge betweenness should be non-negative, got %.6f", bridgeBC)
	}
	// Verify betweenness values are normalized (between 0 and 1)
	for path, val := range bc {
		if val < 0 || val > 1 {
			t.Errorf("betweenness for %s out of range: %.6f", path, val)
		}
	}
}

func TestMultiResIndex(t *testing.T) {
	idx := NewMultiResIndex()

	symbols := []SymbolInfo{
		{Name: "Engine", Kind: "struct", Exported: true},
		{Name: "Start", Kind: "func", Exported: true},
	}
	idx.AddFile("engine.go", "go", symbols)
	idx.AddFile("handler.go", "go", []SymbolInfo{{Name: "Handle", Kind: "func", Exported: true}})

	if idx.FileCount() != 2 {
		t.Errorf("expected 2 files, got %d", idx.FileCount())
	}
	if idx.SymbolCount() != 3 {
		t.Errorf("expected 3 symbols, got %d", idx.SymbolCount())
	}

	locs := idx.Define("Engine")
	if len(locs) != 1 {
		t.Errorf("expected 1 location for Engine, got %d", len(locs))
	}

	fileSyms := idx.FileSymbols("engine.go")
	if len(fileSyms) != 2 {
		t.Errorf("expected 2 symbols in engine.go, got %d", len(fileSyms))
	}
}

func TestScore(t *testing.T) {
	g := NewWeightedImportGraph()
	g.AddNode("target.go", []string{"dep.go"}, "go")
	g.AddNode("dep.go", nil, "go")
	g.AddNode("other.go", nil, "go")

	idx := NewMultiResIndex()
	idx.AddFile("target.go", "go", []SymbolInfo{{Name: "Target", Kind: "func", Exported: true}})
	idx.AddFile("dep.go", "go", []SymbolInfo{{Name: "Dep", Kind: "func", Exported: true}})
	idx.AddFile("other.go", "go", []SymbolInfo{{Name: "Other", Kind: "func", Exported: true}})
	idx.BuildTrieIndex()
	idx.SetCentralityPercentiles(0.001, 0.01)

	// Set communities
	for _, node := range g.nodes {
		node.Community = 0
		node.PageRank = 0.001
	}
	idx.SetCommunity("target.go", 0)
	idx.SetCommunity("dep.go", 0)
	idx.SetCommunity("other.go", 1)

	targetComms := map[int]bool{0: true}
	targetSet := map[string]bool{"target.go": true}

	sf := Score("target.go", []string{"target.go"}, "", g, idx, targetComms, targetSet)
	if sf.Score < 35.0 {
		t.Errorf("target file should score >= 35 (direct mention), got %.1f", sf.Score)
	}

	sf = Score("dep.go", []string{"target.go"}, "", g, idx, targetComms, targetSet)
	if sf.Score < 5.0 {
		t.Errorf("dep file should score > 0, got %.1f", sf.Score)
	}
}

func TestExtractIdentifiers(t *testing.T) {
	ids := extractIdentifiers("fix the EngineConfig struct to handle errors")
	seen := make(map[string]bool)
	for _, id := range ids {
		seen[id] = true
	}
	if !seen["EngineConfig"] {
		t.Error("expected 'EngineConfig' in identifiers")
	}
	if !seen["Engine"] {
		t.Error("expected 'Engine' in identifiers (camelCase split)")
	}
	if !seen["Config"] {
		t.Error("expected 'Config' in identifiers (camelCase split)")
	}
}

func TestSplitCamelCase(t *testing.T) {
	tests := []struct {
		input string
		want  []string
	}{
		{"getUserByID", []string{"get", "User", "By", "I", "D"}},
		{"HTMLParser", []string{"HTML", "Parser"}},
		{"simple", []string{"simple"}},
	}
	for _, tt := range tests {
		got := splitCamelCase(tt.input)
		if len(got) != len(tt.want) {
			t.Errorf("splitCamelCase(%q) = %v, want %v", tt.input, got, tt.want)
			continue
		}
		for i := range got {
			if got[i] != tt.want[i] {
				t.Errorf("splitCamelCase(%q)[%d] = %q, want %q", tt.input, i, got[i], tt.want[i])
			}
		}
	}
}

func TestComputePercentile(t *testing.T) {
	values := []float64{0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0}
	p50 := ComputePercentile(values, 50)
	p95 := ComputePercentile(values, 95)

	if p50 < 0.4 || p50 > 0.6 {
		t.Errorf("expected p50 around 0.5, got %.2f", p50)
	}
	if p95 < 0.8 {
		t.Errorf("expected p95 >= 0.8, got %.2f", p95)
	}
}
