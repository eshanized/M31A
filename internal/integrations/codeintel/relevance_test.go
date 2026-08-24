package codeintel

import (
	"testing"
)

func buildTestScorer() *RelevanceScorer {
	graph := NewCodeGraph()
	graph.AddNode("engine.go", []string{"types.go"}, "go")
	graph.AddNode("types.go", nil, "go")
	graph.AddNode("handler.go", []string{"engine.go", "types.go"}, "go")
	graph.AddNode("server.go", []string{"handler.go", "engine.go"}, "go")
	graph.AddNode("utils.go", nil, "go")

	files := []*FileInfo{
		{
			Path: "engine.go", Language: "go",
			Exports: []SymbolInfo{{Name: "Engine", Kind: "struct", Exported: true}},
			Funcs:   []FuncSignature{{Name: "NewEngine", Exported: true}},
		},
		{
			Path: "types.go", Language: "go",
			Exports: []SymbolInfo{
				{Name: "Config", Kind: "struct", Exported: true},
				{Name: "Status", Kind: "type", Exported: true},
			},
		},
		{
			Path: "handler.go", Language: "go",
			Exports: []SymbolInfo{{Name: "Handler", Kind: "interface", Exported: true}},
		},
		{
			Path: "server.go", Language: "go",
			Exports: []SymbolInfo{{Name: "Server", Kind: "struct", Exported: true}},
		},
		{
			Path: "utils.go", Language: "go",
			Exports: []SymbolInfo{{Name: "FormatName", Kind: "func", Exported: true}},
		},
	}

	index := BuildIndex(files)
	return NewRelevanceScorer(graph, index)
}

func TestRelevanceScorer_DirectMention(t *testing.T) {
	scorer := buildTestScorer()
	result := scorer.Score([]string{"engine.go"}, "", 10)

	if len(result) == 0 {
		t.Fatal("expected at least one result")
	}
	if result[0].Path != "engine.go" {
		t.Errorf("top result: got %q, want engine.go", result[0].Path)
	}
	if result[0].Score < 10.0 {
		t.Errorf("engine.go score: got %.1f, want >= 10.0", result[0].Score)
	}
}

func TestRelevanceScorer_ImportProximity(t *testing.T) {
	scorer := buildTestScorer()
	result := scorer.Score([]string{"engine.go"}, "", 10)

	scoreMap := make(map[string]float64)
	for _, sf := range result {
		scoreMap[sf.Path] = sf.Score
	}

	if _, ok := scoreMap["types.go"]; !ok {
		t.Error("expected types.go in results (imported by engine.go)")
	}
	if _, ok := scoreMap["handler.go"]; !ok {
		t.Error("expected handler.go in results (imports engine.go)")
	}
}

func TestRelevanceScorer_SymbolMatch(t *testing.T) {
	scorer := buildTestScorer()
	result := scorer.Score([]string{"server.go"}, "modify Config struct", 10)

	scoreMap := make(map[string]float64)
	for _, sf := range result {
		scoreMap[sf.Path] = sf.Score
	}

	if score, ok := scoreMap["types.go"]; ok {
		if score < 5.0 {
			t.Errorf("types.go should have high score for Config reference, got %.1f", score)
		}
	}
}

func TestRelevanceScorer_TopNLimits(t *testing.T) {
	scorer := buildTestScorer()
	result := scorer.Score([]string{"engine.go"}, "", 2)

	if len(result) > 2 {
		t.Errorf("expected at most 2 results, got %d", len(result))
	}
}

func TestRelevanceScorer_ScoreOrdering(t *testing.T) {
	scorer := buildTestScorer()
	result := scorer.Score([]string{"handler.go"}, "", 10)

	for i := 1; i < len(result); i++ {
		if result[i].Score > result[i-1].Score {
			t.Errorf("results not sorted: %s (%.1f) > %s (%.1f)",
				result[i].Path, result[i].Score, result[i-1].Path, result[i-1].Score)
		}
	}
}

func TestRelevanceScorer_EmptyTargets(t *testing.T) {
	scorer := buildTestScorer()
	result := scorer.Score(nil, "", 10)
	if len(result) != 0 {
		t.Errorf("expected 0 results for empty targets, got %d", len(result))
	}
}
