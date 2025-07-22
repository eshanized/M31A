package provider

import (
	"fmt"
	"sort"
	"sync"

	m31errors "github.com/eshanized/M31A/internal/errors"
)

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
		return fmt.Errorf("provider name cannot be empty")
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
		return fmt.Errorf("provider name cannot be empty: %w", m31errors.ErrProviderNotFound)
	}
	if _, ok := r.providers[name]; !ok {
		return fmt.Errorf("provider %q not registered: %w", name, m31errors.ErrProviderNotFound)
	}
	r.active = name
	return nil
}

func (r *Registry) Get(name string) (LLMProvider, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	p, ok := r.providers[name]
	if !ok {
		return nil, m31errors.ErrProviderUnreachable
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

func (r *Registry) ActiveProvider() LLMProvider {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return r.providers[r.active]
}
