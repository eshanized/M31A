package codeintel

import (
	"testing"
)

func TestImportGraph_AddAndQuery(t *testing.T) {
	g := NewImportGraph()
	g.AddNode("a.go", []string{"b.go", "c.go"}, "go")
	g.AddNode("b.go", []string{"c.go"}, "go")
	g.AddNode("c.go", nil, "go")

	imports, importedBy := g.Neighbors("a.go")
	if len(imports) != 2 {
		t.Errorf("a.go imports: got %d, want 2", len(imports))
	}
	if len(importedBy) != 0 {
		t.Errorf("a.go importedBy: got %d, want 0", len(importedBy))
	}

	imports, importedBy = g.Neighbors("c.go")
	if len(imports) != 0 {
		t.Errorf("c.go imports: got %d, want 0", len(imports))
	}
	if len(importedBy) != 2 {
		t.Errorf("c.go importedBy: got %d, want 2", len(importedBy))
	}

	imports, importedBy = g.Neighbors("b.go")
	if len(imports) != 1 {
		t.Errorf("b.go imports: got %d, want 1", len(imports))
	}
	if len(importedBy) != 1 {
		t.Errorf("b.go importedBy: got %d, want 1", len(importedBy))
	}
}

func TestImportGraph_Upstream(t *testing.T) {
	g := NewImportGraph()
	// a → b → c → d
	g.AddNode("a.go", []string{"b.go"}, "go")
	g.AddNode("b.go", []string{"c.go"}, "go")
	g.AddNode("c.go", []string{"d.go"}, "go")
	g.AddNode("d.go", nil, "go")

	// depth=1: only direct imports
	up := g.Upstream("a.go", 1)
	if len(up) != 1 || up[0] != "b.go" {
		t.Errorf("depth=1: got %v, want [b.go]", up)
	}

	// depth=2: a→b, b→c
	up = g.Upstream("a.go", 2)
	if len(up) != 2 {
		t.Errorf("depth=2: got %d items, want 2", len(up))
	}

	// depth=0: unlimited
	up = g.Upstream("a.go", 0)
	if len(up) != 3 {
		t.Errorf("depth=0 (unlimited): got %d items, want 3", len(up))
	}
}

func TestImportGraph_Downstream(t *testing.T) {
	g := NewImportGraph()
	// a → b → c → d
	g.AddNode("a.go", []string{"b.go"}, "go")
	g.AddNode("b.go", []string{"c.go"}, "go")
	g.AddNode("c.go", []string{"d.go"}, "go")
	g.AddNode("d.go", nil, "go")

	// depth=1 from d: only c (c imports d)
	down := g.Downstream("d.go", 1)
	if len(down) != 1 || down[0] != "c.go" {
		t.Errorf("depth=1 from d: got %v, want [c.go]", down)
	}

	// depth=0 from d: all upstream dependents (c, b, a)
	down = g.Downstream("d.go", 0)
	if len(down) != 3 {
		t.Errorf("depth=0 from d: got %d items, want 3", len(down))
	}
}

func TestImportGraph_DiamondDependency(t *testing.T) {
	g := NewImportGraph()
	// a → b, a → c, b → d, c → d (diamond)
	g.AddNode("a.go", []string{"b.go", "c.go"}, "go")
	g.AddNode("b.go", []string{"d.go"}, "go")
	g.AddNode("c.go", []string{"d.go"}, "go")
	g.AddNode("d.go", nil, "go")

	up := g.Upstream("a.go", 0)
	if len(up) != 3 {
		t.Errorf("diamond upstream: got %d items, want 3", len(up))
	}

	// d should appear only once despite two paths to it
	seen := make(map[string]int)
	for _, p := range up {
		seen[p]++
	}
	for p, count := range seen {
		if count > 1 {
			t.Errorf("file %s appears %d times in upstream (should be deduplicated)", p, count)
		}
	}
}

func TestImportGraph_NonExistentNode(t *testing.T) {
	g := NewImportGraph()
	imports, importedBy := g.Neighbors("nonexistent.go")
	if imports != nil || importedBy != nil {
		t.Error("expected nil for non-existent node")
	}
	up := g.Upstream("nonexistent.go", 0)
	if len(up) != 0 {
		t.Error("expected empty upstream for non-existent node")
	}
}

func TestImportGraph_NodeCount(t *testing.T) {
	g := NewImportGraph()
	if g.NodeCount() != 0 {
		t.Error("empty graph should have 0 nodes")
	}
	g.AddNode("a.go", nil, "go")
	g.AddNode("b.go", []string{"a.go"}, "go")
	if g.NodeCount() != 2 {
		t.Errorf("got %d nodes, want 2", g.NodeCount())
	}
}

func TestImportGraph_Cycle(t *testing.T) {
	g := NewImportGraph()
	// a → b → c → a (cycle)
	g.AddNode("a.go", []string{"b.go"}, "go")
	g.AddNode("b.go", []string{"c.go"}, "go")
	g.AddNode("c.go", []string{"a.go"}, "go")

	up := g.Upstream("a.go", 0)
	if len(up) != 2 {
		t.Errorf("cycle upstream: got %d, want 2 (b and c, not a again)", len(up))
	}
}
