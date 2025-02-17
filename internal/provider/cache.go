package provider

import (
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

type ModelCache struct {
	mu       sync.RWMutex
	models   map[string]*types.ModelInfo
	fetched  time.Time
	ttl      time.Duration
	staleTTL time.Duration
}

func NewModelCache(ttl time.Duration) *ModelCache {
	return &ModelCache{
		models:   make(map[string]*types.ModelInfo),
		ttl:      ttl,
		staleTTL: 24 * time.Hour,
	}
}

func (c *ModelCache) Get(id string) (*types.ModelInfo, bool) {
	c.mu.RLock()
	defer c.mu.RUnlock()

	if c.IsExpired() {
		return nil, false
	}
	m, ok := c.models[id]
	return m, ok
}

func (c *ModelCache) Set(models []types.ModelInfo) {
	c.mu.Lock()
	defer c.mu.Unlock()

	c.models = make(map[string]*types.ModelInfo, len(models))
	for i := range models {
		c.models[models[i].ID] = &models[i]
	}
	c.fetched = time.Now()
}

func (c *ModelCache) IsExpired() bool {
	return time.Since(c.fetched) > c.ttl
}

func (c *ModelCache) IsStale() bool {
	return time.Since(c.fetched) > c.staleTTL
}

func (c *ModelCache) Len() int {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return len(c.models)
}

func (c *ModelCache) FetchTime() time.Time {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return c.fetched
}
