package provider

import (
	"sync"
	"time"

	"github.com/eshanized/M31A/internal/types"
)

// DefaultCacheRefreshInterval is the default interval for automatic model
// cache refreshes (5 minutes, matching ModelCacheTTL).
const DefaultCacheRefreshInterval = 5 * time.Minute

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

// ModelCacheRefreshTicker provides periodic scheduling for model cache
// refreshes. Tick() returns a receive-only channel that delivers values
// at the configured interval. Call Stop() to shut down the ticker.
type ModelCacheRefreshTicker struct {
	interval time.Duration
	stopCh   chan struct{}
	started  bool
}

// NewModelCacheRefreshTicker creates a new ticker with the given interval.
// If interval is <= 0, DefaultCacheRefreshInterval is used.
func NewModelCacheRefreshTicker(interval time.Duration) *ModelCacheRefreshTicker {
	if interval <= 0 {
		interval = DefaultCacheRefreshInterval
	}
	return &ModelCacheRefreshTicker{
		interval: interval,
		stopCh:   make(chan struct{}),
	}
}

// Interval returns the tick interval duration.
func (t *ModelCacheRefreshTicker) Interval() time.Duration {
	return t.interval
}

// Tick returns a channel that receives the current time each interval.
// The channel is unbuffered; callers should consume promptly.
func (t *ModelCacheRefreshTicker) Tick() <-chan time.Time {
	t.started = true
	ch := make(chan time.Time)
	ticker := time.NewTicker(t.interval)
	go func() {
		defer ticker.Stop()
		for {
			select {
			case ti := <-ticker.C:
				ch <- ti
			case <-t.stopCh:
				close(ch)
				return
			}
		}
	}()
	return ch
}

// Stop terminates the ticker goroutine and closes the output channel.
func (t *ModelCacheRefreshTicker) Stop() {
	if !t.started {
		return
	}
	select {
	case <-t.stopCh:
	default:
		close(t.stopCh)
	}
}
