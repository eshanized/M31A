//! Thread-safe asynchronous LRU cache.
//!
//! Provides bounded in-memory caching with least-recently-used eviction policy,
//! concurrent access synchronization, and hit/miss observability statistics.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::Mutex;

/// Observability statistics for an LRU cache instance.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LruStats {
    pub hits: usize,
    pub misses: usize,
    pub evictions: usize,
    pub entries: usize,
    pub capacity: usize,
}

#[derive(Debug)]
struct InnerCache<K, V> {
    capacity: usize,
    entries: HashMap<K, V>,
    order: VecDeque<K>,
    hits: usize,
    misses: usize,
    evictions: usize,
}

/// A thread-safe, bounded Least-Recently-Used (LRU) cache.
#[derive(Debug)]
pub struct AsyncLruCache<K, V> {
    inner: Mutex<InnerCache<K, V>>,
}

impl<K: Clone + Eq + Hash, V: Clone> AsyncLruCache<K, V> {
    /// Create a new cache with the specified non-zero capacity.
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self {
            inner: Mutex::new(InnerCache {
                capacity: cap,
                entries: HashMap::with_capacity(cap),
                order: VecDeque::with_capacity(cap),
                hits: 0,
                misses: 0,
                evictions: 0,
            }),
        }
    }

    /// Retrieve a cloned value for `key`, marking it as most recently used.
    pub fn get(&self, key: &K) -> Option<V> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(val) = inner.entries.get(key).cloned() {
            inner.hits += 1;

            // Refresh recency in order list
            if let Some(pos) = inner.order.iter().position(|k| k == key) {
                inner.order.remove(pos);
            }
            inner.order.push_back(key.clone());
            Some(val)
        } else {
            inner.misses += 1;
            None
        }
    }

    /// Insert or update `key` with `value`. Returns the previous value if present.
    /// Evicts the least-recently used entry if capacity is exceeded.
    pub fn put(&self, key: K, value: V) -> Option<V> {
        let mut inner = self.inner.lock().unwrap();

        // If already present, remove old position in order queue
        if let Some(pos) = inner.order.iter().position(|k| k == &key) {
            inner.order.remove(pos);
        } else if inner.entries.len() >= inner.capacity {
            // Evict least-recently used entry from front of queue
            if let Some(lru_key) = inner.order.pop_front() {
                inner.entries.remove(&lru_key);
                inner.evictions += 1;
            }
        }

        inner.order.push_back(key.clone());
        inner.entries.insert(key, value)
    }

    /// Retrieve the value for `key` or compute and insert it.
    pub fn get_or_insert_with<F>(&self, key: K, compute: F) -> V
    where
        F: FnOnce() -> V,
    {
        if let Some(existing) = self.get(&key) {
            return existing;
        }

        let computed = compute();
        self.put(key, computed.clone());
        computed
    }

    /// Retrieve the value for `key` or fallibly compute and insert it.
    pub fn get_or_try_insert_with<E, F>(&self, key: K, compute: F) -> Result<V, E>
    where
        F: FnOnce() -> Result<V, E>,
    {
        if let Some(existing) = self.get(&key) {
            return Ok(existing);
        }

        let computed = compute()?;
        self.put(key, computed.clone());
        Ok(computed)
    }

    /// Remove an entry by key.
    pub fn remove(&self, key: &K) -> Option<V> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(pos) = inner.order.iter().position(|k| k == key) {
            inner.order.remove(pos);
        }
        inner.entries.remove(key)
    }

    /// Clear all entries from the cache.
    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.entries.clear();
        inner.order.clear();
    }

    /// Current number of entries stored.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Current cache statistics.
    pub fn stats(&self) -> LruStats {
        let inner = self.inner.lock().unwrap();
        LruStats {
            hits: inner.hits,
            misses: inner.misses,
            evictions: inner.evictions,
            entries: inner.entries.len(),
            capacity: inner.capacity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru_cache_eviction_and_recency() {
        let cache = AsyncLruCache::new(3);

        cache.put("a", 1);
        cache.put("b", 2);
        cache.put("c", 3);

        assert_eq!(cache.len(), 3);

        // Access "a" to mark it as recently used
        assert_eq!(cache.get(&"a"), Some(1));

        // Insert "d", which should evict "b" (since "a" was accessed and "c" was inserted after "b")
        cache.put("d", 4);

        assert_eq!(cache.get(&"b"), None);
        assert_eq!(cache.get(&"a"), Some(1));
        assert_eq!(cache.get(&"c"), Some(3));
        assert_eq!(cache.get(&"d"), Some(4));

        let stats = cache.stats();
        assert_eq!(stats.evictions, 1);
        assert_eq!(stats.hits, 4);
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn test_get_or_insert_with() {
        let cache = AsyncLruCache::new(5);
        let mut compute_count = 0;

        let val1 = cache.get_or_insert_with("computed_key", || {
            compute_count += 1;
            100
        });
        assert_eq!(val1, 100);
        assert_eq!(compute_count, 1);

        // Second access should hit cache and not call closure
        let val2 = cache.get_or_insert_with("computed_key", || {
            compute_count += 1;
            200
        });
        assert_eq!(val2, 100);
        assert_eq!(compute_count, 1);
    }
}
