//! Contextual memory service trait (CTL-01, CTL-02).

use crate::capability::error::CapabilityError;
use async_trait::async_trait;

/// Asynchronous service seam for key-value contextual memory.
#[async_trait]
pub trait MemoryService: Send + Sync + 'static {
    /// Store a key-value entry with optional TTL in seconds.
    async fn store_memory(
        &self,
        key: &str,
        value: &str,
        ttl_secs: Option<u64>,
    ) -> Result<(), CapabilityError>;

    /// Retrieve a key-value entry from memory.
    async fn recall_memory(&self, key: &str) -> Result<Option<String>, CapabilityError>;
}
