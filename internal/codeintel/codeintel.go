package codeintel

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/fileutil"
)

// Indexer provides a unified API for codebase intelligence: parsing, import
// graph traversal, symbol lookup, and relevance scoring.
type Indexer struct {
	workDir string
	parsers []Parser

	mu      sync.RWMutex
	graph   *ImportGraph
	index   *SymbolIndex
	scorer  *RelevanceScorer
	builtAt time.Time
	files   []*FileInfo
}

// NewIndexer creates a new codebase indexer for the given working directory.
// The indexer is lazy — call Build() before using query methods.
func NewIndexer(workDir string) *Indexer {
	return &Indexer{
		workDir: workDir,
		parsers: AllParsers(),
	}
}

// Build parses all source files in the working directory and builds the
// import graph, symbol index, and relevance scorer. Uses incremental
// builds when a previous cache exists — only changed files are reparsed.
func (idx *Indexer) Build(ctx context.Context) error {
	idx.mu.Lock()
	defer idx.mu.Unlock()

	if err := ctx.Err(); err != nil {
		return err
	}

	// Try incremental build first
	if idx.graph != nil && idx.files != nil {
		return idx.buildIncremental(ctx)
	}

	// Try loading from disk cache
	if cached := LoadCache(idx.workDir); cached != nil {
		return idx.buildFromCache(ctx, cached)
	}

	// Full build
	return idx.buildFull(ctx)
}

// buildFull performs a full index build from scratch.
func (idx *Indexer) buildFull(ctx context.Context) error {
	graph, files, err := BuildGraph(idx.workDir, idx.parsers)
	if err != nil {
		return fmt.Errorf("build graph: %w", err)
	}

	symIndex := BuildIndex(files)

	idx.graph = graph
	idx.index = symIndex
	idx.scorer = NewRelevanceScorer(graph, symIndex)
	idx.files = files
	idx.builtAt = time.Now()

	// Save cache
	fileCaches := BuildCacheFromFiles(idx.workDir, files, nil)
	cache := &IndexCache{Files: fileCaches}
	_ = SaveCache(idx.workDir, cache)

	return nil
}

// buildFromCache rebuilds from a disk cache, using cached results for unchanged files.
func (idx *Indexer) buildFromCache(ctx context.Context, cached *IndexCache) error {
	inc := CheckIncremental(idx.workDir, idx.parsers, cached.Files)

	graph := NewImportGraph()
	var allFiles []*FileInfo

	// Add unchanged files from cache
	for _, info := range inc.Cache {
		allFiles = append(allFiles, info)
	}

	// Remove deleted files from graph and index
	if len(inc.Deleted) > 0 {
		for _, path := range inc.Deleted {
			graph.RemoveNode(path)
		}
	}

	// Parse changed/new files
	if len(inc.Changed) > 0 {
		changedFiles, err := idx.parseFiles(ctx, inc.Changed)
		if err != nil {
			return fmt.Errorf("parse changed files: %w", err)
		}
		allFiles = append(allFiles, changedFiles...)
	}

	// Build graph from all files
	for _, f := range allFiles {
		var resolvedImports []string
		for _, imp := range f.Imports {
			p := ParserForFile(f.Path, idx.parsers)
			if p == nil {
				continue
			}
			resolved := resolveImport(idx.workDir, f.Path, imp.Path, p.Language())
			if resolved != "" {
				resolvedImports = append(resolvedImports, resolved)
			}
		}
		graph.AddNode(f.Path, resolvedImports, f.Language)
	}

	symIndex := BuildIndex(allFiles)

	idx.graph = graph
	idx.index = symIndex
	idx.scorer = NewRelevanceScorer(graph, symIndex)
	idx.files = allFiles
	idx.builtAt = time.Now()

	// Save updated cache
	fileCaches := BuildCacheFromFiles(idx.workDir, allFiles, nil)
	// Merge with unchanged cache entries
	for path, fc := range cached.Files {
		if _, ok := fileCaches[path]; !ok {
			fileCaches[path] = fc
		}
	}
	cache := &IndexCache{Files: fileCaches}
	_ = SaveCache(idx.workDir, cache)

	return nil
}

// buildIncremental updates an existing in-memory index with only changed files.
func (idx *Indexer) buildIncremental(ctx context.Context) error {
	cached := LoadCache(idx.workDir)
	if cached == nil {
		// No cache available — fall back to full build
		return idx.buildFull(ctx)
	}

	// Build file cache from current in-memory files
	currentCache := make(map[string]*FileCache)
	for _, f := range idx.files {
		absPath := filepath.Join(idx.workDir, f.Path)
		hash, err := fileHash(absPath)
		if err != nil {
			continue
		}
		info, err := os.Stat(absPath)
		if err != nil {
			continue
		}
		currentCache[f.Path] = &FileCache{
			Path:    f.Path,
			Hash:    hash,
			ModTime: info.ModTime(),
			Info:    f,
		}
	}

	inc := CheckIncremental(idx.workDir, idx.parsers, currentCache)

	// Remove deleted files from graph and index
	if len(inc.Deleted) > 0 {
		for _, path := range inc.Deleted {
			idx.graph.RemoveNode(path)
		}
		idx.index.RemoveFiles(inc.Deleted)
	}

	// Parse changed/new files
	if len(inc.Changed) > 0 {
		changedFiles, err := idx.parseFiles(ctx, inc.Changed)
		if err != nil {
			return fmt.Errorf("parse changed files: %w", err)
		}

		// Update graph and index with changed files
		for _, f := range changedFiles {
			// Remove old entries if file was previously indexed
			idx.graph.RemoveNode(f.Path)
			idx.index.RemoveFile(f.Path)

			var resolvedImports []string
			for _, imp := range f.Imports {
				p := ParserForFile(f.Path, idx.parsers)
				if p == nil {
					continue
				}
				resolved := resolveImport(idx.workDir, f.Path, imp.Path, p.Language())
				if resolved != "" {
					resolvedImports = append(resolvedImports, resolved)
				}
			}
			idx.graph.AddNode(f.Path, resolvedImports, f.Language)
			idx.index.AddFile(f)

			// Update files list
			found := false
			for i, existing := range idx.files {
				if existing.Path == f.Path {
					idx.files[i] = f
					found = true
					break
				}
			}
			if !found {
				idx.files = append(idx.files, f)
			}
		}
	}

	idx.scorer = NewRelevanceScorer(idx.graph, idx.index)
	idx.builtAt = time.Now()

	// Save updated cache
	fileCaches := BuildCacheFromFiles(idx.workDir, idx.files, nil)
	cache := &IndexCache{Files: fileCaches}
	_ = SaveCache(idx.workDir, cache)

	return nil
}

// parseFiles parses a list of relative file paths and returns their FileInfo.
func (idx *Indexer) parseFiles(ctx context.Context, paths []string) ([]*FileInfo, error) {
	var results []*FileInfo
	for _, relPath := range paths {
		if err := ctx.Err(); err != nil {
			return nil, err
		}
		absPath := filepath.Join(idx.workDir, relPath)
		p := ParserForFile(absPath, idx.parsers)
		if p == nil {
			continue
		}
		content, err := os.ReadFile(absPath)
		if err != nil {
			continue
		}
		const maxFileSize = 4096
		if len(content) > maxFileSize {
			content = content[:maxFileSize]
		}
		info, err := p.Parse(relPath, content)
		if err != nil {
			continue
		}
		results = append(results, info)
	}
	return results, nil
}

// IsBuilt reports whether the indexer has been built.
func (idx *Indexer) IsBuilt() bool {
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	return idx.graph != nil
}

// BuiltAt returns when the indexer was last built.
func (idx *Indexer) BuiltAt() time.Time {
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	return idx.builtAt
}

// FileCount returns the number of parsed source files.
func (idx *Indexer) FileCount() int {
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.graph == nil {
		return 0
	}
	return idx.graph.NodeCount()
}

// SymbolCount returns the number of unique symbols.
func (idx *Indexer) SymbolCount() int {
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.index == nil {
		return 0
	}
	return idx.index.SymbolCount()
}

// Upstream returns files that the given path depends on, up to depth levels.
func (idx *Indexer) Upstream(path string, depth int) []string {
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.graph == nil {
		return nil
	}
	return idx.graph.Upstream(path, depth)
}

// Downstream returns files that depend on the given path, up to depth levels.
func (idx *Indexer) Downstream(path string, depth int) []string {
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.graph == nil {
		return nil
	}
	return idx.graph.Downstream(path, depth)
}

// Neighbors returns direct imports and importers of a file.
func (idx *Indexer) Neighbors(path string) (imports []string, importedBy []string) {
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.graph == nil {
		return nil, nil
	}
	return idx.graph.Neighbors(path)
}

// Define returns where a symbol is defined.
func (idx *Indexer) Define(symbol string) []SymbolLocation {
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.index == nil {
		return nil
	}
	return idx.index.Define(symbol)
}

// FileSymbols returns all symbols defined in a file.
func (idx *Indexer) FileSymbols(path string) []SymbolInfo {
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.index == nil {
		return nil
	}
	return idx.index.FileSymbols(path)
}

// SymbolsMatching returns symbols whose names contain the query substring.
func (idx *Indexer) SymbolsMatching(query string) []string {
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.index == nil {
		return nil
	}
	return idx.index.SymbolsMatching(query)
}

// RelevantFiles returns the top-N files most relevant to a task described
// by target files and a text description.
func (idx *Indexer) RelevantFiles(targetFiles []string, description string, topN int) []ScoredFile {
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.scorer == nil {
		return nil
	}
	return idx.scorer.Score(targetFiles, description, topN)
}

// FormatContext generates an LLM-ready text summary of relevant codebase
// intelligence for a set of target files and a task description.
// The output is capped at maxBytes.
func (idx *Indexer) FormatContext(targetFiles []string, description string, topN int, maxBytes int) string {
	idx.mu.RLock()
	defer idx.mu.RUnlock()

	if idx.graph == nil || idx.scorer == nil {
		return ""
	}

	var sb strings.Builder

	scored := idx.scorer.Score(targetFiles, description, topN)
	if len(scored) > 0 {
		sb.WriteString("## Recommended Files (by relevance)\n\n")
		for _, sf := range scored {
			fmt.Fprintf(&sb, "- **%s** (score: %.1f)", sf.Path, sf.Score)
			if len(sf.Reasons) > 0 {
				sb.WriteString(" — " + strings.Join(sf.Reasons[:min(5, len(sf.Reasons))], "; "))
			}
			sb.WriteString("\n")
		}
		sb.WriteString("\n")
	}

	for _, target := range targetFiles {
		imports, importedBy := idx.graph.Neighbors(target)
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

	if description != "" && idx.index != nil {
		words := extractIdentifiers(description)
		var typeMatches []string
		for _, w := range words {
			locs := idx.index.FindTypes(w)
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

// ProjectSummary returns a high-level summary of the project structure
// suitable for the plan phase context.
func (idx *Indexer) ProjectSummary(maxBytes int) string {
	idx.mu.RLock()
	defer idx.mu.RUnlock()

	if idx.graph == nil || idx.index == nil {
		return ""
	}

	var sb strings.Builder
	sb.WriteString("## Codebase Overview\n\n")
	fmt.Fprintf(&sb, "- **Source files**: %d\n", idx.graph.NodeCount())
	fmt.Fprintf(&sb, "- **Unique symbols**: %d\n\n", idx.index.SymbolCount())

	pkgFiles := make(map[string]int)
	for _, f := range idx.files {
		dir := fileutil.DirOf(f.Path)
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
	for _, name := range idx.index.AllSymbols() {
		locs := idx.index.FindTypes(name)
		for _, loc := range locs {
			if isExportedSymbol(name) {
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

func isExportedSymbol(name string) bool {
	if len(name) == 0 {
		return false
	}
	r := rune(name[0])
	return r >= 'A' && r <= 'Z'
}
