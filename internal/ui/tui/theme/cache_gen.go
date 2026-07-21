package theme

// Manager gains a Cache() method that returns the pre-computed StyleCache.
// The cache is rebuilt lazily when the theme changes.

// Cache returns the pre-computed StyleCache for the current theme.
// It is rebuilt on each call if the theme has changed since the last
// cache build. In practice, themes change rarely (user action), so this
// is effectively free during normal operation.
func (m *Manager) Cache() *StyleCache {
	if m.cache == nil {
		m.cache = NewStyleCache(m.current)
	}
	return m.cache
}

// invalidateCache forces a rebuild of the style cache on next Cache() call.
func (m *Manager) invalidateCache() {
	m.cache = nil
}
