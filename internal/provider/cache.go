package provider

import (
	"context"
	"sync"
	"sync/atomic"
	"time"

	"github.com/eshanized/M31A/internal/types"
	"golang.org/x/sync/singleflight"
)

// DefaultCacheRefreshInterval is the default interval for automatic model
// cache refreshes (5 minutes, matching ModelCacheTTL).
const DefaultCacheRefreshInterval = 5 * time.Minute

type ModelCache struct {
	mu         sync.RWMutex
	models     map[string]*types.ModelInfo
	fetched    time.Time
	ttl        time.Duration
	staleTTL   time.Duration
	sfg        singleflight.Group
	refreshing atomic.Bool
}

func NewModelCache(ttl time.Duration) *ModelCache {
	return &ModelCache{
		models:   make(map[string]*types.ModelInfo),
		ttl:      ttl,
		staleTTL: 24 * time.Hour,
	}
}

// NewModelCacheWithStale creates a ModelCache with explicit TTL and stale TTL.
func NewModelCacheWithStale(ttl time.Duration, staleTTL time.Duration) *ModelCache {
	return &ModelCache{
		models:   make(map[string]*types.ModelInfo),
		ttl:      ttl,
		staleTTL: staleTTL,
	}
}

// Refresh deduplicates concurrent calls via singleflight — only one HTTP
// request is made even if multiple goroutines call Refresh simultaneously.
func (c *ModelCache) Refresh(ctx context.Context, fetchFn func(ctx context.Context) ([]types.ModelInfo, error)) ([]types.ModelInfo, error) {
	c.refreshing.Store(true)

	defer c.refreshing.Store(false)

	v, err, _ := c.sfg.Do("refresh", func() (any, error) {
		models, fetchErr := fetchFn(ctx)
		if fetchErr != nil {
			return nil, fetchErr
		}
		c.Set(models)
		return models, nil
	})
	if err != nil {
		return nil, err
	}
	return v.([]types.ModelInfo), nil
}

func (c *ModelCache) Get(id string) (*types.ModelInfo, bool) {
	c.mu.RLock()
	defer c.mu.RUnlock()

	model, ok := c.models[id]
	if !ok {
		return nil, false
	}
	elapsed := time.Since(c.fetched)
	if elapsed > c.ttl && elapsed > c.staleTTL {
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
	c.mu.RLock()
	defer c.mu.RUnlock()
	return time.Since(c.fetched) > c.ttl
}

func (c *ModelCache) IsStale() bool {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return time.Since(c.fetched) > c.staleTTL
}

func (c *ModelCache) IsRefreshing() bool {
	return c.refreshing.Load()
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
	// Return a snapshot of the map. Shallow copy of the map is sufficient
	// since callers typically read fields, not mutate ModelInfo structs.
	// This avoids O(N) deep-copy of all model entries (300+ for OpenRouter).
	result := make(map[string]*types.ModelInfo, len(c.models))
	for k, v := range c.models {
		result[k] = v
	}
	return result
}
