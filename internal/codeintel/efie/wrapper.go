package efie

import (
	"context"
	"strings"
	"sync"
	"time"
)

// EFIEIndexer wraps EFIEIndex to satisfy the codeintel.Indexer interface.
// This allows existing workflow code to use EFIE without modification.
type EFIEIndexer struct {
	efie    *EFIEIndex
	mu      sync.RWMutex
	builtAt time.Time
}

// NewEFIEIndexer creates a new EFIE-backed indexer.
func NewEFIEIndexer(workDir string) *EFIEIndexer {
	return &EFIEIndexer{
		efie: NewEFIEIndex(workDir),
	}
}

// Build constructs the EFIE index.
func (e *EFIEIndexer) Build(ctx context.Context) error {
	e.mu.Lock()
	defer e.mu.Unlock()
	if err := e.efie.Build(ctx); err != nil {
		return err
	}
	e.builtAt = time.Now()
	return nil
}

// IsBuilt reports whether the indexer has been built.
func (e *EFIEIndexer) IsBuilt() bool {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.efie.IsBuilt()
}

// BuiltAt returns when the indexer was last built.
func (e *EFIEIndexer) BuiltAt() time.Time {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.builtAt
}

// FileCount returns the number of parsed source files.
func (e *EFIEIndexer) FileCount() int {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.efie.FileCount()
}

// SymbolCount returns the number of unique symbols.
func (e *EFIEIndexer) SymbolCount() int {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.efie.SymbolCount()
}

// Upstream returns files that the given path depends on.
func (e *EFIEIndexer) Upstream(path string, depth int) []string {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.efie.Upstream(path, depth)
}

// Downstream returns files that depend on the given path.
func (e *EFIEIndexer) Downstream(path string, depth int) []string {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.efie.Downstream(path, depth)
}

// Define returns where a symbol is defined.
func (e *EFIEIndexer) Define(symbol string) []SymbolLocation {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.efie.Define(symbol)
}

// FileSymbols returns all symbols defined in a file.
func (e *EFIEIndexer) FileSymbols(path string) []SymbolInfo {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.efie.FileSymbols(path)
}

// SymbolsMatching returns symbols matching the query.
func (e *EFIEIndexer) SymbolsMatching(query string) []string {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.efie.SymbolsMatching(query)
}

// RelevantFiles returns top-N files relevant to the given targets.
func (e *EFIEIndexer) RelevantFiles(targetFiles []string, description string, topN int) []ScoredFile {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.efie.RelevantFiles(targetFiles, description, topN)
}

// FormatContext generates an LLM-ready text summary.
func (e *EFIEIndexer) FormatContext(targetFiles []string, description string, topN int, maxBytes int) string {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.efie.FormatContext(targetFiles, description, topN, maxBytes)
}

// ProjectSummary returns a high-level summary of the project structure.
func (e *EFIEIndexer) ProjectSummary(maxBytes int) string {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.efie.ProjectSummary(maxBytes)
}

// CommunityOf returns the community ID for a file.
func (e *EFIEIndexer) CommunityOf(path string) int {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.efie.CommunityOf(path)
}

// PageRankOf returns the PageRank score for a file.
func (e *EFIEIndexer) PageRankOf(path string) float64 {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.efie.PageRankOf(path)
}

// Communities returns all communities and their members.
func (e *EFIEIndexer) Communities() map[int][]string {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return e.efie.Communities()
}

// Neighbors returns direct imports and importers of a file.
func (e *EFIEIndexer) Neighbors(path string) (imports []string, importedBy []string) {
	e.mu.RLock()
	defer e.mu.RUnlock()
	if e.efie.graph == nil {
		return nil, nil
	}
	return e.efie.graph.Neighbors(path)
}

// Ensure unused import is referenced
var _ = strings.TrimSpace
