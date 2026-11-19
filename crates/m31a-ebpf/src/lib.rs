//! `m31a-ebpf`: Execution audit event processing and capability diagnostics.
//!
//! Provides caller-supplied process-event correlation, bounded userspace buffering,
//! event-loss tracking, and secret redaction. This build does not attach eBPF programs
//! or collect kernel events; it never replaces authoritative sandbox policies.

pub mod auditor;
pub mod benchmark;
pub mod correlator;
pub mod events;
pub mod probe;
pub mod redaction;
pub mod ring_buffer;

pub use auditor::{AuditQueryFilter, AuditSummaryReport, ExecutionAuditor};
pub use benchmark::{AuditBenchmarkReport, AuditBenchmarker};
pub use correlator::{ExecutionCorrelator, ExecutionProvenance};
pub use events::{AuditEventKind, AuditRecord};
pub use probe::{BpfCapabilityStatus, BpfProbe};
pub use redaction::SecretRedactor;
pub use ring_buffer::BoundedAuditRingBuffer;
