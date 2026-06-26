package efie

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/fileutil"
)

// EFIEIndex is the main EFIE index containing the graph and multi-resolution index.
type EFIEIndex struct {
	workDir string
	graph   *WeightedImportGraph
	index   *MultiResIndex
	files   []*fileInfo
	built   bool

	// Query cache for repeated relevance queries (PERF-41)
	queryCache *QueryCache
}

// NewEFIEIndex creates a new EFIE index for the given working directory.
func NewEFIEIndex(workDir string) *EFIEIndex {
	return &EFIEIndex{
		workDir:    workDir,
		queryCache: NewQueryCache(128), // Cache last 128 queries
	}
}

// Build constructs the EFIE index from source files.
func (e *EFIEIndex) Build(ctx context.Context) error {
	if e.built {
		return nil
	}

	if err := ctx.Err(); err != nil {
		return err
	}

	start := time.Now()

	// Phase 1: Parallel Discovery + Parse
	parsedFiles, fileSet, err := e.parallelParse(ctx)
	if err != nil {
		return fmt.Errorf("parallel parse: %w", err)
	}

	// Phase 2: Graph Construction + Import Resolution
	e.graph = e.buildGraph(parsedFiles, fileSet)

	// Phase 3: Community Detection (2 passes for production speed)
	communities := LouvainDetect_Deterministic(e.graph, 42, 2)
	for _, node := range e.graph.nodes {
		if node.Path == ExternalNode {
			continue
		}
		node.Community = communities[node.Path]
	}

	// Build community adjacency map
	communityAdj := e.buildCommunityAdjacency(communities)

	// Phase 4: Centrality Precomputation (fast mode: in-degree instead of betweenness)
	pageRank := ComputePageRank(e.graph, 10, 0.85) // 10 iterations (was 20)
	// Use in-degree centrality (O(|V|)) instead of betweenness (O(|V|²/5))
	betweenness := ComputeInDegreeCentrality(e.graph)

	for _, node := range e.graph.nodes {
		node.PageRank = pageRank[node.Path]
		node.Betweenness = betweenness[node.Path]
		node.DegreeCentrality = float64(len(node.Imports)+len(node.ImportedBy)) / float64(2*e.graph.NodeCount())
	}

	// Phase 5: Index Construction
	e.index = NewMultiResIndex()

	// File-level
	for _, fi := range parsedFiles {
		e.index.AddFile(fi.Path, fi.Language, fi.Symbols)
	}

	// Community-level
	for _, node := range e.graph.nodes {
		if node.Path != ExternalNode {
			e.index.SetCommunity(node.Path, node.Community)
		}
	}
	e.index.SetCommunityAdj(communityAdj)

	// Importance ranking
	importanceRank := e.buildImportanceRank(pageRank)
	e.index.SetImportanceRank(importanceRank)

	// Centrality percentiles
	var centralityValues []float64
	for _, node := range e.graph.nodes {
		centralityValues = append(centralityValues, node.PageRank)
	}
	p50 := ComputePercentile(centralityValues, 50)
	p95 := ComputePercentile(centralityValues, 95)
	e.index.SetCentralityPercentiles(p50, p95)

	// Symbol Trie
	e.index.BuildTrieIndex()

	// Bloom filters per file
	for _, node := range e.graph.nodes {
		if node.Path == ExternalNode {
			continue
		}
		node.ImportSet = make(map[string]bool, len(node.Imports))
		for _, imp := range node.Imports {
			node.ImportSet[imp] = true
		}
		// Build bloom filter for symbols in this file
		fi := e.findFileInfo(node.Path)
		if fi != nil {
			var symbolNames []string
			for _, s := range fi.Symbols {
				symbolNames = append(symbolNames, s.Name)
			}
			if len(symbolNames) > 0 {
				node.SymbolBloom = NewBloomFilter(len(symbolNames), 0.01)
				for _, name := range symbolNames {
					node.SymbolBloom.Add(name)
				}
			}
		}
	}

	e.files = parsedFiles
	e.built = true

	fmt.Printf("EFIE build completed in %v (%d files, %d communities)\n",
		time.Since(start), e.graph.NodeCount(), len(communityAdj))

	return nil
}

func (e *EFIEIndex) parallelParse(ctx context.Context) ([]*fileInfo, map[string]bool, error) {
	skipDirs := map[string]bool{
		"node_modules": true, "vendor": true, ".git": true,
		".next": true, "dist": true, "build": true, "target": true,
		".venv": true, "venv": true, "__pycache__": true,
	}

	parsers := AllParsers()

	// Walk to collect paths
	var paths []string
	err := filepath.WalkDir(e.workDir, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return nil
		}
		if d.IsDir() {
			if skipDirs[d.Name()] {
				return filepath.SkipDir
			}
			depth := strings.Count(path[len(e.workDir):], string(filepath.Separator))
			if depth > 5 {
				return filepath.SkipDir
			}
			return nil
		}
		relPath, relErr := filepath.Rel(e.workDir, path)
		if relErr != nil {
			return nil
		}
		if p := ParserForFile(relPath, parsers); p != nil {
			paths = append(paths, relPath)
		}
		return nil
	})
	if err != nil {
		return nil, nil, err
	}

	// Parallel parse
	numWorkers := runtime.NumCPU()
	if numWorkers > len(paths) {
		numWorkers = len(paths)
	}
	if numWorkers < 1 {
		numWorkers = 1
	}

	type parseResult struct {
		info *fileInfo
		path string
	}

	resultChan := make(chan parseResult, len(paths))
	pathChan := make(chan string, len(paths))

	for _, p := range paths {
		pathChan <- p
	}
	close(pathChan)

	var wg sync.WaitGroup
	for i := 0; i < numWorkers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for relPath := range pathChan {
				select {
				case <-ctx.Done():
					return
				default:
				}

				absPath := filepath.Join(e.workDir, relPath)
				content, readErr := os.ReadFile(absPath)
				if readErr != nil {
					continue
				}

				p := ParserForFile(relPath, parsers)
				if p == nil {
					continue
				}

				// Non-Go: read first 4KB
				if p.Language() != "go" && len(content) > 4096 {
					content = content[:4096]
				}

				info, parseErr := p.Parse(relPath, content)
				if parseErr != nil {
					continue
				}

				fi := &fileInfo{
					Path:     info.Path,
					Language: info.Language,
					Imports:  info.Imports,
					Symbols:  make([]SymbolInfo, 0),
				}
				// PERF-37: Use map for O(1) deduplication instead of O(N) linear scan
				seenSymbols := make(map[string]bool)
				for _, s := range info.Exports {
					if !seenSymbols[s.Name] {
						seenSymbols[s.Name] = true
						fi.Symbols = append(fi.Symbols, SymbolInfo(s))
					}
				}
				for _, f := range info.Funcs {
					if !seenSymbols[f.Name] {
						seenSymbols[f.Name] = true
						fi.Symbols = append(fi.Symbols, SymbolInfo{
							Name: f.Name, Kind: "func", Exported: f.Exported,
						})
					}
				}

				resultChan <- parseResult{info: fi, path: relPath}
			}
		}()
	}

	go func() {
		wg.Wait()
		close(resultChan)
	}()

	var allFiles []*fileInfo
	fileSet := make(map[string]bool)
	for r := range resultChan {
		allFiles = append(allFiles, r.info)
		fileSet[r.path] = true
	}

	return allFiles, fileSet, nil
}

func (e *EFIEIndex) buildGraph(parsedFiles []*fileInfo, fileSet map[string]bool) *WeightedImportGraph {
	graph := NewWeightedImportGraph()

	for _, fi := range parsedFiles {
		// Use imports already parsed in parallelParse() — no re-reading files
		var resolvedImports []string
		for _, imp := range fi.Imports {
			resolved := resolveImport(e.workDir, fi.Path, imp.Path, fi.Language, fileSet)
			if resolved != "" {
				resolvedImports = append(resolvedImports, resolved)
			} else {
				resolvedImports = append(resolvedImports, ExternalNode)
			}
		}
		graph.AddNode(fi.Path, resolvedImports, fi.Language)
	}

	graph.SetExternal(ExternalNode)
	return graph
}

func (e *EFIEIndex) buildCommunityAdjacency(communities map[string]int) map[int]map[int]bool {
	adj := make(map[int]map[int]bool)
	for _, node := range e.graph.nodes {
		if node.Path == ExternalNode {
			continue
		}
		nodeComm := communities[node.Path]
		for _, imp := range node.Imports {
			if imp == ExternalNode {
				continue
			}
			impComm, ok := communities[imp]
			if ok && impComm != nodeComm {
				if adj[nodeComm] == nil {
					adj[nodeComm] = make(map[int]bool)
				}
				if adj[impComm] == nil {
					adj[impComm] = make(map[int]bool)
				}
				adj[nodeComm][impComm] = true
				adj[impComm][nodeComm] = true
			}
		}
	}
	return adj
}

func (e *EFIEIndex) buildImportanceRank(pageRank map[string]float64) []string {
	type pathScore struct {
		path  string
		score float64
	}
	var pairs []pathScore
	for path, pr := range pageRank {
		if path != ExternalNode {
			pairs = append(pairs, pathScore{path, pr})
		}
	}
	sort.Slice(pairs, func(i, j int) bool {
		return pairs[i].score > pairs[j].score
	})
	rank := make([]string, len(pairs))
	for i, p := range pairs {
		rank[i] = p.path
	}
	return rank
}

func (e *EFIEIndex) findFileInfo(path string) *fileInfo {
	for _, fi := range e.files {
		if fi.Path == path {
			return fi
		}
	}
	return nil
}

// IsBuilt reports whether the index has been built.
func (e *EFIEIndex) IsBuilt() bool {
	return e.built
}

// FileCount returns the number of indexed source files.
func (e *EFIEIndex) FileCount() int {
	if e.index == nil {
		return 0
	}
	return e.index.FileCount()
}

// SymbolCount returns the number of unique symbols.
func (e *EFIEIndex) SymbolCount() int {
	if e.index == nil {
		return 0
	}
	return e.index.SymbolCount()
}

// Upstream returns files that the given path depends on.
func (e *EFIEIndex) Upstream(path string, depth int) []string {
	if e.graph == nil || !e.built {
		return nil
	}
	return e.graph.Upstream(path, depth)
}

// Downstream returns files that depend on the given path.
func (e *EFIEIndex) Downstream(path string, depth int) []string {
	if e.graph == nil || !e.built {
		return nil
	}
	return e.graph.Downstream(path, depth)
}

// Define returns where a symbol is defined.
func (e *EFIEIndex) Define(symbol string) []SymbolLocation {
	if e.index == nil || !e.built {
		return nil
	}
	return e.index.Define(symbol)
}

// FileSymbols returns all symbols defined in a file.
func (e *EFIEIndex) FileSymbols(path string) []SymbolInfo {
	if e.index == nil || !e.built {
		return nil
	}
	return e.index.FileSymbols(path)
}

// SymbolsMatching returns symbols matching the query.
func (e *EFIEIndex) SymbolsMatching(query string) []string {
	if e.index == nil || !e.built {
		return nil
	}
	return e.index.SymbolsMatching(query)
}

// RelevantFiles returns top-N files relevant to the given targets and description.
func (e *EFIEIndex) RelevantFiles(targetFiles []string, description string, topN int) []ScoredFile {
	if e.graph == nil || e.index == nil || !e.built {
		return nil
	}

	// Check cache first (PERF-41)
	cacheKey := ComputeCacheKey(targetFiles, description, topN)
	if cached, ok := e.queryCache.Get(cacheKey); ok {
		return cached
	}

	targetCommunities := make(map[int]bool)
	for _, t := range targetFiles {
		if c, ok := e.index.fileToCommunity[t]; ok {
			targetCommunities[c] = true
		}
	}

	result := Query(e, targetFiles, description, QueryRelevant, topN)

	// Store in cache
	e.queryCache.Put(cacheKey, result)

	return result
}

// CommunityOf returns the community ID for a file.
func (e *EFIEIndex) CommunityOf(path string) int {
	if e.index == nil {
		return -1
	}
	return e.index.CommunityOf(path)
}

// PageRankOf returns the PageRank score for a file.
func (e *EFIEIndex) PageRankOf(path string) float64 {
	if e.graph == nil {
		return 0
	}
	node, ok := e.graph.nodes[path]
	if !ok {
		return 0
	}
	return node.PageRank
}

// Communities returns all communities and their members.
func (e *EFIEIndex) Communities() map[int][]string {
	if e.index == nil {
		return nil
	}
	return e.index.Communities()
}

// FormatContext generates an LLM-ready text summary of relevant codebase intelligence.
func (e *EFIEIndex) FormatContext(targetFiles []string, description string, topN int, maxBytes int) string {
	if e.graph == nil || e.index == nil || !e.built {
		return ""
	}

	var sb strings.Builder

	targetCommunities := make(map[int]bool)
	for _, t := range targetFiles {
		if c, ok := e.index.fileToCommunity[t]; ok {
			targetCommunities[c] = true
		}
	}

	scored := e.RelevantFiles(targetFiles, description, topN)
	if len(scored) > 0 {
		sb.WriteString("## Recommended Files (by relevance)\n\n")
		for _, sf := range scored {
			fmt.Fprintf(&sb, "- **%s** (score: %.1f)", sf.Path, sf.Score)
			if len(sf.Reasons) > 0 {
				n := 5
				if len(sf.Reasons) < n {
					n = len(sf.Reasons)
				}
				sb.WriteString(" — " + strings.Join(sf.Reasons[:n], "; "))
			}
			sb.WriteString("\n")
		}
		sb.WriteString("\n")
	}

	for _, target := range targetFiles {
		imports, importedBy := e.graph.Neighbors(target)
		if len(imports) > 0 || len(importedBy) > 0 {
			fmt.Fprintf(&sb, "### Dependencies for %s\n", target)
			if len(imports) > 0 {
				fmt.Fprintf(&sb, "  Imports: %s\n", strings.Join(imports, ", "))
			}
			if len(importedBy) > 0 {
				fmt.Fprintf(&sb, "  Imported by: %s\n", strings.Join(importedBy, ", "))
			}
			sb.WriteString("\n")
		}
	}

	if description != "" && e.index != nil {
		words := extractIdentifiers(description)
		var typeMatches []string
		for _, w := range words {
			locs := e.index.Define(w)
			for _, loc := range locs {
				typeMatches = append(typeMatches, fmt.Sprintf("- **%s** (%s) defined in %s", w, loc.Kind, loc.File))
			}
		}
		if len(typeMatches) > 0 {
			sb.WriteString("### Type Definitions Referenced in Task\n\n")
			for _, m := range typeMatches {
				sb.WriteString(m + "\n")
			}
			sb.WriteString("\n")
		}
	}

	result := sb.String()
	if maxBytes > 0 && len(result) > maxBytes {
		result = result[:maxBytes] + "\n... (truncated)"
	}
	return result
}

// ProjectSummary returns a high-level summary of the project structure.
func (e *EFIEIndex) ProjectSummary(maxBytes int) string {
	if e.graph == nil || e.index == nil {
		return ""
	}

	var sb strings.Builder
	sb.WriteString("## Codebase Overview\n\n")
	fmt.Fprintf(&sb, "- **Source files**: %d\n", e.graph.NodeCount())
	fmt.Fprintf(&sb, "- **Unique symbols**: %d\n", e.index.SymbolCount())

	pkgFiles := make(map[string]int)
	for _, fi := range e.files {
		dir := fileutil.DirOf(fi.Path)
		pkgFiles[dir]++
	}
	if len(pkgFiles) > 0 {
		sb.WriteString("### Packages/Directories\n\n")
		for dir, count := range pkgFiles {
			fmt.Fprintf(&sb, "- `%s/` (%d files)\n", dir, count)
		}
		sb.WriteString("\n")
	}

	var keyTypes []string
	for _, name := range e.index.AllSymbols() {
		locs := e.index.Define(name)
		for _, loc := range locs {
			if len(name) > 0 && name[0] >= 'A' && name[0] <= 'Z' {
				keyTypes = append(keyTypes, fmt.Sprintf("- **%s** (%s) in %s", name, loc.Kind, loc.File))
			}
		}
		if len(keyTypes) >= 20 {
			break
		}
	}
	if len(keyTypes) > 0 {
		sb.WriteString("### Key Types and Interfaces\n\n")
		for _, kt := range keyTypes {
			sb.WriteString(kt + "\n")
		}
		sb.WriteString("\n")
	}

	result := sb.String()
	if maxBytes > 0 && len(result) > maxBytes {
		result = result[:maxBytes] + "\n... (truncated)"
	}
	return result
}
