package archcheck

import (
	"testing"

	"github.com/eshanized/M31A/internal/integrations/codeintel"
)

func TestDetectSCCs_NoCycle(t *testing.T) {
	// Create a graph with no cycles: A -> B -> C
	graph := codeintel.NewCodeGraph()
	graph.AddNode("a.go", []string{"b.go"}, "go")
	graph.AddNode("b.go", []string{"c.go"}, "go")
	graph.AddNode("c.go", []string{}, "go")

	cycles := DetectSCCs(graph)
	if len(cycles) != 0 {
		t.Errorf("expected no cycles, got %d: %v", len(cycles), cycles)
	}
}

func TestDetectSCCs_SimpleCycle(t *testing.T) {
	// Create a simple 2-node cycle: A <-> B
	graph := codeintel.NewCodeGraph()
	graph.AddNode("a.go", []string{"b.go"}, "go")
	graph.AddNode("b.go", []string{"a.go"}, "go")

	cycles := DetectSCCs(graph)
	if len(cycles) != 1 {
		t.Errorf("expected 1 cycle, got %d: %v", len(cycles), cycles)
	}
	if len(cycles[0]) != 2 {
		t.Errorf("expected cycle of length 2, got %d: %v", len(cycles[0]), cycles[0])
	}
}

func TestDetectSCCs_MultipleCycles(t *testing.T) {
	// Create graph with two separate cycles: A <-> B and C <-> D -> E -> C
	graph := codeintel.NewCodeGraph()
	graph.AddNode("a.go", []string{"b.go"}, "go")
	graph.AddNode("b.go", []string{"a.go"}, "go")
	graph.AddNode("c.go", []string{"d.go"}, "go")
	graph.AddNode("d.go", []string{"e.go"}, "go")
	graph.AddNode("e.go", []string{"c.go"}, "go")

	cycles := DetectSCCs(graph)
	if len(cycles) != 2 {
		t.Errorf("expected 2 cycles, got %d: %v", len(cycles), cycles)
	}
}

func TestDetectSCCs_SelfLoop(t *testing.T) {
	// Self-loop (A imports A) - not typically possible in real code but test anyway
	graph := codeintel.NewCodeGraph()
	graph.AddNode("a.go", []string{"a.go"}, "go")

	cycles := DetectSCCs(graph)
	// Tarjan considers single-node SCC with self-loop as a cycle
	// But our filter requires size > 1, so this should return 0
	if len(cycles) != 0 {
		t.Errorf("expected no cycles for self-loop (filtered out), got %d: %v", len(cycles), cycles)
	}
}

func TestDetectSCCs_ThreeNodeCycle(t *testing.T) {
	// Create a 3-node cycle: A -> B -> C -> A
	graph := codeintel.NewCodeGraph()
	graph.AddNode("a.go", []string{"b.go"}, "go")
	graph.AddNode("b.go", []string{"c.go"}, "go")
	graph.AddNode("c.go", []string{"a.go"}, "go")

	cycles := DetectSCCs(graph)
	if len(cycles) != 1 {
		t.Errorf("expected 1 cycle, got %d: %v", len(cycles), cycles)
	}
	if len(cycles[0]) != 3 {
		t.Errorf("expected cycle of length 3, got %d: %v", len(cycles[0]), cycles[0])
	}
}

func TestDetectSCCs_DiamondPattern(t *testing.T) {
	// Diamond pattern: A -> B, A -> C, B -> D, C -> D (no cycles)
	graph := codeintel.NewCodeGraph()
	graph.AddNode("a.go", []string{"b.go", "c.go"}, "go")
	graph.AddNode("b.go", []string{"d.go"}, "go")
	graph.AddNode("c.go", []string{"d.go"}, "go")
	graph.AddNode("d.go", []string{}, "go")

	cycles := DetectSCCs(graph)
	if len(cycles) != 0 {
		t.Errorf("expected no cycles in diamond pattern, got %d: %v", len(cycles), cycles)
	}
}

func TestDetectSCCs_EmptyGraph(t *testing.T) {
	graph := codeintel.NewCodeGraph()
	cycles := DetectSCCs(graph)
	if len(cycles) != 0 {
		t.Errorf("expected no cycles in empty graph, got %d", len(cycles))
	}
}

func TestDetectSCCs_SingleNode(t *testing.T) {
	graph := codeintel.NewCodeGraph()
	graph.AddNode("a.go", []string{}, "go")
	cycles := DetectSCCs(graph)
	if len(cycles) != 0 {
		t.Errorf("expected no cycles in single node graph, got %d", len(cycles))
	}
}

func TestDetectSCCSCallback_CustomGraph(t *testing.T) {
	// Test the callback version with a custom graph structure
	cycles := DetectSCCSCallback(
		func() []string { return []string{"a", "b", "c"} },
		func(node string) []string {
			switch node {
			case "a":
				return []string{"b"}
			case "b":
				return []string{"c"}
			case "c":
				return []string{"a"}
			}
			return nil
		},
	)

	if len(cycles) != 1 {
		t.Errorf("expected 1 cycle, got %d: %v", len(cycles), cycles)
	}
	if len(cycles[0]) != 3 {
		t.Errorf("expected cycle of length 3, got %d: %v", len(cycles[0]), cycles[0])
	}
}