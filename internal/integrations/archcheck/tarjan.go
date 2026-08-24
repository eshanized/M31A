package archcheck

import (
	"github.com/eshanized/M31A/internal/integrations/codeintel"
	"github.com/looplab/tarjan"
)

// DetectSCCs finds strongly connected components in the import graph using Tarjan's algorithm.
// Returns a slice of cycles, where each cycle is a slice of file paths.
// Single-node SCCs (no cycle) are filtered out.
func DetectSCCs(graph *codeintel.CodeGraph) [][]string {
	// Build adjacency list for tarjan
	connections := make(map[interface{}][]interface{})
	allPaths := graph.AllPaths()

	// Initialize all nodes in connections (even those with no edges)
	for _, path := range allPaths {
		connections[path] = []interface{}{}
	}

	// Add edges from imports using public Neighbors method
	for _, path := range allPaths {
		imports, _ := graph.Neighbors(path)
		var edges []interface{}
		for _, imp := range imports {
			edges = append(edges, imp)
		}
		connections[path] = edges
	}

	// Find strongly connected components
	sccs := tarjan.Connections(connections)

	// Filter SCCs with size > 1 (single-node SCCs are not cycles)
	var cycles [][]string
	for _, scc := range sccs {
		if len(scc) > 1 {
			cycle := make([]string, len(scc))
			for i, v := range scc {
				cycle[i] = v.(string)
			}
			cycles = append(cycles, cycle)
		}
	}

	return cycles
}

// DetectSCCSCallback is a more flexible version that accepts a custom graph traversal function.
// This is useful for testing with different graph structures.
func DetectSCCSCallback(getNodes func() []string, getEdges func(string) []string) [][]string {
	connections := make(map[interface{}][]interface{})

	nodes := getNodes()
	for _, node := range nodes {
		connections[node] = []interface{}{}
	}

	for _, node := range nodes {
		edges := getEdges(node)
		ifaceEdges := make([]interface{}, len(edges))
		for i, e := range edges {
			ifaceEdges[i] = e
		}
		connections[node] = ifaceEdges
	}

	sccs := tarjan.Connections(connections)

	var cycles [][]string
	for _, scc := range sccs {
		if len(scc) > 1 {
			cycle := make([]string, len(scc))
			for i, v := range scc {
				cycle[i] = v.(string)
			}
			cycles = append(cycles, cycle)
		}
	}

	return cycles
}