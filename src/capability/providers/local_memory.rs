//! Local in-memory contextual memory capability provider (CTL-02, D-01, D-03).

use crate::capability::error::CapabilityError;
use crate::capability::traits::memory::MemoryService;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

struct MemoryEntry {
    value: String,
    expires_at: Option<Instant>,
}

/// Native local in-memory key-value memory store.
pub struct LocalMemoryProvider {
    store: Arc<RwLock<HashMap<String, MemoryEntry>>>,
}

impl Default for LocalMemoryProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalMemoryProvider {
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl MemoryService for LocalMemoryProvider {
    async fn store_memory(
        &self,
        key: &str,
        value: &str,
        ttl_secs: Option<u64>,
    ) -> Result<(), CapabilityError> {
        let expires_at = ttl_secs.map(|s| Instant::now() + Duration::from_secs(s));
        let mut store = self.store.write().unwrap();
        store.insert(
            key.to_string(),
            MemoryEntry {
                value: value.to_string(),
                expires_at,
            },
        );
        Ok(())
    }

    async fn recall_memory(&self, key: &str) -> Result<Option<String>, CapabilityError> {
        let mut store = self.store.write().unwrap();
        if let Some(entry) = store.get(key) {
            if let Some(exp) = entry.expires_at
                && Instant::now() > exp
            {
                store.remove(key);
                return Ok(None);
            }
            return Ok(Some(entry.value.clone()));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_local_memory_provider() {
        let provider = LocalMemoryProvider::new();
        provider.store_memory("foo", "bar", None).await.unwrap();
        let val = provider.recall_memory("foo").await.unwrap();
        assert_eq!(val, Some("bar".to_string()));

        let missing = provider.recall_memory("nonexistent").await.unwrap();
        assert_eq!(missing, None);
    }
}
