package codeintel

import (
	"context"
	"fmt"
	"os"
	"strings"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/codeintel/efie"
	"github.com/eshanized/M31A/internal/fileutil"
)

// useEFIE reports whether EFIE should be used as the backend.
// Set USE_EFIE=true to enable the EFIE backend.
func useEFIE() bool {
	return os.Getenv("USE_EFIE") == "true"
}

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

	// EFIE backend (optional)
	efieIndexer *efie.EFIEIndexer
}

// NewIndexer creates a new codebase indexer for the given working directory.
// The indexer is lazy — call Build() before using query methods.
// When USE_EFIE=true, the EFIE backend is used instead.
func NewIndexer(workDir string) *Indexer {
	idx := &Indexer{
		workDir: workDir,
		parsers: AllParsers(),
	}
	if useEFIE() {
		idx.efieIndexer = efie.NewEFIEIndexer(workDir)
	}
	return idx
}

// Build parses all source files in the working directory and builds the
// import graph, symbol index, and relevance scorer.
func (idx *Indexer) Build(ctx context.Context) error {
	if idx.efieIndexer != nil {
		return idx.efieIndexer.Build(ctx)
	}

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
	if idx.efieIndexer != nil {
		return idx.efieIndexer.IsBuilt()
	}
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	return idx.graph != nil
}

// BuiltAt returns when the indexer was last built.
func (idx *Indexer) BuiltAt() time.Time {
	if idx.efieIndexer != nil {
		return idx.efieIndexer.BuiltAt()
	}
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	return idx.builtAt
}

// FileCount returns the number of parsed source files.
func (idx *Indexer) FileCount() int {
	if idx.efieIndexer != nil {
		return idx.efieIndexer.FileCount()
	}
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.graph == nil {
		return 0
	}
	return idx.graph.NodeCount()
}

// SymbolCount returns the number of unique symbols.
func (idx *Indexer) SymbolCount() int {
	if idx.efieIndexer != nil {
		return idx.efieIndexer.SymbolCount()
	}
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.index == nil {
		return 0
	}
	return idx.index.SymbolCount()
}

// Upstream returns files that the given path depends on, up to depth levels.
func (idx *Indexer) Upstream(path string, depth int) []string {
	if idx.efieIndexer != nil {
		return idx.efieIndexer.Upstream(path, depth)
	}
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.graph == nil {
		return nil
	}
	return idx.graph.Upstream(path, depth)
}

// Downstream returns files that depend on the given path, up to depth levels.
func (idx *Indexer) Downstream(path string, depth int) []string {
	if idx.efieIndexer != nil {
		return idx.efieIndexer.Downstream(path, depth)
	}
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.graph == nil {
		return nil
	}
	return idx.graph.Downstream(path, depth)
}

// Neighbors returns direct imports and importers of a file.
func (idx *Indexer) Neighbors(path string) (imports []string, importedBy []string) {
	if idx.efieIndexer != nil {
		return idx.efieIndexer.Neighbors(path)
	}
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.graph == nil {
		return nil, nil
	}
	return idx.graph.Neighbors(path)
}

// Define returns where a symbol is defined.
func (idx *Indexer) Define(symbol string) []SymbolLocation {
	if idx.efieIndexer != nil {
		locs := idx.efieIndexer.Define(symbol)
		if locs == nil {
			return nil
		}
		result := make([]SymbolLocation, len(locs))
		for i, loc := range locs {
			result[i] = SymbolLocation{File: loc.File, Kind: loc.Kind}
		}
		return result
	}
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.index == nil {
		return nil
	}
	return idx.index.Define(symbol)
}

// FileSymbols returns all symbols defined in a file.
func (idx *Indexer) FileSymbols(path string) []SymbolInfo {
	if idx.efieIndexer != nil {
		syms := idx.efieIndexer.FileSymbols(path)
		if syms == nil {
			return nil
		}
		result := make([]SymbolInfo, len(syms))
		for i, s := range syms {
			result[i] = SymbolInfo{Name: s.Name, Kind: s.Kind, Exported: s.Exported}
		}
		return result
	}
	idx.mu.RLock()
	defer idx.mu.RUnlock()
	if idx.index == nil {
		return nil
	}
	return idx.index.FileSymbols(path)
}

// SymbolsMatching returns symbols whose names contain the query substring.
func (idx *Indexer) SymbolsMatching(query string) []string {
	if idx.efieIndexer != nil {
		return idx.efieIndexer.SymbolsMatching(query)
	}
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
	if idx.efieIndexer != nil {
		efieScored := idx.efieIndexer.RelevantFiles(targetFiles, description, topN)
		if efieScored == nil {
			return nil
		}
		result := make([]ScoredFile, len(efieScored))
		for i, sf := range efieScored {
			result[i] = ScoredFile{Path: sf.Path, Score: sf.Score, Reasons: sf.Reasons}
		}
		return result
	}
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
	if idx.efieIndexer != nil {
		return idx.efieIndexer.FormatContext(targetFiles, description, topN, maxBytes)
	}

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
	if idx.efieIndexer != nil {
		return idx.efieIndexer.ProjectSummary(maxBytes)
	}

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
