//! Bounded Audit Event Ring Buffer and Loss Tracker.
//!
//! Provides lock-free or low-contention bounded buffering for high-throughput
//! kernel execution events. Tracks dropped events and buffer overflow metrics.

use super::events::{AuditEventKind, AuditRecord};
use std::collections::VecDeque;
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct BoundedAuditRingBuffer {
    capacity: usize,
    buffer: RwLock<VecDeque<AuditRecord>>,
    events_emitted: AtomicU64,
    events_lost: AtomicU64,
}

impl BoundedAuditRingBuffer {
    pub const DEFAULT_CAPACITY: usize = 4096;

    pub fn new(capacity: usize) -> Self {
        let cap = if capacity == 0 {
            Self::DEFAULT_CAPACITY
        } else {
            capacity
        };

        Self {
            capacity: cap,
            buffer: RwLock::new(VecDeque::with_capacity(cap)),
            events_emitted: AtomicU64::new(0),
            events_lost: AtomicU64::new(0),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn events_emitted(&self) -> u64 {
        self.events_emitted.load(Ordering::Relaxed)
    }

    pub fn events_lost(&self) -> u64 {
        self.events_lost.load(Ordering::Relaxed)
    }

    /// Pushes an event into the ring buffer. If capacity is exhausted,
    /// drops the event and updates loss counters without blocking caller.
    pub fn push(&self, record: AuditRecord) -> bool {
        self.events_emitted.fetch_add(1, Ordering::Relaxed);

        if let Ok(mut lock) = self.buffer.write() {
            if lock.len() >= self.capacity {
                let lost = self.events_lost.fetch_add(1, Ordering::Relaxed) + 1;
                // If capacity is breached, drop the oldest record to accommodate newest, or drop record
                lock.pop_front();
                // Insert overflow notification
                lock.push_back(AuditRecord::new(
                    record.turn_id.clone(),
                    record.sandbox_id.clone(),
                    AuditEventKind::RingBufferOverflow {
                        lost_events_count: lost,
                        buffer_capacity: self.capacity,
                    },
                ));
                return false;
            }

            lock.push_back(record);
            true
        } else {
            self.events_lost.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    /// Drains all currently queued events from the buffer.
    pub fn drain(&self) -> Vec<AuditRecord> {
        if let Ok(mut lock) = self.buffer.write() {
            lock.drain(..).collect()
        } else {
            Vec::new()
        }
    }

    /// Peeks snapshot of events matching a predicate.
    pub fn snapshot(&self) -> Vec<AuditRecord> {
        if let Ok(lock) = self.buffer.read() {
            lock.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    pub fn len(&self) -> usize {
        self.buffer.read().map(|l| l.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_push_drain() {
        let ring = BoundedAuditRingBuffer::new(10);
        for i in 0..5 {
            let record = AuditRecord::new(
                Some("turn-1".to_string()),
                Some("sb-1".to_string()),
                AuditEventKind::ProcessExit {
                    pid: 1000 + i,
                    ppid: 1,
                    exit_code: 0,
                    duration_ms: 10,
                },
            );
            assert!(ring.push(record));
        }

        assert_eq!(ring.len(), 5);
        assert_eq!(ring.events_emitted(), 5);
        assert_eq!(ring.events_lost(), 0);

        let drained = ring.drain();
        assert_eq!(drained.len(), 5);
        assert_eq!(ring.len(), 0);
    }

    #[test]
    fn test_ring_buffer_overflow_tracking() {
        let ring = BoundedAuditRingBuffer::new(3);
        for i in 0..10 {
            let record = AuditRecord::new(
                Some("turn-1".to_string()),
                Some("sb-1".to_string()),
                AuditEventKind::ProcessExit {
                    pid: 1000 + i,
                    ppid: 1,
                    exit_code: 0,
                    duration_ms: 10,
                },
            );
            ring.push(record);
        }

        assert_eq!(ring.events_emitted(), 10);
        assert!(ring.events_lost() > 0);
        let drained = ring.drain();
        // Check that at least one overflow record is present in drained buffer
        assert!(
            drained
                .iter()
                .any(|r| matches!(r.kind, AuditEventKind::RingBufferOverflow { .. }))
        );
    }
}
