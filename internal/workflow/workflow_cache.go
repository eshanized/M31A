package workflow

import (
	"sync"

	"github.com/eshanized/M31A/internal/provider"
	m31types "github.com/eshanized/M31A/internal/types"
)

// WorkflowCache manages all cached data previously embedded in WorkflowState.
// It provides build-once patterns for tool definitions and prompts,
// and keyed caches for project state and parsed plans.
type WorkflowCache struct {
	// Cached tool definitions (built once, reused for all LLM calls)
	toolDefs     []provider.ToolDefinition
	toolDefsOnce sync.Once

	// Cached system prompt static portions
	basePrompt     string
	basePromptOnce sync.Once

	// Cached full system prompts per extras signature (PERF-24 extension)
	fullPrompts   map[string]string
	fullPromptsMu sync.Mutex

	// Mutex for project and plan caches
	projectMu sync.RWMutex
	// Cached project state for execute phase (H15 fix)
	project   *m31types.ProjectState
	projectID string // session ID for invalidation

	// Cached project state shared across all context builders (PERF-29)
	projectShared   *m31types.ProjectState
	projectSharedID string

	// Cached parsed plan for execute phase (H15 fix)
	plan    *m31types.Plan
	planMD5 string // MD5 of planMarkdown for invalidation

	// Dynamic context change detection
	contextSnapshot map[string]string
	dynamicContext  string
}

// NewWorkflowCache creates a new WorkflowCache.
func NewWorkflowCache() *WorkflowCache {
	return &WorkflowCache{
		fullPrompts:     make(map[string]string),
		contextSnapshot: make(map[string]string),
	}
}

// GetToolDefs returns cached tool definitions, building them once using buildFn.
// Subsequent calls return the cached result without calling buildFn again.
func (wc *WorkflowCache) GetToolDefs(buildFn func() []provider.ToolDefinition) []provider.ToolDefinition {
	wc.toolDefsOnce.Do(func() {
		wc.toolDefs = buildFn()
	})
	return wc.toolDefs
}

// GetBasePrompt returns cached base prompt, building it once using buildFn.
// Subsequent calls return the cached result without calling buildFn again.
func (wc *WorkflowCache) GetBasePrompt(buildFn func() string) string {
	wc.basePromptOnce.Do(func() {
		wc.basePrompt = buildFn()
	})
	return wc.basePrompt
}

// GetFullPrompt returns cached full prompt for the given key, building it once using buildFn.
// Subsequent calls with the same key return the cached result without calling buildFn again.
func (wc *WorkflowCache) GetFullPrompt(key string, buildFn func() string) string {
	wc.fullPromptsMu.Lock()
	defer wc.fullPromptsMu.Unlock()

	if cached, ok := wc.fullPrompts[key]; ok {
		return cached
	}
	result := buildFn()
	wc.fullPrompts[key] = result
	return result
}

// SetProject caches project state with the given session ID.
func (wc *WorkflowCache) SetProject(id string, project *m31types.ProjectState) {
	wc.projectMu.Lock()
	wc.project = project
	wc.projectID = id
	wc.projectMu.Unlock()
}

// GetProject returns cached project state if the ID matches, nil otherwise.
func (wc *WorkflowCache) GetProject(id string) *m31types.ProjectState {
	wc.projectMu.RLock()
	defer wc.projectMu.RUnlock()
	if wc.projectID == id && wc.project != nil {
		return wc.project
	}
	return nil
}

// SetProjectShared caches shared project state with the given session ID.
func (wc *WorkflowCache) SetProjectShared(id string, project *m31types.ProjectState) {
	wc.projectMu.Lock()
	wc.projectShared = project
	wc.projectSharedID = id
	wc.projectMu.Unlock()
}

// GetProjectShared returns cached shared project state if the ID matches, nil otherwise.
func (wc *WorkflowCache) GetProjectShared(id string) *m31types.ProjectState {
	wc.projectMu.RLock()
	defer wc.projectMu.RUnlock()
	if wc.projectSharedID == id && wc.projectShared != nil {
		return wc.projectShared
	}
	return nil
}

// SetPlan caches parsed plan with MD5 for invalidation.
func (wc *WorkflowCache) SetPlan(md5 string, plan *m31types.Plan) {
	wc.projectMu.Lock()
	wc.plan = plan
	wc.planMD5 = md5
	wc.projectMu.Unlock()
}

// GetPlan returns cached plan if the MD5 matches, nil otherwise.
func (wc *WorkflowCache) GetPlan(md5 string) *m31types.Plan {
	wc.projectMu.RLock()
	defer wc.projectMu.RUnlock()
	if wc.planMD5 == md5 && wc.plan != nil {
		return wc.plan
	}
	return nil
}

// SetDynamicContext caches the dynamic context string and snapshot.
func (wc *WorkflowCache) SetDynamicContext(snapshot map[string]string, contextStr string) {
	wc.contextSnapshot = snapshot
	wc.dynamicContext = contextStr
}

// GetDynamicContext returns the cached dynamic context string.
func (wc *WorkflowCache) GetDynamicContext() string {
	return wc.dynamicContext
}

// GetContextSnapshot returns the cached context snapshot.
func (wc *WorkflowCache) GetContextSnapshot() map[string]string {
	return wc.contextSnapshot
}

// InvalidateAll clears all caches (for phase transitions).
func (wc *WorkflowCache) InvalidateAll() {
	wc.fullPromptsMu.Lock()
	wc.fullPrompts = make(map[string]string)
	wc.fullPromptsMu.Unlock()

	wc.projectMu.Lock()
	wc.project = nil
	wc.projectID = ""
	wc.projectShared = nil
	wc.projectSharedID = ""
	wc.plan = nil
	wc.planMD5 = ""
	wc.projectMu.Unlock()
}
