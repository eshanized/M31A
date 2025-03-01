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

	model, ok := c.models[id]
	if !ok {
		return nil, false
	}
	if c.IsExpired() && c.IsStale() {
		return nil, false
	}
	return model, true
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

func (c *ModelCache) Models() map[string]*types.ModelInfo {
	c.mu.RLock()
	defer c.mu.RUnlock()
	result := make(map[string]*types.ModelInfo, len(c.models))
	for k, v := range c.models {
		result[k] = v
	}
	return result
}
