//! Shared buffer budgeting and transfer framing guards for network proxying.
//!
//! Enforces bounded memory allocation, slot concurrency limits, and prevents HTTP request
//! smuggling or memory exhaustion during large transfers.

use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Maximum slots allowed for concurrent request transformation buffers.
pub const BUFFER_SLOT_COUNT: usize = 8;

/// Default maximum request body size permitted (16 MiB).
pub const MAX_REQUEST_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Default maximum response body size permitted (64 MiB).
pub const MAX_RESPONSE_BODY_BYTES: usize = 64 * 1024 * 1024;

/// Default acquire timeout for buffer reservation (5 seconds).
pub const DEFAULT_RESERVE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BodyBudgetError {
    #[error("request buffer capacity unavailable (timeout acquiring permit)")]
    CapacityUnavailable,

    #[error("request buffer budget semaphore closed")]
    BudgetClosed,

    #[error("body size ({actual} bytes) exceeds configured maximum limit of {max} bytes")]
    BodyExceedsLimit { actual: usize, max: usize },

    #[error("invalid transfer framing: {0}")]
    InvalidFraming(String),

    #[error("read error: {0}")]
    IoError(String),
}

/// Manages concurrent buffer permits for in-flight requests.
#[derive(Clone, Debug)]
pub struct BodyBudgetManager {
    slots: Arc<Semaphore>,
    max_request_bytes: usize,
    max_response_bytes: usize,
}

impl Default for BodyBudgetManager {
    fn default() -> Self {
        Self::new(
            BUFFER_SLOT_COUNT,
            MAX_REQUEST_BODY_BYTES,
            MAX_RESPONSE_BODY_BYTES,
        )
    }
}

impl BodyBudgetManager {
    pub fn new(slot_count: usize, max_request_bytes: usize, max_response_bytes: usize) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(slot_count)),
            max_request_bytes,
            max_response_bytes,
        }
    }

    pub fn max_request_bytes(&self) -> usize {
        self.max_request_bytes
    }

    pub fn max_response_bytes(&self) -> usize {
        self.max_response_bytes
    }

    pub fn available_slots(&self) -> usize {
        self.slots.available_permits()
    }

    /// Acquires an owned permit with the default 5-second timeout.
    pub async fn reserve(&self) -> Result<Arc<OwnedSemaphorePermit>, BodyBudgetError> {
        self.reserve_with_timeout(DEFAULT_RESERVE_TIMEOUT).await
    }

    /// Acquires an owned permit with a custom timeout.
    pub async fn reserve_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<Arc<OwnedSemaphorePermit>, BodyBudgetError> {
        let permit = tokio::time::timeout(timeout, self.slots.clone().acquire_owned())
            .await
            .map_err(|_| BodyBudgetError::CapacityUnavailable)?
            .map_err(|_| BodyBudgetError::BudgetClosed)?;

        Ok(Arc::new(permit))
    }

    /// Reads an async stream until EOF up to the configured request body limit,
    /// growing geometrically without allocating beyond `max_request_bytes`.
    pub async fn read_bounded_request<R: AsyncRead + Unpin>(
        &self,
        reader: &mut R,
    ) -> Result<Vec<u8>, BodyBudgetError> {
        collect_bounded_body(reader, self.max_request_bytes).await
    }

    /// Reads an async stream until EOF up to the configured response body limit.
    pub async fn read_bounded_response<R: AsyncRead + Unpin>(
        &self,
        reader: &mut R,
    ) -> Result<Vec<u8>, BodyBudgetError> {
        collect_bounded_body(reader, self.max_response_bytes).await
    }
}

/// Identifies Amazon EventStream payload framing.
pub fn is_amazon_event_stream(content_type: Option<&str>) -> bool {
    let Some(ct) = content_type else { return false };
    ct.split(';').next().is_some_and(|mime| {
        mime.trim()
            .eq_ignore_ascii_case("application/vnd.amazon.eventstream")
    })
}

/// Validates transfer encoding framing to prevent HTTP smuggling.
///
/// Rules:
/// - If `Transfer-Encoding` is present, it must be `chunked`.
/// - If `Transfer-Encoding` is present, `Content-Length` MUST NOT be present simultaneously.
pub fn supported_transfer_framing(
    transfer_encoding: Option<&str>,
    content_length: Option<&str>,
) -> Result<bool, BodyBudgetError> {
    match transfer_encoding {
        None => Ok(true),
        Some(te) => {
            let te_clean = te.trim();
            if !te_clean.eq_ignore_ascii_case("chunked") {
                return Err(BodyBudgetError::InvalidFraming(format!(
                    "unsupported transfer encoding: {te}"
                )));
            }
            if content_length.is_some() {
                return Err(BodyBudgetError::InvalidFraming(
                    "conflicting Content-Length and Transfer-Encoding headers (HTTP smuggling vector)".to_string(),
                ));
            }
            Ok(true)
        }
    }
}

/// Verifies whether a claimed `Content-Length` header strictly matches the actual byte count.
pub fn content_length_matches(content_length_header: Option<&str>, actual_size: usize) -> bool {
    match content_length_header {
        None => true,
        Some(val) => {
            let val = val.trim();
            if val.is_empty() || !val.bytes().all(|b| b.is_ascii_digit()) {
                return false;
            }
            val.parse::<usize>().ok() == Some(actual_size)
        }
    }
}

/// Reads an asynchronous stream into a byte buffer up to `max_bytes`.
/// Employs geometric buffer growth up to the limit to avoid oversized pre-allocations.
pub async fn collect_bounded_body<R: AsyncRead + Unpin>(
    reader: &mut R,
    max_bytes: usize,
) -> Result<Vec<u8>, BodyBudgetError> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];

    loop {
        let n = reader
            .read(&mut chunk)
            .await
            .map_err(|e| BodyBudgetError::IoError(e.to_string()))?;

        if n == 0 {
            break;
        }

        if bytes.len() + n > max_bytes {
            return Err(BodyBudgetError::BodyExceedsLimit {
                actual: bytes.len() + n,
                max: max_bytes,
            });
        }

        let needed = bytes.len() + n;
        if needed > bytes.capacity() {
            let new_cap = max_bytes.min(bytes.capacity().saturating_mul(2).max(needed));
            bytes.reserve_exact(new_cap - bytes.len());
        }

        bytes.extend_from_slice(&chunk[..n]);
    }

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[tokio::test]
    async fn test_budget_reservation_and_concurrency() {
        let manager = BodyBudgetManager::new(2, 1024, 2048);
        assert_eq!(manager.available_slots(), 2);

        let p1 = manager.reserve().await.expect("p1 acquire");
        assert_eq!(manager.available_slots(), 1);

        let p2 = manager.reserve().await.expect("p2 acquire");
        assert_eq!(manager.available_slots(), 0);

        // Third reservation should time out
        let res3 = manager
            .reserve_with_timeout(Duration::from_millis(50))
            .await;
        assert_eq!(res3.unwrap_err(), BodyBudgetError::CapacityUnavailable);

        // Drop p1, slot freed
        drop(p1);
        assert_eq!(manager.available_slots(), 1);

        let p3 = manager
            .reserve_with_timeout(Duration::from_millis(50))
            .await;
        assert!(p3.is_ok());

        drop(p2);
        drop(p3);
        assert_eq!(manager.available_slots(), 2);
    }

    #[tokio::test]
    async fn test_collect_bounded_body_success() {
        let raw = b"hello world from stream".to_vec();
        let mut cursor = Cursor::new(raw.clone());
        let collected = collect_bounded_body(&mut cursor, 100)
            .await
            .expect("read body");
        assert_eq!(collected, raw);
    }

    #[tokio::test]
    async fn test_collect_bounded_body_exceeds_limit() {
        let raw = vec![0u8; 1000];
        let mut cursor = Cursor::new(raw);
        let err = collect_bounded_body(&mut cursor, 500).await.unwrap_err();
        match err {
            BodyBudgetError::BodyExceedsLimit { actual, max } => {
                assert!(actual > 500);
                assert_eq!(max, 500);
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn test_is_amazon_event_stream() {
        assert!(is_amazon_event_stream(Some(
            "application/vnd.amazon.eventstream"
        )));
        assert!(is_amazon_event_stream(Some(
            "application/vnd.amazon.eventstream; charset=utf-8"
        )));
        assert!(!is_amazon_event_stream(Some("application/json")));
        assert!(!is_amazon_event_stream(None));
    }

    #[test]
    fn test_supported_transfer_framing() {
        // Normal identity / absent headers
        assert!(supported_transfer_framing(None, Some("123")).unwrap());
        assert!(supported_transfer_framing(None, None).unwrap());

        // Valid chunked without Content-Length
        assert!(supported_transfer_framing(Some("chunked"), None).unwrap());
        assert!(supported_transfer_framing(Some("CHUNKED"), None).unwrap());

        // Conflicting chunked and Content-Length -> rejection
        let err = supported_transfer_framing(Some("chunked"), Some("50")).unwrap_err();
        assert!(matches!(err, BodyBudgetError::InvalidFraming(_)));

        // Unsupported encoding
        let err = supported_transfer_framing(Some("gzip"), None).unwrap_err();
        assert!(matches!(err, BodyBudgetError::InvalidFraming(_)));
    }

    #[test]
    fn test_content_length_matches() {
        assert!(content_length_matches(None, 42));
        assert!(content_length_matches(Some("100"), 100));
        assert!(!content_length_matches(Some("100"), 99));
        assert!(!content_length_matches(Some("not-a-number"), 100));
        assert!(!content_length_matches(Some(""), 0));
    }
}
