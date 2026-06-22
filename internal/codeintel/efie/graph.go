package efie

// ExternalNode is the sentinel path for unresolved/external imports.
const ExternalNode = "__external__"

// WeightedNode extends a basic graph node with precomputed importance metrics.
type WeightedNode struct {
	Path       string
	Imports    []string // files this file imports (direct edges)
	ImportedBy []string // files that import this file (reverse edges)

	Language string

	// Precomputed importance metrics
	PageRank       float64
	Betweenness    float64
	Community      int
	DegreeCentrality float64

	// Semantic fingerprint
	SymbolBloom *BloomFilter
	ImportSet   map[string]bool
}

// WeightedImportGraph is a directed dependency graph with precomputed metrics.
type WeightedImportGraph struct {
	nodes        map[string]*WeightedNode
	nCommunities int
	maxCentrality float64
}

// NewWeightedImportGraph creates an empty weighted import graph.
func NewWeightedImportGraph() *WeightedImportGraph {
	return &WeightedImportGraph{
		nodes: make(map[string]*WeightedNode),
	}
}

// AddNode adds a file to the graph with its imports.
func (g *WeightedImportGraph) AddNode(path string, imports []string, language string) {
	node, exists := g.nodes[path]
	if exists {
		node.Imports = imports
		node.Language = language
	} else {
		node = &WeightedNode{
			Path:     path,
			Imports:  imports,
			Language: language,
		}
		g.nodes[path] = node
	}
	for _, imp := range imports {
		g.addReverseEdge(imp, path)
	}
}

func (g *WeightedImportGraph) addReverseEdge(target, source string) {
	node, ok := g.nodes[target]
	if !ok {
		node = &WeightedNode{Path: target}
		g.nodes[target] = node
	}
	for _, imp := range node.ImportedBy {
		if imp == source {
			return
		}
	}
	node.ImportedBy = append(node.ImportedBy, source)
}

// SetExternal marks the external node so it can be excluded from
// community detection.
func (g *WeightedImportGraph) SetExternal(path string) {
	// External node is a sentinel; just ensure it exists
	if _, ok := g.nodes[path]; !ok {
		g.nodes[path] = &WeightedNode{Path: path}
	}
}

// HasNode reports whether the graph contains a node for the given path.
func (g *WeightedImportGraph) HasNode(path string) bool {
	_, ok := g.nodes[path]
	return ok
}

// NodeCount returns the number of nodes in the graph.
func (g *WeightedImportGraph) NodeCount() int {
	return len(g.nodes)
}

// EdgeCount returns the total number of directed edges.
func (g *WeightedImportGraph) EdgeCount() int {
	count := 0
	for _, node := range g.nodes {
		count += len(node.Imports)
	}
	return count
}

// Neighbors returns direct imports and direct importers of a file.
func (g *WeightedImportGraph) Neighbors(path string) (imports []string, importedBy []string) {
	node, ok := g.nodes[path]
	if !ok {
		return nil, nil
	}
	return node.Imports, node.ImportedBy
}

// AllPaths returns all file paths in the graph.
func (g *WeightedImportGraph) AllPaths() []string {
	paths := make([]string, 0, len(g.nodes))
	for p := range g.nodes {
		paths = append(paths, p)
	}
	return paths
}

// AllNodes returns all nodes in the graph.
func (g *WeightedImportGraph) AllNodes() []*WeightedNode {
	nodes := make([]*WeightedNode, 0, len(g.nodes))
	for _, n := range g.nodes {
		nodes = append(nodes, n)
	}
	return nodes
}

// Upstream returns all files that `path` depends on, up to `depth` levels.
func (g *WeightedImportGraph) Upstream(path string, depth int) []string {
	return g.traverse(path, depth, func(n *WeightedNode) []string { return n.Imports })
}

// Downstream returns all files that depend on `path`, up to `depth` levels.
func (g *WeightedImportGraph) Downstream(path string, depth int) []string {
	return g.traverse(path, depth, func(n *WeightedNode) []string { return n.ImportedBy })
}

func (g *WeightedImportGraph) traverse(start string, maxDepth int, next func(*WeightedNode) []string) []string {
	visited := make(map[string]bool)
	visited[start] = true

	type entry struct {
		path  string
		depth int
	}
	queue := []entry{{path: start, depth: 0}}
	var result []string

	for front := 0; front < len(queue); front++ {
		cur := queue[front]
		if maxDepth > 0 && cur.depth >= maxDepth {
			continue
		}
		node, ok := g.nodes[cur.path]
		if !ok {
			continue
		}
		for _, neighbor := range next(node) {
			if !visited[neighbor] {
				visited[neighbor] = true
				result = append(result, neighbor)
				queue = append(queue, entry{path: neighbor, depth: cur.depth + 1})
			}
		}
	}
	return result
}

// RebuildEdges rebuilds edges for a single node and its neighbors.
func (g *WeightedImportGraph) RebuildEdges(path string, newImports []string) {
	node, ok := g.nodes[path]
	if !ok {
		return
	}
	// Remove old reverse edges
	for _, oldImp := range node.Imports {
		if target, ok := g.nodes[oldImp]; ok {
			var filtered []string
			for _, s := range target.ImportedBy {
				if s != path {
					filtered = append(filtered, s)
				}
			}
			target.ImportedBy = filtered
		}
	}
	// Set new imports and add reverse edges
	node.Imports = newImports
	for _, imp := range newImports {
		g.addReverseEdge(imp, path)
	}
}
