//! Dynamic Bearer Token Provider with Asynchronous Refresh and Expiry Tracking.
//!
//! Provides thread-safe, deduplicated token caching and automatic background refresh
//! for OAuth2, Workload Identity, and expiring gateway credentials before expiration.

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Cached token with expiration epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedToken {
    pub token: String,
    pub expires_at_epoch_ms: u64,
}

impl CachedToken {
    pub fn new(token: impl Into<String>, expires_in: Duration) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let expires_at = now + expires_in.as_millis() as u64;

        Self {
            token: token.into(),
            expires_at_epoch_ms: expires_at,
        }
    }

    /// Checks if token is expired, applying a safety margin (default 30 seconds).
    pub fn is_expired_with_margin(&self, margin_secs: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let margin_ms = margin_secs * 1000;

        self.expires_at_epoch_ms <= now + margin_ms
    }
}

/// Abstract interface for obtaining Bearer authentication tokens.
#[async_trait]
pub trait TokenProvider: Send + Sync {
    /// Retrieve a valid token, refreshing if necessary.
    async fn get_token(&self) -> Result<String>;
}

/// Simple static token provider (e.g. constant API keys).
#[derive(Debug, Clone)]
pub struct StaticTokenProvider {
    token: String,
}

impl StaticTokenProvider {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }
}

#[async_trait]
impl TokenProvider for StaticTokenProvider {
    async fn get_token(&self) -> Result<String> {
        Ok(self.token.clone())
    }
}

/// Asynchronous refresh callback type.
pub type RefreshFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<CachedToken>> + Send>>;
pub type RefreshFn = Box<dyn Fn() -> RefreshFuture + Send + Sync>;

/// Dynamic token provider with thread-safe caching and deduplicated refresh.
pub struct DynamicTokenProvider {
    cached: Arc<RwLock<Option<CachedToken>>>,
    refresh_fn: RefreshFn,
    safety_margin_secs: u64,
}

impl DynamicTokenProvider {
    pub fn new<F>(refresh_fn: F) -> Self
    where
        F: Fn() -> RefreshFuture + Send + Sync + 'static,
    {
        Self {
            cached: Arc::new(RwLock::new(None)),
            refresh_fn: Box::new(refresh_fn),
            safety_margin_secs: 30,
        }
    }

    pub fn with_safety_margin(mut self, secs: u64) -> Self {
        self.safety_margin_secs = secs;
        self
    }
}

#[async_trait]
impl TokenProvider for DynamicTokenProvider {
    async fn get_token(&self) -> Result<String> {
        // Fast read lock check
        {
            let read_guard = self.cached.read().await;
            if let Some(cached) = &*read_guard
                && !cached.is_expired_with_margin(self.safety_margin_secs)
            {
                return Ok(cached.token.clone());
            }
        }

        // Token is missing or expired, acquire write lock
        let mut write_guard = self.cached.write().await;

        // Double check after acquiring write lock
        if let Some(cached) = &*write_guard
            && !cached.is_expired_with_margin(self.safety_margin_secs)
        {
            return Ok(cached.token.clone());
        }

        // Perform token refresh
        let new_token = (self.refresh_fn)()
            .await
            .context("Failed refreshing dynamic Bearer token")?;
        let token_str = new_token.token.clone();
        *write_guard = Some(new_token);

        Ok(token_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_static_token_provider() {
        let provider = StaticTokenProvider::new("sk-test-123");
        let token = provider.get_token().await.unwrap();
        assert_eq!(token, "sk-test-123");
    }

    #[tokio::test]
    async fn test_dynamic_token_refresh_and_caching() {
        let refresh_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&refresh_count);

        let provider = DynamicTokenProvider::new(move || {
            let count = Arc::clone(&count_clone);
            Box::pin(async move {
                let current = count.fetch_add(1, Ordering::SeqCst);
                // Return token valid for 1 hour
                Ok(CachedToken::new(
                    format!("token-{}", current),
                    Duration::from_secs(3600),
                ))
            })
        });

        // First call triggers refresh
        let t1 = provider.get_token().await.unwrap();
        assert_eq!(t1, "token-0");
        assert_eq!(refresh_count.load(Ordering::SeqCst), 1);

        // Second call should return cached token without triggering refresh
        let t2 = provider.get_token().await.unwrap();
        assert_eq!(t2, "token-0");
        assert_eq!(refresh_count.load(Ordering::SeqCst), 1);
    }
}
