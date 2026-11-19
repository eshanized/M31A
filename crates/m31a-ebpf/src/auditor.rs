//! Authoritative Execution Auditor.
//!
//! Orchestrates capability probing, process provenance correlation,
//! secret redaction, ring buffer event management, and structured audit export.

use super::correlator::ExecutionCorrelator;
use super::events::{AuditEventKind, AuditRecord};
use super::probe::{BpfCapabilityStatus, BpfProbe};
use super::redaction::SecretRedactor;
use super::ring_buffer::BoundedAuditRingBuffer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Filter parameters for querying audit records.
#[derive(Debug, Clone, Default)]
pub struct AuditQueryFilter {
    pub turn_id: Option<String>,
    pub sandbox_id: Option<String>,
    pub pid: Option<u32>,
    pub event_kind_prefix: Option<String>,
}

/// Summary statistics for an audit session or window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummaryReport {
    pub capability_status: BpfCapabilityStatus,
    pub total_events_emitted: u64,
    pub total_events_lost: u64,
    pub drop_rate_percentage: f64,
    pub queued_events_count: usize,
    pub active_processes_tracked: usize,
    pub process_exec_count: usize,
    pub network_connect_count: usize,
    pub file_open_count: usize,
}

pub struct ExecutionAuditor {
    capability: BpfCapabilityStatus,
    correlator: Arc<ExecutionCorrelator>,
    ring_buffer: Arc<BoundedAuditRingBuffer>,
}

impl Default for ExecutionAuditor {
    fn default() -> Self {
        Self::new(BoundedAuditRingBuffer::DEFAULT_CAPACITY)
    }
}

impl ExecutionAuditor {
    pub fn new(ring_buffer_capacity: usize) -> Self {
        let capability = BpfProbe::probe();
        Self {
            capability,
            correlator: Arc::new(ExecutionCorrelator::new()),
            ring_buffer: Arc::new(BoundedAuditRingBuffer::new(ring_buffer_capacity)),
        }
    }

    pub fn capability(&self) -> &BpfCapabilityStatus {
        &self.capability
    }

    pub fn correlator(&self) -> &Arc<ExecutionCorrelator> {
        &self.correlator
    }

    pub fn ring_buffer(&self) -> &Arc<BoundedAuditRingBuffer> {
        &self.ring_buffer
    }

    /// Ingests a raw event, applies secret redaction, correlates with provenance,
    /// and pushes into the bounded ring buffer.
    pub fn ingest_event(&self, kind: AuditEventKind) {
        // Capture exiting provenance before removal; never inherit an exiting PID
        // from its parent, which would reinsert stale identity after process death.
        let exit_provenance = match &kind {
            AuditEventKind::ProcessExit { pid, .. } => {
                let provenance = self.correlator.lookup_pid(*pid);
                self.correlator.deregister_pid(*pid);
                provenance
            }
            _ => None,
        };

        // Redact secrets
        let redacted_kind = match kind {
            AuditEventKind::ProcessExec {
                pid,
                ppid,
                uid,
                binary_path,
                args,
            } => AuditEventKind::ProcessExec {
                pid,
                ppid,
                uid,
                binary_path: SecretRedactor::redact_path(&binary_path),
                args: SecretRedactor::redact_args(&args),
            },
            AuditEventKind::FileOpen {
                pid,
                path,
                flags,
                read_only,
            } => AuditEventKind::FileOpen {
                pid,
                path: SecretRedactor::redact_path(&path),
                flags,
                read_only,
            },
            AuditEventKind::ProcessExit {
                pid,
                ppid,
                exit_code,
                duration_ms,
            } => AuditEventKind::ProcessExit {
                pid,
                ppid,
                exit_code,
                duration_ms,
            },
            other => other,
        };

        // Correlate with provenance
        let (turn_id, sandbox_id) = match &redacted_kind {
            AuditEventKind::ProcessExec { pid, ppid, .. } => {
                if let Some(prov) = self.correlator.inherit_or_lookup(*pid, *ppid) {
                    (Some(prov.turn_id), Some(prov.sandbox_id))
                } else {
                    (None, None)
                }
            }
            AuditEventKind::ProcessExit { .. } => {
                if let Some(prov) = exit_provenance {
                    (Some(prov.turn_id), Some(prov.sandbox_id))
                } else {
                    (None, None)
                }
            }
            AuditEventKind::FileOpen { pid, .. } | AuditEventKind::NetworkConnect { pid, .. } => {
                if let Some(prov) = self.correlator.inherit_or_lookup(*pid, 0) {
                    (Some(prov.turn_id), Some(prov.sandbox_id))
                } else {
                    (None, None)
                }
            }
            AuditEventKind::RingBufferOverflow { .. } => (None, None),
        };

        let record = AuditRecord::new(turn_id, sandbox_id, redacted_kind);
        self.ring_buffer.push(record);
    }

    /// Queries audit records matching the specified filter.
    pub fn query_events(&self, filter: &AuditQueryFilter) -> Vec<AuditRecord> {
        let snapshot = self.ring_buffer.snapshot();
        snapshot
            .into_iter()
            .filter(|r| {
                if let Some(ref tid) = filter.turn_id
                    && r.turn_id.as_deref() != Some(tid)
                {
                    return false;
                }
                if let Some(ref sbid) = filter.sandbox_id
                    && r.sandbox_id.as_deref() != Some(sbid)
                {
                    return false;
                }
                if let Some(pid) = filter.pid
                    && r.pid() != Some(pid)
                {
                    return false;
                }
                true
            })
            .collect()
    }

    /// Generates structured summary report.
    pub fn summary(&self) -> AuditSummaryReport {
        let emitted = self.ring_buffer.events_emitted();
        let lost = self.ring_buffer.events_lost();
        let drop_rate = if emitted > 0 {
            (lost as f64 / emitted as f64) * 100.0
        } else {
            0.0
        };

        let snapshot = self.ring_buffer.snapshot();
        let mut process_exec_count = 0;
        let mut network_connect_count = 0;
        let mut file_open_count = 0;

        for r in &snapshot {
            match &r.kind {
                AuditEventKind::ProcessExec { .. } => process_exec_count += 1,
                AuditEventKind::NetworkConnect { .. } => network_connect_count += 1,
                AuditEventKind::FileOpen { .. } => file_open_count += 1,
                _ => {}
            }
        }

        AuditSummaryReport {
            capability_status: self.capability.clone(),
            total_events_emitted: emitted,
            total_events_lost: lost,
            drop_rate_percentage: drop_rate,
            queued_events_count: snapshot.len(),
            active_processes_tracked: self.correlator.active_count(),
            process_exec_count,
            network_connect_count,
            file_open_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auditor_ingest_redact_and_query() {
        let auditor = ExecutionAuditor::new(100);
        auditor
            .correlator()
            .register_execution(2000, "turn-101", "sb-sandbox", "cargo build");

        auditor.ingest_event(AuditEventKind::ProcessExec {
            pid: 2000,
            ppid: 1,
            uid: 1000,
            binary_path: "/usr/bin/cargo".to_string(),
            args: vec![
                "cargo".to_string(),
                "build".to_string(),
                "--token=secret-token".to_string(),
            ],
        });

        auditor.ingest_event(AuditEventKind::FileOpen {
            pid: 2000,
            path: "/workspace/.env".to_string(),
            flags: 0,
            read_only: true,
        });

        let events = auditor.query_events(&AuditQueryFilter {
            turn_id: Some("turn-101".to_string()),
            ..Default::default()
        });

        assert_eq!(events.len(), 2);
        // Verify redaction was applied
        if let AuditEventKind::ProcessExec { ref args, .. } = events[0].kind {
            assert_eq!(args[2], "--token=[REDACTED_SECRET]");
        } else {
            panic!("Expected ProcessExec");
        }

        if let AuditEventKind::FileOpen { ref path, .. } = events[1].kind {
            assert!(path.contains("[PROTECTED_PATH]"));
        } else {
            panic!("Expected FileOpen");
        }

        let summary = auditor.summary();
        assert_eq!(summary.total_events_emitted, 2);
        assert_eq!(summary.total_events_lost, 0);
        assert_eq!(summary.process_exec_count, 1);
        assert_eq!(summary.file_open_count, 1);
    }
}
