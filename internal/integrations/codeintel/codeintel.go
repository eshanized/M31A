package codeintel

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/core/types"
	"github.com/eshanized/M31A/internal/infrastructure/fileutil"
)

// Indexer provides a unified API for codebase intelligence: parsing, import
// graph traversal, symbol lookup, and relevance scoring.
type Indexer struct {
	workDir string
	parsers []Parser
	store   types.EventStore

	mu          sync.RWMutex
	graph       *CodeGraph
	index       *SymbolIndex
	scorer      *RelevanceScorer
	builtAt     time.Time
	files       []*FileInfo
	watcher     *FileWatcher
	watchCtx    context.Context
	watchCancel context.CancelFunc
}

// NewIndexer creates a new codebase indexer for the given working directory.
// The indexer is lazy — call Build() before using query methods.
func NewIndexer(workDir string) *Indexer {
	return NewIndexerWithStore(workDir, nil)
}

// NewIndexerWithStore creates a new codebase indexer with an optional EventStore.
// If store is nil, events are not emitted (backward compatible).
func NewIndexerWithStore(workDir string, store types.EventStore) *Indexer {
	return &Indexer{
		workDir: workDir,
		parsers: AllParsers(),
		store:   store,
	}
}

// NewIndexerWithWatcher creates a new codebase indexer with an integrated FileWatcher.
// The watcher is created but not started — call StartWatching to begin watching.
func NewIndexerWithWatcher(workDir string, store types.EventStore, logger *slog.Logger) (*Indexer, error) {
	idx := &Indexer{
		workDir: workDir,
		parsers: AllParsers(),
		store:   store,
	}
	watcher, err := NewFileWatcher(workDir, idx.parsers, logger)
	if err != nil {
		return nil, err
	}
	idx.watcher = watcher
	return idx, nil
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

// StartWatching starts the file watcher and launches a background goroutine
// that consumes watch events and applies incremental updates.
// The context controls the lifetime of the watching goroutine.
func (idx *Indexer) StartWatching(ctx context.Context) error {
	idx.mu.Lock()
	defer idx.mu.Unlock()

	if idx.watcher == nil {
		return fmt.Errorf("watcher not initialized — use NewIndexerWithWatcher")
	}
	if idx.watchCancel != nil {
		return fmt.Errorf("watcher already started")
	}

	watchCtx, cancel := context.WithCancel(ctx)
	idx.watchCtx = watchCtx
	idx.watchCancel = cancel

	idx.watcher.Start(watchCtx)

	// Launch goroutine to handle watch events
	go func() {
		for {
			select {
			case <-watchCtx.Done():
				return
			case event, ok := <-idx.watcher.Events():
				if !ok {
					return
				}
				if err := idx.handleWatchEvent(watchCtx, event); err != nil {
					idx.watcher.logger.Error("handle watch event", "error", err, "path", event.Path, "type", event.EventType)
				}
			}
		}
	}()

	return nil
}

// StopWatching stops the file watcher and cleans up resources.
func (idx *Indexer) StopWatching() {
	idx.mu.Lock()
	defer idx.mu.Unlock()

	if idx.watchCancel != nil {
		idx.watchCancel()
		idx.watchCancel = nil
		idx.watchCtx = nil
	}
	if idx.watcher != nil {
		idx.watcher.Stop()
	}
}

// handleWatchEvent processes a single file watcher event and updates the index incrementally.
func (idx *Indexer) handleWatchEvent(ctx context.Context, event WatcherEvent) error {
	idx.mu.Lock()
	defer idx.mu.Unlock()

	relPath, err := filepath.Rel(idx.workDir, event.Path)
	if err != nil {
		return err
	}

	// Skip if not a parseable file
	p := ParserForFile(event.Path, idx.parsers)
	if p == nil {
		return nil
	}

	switch event.EventType {
	case WatcherModified, WatcherCreated:
		// Parse the file
		content, err := os.ReadFile(event.Path)
		if err != nil {
			return err
		}
		const maxFileSize = 4096
		if len(content) > maxFileSize {
			content = content[:maxFileSize]
		}
		info, err := p.Parse(relPath, content)
		if err != nil {
			return err
		}

		// Remove old entries if file was previously indexed
		idx.graph.RemoveNode(relPath)
		idx.index.RemoveFile(relPath)

		// Resolve imports
		var resolvedImports []string
		for _, imp := range info.Imports {
			resolved := resolveImport(idx.workDir, relPath, imp.Path, p.Language())
			if resolved != "" {
				resolvedImports = append(resolvedImports, resolved)
			}
		}

		// Add to graph and index
		idx.graph.AddNode(relPath, resolvedImports, info.Language)
		idx.index.AddFile(info)

		// Update files list
		found := false
		for i, existing := range idx.files {
			if existing.Path == relPath {
				idx.files[i] = info
				found = true
				break
			}
		}
		if !found {
			idx.files = append(idx.files, info)
		}

		// Emit events if store is available
		if idx.store != nil {
			hash := [32]byte{}
			fileInfo, _ := os.Stat(event.Path)
			var modTime time.Time
			if fileInfo != nil {
				modTime = fileInfo.ModTime()
			}
			filePayload := FileIndexedPayload{
				Path:     relPath,
				Language: info.Language,
				Symbols:  info.Exports,
				Imports:  info.Imports,
				Hash:     hash,
				ModTime:  modTime,
			}
			if err := EmitFileIndexed(ctx, idx.store, filePayload); err != nil {
				return err
			}
			for _, sym := range info.Exports {
				symPayload := SymbolDefinedPayload{
					Name:     sym.Name, Kind: sym.Kind, File: relPath,
					Line: 0, Exported: sym.Exported,
				}
				if err := EmitSymbolDefined(ctx, idx.store, symPayload); err != nil {
					return err
				}
			}
			for _, imp := range info.Imports {
				if imp.ResolvedTo != "" {
					impPayload := ImportResolvedPayload{
						From: relPath, To: imp.ResolvedTo, ImportPath: imp.Path,
						ResolvedPath: imp.ResolvedTo,
					}
					if err := EmitImportResolved(ctx, idx.store, impPayload); err != nil {
						return err
					}
				}
			}
			for _, cs := range info.CallSites {
				callPayload := CallEdgeAddedPayload{
					CallerFile: relPath, CallerLine: cs.Line, CallerName: cs.CallerName,
					CalleeFile: "", CalleeLine: 0, CalleeName: cs.CalleeName,
				}
				if err := EmitCallEdgeAdded(ctx, idx.store, callPayload); err != nil {
					return err
				}
			}
		}

	case WatcherDeleted:
		// Remove from graph and index
		idx.graph.RemoveNode(relPath)
		idx.index.RemoveFile(relPath)

		// Remove from files list
		newFiles := make([]*FileInfo, 0, len(idx.files))
		for _, f := range idx.files {
			if f.Path != relPath {
				newFiles = append(newFiles, f)
			}
		}
		idx.files = newFiles

		// Emit events if store is available
		if idx.store != nil {
			delPayload := FileDeletedPayload{Path: relPath, WasIndexed: true}
			if err := EmitFileDeleted(ctx, idx.store, delPayload); err != nil {
				return err
			}
		}
	}

	// Update scorer and save cache
	idx.scorer = NewRelevanceScorer(idx.graph, idx.index)
	idx.builtAt = time.Now()

	fileCaches := BuildCacheFromFiles(idx.workDir, idx.files, nil)
	cache := &IndexCache{Files: fileCaches}
	_ = SaveCache(idx.workDir, cache)

	return nil
}

// BuildIncremental triggers an incremental rebuild of the index.
// This is the public method for manual incremental rebuilds (e.g., via `m31a index --incremental`).
func (idx *Indexer) BuildIncremental(ctx context.Context) error {
	idx.mu.Lock()
	defer idx.mu.Unlock()

	if idx.graph == nil {
		return idx.buildFull(ctx)
	}
	return idx.buildIncremental(ctx)
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

	// Emit code intelligence events to EventStore
	if idx.store != nil {
		if err := idx.emitBuildEvents(ctx, files, graph); err != nil {
			return fmt.Errorf("emit build events: %w", err)
		}
	}

	// Save cache
	fileCaches := BuildCacheFromFiles(idx.workDir, files, nil)
	cache := &IndexCache{Files: fileCaches}
	_ = SaveCache(idx.workDir, cache)

	return nil
}

// emitBuildEvents emits code intelligence events for all files, symbols, imports, call edges, and inheritance edges.
func (idx *Indexer) emitBuildEvents(ctx context.Context, files []*FileInfo, graph *CodeGraph) error {
	// Emit FileIndexed and SymbolDefined events for each file
	for _, f := range files {
		// FileIndexed event
		hash := [32]byte{} // Would be computed from file content in practice
		info, _ := os.Stat(filepath.Join(idx.workDir, f.Path))
		var modTime time.Time
		if info != nil {
			modTime = info.ModTime()
		}
		filePayload := FileIndexedPayload{
			Path:     f.Path,
			Language: f.Language,
			Symbols:  f.Exports,
			Imports:  f.Imports,
			Hash:     hash,
			ModTime:  modTime,
		}
		if err := EmitFileIndexed(ctx, idx.store, filePayload); err != nil {
			return err
		}

		// SymbolDefined events for each exported symbol
		for _, sym := range f.Exports {
			symPayload := SymbolDefinedPayload{
				Name:     sym.Name,
				Kind:     sym.Kind,
				File:     f.Path,
				Line:     0, // Line not tracked in SymbolInfo
				Exported: sym.Exported,
			}
			if err := EmitSymbolDefined(ctx, idx.store, symPayload); err != nil {
				return err
			}
		}

		// ImportResolved events for resolved imports
		for _, imp := range f.Imports {
			if imp.ResolvedTo != "" {
				impPayload := ImportResolvedPayload{
					From:         f.Path,
					To:           imp.ResolvedTo,
					ImportPath:   imp.Path,
					ResolvedPath: imp.ResolvedTo,
				}
				if err := EmitImportResolved(ctx, idx.store, impPayload); err != nil {
					return err
				}
			}
		}

		// CallEdgeAdded events from call sites
		for _, cs := range f.CallSites {
			callPayload := CallEdgeAddedPayload{
				CallerFile: f.Path,
				CallerLine: cs.Line,
				CallerName: cs.CallerName,
				CalleeFile: "", // Callee file not directly tracked in CallSiteInfo
				CalleeLine: 0,
				CalleeName: cs.CalleeName,
			}
			if err := EmitCallEdgeAdded(ctx, idx.store, callPayload); err != nil {
				return err
			}
		}
	}

	// Emit CallEdgeAdded for all call edges in the graph
	for _, edge := range graph.AllCallEdges() {
		callPayload := CallEdgeAddedPayload{
			CallerFile: edge.CallerFile,
			CallerLine: edge.CallerLine,
			CallerName: edge.CallerName,
			CalleeFile: edge.CalleeFile,
			CalleeLine: edge.CalleeLine,
			CalleeName: edge.CalleeName,
		}
		if err := EmitCallEdgeAdded(ctx, idx.store, callPayload); err != nil {
			return err
		}
	}

	// Emit InheritanceEdgeAdded for all inheritance edges
	for _, edge := range graph.AllInheritanceEdges() {
		inhPayload := InheritanceEdgeAddedPayload{
			Child:      edge.Child,
			Parent:     edge.Parent,
			ChildFile:  edge.ChildFile,
			ParentFile: edge.ParentFile,
			Kind:       edge.Kind,
		}
		if err := EmitInheritanceEdgeAdded(ctx, idx.store, inhPayload); err != nil {
			return err
		}
	}

	return nil
}

// buildFromCache rebuilds from a disk cache, using cached results for unchanged files.
func (idx *Indexer) buildFromCache(ctx context.Context, cached *IndexCache) error {
	inc := CheckIncremental(idx.workDir, idx.parsers, cached.Files)

	graph := NewCodeGraph()
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
		var changedFiles []*FileInfo
		var err error
		// Use parallel parsing for larger file sets (>= 10 files)
		if len(inc.Changed) >= 10 {
			changedFiles, err = idx.parseFilesParallel(ctx, inc.Changed)
		} else {
			changedFiles, err = idx.parseFiles(ctx, inc.Changed)
		}
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

	// Emit events for deleted files
	if idx.store != nil && len(inc.Deleted) > 0 {
		for _, path := range inc.Deleted {
			delPayload := FileDeletedPayload{Path: path, WasIndexed: true}
			if err := EmitFileDeleted(ctx, idx.store, delPayload); err != nil {
				return err
			}
			// Emit SymbolRemoved for symbols in deleted files (simplified)
			// In practice, we'd track which symbols were in the deleted file
		}
	}

	// Remove deleted files from graph and index
	if len(inc.Deleted) > 0 {
		for _, path := range inc.Deleted {
			idx.graph.RemoveNode(path)
		}
		idx.index.RemoveFiles(inc.Deleted)
	}

	// Parse changed/new files
	if len(inc.Changed) > 0 {
		var changedFiles []*FileInfo
		var err error
		// Use parallel parsing for larger file sets (>= 10 files)
		if len(inc.Changed) >= 10 {
			changedFiles, err = idx.parseFilesParallel(ctx, inc.Changed)
		} else {
			changedFiles, err = idx.parseFiles(ctx, inc.Changed)
		}
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

		// Emit events for changed/new files
		if idx.store != nil {
			for _, f := range changedFiles {
				hash := [32]byte{}
				info, _ := os.Stat(filepath.Join(idx.workDir, f.Path))
				var modTime time.Time
				if info != nil {
					modTime = info.ModTime()
				}
				filePayload := FileIndexedPayload{
					Path:     f.Path,
					Language: f.Language,
					Symbols:  f.Exports,
					Imports:  f.Imports,
					Hash:     hash,
					ModTime:  modTime,
				}
				if err := EmitFileIndexed(ctx, idx.store, filePayload); err != nil {
					return err
				}
				for _, sym := range f.Exports {
					symPayload := SymbolDefinedPayload{
						Name:     sym.Name, Kind: sym.Kind, File: f.Path,
						Line: 0, Exported: sym.Exported,
					}
					if err := EmitSymbolDefined(ctx, idx.store, symPayload); err != nil {
						return err
					}
				}
				for _, imp := range f.Imports {
					if imp.ResolvedTo != "" {
						impPayload := ImportResolvedPayload{
							From: f.Path, To: imp.ResolvedTo, ImportPath: imp.Path,
							ResolvedPath: imp.ResolvedTo,
						}
						if err := EmitImportResolved(ctx, idx.store, impPayload); err != nil {
							return err
						}
					}
				}
				for _, cs := range f.CallSites {
					callPayload := CallEdgeAddedPayload{
						CallerFile: f.Path, CallerLine: cs.Line, CallerName: cs.CallerName,
						CalleeFile: "", CalleeLine: 0, CalleeName: cs.CalleeName,
					}
					if err := EmitCallEdgeAdded(ctx, idx.store, callPayload); err != nil {
						return err
					}
				}
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

// parseFilesParallel parses files concurrently using goroutines bounded by
// runtime.NumCPU(). Errors are collected and returned together (D-12).
func (idx *Indexer) parseFilesParallel(ctx context.Context, paths []string) ([]*FileInfo, error) {
	results := make([]*FileInfo, len(paths))
	errs := make([]error, len(paths))
	var wg sync.WaitGroup
	sem := make(chan struct{}, runtime.NumCPU())

	for i, relPath := range paths {
		wg.Add(1)
		sem <- struct{}{}
		go func(i int, relPath string) {
			defer wg.Done()
			defer func() { <-sem }()

			if err := ctx.Err(); err != nil {
				errs[i] = err
				return
			}
			absPath := filepath.Join(idx.workDir, relPath)
			p := ParserForFile(absPath, idx.parsers)
			if p == nil {
				return
			}
			content, err := os.ReadFile(absPath)
			if err != nil {
				errs[i] = err
				return
			}
			const maxFileSize = 4096
			if len(content) > maxFileSize {
				content = content[:maxFileSize]
			}
			info, err := p.Parse(relPath, content)
			if err != nil {
				errs[i] = err
				return
			}
			results[i] = info
		}(i, relPath)
	}
	wg.Wait()

	// Collect errors (D-12: report together)
	var allErrors []error
	for _, err := range errs {
		if err != nil {
			allErrors = append(allErrors, err)
		}
	}
	if len(allErrors) > 0 {
		return nil, fmt.Errorf("parse %d files: %w", len(allErrors), errors.Join(allErrors...))
	}

	// Filter nil results
	var filtered []*FileInfo
	for _, r := range results {
		if r != nil {
			filtered = append(filtered, r)
		}
	}
	return filtered, nil
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

// CodeIntelProjection rebuilds an in-memory graph from EventStore events.
// It provides a queryable view of the code intelligence graph.
type CodeIntelProjection struct {
	mu       sync.RWMutex
	graph    *CodeGraph
	files    map[string]*FileInfo
	symbols  map[string][]SymbolLocation
}

// NewCodeIntelProjection creates a new projection.
func NewCodeIntelProjection() *CodeIntelProjection {
	return &CodeIntelProjection{
		graph:   NewCodeGraph(),
		files:   make(map[string]*FileInfo),
		symbols: make(map[string][]SymbolLocation),
	}
}

// RebuildFromEvents replays all codeintel events from the EventStore to rebuild the in-memory graph.
func (p *CodeIntelProjection) RebuildFromEvents(ctx context.Context, store types.EventStore) error {
	if store == nil {
		return nil
	}

	// Query all codeintel events
	q := types.Query{
		Type:   nil, // Get all types
		Limit:  10000,
		Offset: 0,
	}
	events, err := store.Query(ctx, q)
	if err != nil {
		return fmt.Errorf("query events: %w", err)
	}

	// Filter for codeintel events (source == "codeintel")
	var codeintelEvents []types.Event
	for _, evt := range events {
		if evt.Metadata.Source == "codeintel" {
			codeintelEvents = append(codeintelEvents, evt)
		}
	}

	// Replay events
	for _, evt := range codeintelEvents {
		if err := p.ApplyEvent(ctx, evt); err != nil {
			return fmt.Errorf("apply event %s: %w", evt.Type, err)
		}
	}

	return nil
}

// ApplyEvent applies a single event to the in-memory state.
func (p *CodeIntelProjection) ApplyEvent(ctx context.Context, evt types.Event) error {
	p.mu.Lock()
	defer p.mu.Unlock()

	switch evt.Type {
	case types.EventFileIndexed:
		var payload FileIndexedPayload
		if err := json.Unmarshal(evt.Payload, &payload); err != nil {
			return err
		}
		p.files[payload.Path] = &FileInfo{
			Path:     payload.Path,
			Language: payload.Language,
			Imports:  payload.Imports,
			Exports:  payload.Symbols,
		}
		for _, sym := range payload.Symbols {
			p.symbols[sym.Name] = append(p.symbols[sym.Name], SymbolLocation{
				File: payload.Path, Kind: sym.Kind,
			})
			p.graph.AddNode(payload.Path, nil, payload.Language)
		}
		for _, imp := range payload.Imports {
			if imp.ResolvedTo != "" {
				p.graph.AddNode(payload.Path, []string{imp.ResolvedTo}, payload.Language)
			}
		}

	case types.EventSymbolDefined:
		var payload SymbolDefinedPayload
		if err := json.Unmarshal(evt.Payload, &payload); err != nil {
			return err
		}
		p.symbols[payload.Name] = append(p.symbols[payload.Name], SymbolLocation{
			File: payload.File, Kind: payload.Kind,
		})

	case types.EventImportResolved:
		var payload ImportResolvedPayload
		if err := json.Unmarshal(evt.Payload, &payload); err != nil {
			return err
		}
		p.graph.AddNode(payload.From, []string{payload.To}, "")

	case types.EventCallEdgeAdded:
		var payload CallEdgeAddedPayload
		if err := json.Unmarshal(evt.Payload, &payload); err != nil {
			return err
		}
		p.graph.AddCallEdge(CallEdge{
			CallerFile: payload.CallerFile, CallerLine: payload.CallerLine,
			CallerName: payload.CallerName, CalleeFile: payload.CalleeFile,
			CalleeLine: payload.CalleeLine, CalleeName: payload.CalleeName,
		})

	case types.EventInheritanceEdgeAdded:
		var payload InheritanceEdgeAddedPayload
		if err := json.Unmarshal(evt.Payload, &payload); err != nil {
			return err
		}
		p.graph.AddInheritanceEdge(InheritanceEdge{
			Child: payload.Child, Parent: payload.Parent,
			ChildFile: payload.ChildFile, ParentFile: payload.ParentFile,
			Kind: payload.Kind,
		})

	case types.EventTypeHierarchyEdgeAdded:
		var payload TypeHierarchyEdgeAddedPayload
		if err := json.Unmarshal(evt.Payload, &payload); err != nil {
			return err
		}
		p.graph.AddTypeHierarchyEdge(TypeHierarchyEdge{
			Subtype: payload.Subtype, Supertype: payload.Supertype,
			SubtypeFile: payload.SubtypeFile, SupertypeFile: payload.SupertypeFile,
		})

	case types.EventFileDeleted:
		var payload FileDeletedPayload
		if err := json.Unmarshal(evt.Payload, &payload); err != nil {
			return err
		}
		delete(p.files, payload.Path)
		p.graph.RemoveNode(payload.Path)

	case types.EventSymbolRemoved:
		var payload SymbolRemovedPayload
		if err := json.Unmarshal(evt.Payload, &payload); err != nil {
			return err
		}
		// Remove symbol from index
		locs := p.symbols[payload.Name]
		for i := len(locs) - 1; i >= 0; i-- {
			if locs[i].File == payload.File {
				p.symbols[payload.Name] = append(locs[:i], locs[i+1:]...)
			}
		}
		if len(p.symbols[payload.Name]) == 0 {
			delete(p.symbols, payload.Name)
		}
	}

	return nil
}

// Graph returns the in-memory code graph (thread-safe read).
func (p *CodeIntelProjection) Graph() *CodeGraph {
	p.mu.RLock()
	defer p.mu.RUnlock()
	return p.graph
}

// Files returns the file map (thread-safe read).
func (p *CodeIntelProjection) Files() map[string]*FileInfo {
	p.mu.RLock()
	defer p.mu.RUnlock()
	result := make(map[string]*FileInfo, len(p.files))
	for k, v := range p.files {
		result[k] = v
	}
	return result
}

// Symbols returns the symbol index (thread-safe read).
func (p *CodeIntelProjection) Symbols() map[string][]SymbolLocation {
	p.mu.RLock()
	defer p.mu.RUnlock()
	result := make(map[string][]SymbolLocation, len(p.symbols))
	for k, v := range p.symbols {
		result[k] = v
	}
	return result
}

// Projection returns a new CodeIntelProjection for querying.
func (idx *Indexer) Projection() *CodeIntelProjection {
	return NewCodeIntelProjection()
}
