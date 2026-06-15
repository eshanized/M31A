package codeintel

import (
	"context"
	"fmt"
	"strings"
	"sync"
	"time"
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
// import graph, symbol index, and relevance scorer.
func (idx *Indexer) Build(ctx context.Context) error {
	idx.mu.Lock()
	defer idx.mu.Unlock()

	if err := ctx.Err(); err != nil {
		return err
	}

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

	return nil
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
		dir := dirOf(f.Path)
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

func dirOf(path string) string {
	for i := len(path) - 1; i >= 0; i-- {
		if path[i] == '/' || path[i] == '\\' {
			if i == 0 {
				return "."
			}
			return path[:i]
		}
	}
	return "."
}

func isExportedSymbol(name string) bool {
	if len(name) == 0 {
		return false
	}
	r := rune(name[0])
	return r >= 'A' && r <= 'Z'
}
