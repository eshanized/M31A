package provider

import (
	"fmt"
	"sort"
	"sync"

	m31errors "github.com/eshanized/M31A/internal/core/errors"
)

// RegistryInterface is the common interface for both Registry and LazyRegistry.
// Callers should accept this interface instead of *Registry to support lazy initialization.
type RegistryInterface interface {
	Register(name string, p LLMProvider) error
	Active() string
	SetActive(name string) error
	TrySetActive(name string) (LLMProvider, error)
	RollbackActive(fromName, toName string) bool
	Get(name string) (LLMProvider, error)
	List() []string
	ListAll() []string
	ActiveProvider() LLMProvider
}

type Registry struct {
	mu        sync.RWMutex
	providers map[string]LLMProvider
	active    string
}

func NewRegistry() *Registry {
	return &Registry{
		providers: make(map[string]LLMProvider),
	}
}

func (r *Registry) Register(name string, p LLMProvider) error {
	if name == "" {
		return fmt.Errorf("provider name cannot be empty: %w", m31errors.ErrInvalidProvider)
	}
	r.mu.Lock()
	defer r.mu.Unlock()
	r.providers[name] = p
	if r.active == "" {
		r.active = name
	}
	return nil
}

func (r *Registry) Active() string {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.active
}

func (r *Registry) SetActive(name string) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	if name == "" {
		return fmt.Errorf("provider name cannot be empty: %w", m31errors.ErrInvalidProvider)
	}
	if _, ok := r.providers[name]; !ok {
		return fmt.Errorf("provider %q not registered: %w", name, m31errors.ErrProviderNotFound)
	}
	r.active = name
	return nil
}

// TrySetActive atomically sets the provider as active if it exists.
// Returns the provider and nil on success. This prevents TOCTOU races
// where another goroutine could SetActive between Get and SetActive.
func (r *Registry) TrySetActive(name string) (LLMProvider, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	p, ok := r.providers[name]
	if !ok {
		return nil, fmt.Errorf("provider %q not registered: %w", name, m31errors.ErrProviderNotFound)
	}
	r.active = name
	return p, nil
}

// RollbackActive reverts the active provider to the given name if the current
// active provider matches fromName. Used when a health check fails after
// TrySetActive, preventing the system from being left with an unhealthy active provider.
func (r *Registry) RollbackActive(fromName, toName string) bool {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.active == fromName {
		if _, ok := r.providers[toName]; ok {
			r.active = toName
			return true
		}
		return false
	}
	return false
}

func (r *Registry) Get(name string) (LLMProvider, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	p, ok := r.providers[name]
	if !ok {
		return nil, m31errors.ErrProviderNotFound
	}
	return p, nil
}

func (r *Registry) List() []string {
	r.mu.RLock()
	defer r.mu.RUnlock()
	names := make([]string, 0, len(r.providers))
	for name := range r.providers {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

// ListAll returns all registered provider names without any decoration.
// It delegates to List() to avoid code duplication.
func (r *Registry) ListAll() []string {
	return r.List()
}

func (r *Registry) ActiveProvider() LLMProvider {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.providers[r.active]
}

// LazyRegistry wraps a Registry and defers provider registration until first access.
// The initFn closure is called exactly once via sync.Once on first method invocation.
type LazyRegistry struct {
	once   sync.Once
	initFn func() *Registry
	inner  *Registry
}

// NewLazyRegistry creates a LazyRegistry that will call initFn exactly once
// when any method requiring the inner registry is first invoked.
func NewLazyRegistry(initFn func() *Registry) *LazyRegistry {
	return &LazyRegistry{initFn: initFn}
}

func (lr *LazyRegistry) init() {
	lr.once.Do(func() {
		lr.inner = lr.initFn()
	})
}

func (lr *LazyRegistry) Register(name string, p LLMProvider) error {
	lr.init()
	return lr.inner.Register(name, p)
}

func (lr *LazyRegistry) Active() string {
	lr.init()
	return lr.inner.Active()
}

func (lr *LazyRegistry) SetActive(name string) error {
	lr.init()
	return lr.inner.SetActive(name)
}

func (lr *LazyRegistry) TrySetActive(name string) (LLMProvider, error) {
	lr.init()
	return lr.inner.TrySetActive(name)
}

func (lr *LazyRegistry) RollbackActive(fromName, toName string) bool {
	lr.init()
	return lr.inner.RollbackActive(fromName, toName)
}

func (lr *LazyRegistry) Get(name string) (LLMProvider, error) {
	lr.init()
	return lr.inner.Get(name)
}

func (lr *LazyRegistry) List() []string {
	lr.init()
	return lr.inner.List()
}

func (lr *LazyRegistry) ListAll() []string {
	lr.init()
	return lr.inner.ListAll()
}

func (lr *LazyRegistry) ActiveProvider() LLMProvider {
	lr.init()
	return lr.inner.ActiveProvider()
}
