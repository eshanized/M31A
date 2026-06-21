package context

import (
	"context"
	"sort"
	"sync"
)

// Change describes a context source that changed between reconciliations.
type Change struct {
	Key     string
	Type    ChangeType
	Content string
}

// ChangeType describes the kind of change.
type ChangeType int

const (
	ChangeAdded   ChangeType = iota
	ChangeUpdated
	ChangeRemoved
)

// Registry manages an ordered set of ContextSources and detects changes
// between reconciliation calls.
type Registry struct {
	mu      sync.RWMutex
	sources []ContextSource
}

// NewRegistry creates a Registry with the given sources.
func NewRegistry(sources ...ContextSource) *Registry {
	return &Registry{sources: sources}
}

// Add registers a new context source.
func (r *Registry) Add(source ContextSource) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.sources = append(r.sources, source)
}

// LoadAll loads all sources concurrently and returns a snapshot map.
func (r *Registry) LoadAll(ctx context.Context) map[string]string {
	r.mu.RLock()
	sources := make([]ContextSource, len(r.sources))
	copy(sources, r.sources)
	r.mu.RUnlock()

	snapshot := make(map[string]string, len(sources))
	var mu sync.Mutex
	var wg sync.WaitGroup

	for _, src := range sources {
		wg.Add(1)
		go func(s ContextSource) {
			defer wg.Done()
			val, err := s.Load(ctx)
			if err != nil {
				return
			}
			mu.Lock()
			snapshot[s.Key()] = val
			mu.Unlock()
		}(src)
	}

	wg.Wait()
	return snapshot
}

// Reconcile compares the current snapshot against a previous one and returns
// the list of changes. Sources are evaluated in key-sorted order for
// deterministic output.
func (r *Registry) Reconcile(ctx context.Context, previous map[string]string) []Change {
	current := r.LoadAll(ctx)
	var changes []Change

	// Detect added and updated
	keys := make([]string, 0, len(current))
	for k := range current {
		keys = append(keys, k)
	}
	sort.Strings(keys)

	for _, key := range keys {
		newVal := current[key]
		oldVal, existed := previous[key]
		if !existed {
			rendered := r.renderSource(key, newVal)
			if rendered != "" {
				changes = append(changes, Change{Key: key, Type: ChangeAdded, Content: rendered})
			}
		} else if oldVal != newVal {
			rendered := r.renderUpdate(key, oldVal, newVal)
			if rendered != "" {
				changes = append(changes, Change{Key: key, Type: ChangeUpdated, Content: rendered})
			}
		}
	}

	// Detect removed
	for key, oldVal := range previous {
		if _, exists := current[key]; !exists {
			rendered := r.renderRemoval(key, oldVal)
			if rendered != "" {
				changes = append(changes, Change{Key: key, Type: ChangeRemoved, Content: rendered})
			}
		}
	}

	return changes
}

func (r *Registry) findSource(key string) ContextSource {
	r.mu.RLock()
	defer r.mu.RUnlock()
	for _, s := range r.sources {
		if s.Key() == key {
			return s
		}
	}
	return nil
}

func (r *Registry) renderSource(key, value string) string {
	if s := r.findSource(key); s != nil {
		return s.Render(value)
	}
	return value
}

func (r *Registry) renderUpdate(key, oldVal, newVal string) string {
	if s := r.findSource(key); s != nil {
		return s.RenderUpdate(oldVal, newVal)
	}
	return newVal
}

func (r *Registry) renderRemoval(key, oldVal string) string {
	if s := r.findSource(key); s != nil {
		return s.RenderRemoval(oldVal)
	}
	return ""
}
