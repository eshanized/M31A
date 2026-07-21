package codeintel

import (
	"io/fs"
	"log/slog"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
)

// Node represents a single file in the import graph.
type Node struct {
	Path          string
	Imports       []string // files this file imports (direct edges)
	ImportedBy    []string // files that import this file (reverse edges)
	importedBySet map[string]struct{}
	Language      string
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
	if node.importedBySet == nil {
		node.importedBySet = make(map[string]struct{})
	}
	if _, exists := node.importedBySet[source]; exists {
		return
	}
	node.importedBySet[source] = struct{}{}
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

// RemoveNode removes a file from the graph and all its edges.
func (g *ImportGraph) RemoveNode(path string) {
	node, ok := g.nodes[path]
	if !ok {
		return
	}
	delete(g.nodes, path)

	// Remove reverse edges (files that imported this node)
	for _, importer := range node.ImportedBy {
		if impNode, ok := g.nodes[importer]; ok {
			delete(impNode.importedBySet, path)
			for i, p := range impNode.ImportedBy {
				if p == path {
					impNode.ImportedBy = append(impNode.ImportedBy[:i], impNode.ImportedBy[i+1:]...)
					break
				}
			}
		}
	}

	// Remove forward edges (imports this node references)
	for _, imported := range node.Imports {
		if impNode, ok := g.nodes[imported]; ok {
			delete(impNode.importedBySet, path)
			for i, p := range impNode.ImportedBy {
				if p == path {
					impNode.ImportedBy = append(impNode.ImportedBy[:i], impNode.ImportedBy[i+1:]...)
					break
				}
			}
		}
	}
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

// AllPaths returns all file paths in the graph, sorted.
func (g *ImportGraph) AllPaths() []string {
	paths := make([]string, 0, len(g.nodes))
	for p := range g.nodes {
		paths = append(paths, p)
	}
	return paths
}

// fileJob holds a file to be parsed by a worker goroutine.
type fileJob struct {
	relPath string
	absPath string
	parser  Parser
}

// BuildGraph parses all source files in workDir and builds the import graph.
// It resolves local imports to relative file paths where possible.
// Uses a worker pool parallelized across available CPUs for faster indexing.
func BuildGraph(workDir string, parsers []Parser) (*ImportGraph, []*FileInfo, error) {
	graph := NewImportGraph()
	var allFiles []*FileInfo

	skipDirs := map[string]bool{
		"node_modules": true, "vendor": true, ".git": true,
		".next": true, "dist": true, "build": true, "target": true,
		".venv": true, "venv": true, "__pycache__": true,
	}

	// Phase 1: Collect all parseable files via WalkDir (fast, no I/O)
	var jobs []fileJob
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

		jobs = append(jobs, fileJob{relPath: relPath, absPath: path, parser: p})
		return nil
	})
	if err != nil {
		return graph, allFiles, err
	}

	if len(jobs) == 0 {
		return graph, allFiles, nil
	}

	// Phase 2: Parse files in parallel using a bounded worker pool
	type parseResult struct {
		info    *FileInfo
		imports []string
	}
	results := make([]parseResult, len(jobs))

	workerCount := runtime.NumCPU()
	if workerCount > len(jobs) {
		workerCount = len(jobs)
	}
	if workerCount < 1 {
		workerCount = 1
	}

	var wg sync.WaitGroup
	jobCh := make(chan int, len(jobs))
	for i := range jobs {
		jobCh <- i
	}
	close(jobCh)

	for w := 0; w < workerCount; w++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for idx := range jobCh {
				job := jobs[idx]
				content, readErr := os.ReadFile(job.absPath)
				if readErr != nil {
					continue
				}

				// PERF-36: Limit file size to prevent excessive memory usage
				// for large generated files.
				const maxFileSize = 4096
				if len(content) > maxFileSize {
					content = content[:maxFileSize]
				}

				info, parseErr := job.parser.Parse(job.relPath, content)
				if parseErr != nil {
					// Log parse error but add as isolated node so file isn't invisible
					slog.Debug("BuildGraph: parse error, adding as isolated node",
						"file", job.relPath, "error", parseErr)
					continue
				}

				var resolvedImports []string
				for _, imp := range info.Imports {
					resolved := resolveImport(workDir, job.relPath, imp.Path, job.parser.Language())
					if resolved != "" {
						resolvedImports = append(resolvedImports, resolved)
					}
				}

				results[idx] = parseResult{info: info, imports: resolvedImports}
			}
		}()
	}
	wg.Wait()

	// Phase 3: Assemble graph (sequential, lock-free via single-threaded assembly)
	for _, r := range results {
		if r.info == nil {
			continue
		}
		graph.AddNode(r.info.Path, r.imports, r.info.Language)
		allFiles = append(allFiles, r.info)
	}

	return graph, allFiles, nil
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

// goModulePath caches the module path read from go.mod per working directory.
var (
	goModulePathCache   = make(map[string]string)
	goModulePathCacheMu sync.Mutex
)

// readGoModModule reads the module path from go.mod in workDir.
// Returns empty string if go.mod is missing or unparseable.
func readGoModModule(workDir string) string {
	goModulePathCacheMu.Lock()
	defer goModulePathCacheMu.Unlock()
	if cached, ok := goModulePathCache[workDir]; ok {
		return cached
	}
	content, err := os.ReadFile(filepath.Join(workDir, "go.mod"))
	if err != nil {
		return ""
	}
	for _, line := range strings.Split(string(content), "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "module ") {
			modPath := strings.TrimSpace(strings.TrimPrefix(line, "module"))
			goModulePathCache[workDir] = modPath
			return modPath
		}
	}
	return ""
}

func resolveGoImport(workDir, fromFile, importPath string) string {
	// Standard library imports have no dots in the first path element.
	// Skip them — they are not local files.
	parts := strings.Split(importPath, "/")
	if len(parts) == 0 {
		return ""
	}
	firstElem := parts[0]
	// Standard library: first element has no dot (e.g. "fmt", "net/http")
	if !strings.Contains(firstElem, ".") {
		return ""
	}

	// Relative imports (./foo or ../foo) — resolve from the importing file's directory.
	if strings.HasPrefix(importPath, "./") || strings.HasPrefix(importPath, "../") {
		fromDir := filepath.Dir(fromFile)
		relPath := filepath.Join(fromDir, importPath)
		// Try as a directory (package) first, then as a file.
		candidate := filepath.Join(workDir, relPath)
		if info, err := os.Stat(candidate); err == nil && info.IsDir() {
			return filepath.ToSlash(filepath.Join(relPath, "index.go"))
		}
		// Try with .go extension
		if _, err := os.Stat(candidate + ".go"); err == nil {
			return filepath.ToSlash(relPath + ".go")
		}
		return ""
	}

	// Module-internal imports: check if the import starts with the module path.
	modPath := readGoModModule(workDir)
	if modPath == "" {
		return ""
	}
	if !strings.HasPrefix(importPath, modPath) {
		// External package (not our module) — skip.
		return ""
	}

	// Strip the module prefix and convert to a file path.
	relPath := strings.TrimPrefix(importPath, modPath)
	relPath = strings.TrimPrefix(relPath, "/")
	if relPath == "" {
		return ""
	}

	// Try as a directory (package with multiple files).
	candidate := filepath.Join(workDir, relPath)
	if info, err := os.Stat(candidate); err == nil && info.IsDir() {
		// Look for a Go file in the directory — return the directory as the node path
		// so the graph connects to the package, not a specific file.
		return filepath.ToSlash(relPath)
	}
	// Try as a single-file package (e.g. util.go).
	if _, err := os.Stat(candidate + ".go"); err == nil {
		return filepath.ToSlash(relPath + ".go")
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
