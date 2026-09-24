//! Streaming Artifact Quota Enforcer & Protection (BST-01, D-08).
//!
//! Enforces byte-level limits on single artifact sizes and cumulative mission storage
//! with strict exemption for verification, audit, checkpoint, and completion artifacts.

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use thiserror::Error;
use tokio::io::AsyncWrite;

/// Quota enforcement violations.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum QuotaError {
    #[error("Single artifact size limit exceeded: {actual} > {limit} bytes")]
    ArtifactSizeExceeded { limit: u64, actual: u64 },

    #[error("Cumulative mission storage quota exceeded: {actual} > {limit} bytes")]
    MissionQuotaExceeded { limit: u64, actual: u64 },
}

/// Eviction exemption classification for durable artifacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactExemption {
    /// Verification checks, checkpoints, audit logs, completion reports — NEVER evicted.
    Protected,
    /// Ephemeral build logs, tool outputs, debug traces — subject to quota bounds.
    Standard,
}

/// Streaming writer enforcing byte quotas in real time without silent truncation.
pub struct StreamingQuotaWriter<W> {
    inner: W,
    max_artifact_bytes: Option<u64>,
    max_mission_bytes: Option<u64>,
    bytes_written: u64,
    cumulative_counter: Option<Arc<AtomicU64>>,
    exemption: ArtifactExemption,
}

impl<W> StreamingQuotaWriter<W> {
    pub fn new(
        inner: W,
        max_artifact_bytes: Option<u64>,
        max_mission_bytes: Option<u64>,
        cumulative_counter: Option<Arc<AtomicU64>>,
        exemption: ArtifactExemption,
    ) -> Self {
        Self {
            inner,
            max_artifact_bytes,
            max_mission_bytes,
            bytes_written: 0,
            cumulative_counter,
            exemption,
        }
    }

    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for StreamingQuotaWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let incoming_len = buf.len() as u64;

        // Exempt protected artifacts from hard halts if configured
        if self.exemption != ArtifactExemption::Protected {
            // Check single artifact limit
            if let Some(max_single) = self.max_artifact_bytes
                && self.bytes_written + incoming_len > max_single
            {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::FileTooLarge,
                    format!(
                        "Artifact size {} exceeds limit of {} bytes",
                        self.bytes_written + incoming_len,
                        max_single
                    ),
                )));
            }

            // Check cumulative mission quota
            if let (Some(max_cum), Some(counter)) =
                (self.max_mission_bytes, &self.cumulative_counter)
            {
                let current_cum = counter.load(Ordering::SeqCst);
                if current_cum + incoming_len > max_cum {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::OutOfMemory,
                        format!(
                            "Mission cumulative quota {} exceeds limit of {} bytes",
                            current_cum + incoming_len,
                            max_cum
                        ),
                    )));
                }
            }
        }

        match Pin::new(&mut self.inner).poll_write(cx, buf) {
            Poll::Ready(Ok(n)) => {
                let n_u64 = n as u64;
                self.bytes_written += n_u64;
                if let Some(counter) = &self.cumulative_counter {
                    counter.fetch_add(n_u64, Ordering::SeqCst);
                }
                Poll::Ready(Ok(n))
            }
            other => other,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Quota coordinator managing limits across multiple artifact writes.
#[derive(Debug, Clone)]
pub struct QuotaEnforcer {
    max_artifact_bytes: Option<u64>,
    max_mission_bytes: Option<u64>,
    cumulative_bytes: Arc<AtomicU64>,
}

impl QuotaEnforcer {
    pub fn new(max_artifact_bytes: Option<u64>, max_mission_bytes: Option<u64>) -> Self {
        Self {
            max_artifact_bytes,
            max_mission_bytes,
            cumulative_bytes: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn current_cumulative_bytes(&self) -> u64 {
        self.cumulative_bytes.load(Ordering::SeqCst)
    }

    /// Check if an artifact is eligible for eviction.
    pub fn can_evict(&self, exemption: ArtifactExemption) -> bool {
        match exemption {
            ArtifactExemption::Protected => false,
            ArtifactExemption::Standard => true,
        }
    }

    /// Pre-check raw bytes against quotas.
    pub fn check_bytes(&self, size: u64, exemption: ArtifactExemption) -> Result<(), QuotaError> {
        if exemption == ArtifactExemption::Protected {
            return Ok(());
        }

        if let Some(max_single) = self.max_artifact_bytes
            && size > max_single
        {
            return Err(QuotaError::ArtifactSizeExceeded {
                limit: max_single,
                actual: size,
            });
        }

        if let Some(max_cum) = self.max_mission_bytes {
            let current = self.cumulative_bytes.load(Ordering::SeqCst);
            if current + size > max_cum {
                return Err(QuotaError::MissionQuotaExceeded {
                    limit: max_cum,
                    actual: current + size,
                });
            }
        }

        Ok(())
    }

    /// Wrap an underlying AsyncWrite stream with real-time quota checks.
    pub fn wrap_writer<W: AsyncWrite + Unpin>(
        &self,
        inner: W,
        exemption: ArtifactExemption,
    ) -> StreamingQuotaWriter<W> {
        StreamingQuotaWriter::new(
            inner,
            self.max_artifact_bytes,
            self.max_mission_bytes,
            Some(self.cumulative_bytes.clone()),
            exemption,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn test_streaming_quota_writer_enforces_limit() {
        let enforcer = QuotaEnforcer::new(Some(10), Some(100));
        let mut sink = Vec::new();
        let mut writer = enforcer.wrap_writer(&mut sink, ArtifactExemption::Standard);

        // Write 6 bytes: ok
        writer.write_all(b"hello!").await.unwrap();

        // Write another 6 bytes: exceeds 10 bytes limit -> error!
        let err = writer.write_all(b"world!").await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::FileTooLarge);
    }

    #[tokio::test]
    async fn test_protected_artifacts_exempt_from_quota() {
        let enforcer = QuotaEnforcer::new(Some(5), Some(10));
        let mut sink = Vec::new();
        let mut writer = enforcer.wrap_writer(&mut sink, ArtifactExemption::Protected);

        // Writing 20 bytes on protected stream succeeds despite exceeding 5 byte limit
        assert!(
            writer
                .write_all(b"this is important evidence")
                .await
                .is_ok()
        );
        assert!(!enforcer.can_evict(ArtifactExemption::Protected));
    }
}
