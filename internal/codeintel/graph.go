package codeintel

import (
	"io/fs"
	"os"
	"path/filepath"
	"strings"
)

// Node represents a single file in the import graph.
type Node struct {
	Path       string
	Imports    []string // files this file imports (direct edges)
	ImportedBy []string // files that import this file (reverse edges)
	Language   string
}

// ImportGraph is a directed dependency graph between source files.
type ImportGraph struct {
	nodes map[string]*Node // relative path → node
}

// NewImportGraph creates an empty import graph.
func NewImportGraph() *ImportGraph {
	return &ImportGraph{nodes: make(map[string]*Node)}
}

// AddNode adds a file to the graph with its imports.
// All paths must be relative to the working directory.
// If the node already exists as a stub (from a reverse edge), it is updated.
func (g *ImportGraph) AddNode(path string, imports []string, language string) {
	node, exists := g.nodes[path]
	if exists {
		node.Imports = imports
		node.Language = language
	} else {
		node = &Node{
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

func (g *ImportGraph) addReverseEdge(target, source string) {
	node, ok := g.nodes[target]
	if !ok {
		node = &Node{Path: target}
		g.nodes[target] = node
	}
	node.ImportedBy = append(node.ImportedBy, source)
}

// HasNode reports whether the graph contains a node for the given path.
func (g *ImportGraph) HasNode(path string) bool {
	_, ok := g.nodes[path]
	return ok
}

// NodeCount returns the number of nodes in the graph.
func (g *ImportGraph) NodeCount() int {
	return len(g.nodes)
}

// Neighbors returns direct imports and direct importers of a file.
func (g *ImportGraph) Neighbors(path string) (imports []string, importedBy []string) {
	node, ok := g.nodes[path]
	if !ok {
		return nil, nil
	}
	return node.Imports, node.ImportedBy
}

// Upstream returns all files that `path` depends on, up to `depth` levels.
// depth=1 returns direct imports only. depth=0 means unlimited.
func (g *ImportGraph) Upstream(path string, depth int) []string {
	return g.traverse(path, depth, func(n *Node) []string { return n.Imports })
}

// Downstream returns all files that depend on `path`, up to `depth` levels.
// depth=1 returns direct importers only. depth=0 means unlimited.
func (g *ImportGraph) Downstream(path string, depth int) []string {
	return g.traverse(path, depth, func(n *Node) []string { return n.ImportedBy })
}

func (g *ImportGraph) traverse(start string, maxDepth int, next func(*Node) []string) []string {
	visited := make(map[string]bool)
	visited[start] = true

	type entry struct {
		path  string
		depth int
	}
	queue := []entry{{path: start, depth: 0}}
	var result []string

	for len(queue) > 0 {
		cur := queue[0]
		queue = queue[1:]

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

// AllPaths returns all file paths in the graph, sorted.
func (g *ImportGraph) AllPaths() []string {
	paths := make([]string, 0, len(g.nodes))
	for p := range g.nodes {
		paths = append(paths, p)
	}
	return paths
}

// BuildGraph parses all source files in workDir and builds the import graph.
// It resolves local imports to relative file paths where possible.
func BuildGraph(workDir string, parsers []Parser) (*ImportGraph, []*FileInfo, error) {
	graph := NewImportGraph()
	var allFiles []*FileInfo

	skipDirs := map[string]bool{
		"node_modules": true, "vendor": true, ".git": true,
		".next": true, "dist": true, "build": true, "target": true,
		".venv": true, "venv": true, "__pycache__": true,
	}

	err := filepath.WalkDir(workDir, func(path string, d fs.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		if d.IsDir() {
			if skipDirs[d.Name()] {
				return filepath.SkipDir
			}
			depth := strings.Count(path[len(workDir):], string(filepath.Separator))
			if depth > 5 {
				return filepath.SkipDir
			}
			return nil
		}

		relPath, relErr := filepath.Rel(workDir, path)
		if relErr != nil {
			return nil
		}

		p := ParserForFile(path, parsers)
		if p == nil {
			return nil
		}

		content, readErr := os.ReadFile(path)
		if readErr != nil {
			return nil
		}

		info, parseErr := p.Parse(relPath, content)
		if parseErr != nil {
			return nil
		}

		var resolvedImports []string
		for _, imp := range info.Imports {
			resolved := resolveImport(workDir, relPath, imp.Path, p.Language())
			if resolved != "" {
				resolvedImports = append(resolvedImports, resolved)
			}
		}

		graph.AddNode(relPath, resolvedImports, info.Language)
		allFiles = append(allFiles, info)

		return nil
	})

	return graph, allFiles, err
}

// resolveImport attempts to map an import path to a local file path.
// Returns empty string if the import is external (not a local file).
func resolveImport(workDir, fromFile, importPath, language string) string {
	switch language {
	case "go":
		return resolveGoImport(workDir, fromFile, importPath)
	case "typescript":
		return resolveTSImport(workDir, fromFile, importPath)
	case "python":
		return resolvePyImport(workDir, fromFile, importPath)
	case "rust":
		return resolveRustImport(workDir, fromFile, importPath)
	}
	return ""
}

func resolveGoImport(workDir, fromFile, importPath string) string {
	if !strings.Contains(importPath, ".") {
		return ""
	}
	return ""
}

func resolveTSImport(workDir, fromFile, importPath string) string {
	if !strings.HasPrefix(importPath, ".") && !strings.HasPrefix(importPath, "/") {
		return ""
	}

	fromDir := filepath.Dir(fromFile)
	candidate := filepath.Join(fromDir, importPath)

	extensions := []string{".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"}
	for _, ext := range extensions {
		relPath := candidate + ext
		if _, err := os.Stat(filepath.Join(workDir, relPath)); err == nil {
			return filepath.ToSlash(relPath)
		}
	}

	if filepath.Ext(candidate) == "" {
		for _, ext := range extensions {
			relPath := filepath.Join(candidate, "index"+ext)
			if _, err := os.Stat(filepath.Join(workDir, relPath)); err == nil {
				return filepath.ToSlash(relPath)
			}
		}
	}

	return ""
}

func resolvePyImport(workDir, fromFile, importPath string) string {
	if strings.HasPrefix(importPath, ".") {
		fromDir := filepath.Dir(fromFile)
		depth := strings.Count(importPath, ".") - 1
		base := fromDir
		for i := 0; i < depth; i++ {
			base = filepath.Dir(base)
		}
		module := strings.TrimLeft(importPath, ".")
		if module == "" {
			return ""
		}
		modulePath := strings.ReplaceAll(module, ".", string(filepath.Separator))
		relPath := filepath.Join(base, modulePath) + ".py"
		if _, err := os.Stat(filepath.Join(workDir, relPath)); err == nil {
			return filepath.ToSlash(relPath)
		}
		initPath := filepath.Join(base, modulePath, "__init__.py")
		if _, err := os.Stat(filepath.Join(workDir, initPath)); err == nil {
			return filepath.ToSlash(initPath)
		}
		return ""
	}

	modulePath := strings.ReplaceAll(importPath, ".", string(filepath.Separator))
	relPath := modulePath + ".py"
	if _, err := os.Stat(filepath.Join(workDir, relPath)); err == nil {
		return filepath.ToSlash(relPath)
	}
	initPath := filepath.Join(modulePath, "__init__.py")
	if _, err := os.Stat(filepath.Join(workDir, initPath)); err == nil {
		return filepath.ToSlash(initPath)
	}
	return ""
}

func resolveRustImport(workDir, fromFile, usePath string) string {
	if !strings.HasPrefix(usePath, "crate::") {
		return ""
	}

	modulePath := strings.TrimPrefix(usePath, "crate::")
	parts := strings.Split(modulePath, "::")
	candidate := filepath.Join(parts...) + ".rs"
	if _, err := os.Stat(filepath.Join(workDir, "src", candidate)); err == nil {
		return filepath.ToSlash(filepath.Join("src", candidate))
	}
	modCandidate := filepath.Join(parts...)
	modPath := filepath.Join(modCandidate, "mod.rs")
	if _, err := os.Stat(filepath.Join(workDir, "src", modPath)); err == nil {
		return filepath.ToSlash(filepath.Join("src", modPath))
	}
	return ""
}
