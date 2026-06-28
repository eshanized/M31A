package workflow

import (
	"context"
	"strings"
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/codeintel"
)

// DynamicContextRegistry maintains project knowledge and provides dynamic context
// for LLM calls. Integrates with code intelligence and knowledge subsystem.
type DynamicContextRegistry struct {
	mu         sync.RWMutex
	knowledge  *Knowledge
	codeIntel  *codeintel.Indexer
	workDir    string
	lastUpdate time.Time
	updateInt  time.Duration
}

// NewDynamicContextRegistry creates a new registry.
func NewDynamicContextRegistry(workDir string, ci *codeintel.Indexer) *DynamicContextRegistry {
	return &DynamicContextRegistry{
		knowledge: NewKnowledge(),
		codeIntel: ci,
		workDir:   workDir,
		updateInt: 5 * time.Minute,
	}
}

// Reconcile checks if context sources have changed and returns updates.
func (r *DynamicContextRegistry) Reconcile(ctx context.Context, snapshot map[string]string) []ContextChange {
	var changes []ContextChange

	r.mu.RLock()
	needsRefresh := time.Since(r.lastUpdate) > r.updateInt
	ci := r.codeIntel
	r.mu.RUnlock()

	if needsRefresh {
		r.refreshKnowledge(ctx)

		r.mu.Lock()
		r.lastUpdate = time.Now()
		r.mu.Unlock()

		if ci != nil {
			changes = append(changes, ContextChange{
				Source: "knowledge",
				Type:   "refresh",
			})
		}
	}

	return changes
}

// LoadAll returns the current snapshot of all context sources.
func (r *DynamicContextRegistry) LoadAll(ctx context.Context) map[string]string {
	snapshot := make(map[string]string)

	r.mu.RLock()
	k := r.knowledge
	r.mu.RUnlock()

	if ctxStr := k.FormatContext(4000); ctxStr != "" {
		snapshot["knowledge"] = ctxStr
	}

	return snapshot
}

// GetKnowledge returns the project knowledge instance.
func (r *DynamicContextRegistry) GetKnowledge() *Knowledge {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.knowledge
}

// refreshKnowledge updates knowledge from code intelligence.
func (r *DynamicContextRegistry) refreshKnowledge(ctx context.Context) {
	r.mu.RLock()
	ci := r.codeIntel
	wd := r.workDir
	r.mu.RUnlock()

	if ci == nil {
		return
	}

	files := r.listProjectFiles()
	r.knowledge.DetectConventions(wd, files)
}

// listProjectFiles returns basic file info for the project.
func (r *DynamicContextRegistry) listProjectFiles() []*codeintel.FileInfo {
	r.mu.RLock()
	ci := r.codeIntel
	r.mu.RUnlock()

	if ci == nil {
		return nil
	}

	symbols := ci.SymbolCount()
	if symbols <= 0 {
		return nil
	}

	summary := ci.ProjectSummary(1000)
	if summary == "" {
		return nil
	}

	var files []*codeintel.FileInfo
	for _, line := range strings.Split(summary, "\n") {
		if line != "" {
			files = append(files, &codeintel.FileInfo{
				Path: line,
			})
		}
	}
	return files
}

// ContextChange represents a change in a context source.
type ContextChange struct {
	Source string // e.g., "knowledge", "conventions", "patterns"
	Type   string // e.g., "refresh", "update", "new"
}
