# EGINE Installation and Usage Guide

## Overview

EGINE (Eshanized Graph Intelligence Network Engine) is a graph-theoretic algorithm library for AI context management and codebase intelligence. It provides efficient implementations of PageRank, Louvain community detection, betweenness centrality, Bloom filters, and Trie-based prefix matching.

## Requirements

- Go 1.25 or later
- Git

## Installation

### From Source

```bash
git clone https://github.com/eshanized/M31A.git
cd M31A
go build ./...
```

### Using `go get`

```bash
go get github.com/eshanized/M31A@latest
```

## Quick Start

### Basic Graph Operations

```go
package main

import (
    "fmt"
    "github.com/eshanized/M31A/graph"
)

func main() {
    // Create a new directed graph
    g := graph.NewDirectedGraph()

    // Add nodes
    g.AddNode("main.go")
    g.AddNode("utils.go")
    g.AddNode("config.go")

    // Add edges (import relationships)
    g.AddEdge("main.go", "utils.go", 1.0)
    g.AddEdge("main.go", "config.go", 1.0)
    g.AddEdge("utils.go", "config.go", 0.5)

    // Compute PageRank
    pagerank := graph.PageRank(g, 0.85, 20)
    for node, score := range pagerank {
        fmt.Printf("%s: %.4f\n", node, score)
    }
}
```

### Community Detection

```go
// Detect communities using Louvain algorithm
communities := graph.LouvainCommunityDetection(g)
for i, community := range communities {
    fmt.Printf("Community %d: %v\n", i, community)
}
```

### Symbol Search with Trie

```go
import "github.com/eshanized/M31A/index"

// Create a Trie index
trie := index.NewTrie()

// Insert symbols
trie.Insert("fmt.Println", "fmt")
trie.Insert("fmt.Sprintf", "fmt")
trie.Insert("os.Open", "os")

// Search with prefix
results := trie.Search("fmt.")
fmt.Println(results) // [fmt.Println, fmt.Sprintf]
```

### Bloom Filter Pre-filter

```go
import "github.com/eshanized/M31A/bloom"

// Create a Bloom filter
bf := bloom.New(1000, 0.01) // 1000 capacity, 1% false positive rate

// Add elements
bf.Add("important_function")
bf.Add("critical_path")

// Check membership (may have false positives)
if bf.MightContain("important_function") {
    // Definitely exists
}
if bf.MightContain("unknown_function") {
    // Might exist (false positive) or definitely doesn't exist
}
```

## Configuration

### Graph Construction

```go
// Build graph from codebase
config := &graph.BuildConfig{
    IncludeTests:      false,
    IncludeVendor:     false,
    MaxFileSize:       1024 * 1024, // 1MB
    MinEdgeWeight:     0.1,
    IncludeGenerated:  false,
}

g := graph.BuildFromDirectory("/path/to/codebase", config)
```

### PageRank Parameters

```go
// Custom PageRank configuration
config := &graph.PageRankConfig{
    DampingFactor: 0.85,
    MaxIterations: 50,
    Convergence:   1e-6,
    IncludeDangling: true,
}

pagerank := graph.PageRankWithConfig(g, config)
```

### Community Detection Parameters

```go
// Custom Louvain configuration
config := &graph.LouvainConfig{
    Resolution:    1.0,
    MaxLevels:     10,
    MinImprovement: 0.0001,
}

communities := graph.LouvainWithConfig(g, config)
```

## API Reference

### Core Types

- `graph.DirectedGraph` - Directed graph implementation
- `graph.Node` - Graph node
- `graph.Edge` - Directed edge with weight
- `index.Trie` - Prefix tree for symbol search
- `bloom.Filter` - Bloom filter for probabilistic membership

### Key Functions

- `graph.PageRank(g, d, iterations)` - Compute PageRank scores
- `graph.LouvainCommunityDetection(g)` - Detect communities
- `graph.BetweennessCentrality(g)` - Compute betweenness centrality
- `graph.TopologicalSort(g)` - Topological ordering
- `index.NewTrie()` - Create new Trie
- `bloom.New(capacity, fpRate)` - Create Bloom filter

## Performance

Benchmarks on representative codebases:

| Operation | Time Complexity | Space Complexity |
|-----------|----------------|------------------|
| PageRank (20 iterations) | O(k·E) | O(V) |
| Louvain | O(E) average | O(V + E) |
| Betweenness (sampled) | O(V·m) | O(V + E) |
| Trie lookup | O(L) | O(N·Σ) |
| Bloom filter check | O(k) | O(m) |

Where:
- V = vertices, E = edges
- k = iterations, m = Bloom filter size
- L = key length, N = entries, Σ = alphabet size

## Examples

See the `examples/` directory for complete working examples:

- `examples/basic/main.go` - Basic graph operations
- `examples/community/main.go` - Community detection
- `examples/search/main.go` - Symbol search
- `examples/analysis/main.go` - Codebase analysis

## Troubleshooting

### Out of Memory

For large codebases (>1M LOC), increase Go's memory limit:

```bash
export GOGC=100
export GOMEMLIMIT=4GiB
```

### Slow Performance

Ensure you're using the parallelized versions:

```go
// Use parallel PageRank
pagerank := graph.PageRankParallel(g, 0.85, 20, runtime.NumCPU())

// Use approximate betweenness
centrality := graph.ApproximateBetweenness(g, 100) // sample 100 nodes
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License - see [LICENSE](../LICENSE) for details.
